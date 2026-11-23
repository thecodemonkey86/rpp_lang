//! Debug-Interpreter von R++ (Meilenstein M4, Referenzimplementierung nach R-12.3).
//!
//! Der Interpreter führt das geprüfte Programm aus. Was die Prüfung entschieden hat
//! (gewählte Überladung, Moves, Zieltypen, Erfassungen), liest er aus `Program::rt`
//! und wiederholt es nicht. Er setzt die Laufzeitregeln um:
//! - deterministische Freigabe: Besitzer geben ihre Werte am Blockende, bei Zuweisung
//!   und beim Unwinding frei; Destruktor vor den Feldern, die Basisklasse zuletzt (V-7);
//!   shared-Objekte mit Referenzzähler (R-12.11);
//! - Konstruktion: Basisklasse, Feldinitialisierer, Konstruktorrumpf; wirft ein
//!   Konstruktor, werden die gesetzten Felder und der fertige Basisanteil freigegeben (R-9.13);
//! - Exceptions und Programmfehler mit Unwinding und `finally` (Kapitel 9); Programmfehler
//!   werden nicht gefangen (R-9.14);
//! - Zahlen nach Kapitel 4 mit den Unterschieden der Modi aus R-12.5;
//! - Programmende nach R-12.9: Rückgabewert von `main`, 1 bei einer Exception, 2 bei
//!   einem Programmfehler.

mod stdlib;
mod save_file;
pub mod value;

use rpp_sema::defs::{AstRef, MethodInfo, PInfo, Program, Ret};
use rpp_sema::rt::Target;
use rpp_sema::ty::{self, DefId, Form as TForm, Mode, Ty};
use rpp_syntax::ast::{self, BinaryOp as B, ExprKind as E, Primitive as P, StmtKind as S, UnaryOp as U};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use value::{Cellv, Closure, Form, ORef, Obj, Value};

/// Garantiert sind 10 000 verschachtelte Aufrufe (R-12.10).
const MAX_DEPTH: usize = 12_000;

pub struct Options {
    pub release: bool,
    /// Prüfmodus: Invarianten I-1, I-2, I-4, I-5 aus Kapitel 5, Abschnitt 6.2, und Lecks
    pub check: bool,
    pub args: Vec<String>,
    pub input: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub stdout: String,
    pub stderr: String,
    pub exit: i32,
}

/// Führt `main` des Programms aus (in einem eigenen Thread mit großem Stack).
pub fn run(p: &'static Program, opts: Options) -> Outcome {
    let h = std::thread::Builder::new()
        .stack_size(2 << 30)
        .spawn(move || {
            let mut it = Interp::new(p, opts.release, opts.input);
            it.check = opts.check;
            it.main(&opts.args)
        })
        .expect("Thread für den Interpreter");
    match h.join() {
        Ok(o) => o,
        Err(e) => {
            let msg = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
            Outcome { stdout: String::new(), stderr: format!("interner Fehler des Interpreters: {msg}"), exit: 3 }
        }
    }
}

pub struct Fatal {
    pub name: &'static str,
    pub msg: String,
    pub trace: Vec<String>,
}

pub enum Flow {
    Return(Value),
    Break,
    Continue,
    Throw(Value, Vec<String>),
    Fatal(Fatal),
}

type R<T> = Result<T, Flow>;

struct Frame {
    scopes: Vec<Vec<(String, Cellv)>>,
    env: Vec<(String, Cellv)>,
    this: Option<Value>,
    class: Option<DefId>,
    file: usize,
    label: String,
    /// Sperren bis zum Ende eines Blocks (Tiefe, Objekt, schreibend)
    locks: Vec<(usize, ORef, bool)>,
    /// Typargumente der generischen Methode (Typ-Parameter, Typ), R-7.26
    mtargs: Vec<(usize, Ty)>,
}

struct Layout {
    index: HashMap<String, usize>,
    /// Klassen von der Wurzel bis zur Klasse selbst, mit ihren Feldern
    levels: Vec<(DefId, std::ops::Range<usize>)>,
    len: usize,
}

enum Slot {
    Cell(Cellv),
    Field(ORef, usize),
}

/// Konstruktor: Verweis auf die Deklaration und Parameter.
#[derive(Clone)]
struct Ctor {
    ast: AstRef,
    params: Vec<PInfo>,
}

struct Interp {
    save_files: HashMap<*const Obj, save_file::SaveFile>,
    p: &'static Program,
    release: bool,
    out: String,
    err: String,
    input: String,
    in_pos: usize,
    frames: Vec<Frame>,
    temps: Vec<Vec<Value>>,
    layouts: HashMap<DefId, Rc<Layout>>,
    statics: HashMap<(DefId, String), Value>,
    /// Anzahl der Programmfehler; ab dem zweiten (beim Aufräumen) endet das Programm sofort (R-9.14)
    fatals: Cell<usize>,
    /// Ein geliehenes Argument wird gebunden, nicht gelesen: keine Prüfung der eigenen Sperre
    binding: Cell<bool>,
    /// Argumente des nächsten `args`, die der Aufruf gesperrt hat
    locked_args: Vec<usize>,
    check: bool,
    /// Typargumente für das nächste `new_object`
    pending_targs: Vec<Ty>,
    /// alle lebenden Objekte (Prüfmodus); fest gehalten, damit auch ein Objekt sichtbar
    /// bleibt, das R++ nie freigibt, obwohl kein Verweis mehr besteht
    heap: Vec<ORef>,
    shared_used: bool,
}

fn prim_of(t: &Ty) -> Option<P> {
    match t {
        Ty::Prim(p) => Some(*p),
        Ty::Nullable(x) | Ty::Formed(_, x) => prim_of(x),
        _ => None,
    }
}

fn class_of(t: &Ty) -> Option<DefId> {
    match t.core() {
        Ty::Named(d, _) => Some(*d),
        _ => None,
    }
}

fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('0') => out.push('\0'),
            Some('u') => {
                let mut hex = String::new();
                if it.peek() == Some(&'{') {
                    it.next();
                    for c in it.by_ref() {
                        if c == '}' {
                            break;
                        }
                        hex.push(c);
                    }
                } else {
                    for _ in 0..4 {
                        if let Some(c) = it.next() {
                            hex.push(c);
                        }
                    }
                }
                if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    out.push(ch);
                }
            }
            Some(o) => out.push(o),
            None => {}
        }
    }
    out
}

fn make_int(v: i128, p: P) -> Value {
    match p {
        P::I8 => Value::I8(v as i8),
        P::I16 => Value::I16(v as i16),
        P::I32 => Value::I32(v as i32),
        P::I64 => Value::I64(v as i64),
        P::Ui8 => Value::U8(v as u8),
        P::Ui16 => Value::U16(v as u16),
        P::Ui32 => Value::U32(v as u32),
        P::Ui64 => Value::U64(v as u64),
        P::F32 => Value::F32(v as f32),
        P::F64 => Value::F64(v as f64),
        P::Byte => Value::Byte(v as u8),
        P::Char => Value::Char(char::from_u32(v as u32).unwrap_or('\0')),
        _ => Value::I32(v as i32),
    }
}

fn prim_of_value(v: &Value) -> Option<P> {
    Some(match v {
        Value::I8(_) => P::I8,
        Value::I16(_) => P::I16,
        Value::I32(_) => P::I32,
        Value::I64(_) => P::I64,
        Value::U8(_) => P::Ui8,
        Value::U16(_) => P::Ui16,
        Value::U32(_) => P::Ui32,
        Value::U64(_) => P::Ui64,
        Value::F32(_) => P::F32,
        Value::F64(_) => P::F64,
        Value::Bool(_) => P::Bool,
        Value::Char(_) => P::Char,
        Value::Byte(_) => P::Byte,
        Value::Str(_) => P::String,
        _ => return None,
    })
}

fn fmt_f64(x: f64) -> String {
    if x.is_nan() {
        "NaN".into()
    } else if x.is_infinite() {
        if x > 0.0 { "Infinity".into() } else { "-Infinity".into() }
    } else {
        format!("{x}")
    }
}

fn to_text(v: &Value) -> String {
    match v {
        Value::Bool(b) => b.to_string(),
        Value::Char(c) => c.to_string(),
        Value::Byte(b) => b.to_string(),
        Value::F32(x) => {
            if x.is_finite() { format!("{x}") } else { fmt_f64(*x as f64) }
        }
        Value::F64(x) => fmt_f64(*x),
        Value::Str(s) => s.to_string(),
        Value::Null => "null".into(),
        other => other.as_i128().map(|i| i.to_string()).unwrap_or_default(),
    }
}

impl Interp {
    fn new(p: &'static Program, release: bool, input: String) -> Interp {
        Interp { p, release, out: String::new(), err: String::new(), input, in_pos: 0, frames: Vec::new(), temps: Vec::new(), layouts: HashMap::new(), statics: HashMap::new(), fatals: Cell::new(0), binding: Cell::new(false), locked_args: Vec::new(), check: false, pending_targs: Vec::new(), heap: Vec::new(), shared_used: false, save_files: HashMap::new() }
    }

    // ------------------------------------------------------------ Hilfen

    fn frame(&mut self) -> &mut Frame {
        self.frames.last_mut().unwrap()
    }

    fn file(&self) -> usize {
        self.frames.last().map(|f| f.file).unwrap_or(0)
    }

    fn key(&self, e: &ast::Expr) -> (usize, u32) {
        (self.file(), e.id)
    }

    fn trace(&self) -> Vec<String> {
        self.frames.iter().rev().map(|f| f.label.clone()).collect()
    }

    fn fatal(&self, name: &'static str, msg: impl Into<String>) -> Flow {
        self.fatals.set(self.fatals.get() + 1);
        Flow::Fatal(Fatal { name, msg: msg.into(), trace: self.trace() })
    }

    /// Ergebnis eines Blocks und seines Aufräumens zusammenführen: Ein Programmfehler beim
    /// Aufräumen geht vor; trifft er auf einen schon laufenden Programmfehler, bleibt der
    /// erste die Ursache, der zweite wird angehängt (R-9.14).
    fn combine(r: R<()>, d: R<()>) -> R<()> {
        match (r, d) {
            (Err(Flow::Fatal(mut a)), Err(Flow::Fatal(b))) => {
                a.msg = format!("{}; beim Aufräumen außerdem {}: {}", a.msg, b.name, b.msg);
                Err(Flow::Fatal(a))
            }
            (_, Err(e @ Flow::Fatal(_))) => Err(e),
            (Err(e), _) => Err(e),
            (Ok(()), d) => d,
        }
    }

    fn internal(&self, msg: impl Into<String>) -> Flow {
        self.fatal("InternalError", msg)
    }

    fn label(&self, owner: DefId, name: &str) -> String {
        if owner == usize::MAX {
            return name.to_string();
        }
        format!("{}.{}", self.p.defs[owner].name, name)
    }

    fn members(&self, d: DefId) -> &'static [ast::ClassMember] {
        let def = &self.p.defs[d];
        match &self.p.ws.files[def.file].unit.decls[def.decl] {
            ast::TopDecl::Class(c) => &c.members,
            ast::TopDecl::Mixin(m) => &m.members,
            _ => &[],
        }
    }

    fn member_at(&self, a: AstRef) -> Option<&'static ast::ClassMember> {
        match self.p.ws.files.get(a.0)?.unit.decls.get(a.1)? {
            ast::TopDecl::Class(c) => c.members.get(a.2),
            ast::TopDecl::Mixin(m) => m.members.get(a.2),
            _ => None,
        }
    }

    fn base_of(&self, d: DefId) -> Option<DefId> {
        self.p.defs[d].base.as_ref().and_then(class_of)
    }

    fn layout(&mut self, d: DefId) -> Rc<Layout> {
        if let Some(l) = self.layouts.get(&d) {
            return l.clone();
        }
        let mut chain = vec![d];
        while let Some(b) = self.base_of(*chain.last().unwrap()) {
            chain.push(b);
        }
        chain.reverse();
        let mut index = HashMap::new();
        let mut levels = Vec::new();
        let mut n = 0;
        for c in chain {
            let start = n;
            for f in self.p.defs[c].fields.iter().filter(|f| !f.is_static) {
                index.insert(f.name.clone(), n);
                n += 1;
            }
            levels.push((c, start..n));
        }
        let l = Rc::new(Layout { index, levels, len: n });
        self.layouts.insert(d, l.clone());
        l
    }

    fn is_subclass(&self, sub: DefId, sup: DefId) -> bool {
        sub == sup || self.p.is_subclass(sub, sup) || self.p.upcasts(&Ty::Named(sub, Vec::new()), &Ty::Named(sup, Vec::new()))
    }

    fn lookup(&self, name: &str) -> Option<Cellv> {
        let f = self.frames.last()?;
        for s in f.scopes.iter().rev() {
            if let Some((_, c)) = s.iter().rev().find(|(n, _)| n == name) {
                return Some(c.clone());
            }
        }
        f.env.iter().rev().find(|(n, _)| n == name).map(|(_, c)| c.clone())
    }

    fn declare(&mut self, name: &str, v: Value) {
        let c = Rc::new(RefCell::new(v));
        self.frame().scopes.last_mut().unwrap().push((name.to_string(), c));
    }

    fn push_scope(&mut self) {
        self.frame().scopes.push(Vec::new());
    }

    /// Verlässt einen Gültigkeitsbereich: Variablen in umgekehrter Reihenfolge freigeben (V-7).
    fn pop_scope(&mut self) -> R<()> {
        let depth = self.frame().scopes.len();
        let mut held = std::mem::take(&mut self.frame().locks);
        for (_, o, m) in held.iter().filter(|(d, _, _)| *d >= depth) {
            Self::release(o, *m);
        }
        held.retain(|(d, _, _)| *d < depth);
        self.frame().locks = held;
        let vars = self.frame().scopes.pop().unwrap_or_default();
        let mut result = Ok(());
        for (_, c) in vars.into_iter().rev() {
            let v = c.replace(Value::Moved);
            let d = self.drop_value(v);
            result = Self::combine(result, d);
        }
        result
    }

    fn with_scope(&mut self, f: impl FnOnce(&mut Self) -> R<()>) -> R<()> {
        self.push_scope();
        let r = f(self);
        let d = self.pop_scope();
        Self::combine(r, d)
    }

    // ------------------------------------------------------------ Sperren (V-1)

    /// Objekt hinter der Wurzelvariablen eines Orts, wenn sie ein shared-Verweis ist.
    fn shared_root(&self, e: &ast::Expr) -> Option<ORef> {
        match &e.kind {
            E::Paren(x) | E::Member { object: x, .. } | E::Index { object: x, .. } => self.shared_root(x),
            E::Name(n) => match &*self.lookup(&n.name)?.borrow() {
                Value::Obj(o, Form::Shared) => Some(o.clone()),
                _ => None,
            },
            _ => None,
        }
    }

    fn check_lock(&self, o: &ORef, mutable: bool) -> R<()> {
        let b = o.lock.get();
        if (!mutable && b == -1) || (mutable && b != 0) {
            let how = if b == -1 { "schreibend" } else { "lesend" };
            return Err(self.fatal("BorrowError", format!("das shared-Objekt ist {how} gesperrt")));
        }
        Ok(())
    }

    fn acquire(&self, o: &ORef, mutable: bool) -> R<()> {
        self.check_lock(o, mutable)?;
        o.lock.set(if mutable { -1 } else { o.lock.get() + 1 });
        Ok(())
    }

    fn release(o: &ORef, mutable: bool) {
        o.lock.set(if mutable { 0 } else { (o.lock.get() - 1).max(0) });
    }

    /// Zugriff auf einen Ort mit shared-Wurzel prüfen.
    fn check_place(&self, e: &ast::Expr, mutable: bool) -> R<()> {
        if self.binding.get() {
            return Ok(());
        }
        match self.shared_root(e) {
            Some(o) => self.check_lock(&o, mutable).map_err(|f| self.at(f, e)),
            None => Ok(()),
        }
    }

    /// Ort eines Programmfehlers in der Meldung nennen (Zeile im Quelltext).
    fn at(&self, f: Flow, e: &ast::Expr) -> Flow {
        match f {
            Flow::Fatal(mut x) => {
                x.msg = format!("{} (Zeile {})", x.msg, e.span.line);
                Flow::Fatal(x)
            }
            other => other,
        }
    }

    /// Sperre bis zum Ende des aktuellen Blocks (ref-Variable, Schleife).
    fn lock_scope(&mut self, e: &ast::Expr, mutable: bool) -> R<()> {
        if let Some(o) = self.shared_root(e) {
            self.acquire(&o, mutable).map_err(|f| self.at(f, e))?;
            let depth = self.frame().scopes.len();
            self.frame().locks.push((depth, o, mutable));
        }
        Ok(())
    }

    // ------------------------------------------------------------ Freigabe

    /// Gibt einen besitzten Wert frei (Kapitel 5, Abschnitt 5.3).
    fn drop_value(&mut self, v: Value) -> R<()> {
        match v {
            Value::Obj(o, Form::Own) => self.destruct(&o),
            Value::Obj(o, Form::Shared) => {
                let n = o.rc.get().saturating_sub(1);
                o.rc.set(n);
                if n == 0 {
                    self.destruct(&o)
                } else {
                    Ok(())
                }
            }
            Value::Tuple(vs) | Value::Enum(_, _, vs) => {
                for x in vs.into_iter().rev() {
                    self.drop_value(x)?;
                }
                Ok(())
            }
            Value::Func(c) => {
                if c.own && Rc::strong_count(&c) == 1 {
                    for (_, cell) in c.env.iter().rev() {
                        let x = cell.replace(Value::Moved);
                        self.drop_value(x)?;
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Destruktor, dann Felder in umgekehrter Reihenfolge, Basisklasse zuletzt (V-7).
    fn destruct(&mut self, o: &ORef) -> R<()> {
        if !o.alive.get() || o.dying.get() || self.fatals.get() >= 2 {
            return Ok(());
        }
        o.dying.set(true);
        let result = match o.class {
            None => {
                let elems = std::mem::take(&mut *o.slots.borrow_mut());
                let mut r = Ok(());
                for x in elems {
                    if let Err(e) = self.drop_value(x) {
                        r = Err(e);
                    }
                }
                r
            }
            Some(d) => {
                let lay = self.layout(d);
                let levels: Vec<usize> = (0..lay.levels.len()).rev().collect();
                self.destruct_levels(o, &lay, &levels)
            }
        };
        o.alive.set(false);
        result
    }

    /// V-7: zuerst die Destruktoren (von der Klasse zur Basis), dann die Felder in
    /// umgekehrter Deklarationsreihenfolge, die Felder der Basisklasse zuletzt.
    /// `levels` sind die Stufen, deren Destruktor läuft; freigegeben werden alle gesetzten Felder.
    fn destruct_levels(&mut self, o: &ORef, lay: &Layout, levels: &[usize]) -> R<()> {
        for &i in levels {
            let c = lay.levels[i].0;
            self.run_destructor(o, c)?;
        }
        // auch eingebaute Inhalte hinter den deklarierten Feldern (Map, Set, StringBuilder)
        let n = o.slots.borrow().len().max(lay.len);
        self.drop_fields(o, n)
    }

    fn drop_fields(&mut self, o: &ORef, upto: usize) -> R<()> {
        for k in (0..upto).rev() {
            let x = std::mem::replace(&mut o.slots.borrow_mut()[k], Value::Moved);
            self.drop_value(x)?;
        }
        Ok(())
    }

    fn run_destructor(&mut self, o: &ORef, c: DefId) -> R<()> {
        let body = self.members(c).iter().find_map(|m| match m {
            ast::ClassMember::Destructor(b, _) => Some(b),
            _ => None,
        });
        let Some(body) = body else { return Ok(()) };
        let label = self.label(c, "destructor");
        let file = self.p.defs[c].file;
        self.enter(Frame { scopes: vec![Vec::new()], env: Vec::new(), this: Some(Value::Obj(o.clone(), Form::Borrow)), class: Some(c), file, label, locks: Vec::new(), mtargs: Vec::new() })?;
        let r = self.block(body);
        let r = self.leave(r);
        match r {
            Ok(_) => Ok(()),
            // V-11: Eine Exception, die einen Destruktor verlässt, beendet das Programm.
            Err(Flow::Throw(ex, _)) => {
                let msg = self.describe(&ex);
                Err(self.fatal("DestructorError", format!("Exception verlässt einen Destruktor: {msg}")))
            }
            Err(e) => Err(e),
        }
    }

    fn enter(&mut self, f: Frame) -> R<()> {
        if self.frames.len() >= MAX_DEPTH {
            return Err(self.fatal("StackOverflowError", format!("mehr als {MAX_DEPTH} verschachtelte Aufrufe")));
        }
        self.frames.push(f);
        Ok(())
    }

    /// Rahmen verlassen: Parameter freigeben; `return` wird zum Ergebnis.
    fn leave(&mut self, r: R<()>) -> R<Value> {
        let mut d = Ok(());
        while self.frames.last().is_some_and(|f| !f.scopes.is_empty()) {
            let x = self.pop_scope();
            d = Self::combine(d, x);
        }
        self.frames.pop();
        let (r, v) = match r {
            Ok(()) => (Ok(()), Value::Void),
            Err(Flow::Return(v)) => (Ok(()), v),
            Err(e) => (Err(e), Value::Void),
        };
        Self::combine(r, d).map(|_| v)
    }

    // ------------------------------------------------------------ Start

    fn main(&mut self, args: &[String]) -> Outcome {
        let main = self.p.ws.files.iter().enumerate().find_map(|(i, f)| {
            if self.p.is_std_file(i) {
                return None;
            }
            f.unit.decls.iter().find_map(|d| match d {
                ast::TopDecl::Main(m) => Some((i, m)),
                _ => None,
            })
        });
        let Some((file, m)) = main else {
            return Outcome { stdout: String::new(), stderr: "kein main".into(), exit: 2 };
        };
        let argv: Vec<Value> = args.iter().map(|a| Value::Str(Rc::from(a.as_str()))).collect();
        let vec = self.new_vec(argv);
        let frame = Frame { scopes: vec![Vec::new()], env: Vec::new(), this: None, class: None, file, label: "main".into(), locks: Vec::new(), mtargs: Vec::new() };
        self.frames.push(frame);
        self.declare(&m.args.name, Value::Obj(vec, Form::Own));
        let r = self.block(&m.body);
        let mut r = self.leave(r);
        if self.check && r.is_ok() && !self.shared_used {
            let left = self.heap.iter().filter(|o| o.alive.get()).count();
            if left > 0 {
                r = Err(self.fatal("InvariantViolation", format!("Leck ohne shared: {left} Objekte leben nach dem Ende von main")));
            }
        }
        let exit = match r {
            Ok(v) => v.as_i128().unwrap_or(0) as i32,
            Err(Flow::Throw(ex, trace)) => {
                let msg = self.describe(&ex);
                let name = ex.obj().and_then(|o| o.class).map(|d| self.p.defs[d].name.clone()).unwrap_or_default();
                self.err.push_str(&format!("{name}: {msg}\n"));
                if !self.release {
                    for t in trace {
                        self.err.push_str(&format!("    bei {t}\n"));
                    }
                }
                let _ = self.drop_value(ex);
                1
            }
            Err(Flow::Fatal(f)) if f.name == "InvariantViolation" => {
                self.err.push_str(&format!("interner Fehler (Prüfmodus): {}\n", f.msg));
                3
            }
            Err(Flow::Fatal(f)) => {
                self.err.push_str(&format!("{}: {}\n", f.name, f.msg));
                if !self.release {
                    for t in f.trace {
                        self.err.push_str(&format!("    bei {t}\n"));
                    }
                }
                2
            }
            Err(_) => 0,
        };
        Outcome { stdout: std::mem::take(&mut self.out), stderr: std::mem::take(&mut self.err), exit }
    }

    fn describe(&mut self, ex: &Value) -> String {
        let Some(o) = ex.obj().cloned() else { return String::new() };
        let Some(d) = o.class else { return String::new() };
        let m = self.p.methods_of(&Ty::Named(d, Vec::new()), "describe").into_iter().find(|m| m.params.is_empty() && m.has_body);
        match m {
            Some(m) => match self.call_method(&m, Some(Value::Obj(o, Form::Borrow)), Vec::new()) {
                Ok(v) => v.as_str().unwrap_or("").to_string(),
                Err(_) => String::new(),
            },
            None => String::new(),
        }
    }

    // ------------------------------------------------------------ Anweisungen

    fn block(&mut self, b: &'static ast::Block) -> R<()> {
        self.with_scope(|it| {
            for s in &b.stmts {
                it.stmt(s)?;
                if it.check && it.frames.len() == 1 && it.frames[0].scopes.len() == 2 {
                    it.check_invariants()?;
                }
            }
            Ok(())
        })
    }

    // ------------------------------------------------------------ Prüfmodus

    /// Invarianten aus Kapitel 5, Abschnitt 6.2, zwischen zwei Anweisungen auf oberster Ebene von `main`
    /// (dann ist kein Wert unterwegs; alle Werte liegen in Variablen, Feldern oder Zwischenwerten).
    fn check_invariants(&mut self) -> R<()> {
        use std::collections::HashSet;
        let mut own: HashMap<*const Obj, usize> = HashMap::new();
        let mut sh: HashMap<*const Obj, usize> = HashMap::new();
        let mut problems: Vec<String> = Vec::new();
        let mut seen_cells: HashSet<*const RefCell<Value>> = HashSet::new();
        let mut stack: Vec<(Value, String)> = Vec::new();
        for (i, f) in self.frames.iter().enumerate() {
            for (n, c) in f.scopes.iter().flatten().chain(f.env.iter()) {
                if seen_cells.insert(Rc::as_ptr(c)) {
                    stack.push((c.borrow().clone(), format!("Variable {n} in Rahmen {i}")));
                }
            }
            if let Some(v) = &f.this {
                stack.push((v.clone(), format!("this in Rahmen {i}")));
            }
        }
        for v in self.temps.iter().flatten() {
            stack.push((v.clone(), "Zwischenwert".into()));
        }
        for v in self.statics.values() {
            stack.push((v.clone(), "statisches Feld".into()));
        }
        let live: Vec<ORef> = self.heap.iter().filter(|o| o.alive.get() && !o.dying.get()).cloned().collect();
        for o in &live {
            for (k, v) in o.slots.borrow().iter().enumerate() {
                stack.push((v.clone(), format!("Feld {k} eines Objekts")));
            }
        }
        while let Some((v, wo)) = stack.pop() {
            let form = form_of(&v);
            match v {
                Value::Obj(o, _) | Value::Slice(o, _, _) if form != Form::Weak => {
                    if !o.alive.get() && !o.dying.get() {
                        let rule = if form == Form::Borrow { "I-4" } else { "I-2" };
                        problems.push(format!("{rule}: {wo} verweist auf ein freigegebenes Objekt"));
                        continue;
                    }
                    match form {
                        Form::Own => *own.entry(Rc::as_ptr(&o)).or_default() += 1,
                        Form::Shared => *sh.entry(Rc::as_ptr(&o)).or_default() += 1,
                        _ => {}
                    }
                }
                Value::Tuple(vs) | Value::Enum(_, _, vs) => stack.extend(vs.into_iter().map(|x| (x, wo.clone()))),
                Value::Func(c) if c.own => {
                    for (n, cell) in &c.env {
                        if seen_cells.insert(Rc::as_ptr(cell)) {
                            stack.push((cell.borrow().clone(), format!("erfasste Variable {n}")));
                        }
                    }
                }
                _ => {}
            }
        }
        for o in &live {
            let ptr = Rc::as_ptr(o);
            let (n_own, n_sh) = (own.get(&ptr).copied().unwrap_or(0), sh.get(&ptr).copied().unwrap_or(0));
            let what = o.class.map(|d| self.p.defs[d].name.clone()).unwrap_or_else(|| "Vektor".into());
            if o.rc.get() == 0 {
                if n_own != 1 || n_sh != 0 {
                    problems.push(format!("I-1: {what} hat {n_own} own- und {n_sh} shared-Verweise"));
                }
            } else {
                self.shared_used = true;
                if n_own != 0 || n_sh != o.rc.get() {
                    problems.push(format!("I-1: shared-{what} hat rc={}, aber {n_sh} shared- und {n_own} own-Verweise", o.rc.get()));
                }
            }
            if o.lock.get() != 0 && o.rc.get() < 1 {
                problems.push(format!("I-5: gesperrtes {what} mit rc={}", o.rc.get()));
            }
        }
        self.heap.retain(|o| o.alive.get());
        match problems.first() {
            Some(p) => Err(self.fatal("InvariantViolation", p.clone())),
            None => Ok(()),
        }
    }

    /// Eine Anweisung; temporäre Werte werden an ihrem Ende freigegeben.
    fn stmt(&mut self, s: &'static ast::Stmt) -> R<()> {
        self.temps.push(Vec::new());
        let r = self.stmt_kind(s);
        let temps = self.temps.pop().unwrap_or_default();
        let mut d = Ok(());
        for t in temps.into_iter().rev() {
            if let Err(e) = self.drop_value(t) {
                d = Err(e);
            }
        }
        Self::combine(r, d)
    }

    fn sub_stmt(&mut self, s: &'static ast::Stmt) -> R<()> {
        self.with_scope(|it| it.stmt(s))
    }

    fn cond(&mut self, e: &'static ast::Expr) -> R<bool> {
        self.temps.push(Vec::new());
        let r = self.eval(e).map(|v| v.as_bool());
        let temps = self.temps.pop().unwrap_or_default();
        for t in temps {
            self.drop_value(t)?;
        }
        r
    }

    fn decl_ty(&self, name: &ast::Ident) -> Option<Ty> {
        self.p.rt.decls.get(&(self.file(), name.span)).cloned()
    }

    fn stmt_kind(&mut self, s: &'static ast::Stmt) -> R<()> {
        match &s.kind {
            S::Block(b) => self.block(b),
            S::LocalVar(d) => {
                let v = match &d.init {
                    Some(init) => {
                        let v = self.eval_store(init)?;
                        match self.decl_ty(&d.name) {
                            Some(t) => self.convert(v, &t)?,
                            None => v,
                        }
                    }
                    None => Value::Uninit,
                };
                self.declare(&d.name.name, v);
                Ok(())
            }
            S::Ref { name, init, mutable, .. } => {
                let mut target = init;
                while let E::Paren(x) | E::As { expr: x, .. } = &target.kind {
                    target = x;
                }
                // R-5.24: Nach einer Borrow-Rückgabe bleibt der Empfänger bis zum Blockende gesperrt.
                let v = match &target.kind {
                    E::Call { callee, .. } => {
                        let v = self.eval(init)?;
                        if let E::Member { object, .. } = &callee.kind {
                            self.lock_scope(object, *mutable)?;
                        }
                        v
                    }
                    _ => {
                        let v = self.eval(init)?;
                        self.lock_scope(target, *mutable)?;
                        v
                    }
                };
                let v = self.temp(v);
                self.declare(&name.name, v);
                Ok(())
            }
            S::Tuple(d) => {
                let v = self.eval_store(&d.init)?;
                self.bind_tuple(&d.vars, v)
            }
            S::Expr(es) => self.expr_stmt(es),
            S::Discard(e) => {
                let v = self.eval(e)?;
                self.drop_value(v)
            }
            S::If { cond, then, otherwise } => {
                if self.cond(cond)? {
                    self.sub_stmt(then)
                } else if let Some(o) = otherwise {
                    self.sub_stmt(o)
                } else {
                    Ok(())
                }
            }
            S::While { cond, body } => {
                while self.cond(cond)? {
                    match self.sub_stmt(body) {
                        Err(Flow::Break) => break,
                        Err(Flow::Continue) | Ok(()) => {}
                        Err(e) => return Err(e),
                    }
                }
                Ok(())
            }
            S::DoWhile { body, cond } => {
                loop {
                    match self.sub_stmt(body) {
                        Err(Flow::Break) => break,
                        Err(Flow::Continue) | Ok(()) => {}
                        Err(e) => return Err(e),
                    }
                    if !self.cond(cond)? {
                        break;
                    }
                }
                Ok(())
            }
            S::For { init, cond, update, body } => self.with_scope(|it| {
                match init {
                    Some(ast::ForInit::Var(d)) => {
                        let v = match &d.init {
                            Some(x) => {
                                let v = it.eval_store(x)?;
                                match it.decl_ty(&d.name) {
                                    Some(t) => it.convert(v, &t)?,
                                    None => v,
                                }
                            }
                            None => Value::Uninit,
                        };
                        it.declare(&d.name.name, v);
                    }
                    Some(ast::ForInit::Tuple(d)) => {
                        let v = it.eval_store(&d.init)?;
                        it.bind_tuple(&d.vars, v)?;
                    }
                    Some(ast::ForInit::Exprs(list)) => {
                        for e in list {
                            it.expr_stmt(e)?;
                        }
                    }
                    None => {}
                }
                loop {
                    if let Some(c) = cond {
                        if !it.cond(c)? {
                            break;
                        }
                    }
                    match it.sub_stmt(body) {
                        Err(Flow::Break) => break,
                        Err(Flow::Continue) | Ok(()) => {}
                        Err(e) => return Err(e),
                    }
                    for u in update {
                        it.temps.push(Vec::new());
                        let r = it.expr_stmt(u);
                        it.temps.pop();
                        r?;
                    }
                }
                Ok(())
            }),
            S::ForEach { name, iter, body, borrow_mut, .. } => {
                self.push_scope();
                let r = self.lock_scope(iter, *borrow_mut).and_then(|_| self.for_each(name, iter, body));
                let d = self.pop_scope();
                r.and(d)
            }
            S::Switch { subject, cases } => self.switch(subject, cases),
            S::Return { value, .. } => {
                let v = match value {
                    Some(v) => {
                        let x = self.eval_store(v)?;
                        // ein zurückgegebener Wert ist kein temporärer Wert der Anweisung
                        self.untemp(&x);
                        x
                    }
                    None => Value::Void,
                };
                Err(Flow::Return(v))
            }
            S::Break => Err(Flow::Break),
            S::Continue => Err(Flow::Continue),
            S::Throw(e) => {
                let v = self.eval_store(e)?;
                self.untemp(&v);
                Err(Flow::Throw(v, self.trace()))
            }
            S::Try { body, catches, finally } => self.try_stmt(body, catches, finally.as_ref()),
        }
    }

    fn for_each(&mut self, name: &'static ast::Ident, iter: &'static ast::Expr, body: &'static ast::Stmt) -> R<()> {
        let coll = self.eval(iter)?;
        let coll = self.temp(coll);
        if let Value::Obj(o, _) = &coll {
            if let Some(d) = o.class {
                return self.for_iterable(d, coll.view(), name, body);
            }
        }
        // Zeichen eines Strings einmal zerlegen statt je Schritt zu suchen
        let chars: Vec<char> = match &coll {
            Value::Str(s) => s.chars().collect(),
            _ => Vec::new(),
        };
        let mut i = 0usize;
        loop {
            let item = match &coll {
                Value::Obj(o, _) if o.class.is_none() => {
                    let s = o.slots.borrow();
                    if i >= s.len() {
                        break;
                    }
                    s[i].view()
                }
                Value::Slice(o, a, b) => {
                    if a + i >= *b {
                        break;
                    }
                    o.slots.borrow()[a + i].view()
                }
                Value::Str(_) => match chars.get(i) {
                    Some(c) => Value::Char(*c),
                    None => break,
                },
                _ => return Err(self.internal("for über diesen Wert ist noch nicht umgesetzt (Meilenstein M5)")),
            };
            i += 1;
            let r = self.with_scope(|it| {
                it.declare(&name.name, item);
                it.stmt(body)
            });
            match r {
                Err(Flow::Break) => break,
                Err(Flow::Continue) | Ok(()) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// R-13.10: `for (T x : c)` über eine Klasse mit `Iterable<T, C>`:
    /// `begin()`, solange nicht `done(c)`: `at(c)`, dann `next(c)`.
    fn for_iterable(&mut self, d: DefId, coll: Value, name: &'static ast::Ident, body: &'static ast::Stmt) -> R<()> {
        let ty = Ty::Named(d, Vec::new());
        let find = |it: &Self, n: &str, params: usize| it.p.methods_of(&ty, n).into_iter().find(|m| m.params.len() == params && !m.mut_this);
        let (Some(begin), Some(done), Some(next), Some(at)) = (find(self, "begin", 0), find(self, "done", 1), find(self, "next", 1), find(self, "at", 1)) else {
            return Err(self.internal("die Klasse ist nicht iterierbar"));
        };
        let mut c = self.call_method(&begin, Some(coll.clone()), Vec::new())?;
        loop {
            if self.call_method(&done, Some(coll.clone()), vec![c.clone()])?.as_bool() {
                break;
            }
            let item = self.call_method(&at, Some(coll.clone()), vec![c.clone()])?;
            let r = self.with_scope(|it| {
                it.declare(&name.name, item);
                it.stmt(body)
            });
            match r {
                Err(Flow::Break) => break,
                Err(Flow::Continue) | Ok(()) => {}
                Err(e) => return Err(e),
            }
            c = self.call_method(&next, Some(coll.clone()), vec![c])?;
        }
        Ok(())
    }

    fn bind_tuple(&mut self, vars: &'static [(ast::Type, ast::Ident)], v: Value) -> R<()> {
        let vs = match v {
            Value::Tuple(vs) => vs,
            // R-5.25: Zerlegen eines own-Objekts; es wird ohne Destruktor freigegeben.
            Value::Obj(o, Form::Own) if o.class.is_some() => {
                let vs: Vec<Value> = o.slots.borrow_mut().iter_mut().take(vars.len()).map(|x| std::mem::replace(x, Value::Moved)).collect();
                o.alive.set(false);
                vs
            }
            _ => return Err(self.internal("Tupel oder own-Objekt erwartet")),
        };
        for ((_, name), x) in vars.iter().zip(vs) {
            let x = match self.decl_ty(name) {
                Some(t) => self.convert(x, &t)?,
                None => x,
            };
            self.declare(&name.name, x);
        }
        Ok(())
    }

    fn try_stmt(&mut self, body: &'static ast::Block, catches: &'static [ast::CatchClause], finally: Option<&'static ast::Block>) -> R<()> {
        let mut r = self.block(body);
        if let Err(Flow::Throw(ex, trace)) = r {
            let class = ex.obj().and_then(|o| o.class);
            let chosen = catches.iter().find(|c| {
                let ct = self.decl_ty(&c.name).and_then(|t| class_of(&t));
                matches!((class, ct), (Some(a), Some(b)) if self.is_subclass(a, b))
            });
            r = match chosen {
                Some(c) => self.with_scope(|it| {
                    it.declare(&c.name.name, ex);
                    it.block(&c.body)
                }),
                None => Err(Flow::Throw(ex, trace)),
            };
        }
        if let Some(f) = finally {
            match self.block(f) {
                Ok(()) => {}
                Err(e2) => {
                    // R-9.10: Eine neue Exception ersetzt die laufende.
                    if let Err(Flow::Throw(old, _)) = r {
                        self.drop_value(old)?;
                    }
                    return Err(e2);
                }
            }
        }
        r
    }

    fn switch(&mut self, subject: &'static ast::Expr, cases: &'static [ast::CaseClause]) -> R<()> {
        let v = self.eval(subject)?;
        let v = self.temp(v);
        let mut chosen: Option<(&'static ast::CaseClause, Vec<(String, Value)>)> = None;
        'outer: for c in cases {
            for pat in &c.patterns {
                if let Some(binds) = self.matches(pat, &v)? {
                    chosen = Some((c, binds));
                    break 'outer;
                }
            }
        }
        if chosen.is_none() {
            chosen = cases.iter().find(|c| c.is_default).map(|c| (c, Vec::new()));
        }
        let Some((c, binds)) = chosen else { return Ok(()) };
        let r = self.with_scope(|it| {
            for (n, b) in binds {
                it.declare(&n, b);
            }
            for s in &c.body {
                it.stmt(s)?;
            }
            Ok(())
        });
        match r {
            Err(Flow::Break) => Ok(()),
            other => other,
        }
    }

    fn matches(&mut self, pat: &'static ast::Pattern, v: &Value) -> R<Option<Vec<(String, Value)>>> {
        match pat {
            ast::Pattern::Literal { negative, lit } => {
                let pv = match lit {
                    ast::Literal::Int(s) => {
                        let n = ty::int_value(s).unwrap_or(0);
                        let n = if *negative { -n } else { n };
                        return Ok(if v.as_i128() == Some(n) { Some(Vec::new()) } else { None });
                    }
                    ast::Literal::Char(s) => Value::Char(unescape(s).chars().next().unwrap_or('\0')),
                    ast::Literal::Str(s) => Value::Str(Rc::from(unescape(s).as_str())),
                    ast::Literal::Bool(b) => Value::Bool(*b),
                    _ => Value::Null,
                };
                Ok(if self.equal(v, &pv)? { Some(Vec::new()) } else { None })
            }
            ast::Pattern::Name { name, bindings } => {
                let n = match name {
                    ast::PatternName::Simple(id) => &id.name,
                    ast::PatternName::Static(sr) => &sr.member.name,
                };
                match v {
                    Value::Enum(d, idx, data) => {
                        if self.p.defs[*d].variants.get(*idx).is_some_and(|x| &x.name == n) {
                            let binds = bindings.as_ref().map(|bs| bs.iter().zip(data).map(|(b, x)| (b.name.clone(), x.view())).collect()).unwrap_or_default();
                            Ok(Some(binds))
                        } else {
                            Ok(None)
                        }
                    }
                    _ => Ok(None),
                }
            }
        }
    }

    fn expr_stmt(&mut self, es: &'static ast::ExprStmt) -> R<()> {
        match es {
            ast::ExprStmt::Assign { target, op, value } => self.assign(target, *op, value),
            ast::ExprStmt::Expr(e) => {
                let v = self.eval(e)?;
                self.drop_value(v)
            }
        }
    }

    // ------------------------------------------------------------ Orte

    fn slot(&mut self, e: &'static ast::Expr) -> R<Slot> {
        match &e.kind {
            E::Paren(x) => self.slot(x),
            E::Name(n) => {
                if let Some(c) = self.lookup(&n.name) {
                    return Ok(Slot::Cell(c));
                }
                let this = self.frames.last().and_then(|f| f.this.clone());
                let Some(o) = this.as_ref().and_then(Value::obj).cloned() else { return Err(self.internal(format!("{} ist unbekannt", n.name))) };
                self.field_slot(o, &n.name)
            }
            E::Member { object, name, .. } => {
                let ov = self.eval(object)?;
                let ov = self.temp(ov);
                let Some(o) = ov.obj().cloned() else { return Err(self.internal(format!("kein Objekt für .{}", name.name))) };
                self.field_slot(o, &name.name)
            }
            E::Index { object, index } => {
                let ov = self.eval(object)?;
                let ov = self.temp(ov);
                let i = self.eval(index)?.as_i128().unwrap_or(-1);
                match ov {
                    Value::Obj(o, _) => {
                        let len = o.slots.borrow().len();
                        if i < 0 || i as usize >= len {
                            return Err(self.fatal("IndexError", format!("Index {i} außerhalb von 0..{len}")));
                        }
                        Ok(Slot::Field(o, i as usize))
                    }
                    Value::Slice(o, a, b) => {
                        if i < 0 || a + i as usize >= b {
                            return Err(self.fatal("IndexError", format!("Index {i} außerhalb von 0..{}", b - a)));
                        }
                        Ok(Slot::Field(o, a + i as usize))
                    }
                    _ => Err(self.internal("Index auf diesem Wert")),
                }
            }
            _ => Err(self.internal("kein Zuweisungsziel")),
        }
    }

    fn field_slot(&mut self, o: ORef, name: &str) -> R<Slot> {
        let Some(d) = o.class else { return Err(self.internal("Feld eines Vektors")) };
        let lay = self.layout(d);
        match lay.index.get(name) {
            Some(i) => Ok(Slot::Field(o, *i)),
            None => Err(self.internal(format!("Feld {name} fehlt"))),
        }
    }

    fn get(&self, s: &Slot) -> Value {
        match s {
            Slot::Cell(c) => c.borrow().clone(),
            Slot::Field(o, i) => o.slots.borrow()[*i].clone(),
        }
    }

    fn set(&self, s: &Slot, v: Value) -> Value {
        match s {
            Slot::Cell(c) => c.replace(v),
            Slot::Field(o, i) => std::mem::replace(&mut o.slots.borrow_mut()[*i], v),
        }
    }

    fn assign(&mut self, target: &'static ast::Expr, op: ast::AssignOp, value: &'static ast::Expr) -> R<()> {
        if let E::Tuple(items) = &target.kind {
            let v = self.eval_store(value)?;
            let Value::Tuple(vs) = v else { return Err(self.internal("Tupel erwartet")) };
            for (t, x) in items.iter().zip(vs) {
                let s = self.slot(t)?;
                let x = match self.p.rt.types.get(&self.key(t)).cloned() {
                    Some(tt) => self.convert(x, &tt)?,
                    None => x,
                };
                let old = self.set(&s, x);
                self.drop_value(old)?;
            }
            return Ok(());
        }
        // R-8.2: erst das Ziel, dann der Wert
        if !matches!(target.kind, E::Name(_)) {
            self.check_place(target, true)?;
        }
        let s = self.slot(target)?;
        let tt = self.p.rt.types.get(&self.key(target)).cloned();
        if op == ast::AssignOp::Assign {
            let v = self.eval_store(value)?;
            self.untemp(&v);
            let v = match &tt {
                Some(t) => self.convert(v, t)?,
                None => v,
            };
            let old = self.set(&s, v);
            return self.drop_value(old);
        }
        let cur = self.get(&s).view();
        let rhs = self.eval(value)?;
        let bop = match op {
            ast::AssignOp::Add => B::Add,
            ast::AssignOp::Sub => B::Sub,
            ast::AssignOp::Mul => B::Mul,
            ast::AssignOp::Div => B::Div,
            _ => B::Rem,
        };
        let res = self.arith(bop, &cur, &rhs)?;
        let res = match &tt {
            Some(t) => self.convert_exact(res, t)?,
            None => res,
        };
        self.set(&s, res);
        Ok(())
    }

    // ------------------------------------------------------------ Werte

    /// Besitzt der Wert etwas, das freigegeben werden muss?
    fn owning(v: &Value) -> bool {
        match v {
            Value::Obj(_, Form::Own | Form::Shared) => true,
            Value::Tuple(vs) | Value::Enum(_, _, vs) => vs.iter().any(Self::owning),
            Value::Func(c) => c.own,
            _ => false,
        }
    }

    /// Ein besitzter temporärer Wert, der nur geliehen gebraucht wird: am Ende der
    /// Anweisung freigeben, bis dahin eine Sicht liefern.
    fn temp(&mut self, v: Value) -> Value {
        if Self::owning(&v) {
            let view = v.view();
            if let Some(t) = self.temps.last_mut() {
                t.push(v);
            }
            view
        } else {
            v
        }
    }

    /// Ein Wert, der doch weitergegeben wird, ist kein temporärer Wert.
    fn untemp(&mut self, _v: &Value) {}

    fn new_vec(&mut self, elems: Vec<Value>) -> ORef {
        let o = Rc::new(Obj { class: None, slots: RefCell::new(elems), rc: Cell::new(0), alive: Cell::new(true), dying: Cell::new(false), lock: Cell::new(0), targs: RefCell::new(Vec::new()), index: Default::default() });
        self.register(&o);
        o
    }

    pub(crate) fn register(&mut self, o: &ORef) {
        if self.check {
            self.heap.push(o.clone());
        }
    }

    /// Wert für einen Besitzer (Initialisierer, Zuweisung, own-Argument, return, throw):
    /// own-Variablen werden verschoben, shared-Verweise gezählt kopiert (Kapitel 5, Abschnitt 7).
    fn eval_store(&mut self, e: &'static ast::Expr) -> R<Value> {
        let v = match &e.kind {
            E::Paren(x) => return self.eval_store(x),
            E::Conditional { cond, then, otherwise } => {
                return if self.cond(cond)? { self.eval_store(then) } else { self.eval_store(otherwise) };
            }
            E::Coalesce { left, right } => {
                let l = self.eval_store(left)?;
                return if l.is_null() { self.eval_store(right) } else { Ok(l) };
            }
            _ if self.p.rt.consumes.contains(&self.key(e)) => self.transfer(e)?,
            _ => self.eval(e)?,
        };
        match self.p.rt.stores.get(&self.key(e)).cloned() {
            Some(t) => self.convert(v, &t),
            None => Ok(v),
        }
    }

    /// Ein Wert, der wie in der Prüfung kopiert statt verschoben wird (`movable` in
    /// rpp-sema): Enums ohne Varianten mit Daten, Tupel nur aus solchen Werten und Grundwerten.
    fn plain_value(&self, v: &Value) -> bool {
        match v {
            Value::Obj(..) | Value::Func(_) | Value::Moved | Value::Uninit => false,
            Value::Enum(d, _, _) => self.p.defs[*d].variants.iter().all(|x| x.fields.is_empty()),
            Value::Tuple(vs) => vs.iter().all(|x| self.plain_value(x)),
            _ => true,
        }
    }

    fn transfer(&mut self, e: &'static ast::Expr) -> R<Value> {
        if let E::Name(n) = &e.kind {
            if let Some(c) = self.lookup(&n.name) {
                let cur = c.borrow().clone();
                return Ok(match cur {
                    Value::Obj(o, Form::Shared) | Value::Obj(o, Form::Borrow) if o.rc.get() > 0 => {
                        o.rc.set(o.rc.get() + 1);
                        Value::Obj(o, Form::Shared)
                    }
                    Value::Tuple(_) | Value::Enum(..) if self.plain_value(&cur) => cur,
                    Value::Obj(_, Form::Own) | Value::Tuple(_) | Value::Enum(..) => c.replace(Value::Moved),
                    Value::Func(ref f) if f.own => c.replace(Value::Moved),
                    other => other,
                });
            }
        }
        let v = self.eval(e)?;
        Ok(self.promote(v))
    }

    /// Ergebnis eines Ausdrucks für einen Besitzer: eine geliehene Sicht auf ein
    /// shared-Objekt wird ein gezählter Verweis; besitzte Werte bleiben, wie sie sind.
    #[allow(clippy::only_used_in_recursion)]
    fn promote(&mut self, v: Value) -> Value {
        match v {
            Value::Obj(o, Form::Borrow) if o.rc.get() > 0 => {
                o.rc.set(o.rc.get() + 1);
                Value::Obj(o, Form::Shared)
            }
            Value::Tuple(vs) => Value::Tuple(vs.into_iter().map(|x| self.promote(x)).collect()),
            other => other,
        }
    }

    /// Aus einer Sicht einen besitzten Wert machen, wo das ohne Move geht (shared zählen).
    #[allow(clippy::only_used_in_recursion)]
    fn own_copy(&mut self, v: Value) -> Value {
        match v {
            Value::Obj(o, Form::Borrow) if o.rc.get() > 0 => {
                o.rc.set(o.rc.get() + 1);
                Value::Obj(o, Form::Shared)
            }
            Value::Obj(o, Form::Shared) => {
                o.rc.set(o.rc.get() + 1);
                Value::Obj(o, Form::Shared)
            }
            Value::Tuple(vs) => Value::Tuple(vs.into_iter().map(|x| self.own_copy(x)).collect()),
            other => other,
        }
    }

    /// Umwandlung in den Zieltyp: Zahlen (R-4.6), own → shared (V-9), shared → weak.
    fn convert(&mut self, v: Value, t: &Ty) -> R<Value> {
        match (&v, t) {
            (Value::Null | Value::Void | Value::Moved | Value::Uninit, _) => Ok(v),
            (Value::Obj(o, Form::Own), _) if is_shared_ty(t) => {
                o.rc.set(o.rc.get() + 1);
                Ok(Value::Obj(o.clone(), Form::Shared))
            }
            (Value::Obj(o, Form::Shared | Form::Borrow), _) if is_weak_ty(t) => {
                let w = Value::Obj(o.clone(), Form::Weak);
                if matches!(v, Value::Obj(_, Form::Shared)) {
                    self.drop_value(v)?;
                }
                Ok(w)
            }
            _ => match prim_of(t) {
                Some(p) if ty::is_numeric(p) && prim_of_value(&v).is_some_and(|q| q != p && ty::is_numeric(q)) => Ok(self.num_to(&v, p)),
                _ => Ok(v),
            },
        }
    }

    /// Wie `convert`, aber für Ergebnisse von Verbundzuweisungen (R-8.20): exakt in den Zieltyp.
    fn convert_exact(&mut self, v: Value, t: &Ty) -> R<Value> {
        match prim_of(t) {
            Some(p) if ty::is_integer(p) => match v.as_i128() {
                Some(i) => self.int_result(i, p),
                None => Ok(v),
            },
            Some(p) if ty::is_numeric(p) => Ok(self.num_to(&v, p)),
            _ => Ok(v),
        }
    }

    fn num_to(&self, v: &Value, p: P) -> Value {
        match p {
            P::F32 => Value::F32(match v {
                Value::F32(x) => *x,
                _ => v.as_f64().unwrap_or(0.0) as f32,
            }),
            P::F64 => Value::F64(v.as_f64().unwrap_or(0.0)),
            _ => make_int(v.as_i128().unwrap_or(0), p),
        }
    }

    fn int_result(&self, v: i128, p: P) -> R<Value> {
        if ty::int_fits(v, p) || self.release {
            Ok(make_int(v, p))
        } else {
            Err(self.fatal("OverflowError", format!("Überlauf: {v} passt nicht in {}", ty::prim_name(p))))
        }
    }

    // ------------------------------------------------------------ Ausdrücke

    fn eval(&mut self, e: &'static ast::Expr) -> R<Value> {
        match &e.kind {
            E::Literal(lit) => self.literal(e, lit, false),
            E::Name(n) => {
                if let Some(c) = self.lookup(&n.name) {
                    let v = c.borrow().view();
                    if matches!(v, Value::Moved | Value::Uninit) {
                        return Err(self.internal(format!("{} ist nicht verfügbar", n.name)));
                    }
                    return Ok(v);
                }
                let this = self.frames.last().and_then(|f| f.this.clone());
                if let Some(o) = this.as_ref().and_then(Value::obj).cloned() {
                    if let Some(d) = o.class {
                        if self.layout(d).index.contains_key(&n.name) {
                            let s = self.field_slot(o, &n.name)?;
                            return Ok(self.get(&s).view());
                        }
                    }
                }
                let class = self.frames.last().and_then(|f| f.class);
                if let Some(d) = class {
                    if let Some((f, owner)) = self.p.field_of(&Ty::Named(d, Vec::new()), &n.name) {
                        if f.is_static {
                            return self.static_field(owner, &n.name);
                        }
                    }
                }
                Err(self.internal(format!("{} ist unbekannt", n.name)))
            }
            E::This => Ok(self.frames.last().and_then(|f| f.this.clone()).unwrap_or(Value::Null)),
            E::Base => Err(self.internal("base")),
            E::Paren(x) => self.eval(x),
            E::Tuple(items) => {
                let mut vs = Vec::new();
                for x in items {
                    vs.push(self.eval_store(x)?);
                }
                Ok(Value::Tuple(vs))
            }
            E::Array(items) => {
                let mut vs = Vec::new();
                for x in items {
                    vs.push(self.eval_store(x)?);
                }
                Ok(Value::Obj(self.new_vec(vs), Form::Own))
            }
            E::New { .. } => {
                let info = self.p.rt.calls.get(&self.key(e)).cloned();
                match info.map(|c| c.target) {
                    Some(Target::Ctor(d, m)) => {
                        let ctor = m.map(|m| Ctor { ast: m.ast, params: m.params });
                        let E::New { args, .. } = &e.kind else { unreachable!() };
                        let params = ctor.as_ref().map(|c| c.params.clone()).unwrap_or_default();
                        let vals = self.args(args, &params)?;
                        // Typargumente mit der Belegung der umgebenden Typ-Parameter (R-7.26)
                        let targs = match self.p.rt.types.get(&self.key(e)) {
                            Some(Ty::Named(_, a)) => {
                                let map = self.tmap();
                                a.iter().map(|t| t.subst(&map)).collect()
                            }
                            _ => Vec::new(),
                        };
                        self.pending_targs = targs;
                        let o = self.new_object(d, ctor, vals)?;
                        Ok(Value::Obj(o, Form::Own))
                    }
                    // new T() (R-7.24): die Klasse ergibt sich aus der Belegung von T
                    Some(Target::NewParam(p)) => match self.tmap().get(&p).cloned() {
                        Some(Ty::Named(d, a)) => {
                            let ctor = self.default_ctor(d);
                            self.pending_targs = a;
                            let o = self.new_object(d, ctor, Vec::new())?;
                            Ok(Value::Obj(o, Form::Own))
                        }
                        _ => Err(self.internal("new T(): T ist zur Laufzeit nicht bekannt (Typargumente generischer Methoden werden noch nicht mitgeführt)")),
                    },
                    _ => Err(self.internal("new ohne Konstruktor")),
                }
            }
            E::Cast { expr, .. } => {
                let v = self.eval(expr)?;
                let t = self.p.rt.types.get(&self.key(e)).cloned().unwrap_or(Ty::Error);
                self.cast(v, &t)
            }
            E::Copy(x) => {
                let v = self.eval(x)?;
                let v = self.temp(v);
                self.deep_copy(&v)
            }
            E::Static(_) => match self.p.rt.calls.get(&self.key(e)).map(|c| c.target.clone()) {
                Some(Target::Variant(d, i)) => Ok(Value::Enum(d, i, Vec::new())),
                Some(Target::StaticField(d, n)) => self.static_field(d, &n),
                _ => {
                    let E::Static(sr) = &e.kind else { unreachable!() };
                    match (&sr.owner, sr.member.name.as_str()) {
                        (ast::StaticOwner::Primitive(p), "MIN") => Ok(make_int(ty::int_range(*p).map(|r| r.0).unwrap_or(0), *p)),
                        (ast::StaticOwner::Primitive(p), "MAX") => Ok(make_int(ty::int_range(*p).map(|r| r.1).unwrap_or(0), *p)),
                        _ => Err(self.internal("statischer Wert")),
                    }
                }
            },
            E::Lambda { own, params, body } => self.lambda(e, *own, params, body),
            E::Unary { op, expr } => self.unary(e, *op, expr),
            E::Binary { op, left, right } => self.binary(*op, left, right),
            E::As { expr, .. } => {
                let v = self.eval(expr)?;
                let t = self.p.rt.types.get(&self.key(e)).cloned().unwrap_or(Ty::Error);
                let target = class_of(&t);
                Ok(match (&v, target) {
                    (Value::Obj(o, _), Some(d)) if o.class.is_some_and(|c| self.is_subclass(c, d)) => v.view(),
                    _ => Value::Null,
                })
            }
            E::Coalesce { left, right } => {
                let l = self.eval(left)?;
                if l.is_null() {
                    self.eval(right)
                } else {
                    Ok(l)
                }
            }
            E::Conditional { cond, then, otherwise } => {
                if self.cond(cond)? {
                    self.eval(then)
                } else {
                    self.eval(otherwise)
                }
            }
            E::Member { object, name, safe } => {
                self.check_place(object, false)?;
                let ov = self.eval(object)?;
                if *safe && ov.is_null() {
                    return Ok(Value::Null);
                }
                let ov = self.temp(ov);
                match (&ov, name.name.as_str()) {
                    (Value::Str(s), "length") => Ok(Value::I64(s.chars().count() as i64)),
                    (Value::Str(s), "byteLength") => Ok(Value::I64(s.len() as i64)),
                    (Value::Obj(o, _), "length") if o.class.is_none() => Ok(Value::I64(o.slots.borrow().len() as i64)),
                    (Value::Slice(_, a, b), "length") => Ok(Value::I64((b - a) as i64)),
                    (Value::Obj(o, _), _) => {
                        let s = self.field_slot(o.clone(), &name.name)?;
                        Ok(self.get(&s).view())
                    }
                    _ => Err(self.internal(format!("Feld {} auf diesem Wert", name.name))),
                }
            }
            E::Call { callee, args } => self.call(e, callee, args),
            E::Index { object, index } => {
                let ov = self.eval(object)?;
                let ov = self.temp(ov);
                let i = self.eval(index)?.as_i128().unwrap_or(-1);
                match &ov {
                    Value::Str(s) => {
                        let n = s.chars().count();
                        match (i >= 0).then(|| s.chars().nth(i as usize)).flatten() {
                            Some(c) => Ok(Value::Char(c)),
                            None => Err(self.fatal("IndexError", format!("Index {i} außerhalb von 0..{n}"))),
                        }
                    }
                    _ => {
                        let s = self.slot_of_index(&ov, i)?;
                        Ok(self.get(&s).view())
                    }
                }
            }
            E::Slice { object, from, to } => {
                let ov = self.eval(object)?;
                let ov = self.temp(ov);
                let (o, base, len) = match &ov {
                    Value::Obj(o, _) => (o.clone(), 0, o.slots.borrow().len()),
                    Value::Slice(o, a, b) => (o.clone(), *a, b - a),
                    _ => return Err(self.internal("Bereich auf diesem Wert")),
                };
                let a = match from {
                    Some(x) => self.eval(x)?.as_i128().unwrap_or(0),
                    None => 0,
                };
                let b = match to {
                    Some(x) => self.eval(x)?.as_i128().unwrap_or(0),
                    None => len as i128,
                };
                if a < 0 || b < a || b as usize > len {
                    return Err(self.fatal("IndexError", format!("Bereich {a}..{b} außerhalb von 0..{len}")));
                }
                Ok(Value::Slice(o, base + a as usize, base + b as usize))
            }
            E::PostInc(x) | E::PostDec(x) => {
                let s = self.slot(x)?;
                let cur = self.get(&s);
                let p = prim_of_value(&cur).unwrap_or(P::I32);
                let d = if matches!(e.kind, E::PostInc(_)) { 1 } else { -1 };
                let nv = self.int_result(cur.as_i128().unwrap_or(0) + d, p)?;
                self.set(&s, nv);
                Ok(cur)
            }
        }
    }

    fn slot_of_index(&mut self, ov: &Value, i: i128) -> R<Slot> {
        match ov {
            Value::Obj(o, _) => {
                let len = o.slots.borrow().len();
                if i < 0 || i as usize >= len {
                    return Err(self.fatal("IndexError", format!("Index {i} außerhalb von 0..{len}")));
                }
                Ok(Slot::Field(o.clone(), i as usize))
            }
            Value::Slice(o, a, b) => {
                if i < 0 || a + i as usize >= *b {
                    return Err(self.fatal("IndexError", format!("Index {i} außerhalb von 0..{}", b - a)));
                }
                Ok(Slot::Field(o.clone(), a + i as usize))
            }
            _ => Err(self.internal("Index auf diesem Wert")),
        }
    }

    fn literal(&mut self, e: &ast::Expr, lit: &ast::Literal, negative: bool) -> R<Value> {
        let t = self.p.rt.types.get(&self.key(e)).cloned();
        let p = t.as_ref().and_then(prim_of);
        Ok(match lit {
            ast::Literal::Int(s) => {
                let v = ty::int_value(s).unwrap_or(0);
                let v = if negative { -v } else { v };
                make_int(v, p.unwrap_or(ty::default_int(v)))
            }
            ast::Literal::Float(s) => {
                let clean: String = s.chars().filter(|&c| c != '_').collect();
                let x: f64 = clean.parse().unwrap_or(0.0);
                let x = if negative { -x } else { x };
                if p == Some(P::F32) {
                    Value::F32(if negative { -(clean.parse::<f32>().unwrap_or(0.0)) } else { clean.parse::<f32>().unwrap_or(0.0) })
                } else {
                    Value::F64(x)
                }
            }
            ast::Literal::Str(s) => Value::Str(Rc::from(unescape(s).as_str())),
            ast::Literal::Char(s) => Value::Char(unescape(s).chars().next().unwrap_or('\0')),
            ast::Literal::Bool(b) => Value::Bool(*b),
            ast::Literal::Null => Value::Null,
        })
    }

    fn static_field(&mut self, d: DefId, name: &str) -> R<Value> {
        if let Some(v) = self.statics.get(&(d, name.to_string())) {
            return Ok(v.view());
        }
        let init = self.members(d).iter().find_map(|m| match m {
            ast::ClassMember::Field(f) if f.is_static && f.name.name == name => f.init.as_ref(),
            _ => None,
        });
        let Some(init) = init else { return Err(self.internal(format!("statisches Feld {name}"))) };
        let file = self.p.defs[d].file;
        self.enter(Frame { scopes: vec![Vec::new()], env: Vec::new(), this: None, class: Some(d), file, label: self.label(d, name), locks: Vec::new(), mtargs: Vec::new() })?;
        let r = self.eval_store(init);
        self.frames.pop();
        let v = r?;
        self.statics.insert((d, name.to_string()), v.clone());
        Ok(v.view())
    }

    fn unary(&mut self, e: &'static ast::Expr, op: U, x: &'static ast::Expr) -> R<Value> {
        match op {
            U::Not => Ok(Value::Bool(!self.eval(x)?.as_bool())),
            U::Neg => {
                if let E::Literal(lit @ (ast::Literal::Int(_) | ast::Literal::Float(_))) = &x.kind {
                    return self.literal(e, lit, true);
                }
                let v = self.eval(x)?;
                match v {
                    Value::F32(f) => Ok(Value::F32(-f)),
                    Value::F64(f) => Ok(Value::F64(-f)),
                    _ => {
                        let p = prim_of_value(&v).unwrap_or(P::I32);
                        self.int_result(-v.as_i128().unwrap_or(0), p)
                    }
                }
            }
            U::BitNot => {
                let v = self.eval(x)?;
                let p = prim_of_value(&v).unwrap_or(P::I32);
                Ok(make_int(!v.as_i128().unwrap_or(0), p))
            }
            U::PreInc | U::PreDec => {
                let s = self.slot(x)?;
                let cur = self.get(&s);
                let p = prim_of_value(&cur).unwrap_or(P::I32);
                let d = if op == U::PreInc { 1 } else { -1 };
                let nv = self.int_result(cur.as_i128().unwrap_or(0) + d, p)?;
                self.set(&s, nv.clone());
                Ok(nv)
            }
        }
    }

    fn binary(&mut self, op: B, l: &'static ast::Expr, r: &'static ast::Expr) -> R<Value> {
        match op {
            B::And => Ok(Value::Bool(self.eval(l)?.as_bool() && self.eval(r)?.as_bool())),
            B::Or => Ok(Value::Bool(self.eval(l)?.as_bool() || self.eval(r)?.as_bool())),
            _ => {
                let a = self.eval(l)?;
                let a = self.temp(a);
                let b = self.eval(r)?;
                let b = self.temp(b);
                match op {
                    B::Eq => Ok(Value::Bool(self.equal(&a, &b)?)),
                    B::Ne => Ok(Value::Bool(!self.equal(&a, &b)?)),
                    B::Lt | B::Gt | B::Le | B::Ge => {
                        let c = self.compare(&a, &b)?;
                        Ok(Value::Bool(match op {
                            B::Lt => c == std::cmp::Ordering::Less,
                            B::Gt => c == std::cmp::Ordering::Greater,
                            B::Le => c != std::cmp::Ordering::Greater,
                            _ => c != std::cmp::Ordering::Less,
                        }))
                    }
                    _ => self.arith(op, &a, &b),
                }
            }
        }
    }

    /// Arithmetik, Bitoperationen und Verschiebung (R-4.4 bis R-4.16, R-12.5).
    fn arith(&mut self, op: B, a: &Value, b: &Value) -> R<Value> {
        if let (Value::Str(x), Value::Str(y), B::Add) = (a, b, op) {
            return Ok(Value::Str(Rc::from(format!("{x}{y}").as_str())));
        }
        if let (Value::Byte(x), Value::Byte(y)) = (a, b) {
            return Ok(Value::Byte(match op {
                B::BitAnd => x & y,
                B::BitOr => x | y,
                _ => x ^ y,
            }));
        }
        if matches!(op, B::Shl | B::Shr) {
            let p = prim_of_value(a).unwrap_or(P::I32);
            let bits: i128 = match p {
                P::I8 | P::Ui8 | P::Byte => 8,
                P::I16 | P::Ui16 => 16,
                P::I32 | P::Ui32 => 32,
                _ => 64,
            };
            let mut n = b.as_i128().unwrap_or(0);
            if n < 0 || n >= bits {
                if !self.release {
                    return Err(self.fatal("OverflowError", format!("Schiebeweite {n} außerhalb von 0..{bits}")));
                }
                n = n.rem_euclid(bits);
            }
            let x = a.as_i128().unwrap_or(0);
            let r = if op == B::Shl { x << n } else { x >> n };
            return Ok(make_int(r, p));
        }
        let (Some(pa), Some(pb)) = (prim_of_value(a), prim_of_value(b)) else {
            return Err(self.internal("Arithmetik auf diesen Werten"));
        };
        let p = ty::common_numeric(pa, pb).unwrap_or(pa);
        if ty::is_float(p) {
            if p == P::F32 {
                let (x, y) = (self.num_to(a, P::F32), self.num_to(b, P::F32));
                let (Value::F32(x), Value::F32(y)) = (x, y) else { unreachable!() };
                return Ok(Value::F32(match op {
                    B::Add => x + y,
                    B::Sub => x - y,
                    B::Mul => x * y,
                    B::Div => x / y,
                    _ => x % y,
                }));
            }
            let (x, y) = (a.as_f64().unwrap_or(0.0), b.as_f64().unwrap_or(0.0));
            return Ok(Value::F64(match op {
                B::Add => x + y,
                B::Sub => x - y,
                B::Mul => x * y,
                B::Div => x / y,
                _ => x % y,
            }));
        }
        let (x, y) = (a.as_i128().unwrap_or(0), b.as_i128().unwrap_or(0));
        let r = match op {
            B::Add => x + y,
            B::Sub => x - y,
            B::Mul => x * y,
            B::Div | B::Rem => {
                if y == 0 {
                    return Err(self.fatal("DivisionError", "Division durch null"));
                }
                if op == B::Div {
                    x / y
                } else {
                    x % y
                }
            }
            B::BitAnd => x & y,
            B::BitOr => x | y,
            B::BitXor => x ^ y,
            _ => return Err(self.internal("Operator")),
        };
        if matches!(op, B::BitAnd | B::BitOr | B::BitXor) {
            return Ok(make_int(r, p));
        }
        self.int_result(r, p)
    }

    fn equal(&mut self, a: &Value, b: &Value) -> R<bool> {
        Ok(match (a, b) {
            (Value::Null, Value::Null) => true,
            (Value::Null, _) | (_, Value::Null) => false,
            (Value::Bool(x), Value::Bool(y)) => x == y,
            (Value::Char(x), Value::Char(y)) => x == y,
            (Value::Byte(x), Value::Byte(y)) => x == y,
            (Value::Str(x), Value::Str(y)) => x == y,
            (Value::Enum(d1, i1, _), Value::Enum(d2, i2, _)) => d1 == d2 && i1 == i2,
            (Value::Obj(o, _), Value::Obj(q, _)) => {
                let Some(d) = o.class else { return Ok(Rc::ptr_eq(o, q)) };
                let m = self.p.methods_of(&Ty::Named(d, Vec::new()), "equals").into_iter().find(|m| m.params.len() == 1 && m.has_body);
                match m {
                    Some(m) => self.call_method(&m, Some(a.view()), vec![b.view()])?.as_bool(),
                    None => Rc::ptr_eq(o, q),
                }
            }
            _ => match (a.as_f64(), b.as_f64(), prim_of_value(a), prim_of_value(b)) {
                (Some(_), Some(_), Some(pa), Some(pb)) => {
                    let p = ty::common_numeric(pa, pb).unwrap_or(pa);
                    if ty::is_float(p) {
                        a.as_f64() == b.as_f64()
                    } else {
                        a.as_i128() == b.as_i128()
                    }
                }
                _ => false,
            },
        })
    }

    fn compare(&mut self, a: &Value, b: &Value) -> R<std::cmp::Ordering> {
        use std::cmp::Ordering;
        Ok(match (a, b) {
            (Value::Char(x), Value::Char(y)) => x.cmp(y),
            (Value::Byte(x), Value::Byte(y)) => x.cmp(y),
            (Value::Str(x), Value::Str(y)) => x.cmp(y),
            (Value::Obj(o, _), _) => {
                let Some(d) = o.class else { return Err(self.internal("Vergleich von Vektoren")) };
                let m = self.p.methods_of(&Ty::Named(d, Vec::new()), "compareTo").into_iter().find(|m| m.params.len() == 1 && m.has_body);
                let Some(m) = m else { return Err(self.internal("compareTo fehlt")) };
                let r = self.call_method(&m, Some(a.view()), vec![b.view()])?.as_i128().unwrap_or(0);
                r.cmp(&0)
            }
            _ => {
                let (pa, pb) = (prim_of_value(a).unwrap_or(P::I32), prim_of_value(b).unwrap_or(P::I32));
                let p = ty::common_numeric(pa, pb).unwrap_or(pa);
                if ty::is_float(p) {
                    a.as_f64().partial_cmp(&b.as_f64()).unwrap_or(Ordering::Equal)
                } else {
                    a.as_i128().cmp(&b.as_i128())
                }
            }
        })
    }

    /// `cast<T>(e)` (R-4.8 bis R-4.10, R-12.5).
    fn cast(&mut self, v: Value, t: &Ty) -> R<Value> {
        if let Some(d) = class_of(t) {
            // Ganzzahl → Enum ohne Daten
            let def = &self.p.defs[d];
            let n = v.as_i128().unwrap_or(-1);
            for (i, var) in def.variants.iter().enumerate() {
                if var.value.map(|x| x as i128).unwrap_or(i as i128) == n {
                    return Ok(Value::Enum(d, i, Vec::new()));
                }
            }
            return Err(self.fatal("CastError", format!("{n} ist kein Wert von {}", def.name)));
        }
        let Some(p) = prim_of(t) else { return Ok(v) };
        if let Value::Enum(d, i, _) = &v {
            let n = self.p.defs[*d].variants[*i].value.map(|x| x as i128).unwrap_or(*i as i128);
            return self.cast(make_int(n, P::I64), t);
        }
        if let Value::Char(c) = v {
            return self.cast(Value::U32(c as u32), t);
        }
        if let Value::Byte(b) = v {
            return Ok(make_int(b as i128, p));
        }
        match p {
            P::Char => {
                let n = v.as_i128().unwrap_or(-1);
                match u32::try_from(n).ok().and_then(char::from_u32) {
                    Some(c) => Ok(Value::Char(c)),
                    None => Err(self.fatal("CastError", format!("{n} ist kein Unicode-Zeichen"))),
                }
            }
            P::Byte => Ok(Value::Byte(v.as_i128().unwrap_or(0) as u8)),
            P::F32 | P::F64 => Ok(self.num_to(&v, p)),
            _ if ty::is_integer(p) => {
                let (lo, hi) = ty::int_range(p).unwrap();
                match v {
                    Value::F32(_) | Value::F64(_) => {
                        let x = v.as_f64().unwrap_or(0.0);
                        if x.is_nan() || x.trunc() < lo as f64 || x.trunc() > hi as f64 {
                            if !self.release {
                                return Err(self.fatal("CastError", format!("{} passt nicht in {}", fmt_f64(x), ty::prim_name(p))));
                            }
                            let s = if x.is_nan() { 0 } else if x < 0.0 { lo } else { hi };
                            return Ok(make_int(s, p));
                        }
                        Ok(make_int(x.trunc() as i128, p))
                    }
                    _ => {
                        let n = v.as_i128().unwrap_or(0);
                        if (n < lo || n > hi) && !self.release {
                            return Err(self.fatal("CastError", format!("{n} passt nicht in {}", ty::prim_name(p))));
                        }
                        Ok(make_int(n, p))
                    }
                }
            }
            _ => Ok(v),
        }
    }

    fn deep_copy(&mut self, v: &Value) -> R<Value> {
        match v {
            Value::Obj(o, _) => match o.class {
                None => {
                    let elems: Vec<Value> = o.slots.borrow().clone();
                    let mut out = Vec::new();
                    for x in &elems {
                        out.push(match x {
                            Value::Obj(..) | Value::Tuple(_) | Value::Enum(..) => self.deep_copy(x)?,
                            other => other.clone(),
                        });
                    }
                    Ok(Value::Obj(self.new_vec(out), Form::Own))
                }
                Some(d) => {
                    let lay = self.layout(d);
                    let n = Rc::new(Obj { class: Some(d), slots: RefCell::new(vec![Value::Uninit; lay.len]), rc: Cell::new(0), alive: Cell::new(true), dying: Cell::new(false), lock: Cell::new(0), targs: RefCell::new(o.targs.borrow().clone()), index: Default::default() });
                    self.register(&n);
                    // copy-Rümpfe von der Wurzel bis zur Klasse des Objekts (R-7.11)
                    for (c, range) in lay.levels.clone() {
                        self.init_fields(&n, c)?;
                        let body = self.members(c).iter().find_map(|m| match m {
                            ast::ClassMember::Copy(param, b, _) => Some((param, b)),
                            _ => None,
                        });
                        match body {
                            Some((param, b)) => {
                                let file = self.p.defs[c].file;
                                self.enter(Frame { scopes: vec![Vec::new()], env: Vec::new(), this: Some(Value::Obj(n.clone(), Form::Borrow)), class: Some(c), file, label: self.label(c, "copy"), locks: Vec::new(), mtargs: Vec::new() })?;
                                self.declare(&param.name.name, v.view());
                                let r = self.block(b);
                                self.leave(r)?;
                            }
                            None => {
                                for k in range {
                                    let x = o.slots.borrow()[k].clone();
                                    let x = match &x {
                                        Value::Obj(_, Form::Own) => self.deep_copy(&x)?,
                                        _ => self.own_copy(x),
                                    };
                                    n.slots.borrow_mut()[k] = x;
                                }
                            }
                        }
                    }
                    Ok(Value::Obj(n, Form::Own))
                }
            },
            Value::Tuple(vs) => {
                let mut out = Vec::new();
                for x in vs {
                    out.push(self.deep_copy(x)?);
                }
                Ok(Value::Tuple(out))
            }
            other => Ok(other.clone()),
        }
    }

    // ------------------------------------------------------------ Objekte

    /// `new C(…)`: Basisanteil, Feldinitialisierer, Konstruktorrumpf (R-7.6, R-9.13).
    /// Belegung der Typ-Parameter im aktuellen Rahmen: aus den Typargumenten von `this`,
    /// gesehen von der Klasse, zu der der Rahmen gehört (auch über Basisklassen).
    fn tmap(&self) -> HashMap<usize, Ty> {
        let mut map = HashMap::new();
        let Some(f) = self.frames.last() else { return map };
        for (p, t) in &f.mtargs {
            map.insert(*p, t.clone());
        }
        let (Some(c), Some(Value::Obj(o, _))) = (f.class, &f.this) else { return map };
        let Some(dynamic) = o.class else { return map };
        let st = Ty::Named(dynamic, o.targs.borrow().clone());
        let seen = std::iter::once(st.clone()).chain(self.p.supertypes(&st)).find(|t| matches!(t, Ty::Named(x, _) if *x == c));
        if let Some(Ty::Named(_, args)) = seen {
            for (p, a) in self.p.defs[c].tparams.iter().zip(args) {
                map.insert(*p, a);
            }
        }
        map
    }

    fn new_object(&mut self, d: DefId, ctor: Option<Ctor>, args: Vec<Value>) -> R<ORef> {
        let targs = std::mem::take(&mut self.pending_targs);
        let lay = self.layout(d);
        let o = Rc::new(Obj { class: Some(d), slots: RefCell::new(vec![Value::Uninit; lay.len]), rc: Cell::new(0), alive: Cell::new(true), dying: Cell::new(false), lock: Cell::new(0), targs: RefCell::new(targs), index: Default::default() });
        self.register(&o);
        match self.construct(&o, d, ctor, args) {
            Ok(()) => Ok(o),
            Err(e) => {
                o.alive.set(false);
                Err(e)
            }
        }
    }

    fn init_fields(&mut self, o: &ORef, c: DefId) -> R<()> {
        let file = self.p.defs[c].file;
        let lay = self.layout(o.class.unwrap());
        for m in self.members(c) {
            let ast::ClassMember::Field(f) = m else { continue };
            if f.is_static {
                continue;
            }
            let Some(i) = lay.index.get(&f.name.name).copied() else { continue };
            let v = match &f.init {
                Some(init) => {
                    self.enter(Frame { scopes: vec![Vec::new()], env: Vec::new(), this: Some(Value::Obj(o.clone(), Form::Borrow)), class: Some(c), file, label: self.label(c, &f.name.name), locks: Vec::new(), mtargs: Vec::new() })?;
                    let r = self.eval_store(init);
                    self.frames.pop();
                    r?
                }
                None => {
                    let fty = self.p.defs[c].fields.iter().find(|x| x.name == f.name.name).map(|x| x.ty.clone());
                    if fty.as_ref().is_some_and(is_nullable_ty) {
                        Value::Null
                    } else {
                        continue;
                    }
                }
            };
            o.slots.borrow_mut()[i] = v;
        }
        Ok(())
    }

    fn construct(&mut self, o: &ORef, c: DefId, ctor: Option<Ctor>, args: Vec<Value>) -> R<()> {
        let lay = self.layout(o.class.unwrap());
        let level = lay.levels.iter().position(|(x, _)| *x == c).unwrap_or(0);
        let decl = ctor.as_ref().and_then(|k| match self.member_at(k.ast) {
            Some(ast::ClassMember::Constructor(cd)) => Some(cd),
            _ => None,
        });
        let file = ctor.as_ref().map(|k| k.ast.0).unwrap_or(self.p.defs[c].file);
        self.enter(Frame { scopes: vec![Vec::new()], env: Vec::new(), this: Some(Value::Obj(o.clone(), Form::Borrow)), class: Some(c), file, label: self.label(c, "constructor"), locks: Vec::new(), mtargs: Vec::new() })?;
        if let Some(k) = &ctor {
            for (pi, v) in k.params.iter().zip(args) {
                self.declare(&pi.name, v);
            }
        }
        // 1. Basisanteil
        let mut base_done = false;
        let r: R<()> = (|| {
            if let Some(b) = self.base_of(c) {
                let (bctor, bargs) = match decl.and_then(|cd| cd.base_args.as_ref().map(|a| (cd, a))) {
                    Some((cd, a)) => {
                        let m = self.p.rt.base_ctors.get(&(file, cd.span)).cloned().flatten();
                        let k = m.map(|m| Ctor { ast: m.ast, params: m.params });
                        let params = k.as_ref().map(|k| k.params.clone()).unwrap_or_default();
                        self.temps.push(Vec::new());
                        let vals = self.args(a, &params);
                        let temps = self.temps.pop().unwrap_or_default();
                        for t in temps {
                            self.drop_value(t)?;
                        }
                        (k, vals?)
                    }
                    None => (self.default_ctor(b), Vec::new()),
                };
                self.construct(o, b, bctor, bargs)?;
                base_done = true;
            }
            // 2. Feldinitialisierer, 3. Rumpf
            self.init_fields(o, c)?;
            if let Some(cd) = decl {
                self.block(&cd.body)?;
            }
            Ok(())
        })();
        let left = self.leave(r);
        if let Err(e) = left {
            // R-9.13: Destruktoren des fertigen Basisanteils, dann die gesetzten Felder (V-7)
            if base_done && level > 0 {
                let levels: Vec<usize> = (0..level).rev().collect();
                for i in levels {
                    let c = lay.levels[i].0;
                    self.run_destructor(o, c)?;
                }
            }
            self.drop_fields(o, lay.levels[level].1.end)?;
            o.dying.set(true);
            return Err(e);
        }
        Ok(())
    }

    fn default_ctor(&self, d: DefId) -> Option<Ctor> {
        self.p.defs[d].ctors.iter().find(|c| c.params.is_empty()).map(|c| Ctor { ast: c.ast, params: c.params.clone() })
    }

    // ------------------------------------------------------------ Aufrufe

    /// Argumente nach den Parametern: `own` und shared-Ziele übernehmen, sonst leihen.
    fn args(&mut self, args: &'static [ast::Arg], params: &[PInfo]) -> R<Vec<Value>> {
        let locked = std::mem::take(&mut self.locked_args);
        let mut out = Vec::new();
        for (i, a) in args.iter().enumerate() {
            // own- und shared-/weak-Parameter übernehmen den Wert, auch aus `a ?? b`, `c ? a : b`, `(a)`
            let takes = params.get(i).is_some_and(|p| p.mode == Mode::Own || is_shared_ty(&p.ty) || is_weak_ty(&p.ty));
            let v = if takes || self.p.rt.consumes.contains(&self.key(&a.expr)) {
                self.eval_store(&a.expr)?
            } else {
                // Ein geliehener Ort mit shared-Wurzel ist für den Aufruf gesperrt; er wird
                // gebunden, nicht gelesen (wie im FR++-Modell).
                self.binding.set(locked.contains(&i));
                let v = self.eval(&a.expr);
                self.binding.set(false);
                let v = self.temp(v?);
                match self.p.rt.stores.get(&self.key(&a.expr)).cloned() {
                    Some(t) => self.convert(v, &t)?,
                    None => v,
                }
            };
            let v = match params.get(i) {
                Some(p) if prim_of(&p.ty).is_some() => self.convert(v, &p.ty)?,
                _ => v,
            };
            out.push(v);
        }
        Ok(out)
    }

    fn call(&mut self, e: &'static ast::Expr, callee: &'static ast::Expr, args: &'static [ast::Arg]) -> R<Value> {
        let Some(info) = self.p.rt.calls.get(&self.key(e)).cloned() else {
            // direkt aufgerufenes Lambda
            let f = self.eval(callee)?;
            return self.call_value(f, args);
        };
        match info.target {
            Target::Builtin(name) => self.builtin(&name, args),
            Target::FuncValue => {
                let f = self.eval(callee)?;
                self.call_value(f, args)
            }
            Target::Variant(d, i) => {
                let mut vs = Vec::new();
                for a in args {
                    vs.push(self.eval_store(&a.expr)?);
                }
                Ok(Value::Enum(d, i, vs))
            }
            Target::Lock => {
                let E::Member { object, safe, .. } = &callee.kind else { return Err(self.internal("lock")) };
                let w = self.eval(object)?;
                if *safe && w.is_null() {
                    return Ok(Value::Null);
                }
                Ok(match w {
                    Value::Obj(o, _) if o.alive.get() && o.rc.get() > 0 => {
                        o.rc.set(o.rc.get() + 1);
                        Value::Obj(o, Form::Shared)
                    }
                    _ => Value::Null,
                })
            }
            Target::BuiltinMethod(name) => {
                let E::Member { object, safe, .. } = &callee.kind else { return Err(self.internal("eingebaute Methode")) };
                let recv = self.eval(object)?;
                if *safe && recv.is_null() {
                    return Ok(Value::Null);
                }
                let recv = self.temp(recv);
                let vals = self.args(args, &info.params)?;
                self.builtin_method(&recv, &name, vals)
            }
            Target::BuiltinStatic(p, name) => {
                let vals = self.args(args, &info.params)?;
                self.builtin_static(p, &name, vals)
            }
            Target::Method(m) | Target::Exact(m) => {
                let exact = matches!(self.p.rt.calls.get(&self.key(e)).map(|c| &c.target), Some(Target::Exact(_)));
                let recv = if m.is_static {
                    None
                } else {
                    match &callee.kind {
                        E::Member { object, safe, .. } if !matches!(object.kind, E::Base) => {
                            let r = self.eval(object)?;
                            if *safe && r.is_null() {
                                return Ok(Value::Null);
                            }
                            Some(self.temp(r))
                        }
                        _ => self.frames.last().and_then(|f| f.this.clone()),
                    }
                };
                // V-1: Empfänger und geliehene Argumente mit shared-Wurzel sind während des Aufrufs gesperrt.
                let mut held: Vec<(ORef, bool)> = Vec::new();
                if let E::Member { object, .. } = &callee.kind {
                    if let Some(o) = self.shared_root(object) {
                        self.acquire(&o, m.mut_this).map_err(|f| self.at(f, object))?;
                        held.push((o, m.mut_this));
                    }
                }
                let r = self.locked_call(args, m, recv, exact, &mut held);
                for (o, mutable) in held.iter().rev() {
                    Self::release(o, *mutable);
                }
                r
            }
            Target::Ctor(..) | Target::NewParam(_) | Target::StaticField(..) => Err(self.internal("Aufrufziel")),
        }
    }

    fn locked_call(&mut self, args: &'static [ast::Arg], m: MethodInfo, recv: Option<Value>, exact: bool, held: &mut Vec<(ORef, bool)>) -> R<Value> {
        // Geliehene Objektargumente sperren (R-5.9 zur Laufzeit, V-1); primitive Werte werden gelesen.
        let mut locked = Vec::new();
        for (i, (a, p)) in args.iter().zip(&m.params).enumerate() {
            if p.mode != Mode::Own && prim_of(&p.ty).is_none() && !self.p.rt.consumes.contains(&self.key(&a.expr)) {
                if let Some(o) = self.shared_root(&a.expr) {
                    let mutable = p.mode == Mode::Mut;
                    self.acquire(&o, mutable).map_err(|f| self.at(f, &a.expr))?;
                    held.push((o, mutable));
                    locked.push(i);
                }
            }
        }
        self.locked_args = locked;
        let vals = self.args(args, &m.params)?;
        let targs = m.targs.clone();
        let mut target = match (&recv, exact) {
            (Some(Value::Obj(o, _)), false) => o.class.and_then(|d| self.dispatch(d, &m)).unwrap_or(m),
            _ => m,
        };
        // die Fassung der Unterklasse hat dieselben Typ-Parameter; die Typargumente des Aufrufs gelten weiter
        if target.targs.is_empty() && target.tparams.len() == targs.len() {
            target.targs = targs;
        }
        self.call_method(&target, recv.map(|r| r.view()), vals)
    }

    /// Fassung der Methode in der Klasse des Objekts (virtueller Aufruf, R-7.13, R-7.22).
    fn dispatch(&self, d: DefId, m: &MethodInfo) -> Option<MethodInfo> {
        self.p.methods_of(&Ty::Named(d, Vec::new()), &m.name).into_iter().find(|c| {
            c.has_body
                && c.params.len() == m.params.len()
                && c.params.iter().zip(&m.params).all(|(a, b)| a.mode == b.mode && (a.ty.unformed() == b.ty.unformed() || a.ty.mentions_param() || b.ty.mentions_param()))
        })
    }

    fn call_method(&mut self, m: &MethodInfo, this: Option<Value>, args: Vec<Value>) -> R<Value> {
        if let Some(v) = this.clone().filter(|v| prim_of_value(v).is_some()) {
            if let Some(r) = self.prim_mixin(&m.name, &v, &args) {
                return r;
            }
        }
        if let Some(n) = self.native_name(m) {
            return self.native(&n, m, this, args);
        }
        let Some(ast::ClassMember::Method(md)) = self.member_at(m.ast) else {
            return Err(self.internal(format!("Methode {} ohne Rumpf", m.name)));
        };
        let Some(body) = &md.body else { return Err(self.internal(format!("Methode {} ohne Rumpf", m.name))) };
        // Typargumente einer generischen Methode, gesehen vom Aufrufer (R-7.26)
        let outer = self.tmap();
        let mtargs: Vec<(usize, Ty)> = m.tparams.iter().zip(&m.targs).map(|(p, t)| (*p, t.subst(&outer))).collect();
        self.enter(Frame { scopes: vec![Vec::new()], env: Vec::new(), this, class: Some(m.owner).filter(|o| *o != usize::MAX), file: m.ast.0, label: self.label(m.owner, &m.name), locks: Vec::new(), mtargs })?;
        for (p, v) in m.params.iter().zip(args) {
            self.declare(&p.name, v);
        }
        let r = self.block(body);
        let v = self.leave(r)?;
        match &m.ret {
            Ret::Val(t) => self.convert(v, t),
            _ => Ok(v),
        }
    }

    fn lambda(&mut self, e: &'static ast::Expr, own: bool, params: &'static [ast::Param], body: &'static ast::LambdaBody) -> R<Value> {
        let ps: Vec<(String, Mode)> = params
            .iter()
            .map(|p| (p.name.name.clone(), if p.mutable { Mode::Mut } else if p.ty.ownership == Some(ast::Ownership::Own) { Mode::Own } else { Mode::Read }))
            .collect();
        let (class, file, this) = {
            let f = self.frames.last().unwrap();
            (f.class, f.file, f.this.clone())
        };
        let env: Vec<(String, Cellv)> = if own {
            // R-10.8: Erfassung per Wert, own-Werte werden verschoben
            let names = self.p.rt.captures.get(&self.key(e)).cloned().unwrap_or_default();
            let mut env = Vec::new();
            for n in names {
                let Some(c) = self.lookup(&n) else { continue };
                let cur = c.borrow().clone();
                let v = match cur {
                    Value::Tuple(_) | Value::Enum(..) if self.plain_value(&cur) => cur,
                    Value::Obj(_, Form::Own) | Value::Tuple(_) | Value::Enum(..) => c.replace(Value::Moved),
                    Value::Func(ref g) if g.own => c.replace(Value::Moved),
                    other => self.own_copy(other),
                };
                env.push((n, Rc::new(RefCell::new(v))));
            }
            env
        } else {
            let f = self.frames.last().unwrap();
            let mut env = f.env.clone();
            for s in &f.scopes {
                env.extend(s.iter().cloned());
            }
            env
        };
        let mtargs = self.frames.last().map(|f| f.mtargs.clone()).unwrap_or_default();
        Ok(Value::Func(Rc::new(Closure { own, params: ps, body, file, class, env, this: if own { None } else { this }, mtargs })))
    }

    fn call_value(&mut self, f: Value, args: &'static [ast::Arg]) -> R<Value> {
        let Value::Func(c) = f else { return Err(self.internal("Aufruf eines Werts, der keine Funktion ist")) };
        let mut vals = Vec::new();
        for (i, a) in args.iter().enumerate() {
            let takes = c.params.get(i).is_some_and(|(_, m)| *m == Mode::Own);
            let v = if takes || self.p.rt.consumes.contains(&self.key(&a.expr)) {
                self.eval_store(&a.expr)?
            } else {
                let v = self.eval(&a.expr)?;
                let v = self.temp(v);
                match self.p.rt.stores.get(&self.key(&a.expr)).cloned() {
                    Some(t) => self.convert(v, &t)?,
                    None => v,
                }
            };
            vals.push(v);
        }
        self.call_closure(&c, vals)
    }

    fn call_closure(&mut self, c: &Rc<Closure>, vals: Vec<Value>) -> R<Value> {
        self.enter(Frame { scopes: vec![Vec::new()], env: c.env.clone(), this: c.this.clone(), class: c.class, file: c.file, label: "Lambda".into(), locks: Vec::new(), mtargs: c.mtargs.clone() })?;
        for ((n, _), v) in c.params.iter().zip(vals) {
            self.declare(n, v);
        }
        let r = match c.body {
            ast::LambdaBody::Expr(x) => match self.eval_store(x) {
                Ok(v) => Err(Flow::Return(v)),
                Err(e) => Err(e),
            },
            ast::LambdaBody::Block(b) => self.block(b),
        };
        self.leave(r)
    }

    // ------------------------------------------------------------ Eingebautes

    fn builtin(&mut self, name: &str, args: &'static [ast::Arg]) -> R<Value> {
        match name {
            "print" => {
                let v = self.eval(&args[0].expr)?;
                self.out.push_str(&to_text(&v));
                self.out.push('\n');
                Ok(Value::Void)
            }
            "assert" => {
                if !self.eval(&args[0].expr)?.as_bool() {
                    let msg = self.eval(&args[1].expr)?;
                    return Err(self.fatal("AssertError", to_text(&msg)));
                }
                Ok(Value::Void)
            }
            "fail" => {
                let msg = self.eval(&args[0].expr)?;
                Err(self.fatal("FailError", to_text(&msg)))
            }
            "take" => {
                self.check_place(&args[0].expr, true)?;
                let s = self.slot(&args[0].expr)?;
                Ok(self.set(&s, Value::Null))
            }
            "replace" => {
                let v = self.eval_store(&args[1].expr)?;
                self.check_place(&args[0].expr, true)?;
                let s = self.slot(&args[0].expr)?;
                Ok(self.set(&s, v))
            }
            "same" => {
                let a = self.eval(&args[0].expr)?;
                let b = self.eval(&args[1].expr)?;
                Ok(Value::Bool(matches!((a.obj(), b.obj()), (Some(x), Some(y)) if Rc::ptr_eq(x, y))))
            }
            _ => Err(self.internal(format!("eingebaute Funktion {name}"))),
        }
    }

    fn builtin_method(&mut self, recv: &Value, name: &str, args: Vec<Value>) -> R<Value> {
        if name == "partitionPoint" {
            let (items, start, end) = match recv {
                Value::Obj(o, _) => (o, 0, o.slots.borrow().len()),
                Value::Slice(o, start, end) => (o, *start, *end),
                _ => return Err(self.internal("partitionPoint braucht einen Vektor oder Slice")),
            };
            let Some(Value::Func(predicate)) = args.first() else { return Err(self.internal("Prädikat fehlt")) };
            let (mut lo, mut hi) = (start, end);
            while lo < hi {
                let mid = lo + (hi - lo) / 2;
                let item = items.slots.borrow().get(mid).map(Value::view)
                    .ok_or_else(|| self.fatal("IndexError", "ungültiger Bereich bei partitionPoint"))?;
                if self.call_closure(predicate, vec![item])?.as_bool() {
                    lo = mid + 1;
                } else {
                    hi = mid;
                }
            }
            return Ok(Value::I64((lo - start) as i64));
        }
        let s = |x: String| Value::Str(Rc::from(x.as_str()));
        if let Value::Str(st) = recv {
            let arg_str = |i: usize| args.get(i).and_then(|a| a.as_str().map(str::to_string)).unwrap_or_default();
            return Ok(match name {
                "toString" => recv.clone(),
                "trim" => s(st.trim().to_string()),
                "toUpper" => s(st.to_uppercase()),
                "toLower" => s(st.to_lowercase()),
                "startsWith" => Value::Bool(st.starts_with(arg_str(0).as_str())),
                "endsWith" => Value::Bool(st.ends_with(arg_str(0).as_str())),
                "contains" => Value::Bool(st.contains(arg_str(0).as_str())),
                "indexOf" => match st.find(arg_str(0).as_str()) {
                    Some(b) => Value::I64(st[..b].chars().count() as i64),
                    None => Value::Null,
                },
                "replace" => s(st.replace(arg_str(0).as_str(), arg_str(1).as_str())),
                "substring" => {
                    let (a, b) = (args[0].as_i128().unwrap_or(0), args[1].as_i128().unwrap_or(0));
                    let n = st.chars().count() as i128;
                    if a < 0 || b < a || b > n {
                        return Err(self.fatal("IndexError", format!("Bereich {a}..{b} außerhalb von 0..{n}")));
                    }
                    s(st.chars().skip(a as usize).take((b - a) as usize).collect())
                }
                "split" => {
                    let parts: Vec<Value> = st.split(arg_str(0).as_str()).map(|x| Value::Str(Rc::from(x))).collect();
                    Value::Obj(self.new_vec(parts), Form::Own)
                }
                "bytes" => {
                    let bs: Vec<Value> = st.bytes().map(Value::Byte).collect();
                    Value::Obj(self.new_vec(bs), Form::Own)
                }
                "byteAt" => {
                    let i = args[0].as_i128().unwrap_or(-1);
                    match (i >= 0).then(|| st.as_bytes().get(i as usize)).flatten() {
                        Some(b) => Value::Byte(*b),
                        None => return Err(self.fatal("IndexError", format!("Index {i} außerhalb von 0..{}", st.len()))),
                    }
                }
                _ => return Err(self.internal(format!("string.{name}"))),
            });
        }
        if let Value::Char(c) = recv {
            return Ok(match name {
                "toString" => s(c.to_string()),
                "isLetter" => Value::Bool(c.is_alphabetic()),
                "isDigit" => Value::Bool(c.is_ascii_digit()),
                "isWhitespace" => Value::Bool(c.is_whitespace()),
                "isUpper" => Value::Bool(c.is_uppercase()),
                "isLower" => Value::Bool(c.is_lowercase()),
                "toUpper" => Value::Char(c.to_uppercase().next().unwrap_or(*c)),
                "toLower" => Value::Char(c.to_lowercase().next().unwrap_or(*c)),
                _ => return Err(self.internal(format!("char.{name}"))),
            });
        }
        if let Value::Obj(o, _) = recv {
            if o.class.is_none() {
                return self.vec_method(o, name, args);
            }
        }
        match name {
            "toString" => Ok(s(to_text(recv))),
            "abs" => match recv {
                Value::F32(x) => Ok(Value::F32(x.abs())),
                Value::F64(x) => Ok(Value::F64(x.abs())),
                _ => {
                    let p = prim_of_value(recv).unwrap_or(P::I32);
                    self.int_result(recv.as_i128().unwrap_or(0).abs(), p)
                }
            },
            "min" | "max" => {
                let o = self.compare(recv, &args[0])?;
                let take_recv = (name == "min") == (o != std::cmp::Ordering::Greater);
                Ok(if take_recv { recv.clone() } else { args[0].clone() })
            }
            _ => Err(self.internal(format!("Methode {name}"))),
        }
    }

    fn vec_method(&mut self, o: &ORef, name: &str, mut args: Vec<Value>) -> R<Value> {
        let len = o.slots.borrow().len();
        match name {
            "push" => {
                o.slots.borrow_mut().push(args.remove(0));
                Ok(Value::Void)
            }
            "insert" => {
                let i = args[0].as_i128().unwrap_or(-1);
                if i < 0 || i as usize > len {
                    return Err(self.fatal("IndexError", format!("Index {i} außerhalb von 0..{}", len + 1)));
                }
                let v = args.remove(1);
                o.slots.borrow_mut().insert(i as usize, v);
                Ok(Value::Void)
            }
            "pop" => match o.slots.borrow_mut().pop() {
                Some(v) => Ok(v),
                None => Err(self.fatal("IndexError", "pop auf einem leeren Vektor")),
            },
            "remove" => {
                let i = args[0].as_i128().unwrap_or(-1);
                if i < 0 || i as usize >= len {
                    return Err(self.fatal("IndexError", format!("Index {i} außerhalb von 0..{len}")));
                }
                Ok(o.slots.borrow_mut().remove(i as usize))
            }
            "clear" => {
                let elems = std::mem::take(&mut *o.slots.borrow_mut());
                for x in elems {
                    self.drop_value(x)?;
                }
                Ok(Value::Void)
            }
            "first" => Ok(o.slots.borrow().first().map(Value::view).unwrap_or(Value::Null)),
            "last" => Ok(o.slots.borrow().last().map(Value::view).unwrap_or(Value::Null)),
            "contains" | "indexOf" => {
                let elems: Vec<Value> = o.slots.borrow().iter().map(Value::view).collect();
                for (i, x) in elems.iter().enumerate() {
                    if self.equal(x, &args[0])? {
                        return Ok(if name == "contains" { Value::Bool(true) } else { Value::I64(i as i64) });
                    }
                }
                Ok(if name == "contains" { Value::Bool(false) } else { Value::Null })
            }
            "sort" | "sortBy" => {
                let mut elems = std::mem::take(&mut *o.slots.borrow_mut());
                let f = args.first().cloned();
                // stabiles Einfügesortieren mit Vergleichen über den Interpreter
                let mut i = 1;
                while i < elems.len() {
                    let mut j = i;
                    while j > 0 {
                        let ord = match &f {
                            Some(Value::Func(c)) => {
                                let r = self.call_closure(c, vec![elems[j - 1].view(), elems[j].view()])?;
                                r.as_i128().unwrap_or(0).cmp(&0)
                            }
                            _ => self.compare(&elems[j - 1].view(), &elems[j].view())?,
                        };
                        if ord == std::cmp::Ordering::Greater {
                            elems.swap(j - 1, j);
                            j -= 1;
                        } else {
                            break;
                        }
                    }
                    i += 1;
                }
                *o.slots.borrow_mut() = elems;
                Ok(Value::Void)
            }
            _ => Err(self.internal(format!("Vektor.{name}"))),
        }
    }

    fn builtin_static(&mut self, p: P, name: &str, args: Vec<Value>) -> R<Value> {
        match name {
            "parse" | "tryParse" => {
                let text = args[0].as_str().unwrap_or("").to_string();
                let parsed: Option<Value> = if ty::is_integer(p) {
                    text.trim().parse::<i128>().ok().filter(|v| ty::int_fits(*v, p)).map(|v| make_int(v, p))
                } else {
                    text.trim().parse::<f64>().ok().map(|x| self.num_to(&Value::F64(x), p))
                };
                match (parsed, name) {
                    (Some(v), _) => Ok(v),
                    (None, "tryParse") => Ok(Value::Null),
                    (None, _) => self.throw_std("rpp.core.FormatException", &format!("keine Zahl: {text}")),
                }
            }
            "join" => {
                let sep = args[0].as_str().unwrap_or("").to_string();
                let parts: Vec<String> = match &args[1] {
                    Value::Obj(o, _) => o.slots.borrow().iter().map(to_text).collect(),
                    Value::Slice(o, a, b) => o.slots.borrow()[*a..*b].iter().map(to_text).collect(),
                    _ => Vec::new(),
                };
                Ok(Value::Str(Rc::from(parts.join(&sep).as_str())))
            }
            "fromUtf8" => {
                let bytes: Vec<u8> = match &args[0] {
                    Value::Obj(o, _) => o.slots.borrow().iter().map(|b| b.as_i128().unwrap_or(match b {
                        Value::Byte(x) => *x as i128,
                        _ => 0,
                    }) as u8).collect(),
                    _ => Vec::new(),
                };
                match String::from_utf8(bytes) {
                    Ok(s) => Ok(Value::Str(Rc::from(s.as_str()))),
                    Err(_) => self.throw_std("rpp.core.FormatException", "kein gültiges UTF-8"),
                }
            }
            _ => Err(self.internal(format!("{}::{name}", ty::prim_name(p)))),
        }
    }

    /// Exception einer Klasse der Standardbibliothek mit einem Text werfen.
    fn throw_std(&mut self, class: &str, msg: &str) -> R<Value> {
        let Some(d) = self.p.std_def(class) else { return Err(self.internal(format!("{class} fehlt"))) };
        let ctor = self.p.defs[d].ctors.iter().find(|c| c.params.len() == 1).map(|c| Ctor { ast: c.ast, params: c.params.clone() });
        let o = self.new_object(d, ctor, vec![Value::Str(Rc::from(msg))])?;
        Err(Flow::Throw(Value::Obj(o, Form::Own), self.trace()))
    }
}

fn form_of(v: &Value) -> Form {
    match v {
        Value::Obj(_, f) => *f,
        _ => Form::Borrow,
    }
}

fn is_shared_ty(t: &Ty) -> bool {
    match t {
        Ty::Formed(TForm::Shared, _) => true,
        Ty::Nullable(x) => is_shared_ty(x),
        _ => false,
    }
}

fn is_weak_ty(t: &Ty) -> bool {
    match t {
        Ty::Formed(TForm::Weak, _) => true,
        Ty::Nullable(x) => is_weak_ty(x),
        _ => false,
    }
}

fn is_nullable_ty(t: &Ty) -> bool {
    match t {
        Ty::Nullable(_) => true,
        Ty::Formed(_, x) => is_nullable_ty(x),
        _ => false,
    }
}
