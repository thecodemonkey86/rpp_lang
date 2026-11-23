//! Prüfung von Rümpfen: Anweisungen (Kapitel 8, 9) und Ausdrücke (Kapitel 4, 7, 10, 13),
//! zusammen mit der Flussanalyse aus Meilenstein M3: Typverengung (Kapitel 6), sichere
//! Initialisierung (R-4.46), Moves und Borrows (Kapitel 5) und Warnungen (R-8.27).
//!
//! Der Flusszustand (`flow`) begleitet die Prüfung Anweisung für Anweisung; an
//! Verzweigungen wird er kopiert und danach vereinigt, Schleifen werden bis zum Fixpunkt
//! geprüft (die Vorläufe ohne Meldungen). Loans (`loans`) sind lexikalisch: Sie gelten bis
//! zum Ende eines Aufrufs, einer Anweisung oder eines Blocks.

use crate::defs::*;
use crate::flow::*;
use crate::rt::{CallInfo, Key, Target};
use crate::ty::*;
use rpp_syntax::ast::{self, Access, BinaryOp as B, ExprKind as E, Primitive as P, StmtKind as S, UnaryOp as U};
use rpp_syntax::Span;
use std::collections::{BTreeSet, HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalKind {
    Var,
    Const,
    Param(Mode),
    /// `ref T x` bzw. `ref mut T x` (veränderbar)
    Ref(bool),
    /// Schleifen- und case-Variablen: Borrows (veränderbar bei `mut T x : mut v`)
    Bound(bool),
    /// catch-Variable: besitzt die Exception (R-9.8)
    Catch,
}

#[derive(Debug, Clone)]
struct Local {
    name: String,
    ty: Ty,
    kind: LocalKind,
    id: LocalId,
    /// Anzahl der Rahmen bei der Deklaration; innere Rahmen erfassen die Variable
    frame: usize,
    /// warum der Wert nullbar ist (für Meldungen, etwa R-6.21 nach `as`)
    origin: Vec<&'static str>,
}

impl LocalKind {
    /// Hält die Variable nur einen Borrow (Kapitel 5, Abschnitt 7)?
    fn is_borrow(self) -> bool {
        matches!(self, LocalKind::Param(Mode::Read | Mode::Mut) | LocalKind::Ref(_) | LocalKind::Bound(_))
    }
}

/// Ziel von `break` und `continue`: die Stände an den Sprüngen.
struct Jump {
    is_loop: bool,
    breaks: Vec<Flow>,
    continues: Vec<Flow>,
}

/// Ein Methodenrumpf oder ein Lambda-Rumpf.
struct Frame {
    ret: Option<Ret>,
    inferred: Vec<Ty>,
    loops: usize,
    switches: usize,
    own_lambda: bool,
    in_finally: usize,
    jumps: Vec<Jump>,
    /// Vereinigung aller Stände in umgebenden try-Blöcken (R-6.12, R-9.17)
    tries: Vec<Flow>,
    /// von diesem Rahmen erfasste Variablen äußerer Rahmen
    captures: Vec<LocalId>,
    /// Zugriffe auf erfasste Orte (Wurzel, veränderbar) für die Loan-Prüfung an der Erfassungsstelle
    capture_access: Vec<(LocalId, bool)>,
}

impl Frame {
    fn new(ret: Option<Ret>, own_lambda: bool) -> Frame {
        Frame { ret, inferred: Vec::new(), loops: 0, switches: 0, own_lambda, in_finally: 0, jumps: Vec::new(), tries: Vec::new(), captures: Vec::new(), capture_access: Vec::new() }
    }
}

/// Wohin ein Name in einem Ausdruck zeigt, wenn er kein lokaler Name ist.
pub struct Ctx {
    pub file: usize,
    pub class: Option<DefId>,
    pub is_static: bool,
    pub tparams: Vec<ParamId>,
}

pub struct Checker<'p> {
    p: &'p mut Program,
    ctx: Ctx,
    scopes: Vec<Vec<Local>>,
    frames: Vec<Frame>,
    /// Ist das Ergebnis des zuletzt geprüften äußersten Aufrufs `nodiscard`, bzw. ein Funktionswert?
    last_call: Option<CallFacts>,
    /// Steht der gerade geprüfte Ausdruck direkt als Argument eines Aufrufs?
    as_argument: bool,
    quiet: usize,
    /// Aufrufe, deren Methode einen Borrow zurückgibt (R-5.24)
    ref_calls: HashSet<Span>,
    /// zuletzt gewählte Methode und Ziel des zuletzt geprüften Aufrufs (für `Program::rt`)
    last_chosen: Option<MethodInfo>,
    last_target: Option<Target>,
    last_captures: Vec<String>,
    // ---- Flussanalyse (M3)
    flow: Flow,
    next_id: LocalId,
    loans: Vec<Loan>,
    stmt_depth: usize,
    call_seq: usize,
    /// Variablen, die in den Argumenten der gerade geprüften Aufrufe verschoben wurden
    call_moved: Vec<Vec<LocalId>>,
    /// > 0: Der Ausdruck ist Teil eines größeren Orts, dessen Zugriff schon geprüft ist.
    place_quiet: usize,
    /// Ort des Empfängers für den nächsten `invoke`
    pending_recv: Option<Place>,
    /// Darf `this` verändert werden (`mut`-Methode, Konstruktor, Destruktor, copy)?
    this_mut: bool,
}

#[derive(Debug, Clone, Copy)]
struct CallFacts {
    nodiscard: bool,
    func_value: bool,
    never_returns: bool,
}

fn literal_info(e: &ast::Expr) -> Option<(bool, i128)> {
    match &e.kind {
        E::Literal(ast::Literal::Int(v)) => int_value(v).map(|v| (false, v)),
        E::Unary { op: U::Neg, expr } => match &expr.kind {
            E::Literal(ast::Literal::Int(v)) => int_value(v).map(|v| (false, -v)),
            E::Literal(ast::Literal::Float(_)) => Some((true, 0)),
            _ => None,
        },
        E::Literal(ast::Literal::Float(_)) => Some((true, 0)),
        E::Paren(x) => literal_info(x),
        _ => None,
    }
}

fn is_literal(e: &ast::Expr) -> bool {
    literal_info(e).is_some()
}

impl<'p> Checker<'p> {
    pub fn new(p: &'p mut Program, ctx: Ctx) -> Checker<'p> {
        Checker {
            p,
            ctx,
            scopes: vec![Vec::new()],
            frames: Vec::new(),
            last_call: None,
            as_argument: false,
            quiet: 0,
            ref_calls: HashSet::new(),
            last_chosen: None,
            last_target: None,
            last_captures: Vec::new(),
            flow: Flow::default(),
            next_id: 0,
            loans: Vec::new(),
            stmt_depth: 0,
            call_seq: 0,
            call_moved: Vec::new(),
            place_quiet: 0,
            pending_recv: None,
            this_mut: false,
        }
    }

    fn err(&mut self, span: Span, rule: &'static str, msg: impl Into<String>) {
        if self.quiet == 0 {
            self.p.error(self.ctx.file, span, rule, msg);
        }
    }

    fn key(&self, e: &ast::Expr) -> Key {
        (self.ctx.file, e.id)
    }

    /// Ziel des gerade geprüften Aufrufs für die Ausführung festhalten.
    fn record_call(&mut self, e: &ast::Expr) {
        let Some(target) = self.last_target.take() else { return };
        if self.quiet > 0 {
            return;
        }
        let params = match &target {
            Target::Method(m) | Target::Exact(m) | Target::Ctor(_, Some(m)) => m.params.clone(),
            Target::BuiltinMethod(_) | Target::BuiltinStatic(..) => self.last_chosen.as_ref().map(|m| m.params.clone()).unwrap_or_default(),
            _ => Vec::new(),
        };
        let key = self.key(e);
        self.p.rt.calls.insert(key, CallInfo { target, params });
    }

    fn err_also(&mut self, span: Span, rule: &'static str, also: Vec<&'static str>, msg: impl Into<String>) {
        if self.quiet == 0 {
            self.p.error_also(self.ctx.file, span, rule, also, msg);
        }
    }

    fn warn(&mut self, span: Span, rule: &'static str, msg: impl Into<String>) {
        if self.quiet == 0 {
            self.p.warning(self.ctx.file, span, rule, msg);
        }
    }

    /// Prüft `f`, ohne Fehler zu melden (für Überladung, Ableitung und die Vorläufe von
    /// Schleifen). Flusszustand und Loans bleiben unverändert.
    fn probe<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        self.quiet += 1;
        let saved = self.last_call;
        let flow = self.flow.clone();
        let loans = self.loans.clone();
        let pending = self.pending_recv.take();
        let r = f(self);
        self.last_call = saved;
        self.flow = flow;
        self.loans = loans;
        self.pending_recv = pending;
        self.quiet -= 1;
        r
    }

    fn show(&self, t: &Ty) -> String {
        self.p.show(t)
    }

    fn resolve(&mut self, t: &ast::Type) -> Ty {
        let scope = self.ctx.tparams.clone();
        if self.quiet > 0 {
            let n = self.p.diags.len();
            let r = self.p.resolve_type(self.ctx.file, &scope, t);
            self.p.diags.truncate(n);
            self.p.drop_pending();
            r
        } else {
            let r = self.p.resolve_type(self.ctx.file, &scope, t);
            self.p.check_pending();
            r
        }
    }

    fn self_ty(&self) -> Option<Ty> {
        let d = self.ctx.class?;
        Some(Ty::Named(d, self.p.defs[d].tparams.iter().map(|&p| Ty::Param(p)).collect()))
    }

    fn in_mixin(&self) -> bool {
        self.ctx.class.is_some_and(|d| self.p.defs[d].kind == DefKind::Mixin)
    }

    // ------------------------------------------------------------ Einstiege

    pub fn method(&mut self, m: &MethodInfo, body: &ast::Block) {
        self.frames.push(Frame::new(Some(m.ret.clone()), false));
        self.this_mut = m.mut_this;
        for p in &m.params {
            self.new_local(&p.name, p.ty.clone(), LocalKind::Param(p.mode));
        }
        let completes = self.block(body);
        if completes && m.ret != Ret::Void {
            // R-9.19: assert endet normal und ersetzt kein return
            let also = match body.stmts.last().map(|s| &s.kind) {
                Some(S::Expr(ast::ExprStmt::Expr(e))) if matches!(&e.kind, E::Call { callee, .. } if matches!(&callee.kind, E::Name(n) if n.name == "assert")) => vec!["R-9.19"],
                _ => Vec::new(),
            };
            self.err_also(body.end, "R-8.26", also, format!("{} endet ohne return", m.name));
        }
        self.frames.pop();
    }

    pub fn main(&mut self, m: &ast::MainDecl) {
        self.frames.push(Frame::new(Some(Ret::Val(Ty::Prim(P::I32))), false));
        self.new_local(&m.args.name, Ty::Vector(Box::new(Ty::Prim(P::String))), LocalKind::Param(Mode::Read));
        if self.block(&m.body) {
            self.err(m.body.end, "R-8.26", "main endet ohne return");
        }
        self.frames.pop();
    }

    /// Konstruktor (R-7.6, R-7.7), Destruktor oder copy: Rumpf ohne Rückgabewert.
    pub fn ctor_like(&mut self, params: &[PInfo], base_args: Option<&[ast::Arg]>, body: &ast::Block, check_fields: bool, span: Span) {
        self.frames.push(Frame::new(Some(Ret::Void), false));
        self.this_mut = true;
        for p in params {
            self.new_local(&p.name, p.ty.clone(), LocalKind::Param(p.mode));
        }
        if let Some(args) = base_args {
            let base = self.ctx.class.and_then(|d| self.p.defs[d].base.clone());
            match base {
                Some(b @ Ty::Named(..)) => {
                    self.last_target = None;
                    self.new_object(&b, args, span);
                    if let Some(Target::Ctor(_, m)) = self.last_target.take() {
                        let file = self.ctx.file;
                        self.p.rt.base_ctors.insert((file, span), m);
                    }
                }
                _ => self.err(span, "R-7.6", "die Klasse hat keine Basisklasse"),
            }
        }
        self.block(body);
        if check_fields {
            self.check_fields_set(body);
        }
        self.frames.pop();
    }

    pub fn field_init(&mut self, ty: &Ty, e: &ast::Expr) {
        self.frames.push(Frame::new(None, false));
        let t = self.expr(e, Some(ty));
        self.require(&t, ty, e.span, Some(e));
        self.consume(e, &t, ty, "R-5.3", &[]);
        self.frames.pop();
    }

    /// R-7.7: jedes nicht nullbare Feld ohne Initialisierer wird im Konstruktor gesetzt.
    fn check_fields_set(&mut self, body: &ast::Block) {
        let Some(d) = self.ctx.class else { return };
        let needed: Vec<String> = self.p.defs[d]
            .fields
            .iter()
            .filter(|f| !f.is_static && !f.has_init && !matches!(f.ty, Ty::Nullable(_)))
            .map(|f| f.name.clone())
            .collect();
        if needed.is_empty() {
            return;
        }
        let set = assigned_fields(&body.stmts);
        let missing: Vec<&String> = needed.iter().filter(|n| !set.contains(n.as_str()) && !set.contains("*")).collect();
        if let Some(first) = missing.first() {
            self.err(body.end, "R-7.7", format!("Feld {first} wird im Konstruktor nicht auf jedem Weg gesetzt"));
        }
    }

    // ------------------------------------------------------------ Namen

    fn push_scope(&mut self) {
        self.scopes.push(Vec::new());
    }

    fn pop_scope(&mut self) {
        let depth = self.scopes.len();
        self.loans.retain(|l| !matches!(l.until, Until::Block(d) if d >= depth));
        self.scopes.pop();
    }

    fn find_local(&self, name: &str) -> Option<&Local> {
        self.scopes.iter().rev().flat_map(|s| s.iter().rev()).find(|l| l.name == name)
    }

    fn local_by_id(&self, id: LocalId) -> Option<&Local> {
        self.scopes.iter().flat_map(|s| s.iter()).find(|l| l.id == id)
    }

    fn new_local(&mut self, name: &str, ty: Ty, kind: LocalKind) -> LocalId {
        let id = self.next_id;
        self.next_id += 1;
        let frame = self.frames.len();
        self.scopes.last_mut().unwrap().push(Local { name: name.to_string(), ty, kind, id, frame, origin: Vec::new() });
        id
    }

    fn declare(&mut self, name: &ast::Ident, ty: Ty, kind: LocalKind) -> LocalId {
        if self.quiet == 0 {
            let file = self.ctx.file;
            self.p.rt.decls.insert((file, name.span), ty.clone());
        }
        if self.find_local(&name.name).is_some() {
            self.err(name.span, "R-8.17", format!("{} verdeckt eine sichtbare Variable", name.name));
        }
        self.new_local(&name.name, ty, kind)
    }

    fn frame(&mut self) -> &mut Frame {
        self.frames.last_mut().unwrap()
    }

    // ------------------------------------------------------------ Flussanalyse

    /// Ist der Typ nullbar (auch hinter einer Besitzform)?
    fn nullable(t: &Ty) -> bool {
        match t {
            Ty::Nullable(_) | Ty::Null => true,
            Ty::Formed(_, x) => Self::nullable(x),
            _ => false,
        }
    }

    /// Der verengte Typ (R-6.5): ohne `?`, mit derselben Besitzform.
    fn strip_null(t: &Ty) -> Ty {
        match t {
            Ty::Nullable(x) => (**x).clone(),
            Ty::Formed(f, x) => Ty::Formed(*f, Box::new(Self::strip_null(x))),
            other => other.clone(),
        }
    }

    /// Typ einer lokalen Variablen an dieser Stelle (verengt, falls ein Fakt gilt).
    fn local_type(&self, l: &Local) -> Ty {
        if self.flow.dead || self.flow.nonnull.contains(&l.id) {
            Self::strip_null(&l.ty)
        } else {
            l.ty.clone()
        }
    }

    /// Wird ein Wert dieses Typs verschoben statt kopiert (Kapitel 5, Abschnitt 7)?
    fn movable(&self, t: &Ty) -> bool {
        match t {
            Ty::Named(d, _) => match self.p.defs[*d].kind {
                DefKind::Enum => self.p.defs[*d].variants.iter().any(|v| !v.fields.is_empty()),
                _ => true,
            },
            Ty::Param(_) | Ty::Vector(_) | Ty::Array(..) | Ty::Func(..) => true,
            Ty::Tuple(ts) => ts.iter().any(|t| self.movable(t)),
            Ty::Nullable(x) => self.movable(x),
            _ => false,
        }
    }

    fn is_shared(t: &Ty) -> bool {
        match t {
            Ty::Formed(Form::Shared, _) => true,
            Ty::Nullable(x) => Self::is_shared(x),
            _ => false,
        }
    }

    /// Variable, an der `null` geprüft wird: `x` in `x == null`.
    fn local_id_of(&self, e: &ast::Expr) -> Option<LocalId> {
        match &e.kind {
            E::Paren(x) => self.local_id_of(x),
            E::Name(n) => self.find_local(&n.name).map(|l| l.id),
            _ => None,
        }
    }

    /// `W(c)` und `F(c)` (R-6.6).
    fn facts(&self, e: &ast::Expr) -> (BTreeSet<LocalId>, BTreeSet<LocalId>) {
        let none = || (BTreeSet::new(), BTreeSet::new());
        match &e.kind {
            E::Paren(x) => self.facts(x),
            E::Unary { op: U::Not, expr } => {
                let (w, f) = self.facts(expr);
                (f, w)
            }
            E::Binary { op: B::And, left, right } => {
                let (wa, fa) = self.facts(left);
                let (wb, fb) = self.facts(right);
                (wa.union(&wb).copied().collect(), fa.intersection(&fb).copied().collect())
            }
            E::Binary { op: B::Or, left, right } => {
                let (wa, fa) = self.facts(left);
                let (wb, fb) = self.facts(right);
                (wa.intersection(&wb).copied().collect(), fa.union(&fb).copied().collect())
            }
            E::Binary { op: op @ (B::Eq | B::Ne), left, right } => {
                let is_null = |x: &ast::Expr| matches!(x.kind, E::Literal(ast::Literal::Null));
                let var = if is_null(right) {
                    self.local_id_of(left)
                } else if is_null(left) {
                    self.local_id_of(right)
                } else {
                    None
                };
                let Some(v) = var else { return none() };
                let one: BTreeSet<LocalId> = [v].into_iter().collect();
                if *op == B::Ne {
                    (one, BTreeSet::new())
                } else {
                    (BTreeSet::new(), one)
                }
            }
            _ => none(),
        }
    }

    /// Warum der Wert von `e` nullbar ist (für Meldungen).
    fn null_reason(&self, e: &ast::Expr) -> Vec<&'static str> {
        match &e.kind {
            E::Paren(x) => self.null_reason(x),
            E::Name(n) => match self.find_local(&n.name) {
                Some(l) => {
                    let mut v: Vec<&'static str> = self.flow.lost.get(&l.id).copied().into_iter().collect();
                    v.extend(l.origin.iter().copied());
                    v
                }
                None => vec!["R-6.4"],
            },
            E::Member { .. } | E::Index { .. } => vec!["R-6.4"],
            E::As { .. } => vec!["R-6.21", "R-4.23"],
            E::Call { callee, .. } => match &callee.kind {
                E::Member { name, .. } if name.name == "lock" => vec!["R-6.22", "R-4.41"],
                E::Static(sr) if sr.member.name == "readLine" => vec!["R-13.21"],
                _ => Vec::new(),
            },
            _ => Vec::new(),
        }
    }

    /// Ort eines Ausdrucks (Kapitel 5, Abschnitt 3.1), falls er einer ist.
    fn place_of(&self, e: &ast::Expr) -> Option<Place> {
        match &e.kind {
            E::Paren(x) => self.place_of(x),
            E::This => Some(Place::var(THIS)),
            E::Name(n) => match self.find_local(&n.name) {
                Some(l) => Some(Place::var(l.id)),
                None => {
                    let st = self.self_ty()?;
                    if self.ctx.is_static || self.p.field_of(&st, &n.name).is_none_or(|(f, _)| f.is_static) {
                        return None;
                    }
                    Some(Place::var(THIS).child(&n.name))
                }
            },
            E::Member { object, name, .. } => Some(self.place_of(object)?.child(&name.name)),
            E::Index { object, .. } => Some(self.place_of(object)?.child("[]")),
            _ => None,
        }
    }

    fn place_name(&self, p: &Place) -> String {
        let mut s = if p.root == THIS { "this".to_string() } else { self.local_by_id(p.root).map(|l| l.name.clone()).unwrap_or_default() };
        for f in &p.path {
            if f == "[]" {
                s.push_str("[…]");
            } else {
                s.push('.');
                s.push_str(f);
            }
        }
        s
    }

    /// Rahmen, die die Variable erfassen (innere Lambdas).
    fn capturing_frames(&self, id: LocalId) -> std::ops::Range<usize> {
        let decl = if id == THIS { 1 } else { self.local_by_id(id).map(|l| l.frame).unwrap_or(self.frames.len()) };
        decl.min(self.frames.len())..self.frames.len()
    }

    fn captured_by_own_lambda(&self, id: LocalId) -> bool {
        self.capturing_frames(id).any(|f| self.frames[f].own_lambda)
    }

    /// Zugriff auf einen Ort prüfen: `erlaubt_L(κ, p)`.
    fn access(&mut self, p: &Place, mutable: bool, span: Span) {
        if self.flow.dead {
            return;
        }
        // Zugriffe eines Lambdas auf erfasste Orte prüft die Erfassungsstelle (R-10.7).
        for f in self.capturing_frames(p.root) {
            let acc = &mut self.frames[f].capture_access;
            match acc.iter_mut().find(|(r, _)| *r == p.root) {
                Some(e) => e.1 |= mutable,
                None => acc.push((p.root, mutable)),
            }
        }
        let Some(l) = self.loans.iter().rev().find(|l| l.conflicts(mutable, p)).cloned() else { return };
        let rule = l.rule;
        let how = if l.mutable { "veränderbar" } else { "lesend" };
        let what = if mutable { "verändert" } else { "gelesen" };
        self.err(span, rule, format!("{} ist {how} geliehen und kann hier nicht {what} werden", l.what));
    }

    fn push_loan(&mut self, place: Place, mutable: bool, until: Until, rule: &'static str) {
        let shared_root = place.root != THIS && self.local_by_id(place.root).is_some_and(|l| Self::is_shared(&l.ty));
        let what = self.place_name(&place);
        self.loans.push(Loan { mutable, place, shared_root, until, rule, what });
    }

    /// Schreibrecht (Kapitel 5, Abschnitt 3.2): Die Wurzel des Orts hält keinen lesenden Borrow.
    fn writable(&mut self, p: &Place, span: Span, rule: &'static str) -> bool {
        if p.root == THIS {
            if !self.this_mut {
                self.err(span, rule, "this ist in dieser Methode nur lesend geliehen (Methode ohne mut)");
                return false;
            }
            return true;
        }
        let Some(l) = self.local_by_id(p.root).cloned() else { return true };
        if Self::is_shared(&l.ty) {
            return true;
        }
        if self.captured_by_own_lambda(l.id) {
            self.err(span, "R-10.11", format!("{} ist im own-Lambda erfasst und unveränderlich", l.name));
            return false;
        }
        let ok = match l.kind {
            LocalKind::Param(Mode::Read) | LocalKind::Ref(false) | LocalKind::Bound(false) => false,
            LocalKind::Const => !p.path.is_empty(),
            _ => true,
        };
        if !ok {
            self.err(span, rule, format!("{} ist nur lesend geliehen", l.name));
        }
        ok
    }

    /// Zuweisung an eine lokale Variable: zugewiesen, verfügbar, verengt nach R-6.11.
    fn assigned(&mut self, id: LocalId, value: &Ty) {
        if self.flow.dead {
            return;
        }
        self.flow.unassigned.remove(&id);
        self.flow.moved.remove(&id);
        if Self::nullable(value) && !value.is_error() {
            self.flow.unnarrow(id, "R-6.11");
        } else {
            self.flow.nonnull.insert(id);
            self.flow.lost.remove(&id);
        }
    }

    /// Verwendung von `e` als Wert für einen Besitzer (Kapitel 5, Abschnitt 7): Eine
    /// own-Variable wird verschoben; aus Feldern, Elementen und Borrows kann nicht
    /// verschoben werden (V-3, R-5.3). `reason` begründet den Move in späteren Meldungen.
    fn consume(&mut self, e: &ast::Expr, vt: &Ty, dest: &Ty, reason: &'static str, also: &[&'static str]) {
        if vt.is_error() || dest.is_error() || self.flow.dead {
            return;
        }
        if self.quiet == 0 && !matches!(e.kind, E::Paren(_) | E::Conditional { .. } | E::Coalesce { .. }) {
            let key = self.key(e);
            self.p.rt.consumes.insert(key);
        }
        match &e.kind {
            E::Paren(x) => self.consume(x, vt, dest, reason, also),
            E::Conditional { then, otherwise, .. } => {
                self.consume(then, vt, dest, reason, also);
                self.consume(otherwise, vt, dest, reason, also);
            }
            E::Coalesce { left, right } => {
                self.consume(left, vt, dest, reason, also);
                self.consume(right, vt, dest, reason, also);
            }
            E::Name(n) if self.find_local(&n.name).is_some() => {
                let l = self.find_local(&n.name).cloned().unwrap();
                let dest_plain = !matches!(dest, Ty::Formed(..)) && !matches!(dest, Ty::Nullable(x) if matches!(**x, Ty::Formed(..)));
                if Self::is_shared(&l.ty) && dest_plain && self.movable(dest) {
                    let mut a = vec!["V-9"];
                    a.extend_from_slice(also);
                    self.err_also(e.span, "R-5.3", a, format!("{} ist shared; aus shared wird nie own (copy({}) verwenden)", n.name, n.name));
                    return;
                }
                if !self.movable(&l.ty) {
                    return;
                }
                if l.kind.is_borrow() {
                    let mut a = also.to_vec();
                    if matches!(l.ty.core(), Ty::Func(..)) {
                        a.push("R-10.16");
                    }
                    self.err_also(e.span, "R-5.3", a, format!("{} ist nur geliehen und kann nicht verschoben oder gespeichert werden", n.name));
                    return;
                }
                let frames = self.capturing_frames(l.id);
                if !frames.is_empty() {
                    let own = frames.clone().any(|f| self.frames[f].own_lambda);
                    let (rule, msg) = if own {
                        ("R-10.11", format!("{} ist im own-Lambda erfasst und kann nicht verschoben werden", n.name))
                    } else {
                        ("R-10.7", format!("ein normales Lambda erfasst {} nur geliehen; ein Move ist nicht möglich", n.name))
                    };
                    self.err(e.span, rule, msg);
                    return;
                }
                self.access(&Place::var(l.id), true, e.span);
                self.flow.moved.insert(l.id, reason);
                if let Some(c) = self.call_moved.last_mut() {
                    c.push(l.id);
                }
            }
            E::Call { .. } if self.ref_calls.contains(&e.span) && self.movable(vt) => {
                self.err_also(e.span, "R-5.24", also.to_vec(), "der Aufruf liefert einen Borrow; er wird mit ref gebunden und kann nicht verschoben werden");
            }
            E::Name(_) | E::Member { .. } | E::Index { .. } | E::This => {
                if !self.movable(vt) || matches!(e.kind, E::Member { safe: true, .. }) {
                    return;
                }
                let mut a = vec!["V-3"];
                if Self::nullable(vt) && !matches!(e.kind, E::This) {
                    a.push("R-6.15");
                }
                a.extend_from_slice(also);
                let msg = match e.kind {
                    E::This => "this kann nicht verschoben werden".to_string(),
                    E::Index { .. } => "aus einem Element kann nicht verschoben werden (pop() oder remove() verwenden)".to_string(),
                    _ => "aus einem Feld kann nicht verschoben werden (replace, take oder copy verwenden)".to_string(),
                };
                self.err_also(e.span, "R-5.3", a, msg);
            }
            _ => {}
        }
    }

    /// Stand nach einer Anweisung in alle umgebenden try-Blöcke übernehmen.
    fn note_try_state(&mut self) {
        if self.frames.is_empty() {
            return;
        }
        let f = self.flow.clone();
        for acc in &mut self.frames.last_mut().unwrap().tries {
            *acc = acc.join(&f, "R-6.12", "R-9.17");
        }
    }

    /// Prüft eine Schleife bis zum Fixpunkt (R-6.9, R-5.16). `once` prüft einen Durchlauf
    /// ab dem Kopf und liefert den Stand am Rücksprung und den beim Verlassen über die
    /// Bedingung.
    fn looped(&mut self, once: impl Fn(&mut Self) -> (Flow, Flow)) {
        let entry = self.flow.clone();
        let mut head = entry.clone();
        for _ in 0..6 {
            let h = head.clone();
            let back = self.probe(|c| {
                c.flow = h;
                c.loop_pass(&once).0
            });
            let next = entry.join(&back, "R-6.9", "R-5.16");
            if next.same(&head) {
                break;
            }
            head = next;
        }
        self.flow = head;
        let (_, exit) = self.loop_pass(&once);
        self.flow = exit;
    }

    fn loop_pass(&mut self, once: &impl Fn(&mut Self) -> (Flow, Flow)) -> (Flow, Flow) {
        self.frame().jumps.push(Jump { is_loop: true, breaks: Vec::new(), continues: Vec::new() });
        let (mut back, mut exit) = once(self);
        let j = self.frame().jumps.pop().unwrap();
        for c in &j.continues {
            back = back.join(c, "R-6.9", "R-5.16");
        }
        for b in &j.breaks {
            exit = exit.join(b, "R-6.9", "R-5.16");
        }
        (back, exit)
    }

    /// Stände an `continue` mit dem aktuellen vereinigen (vor dem Update einer for-Schleife).
    fn join_continues(&mut self) {
        let cs = match self.frame().jumps.last_mut() {
            Some(j) => std::mem::take(&mut j.continues),
            None => Vec::new(),
        };
        for c in cs {
            self.flow = self.flow.join(&c, "R-6.9", "R-5.16");
        }
    }

    // ------------------------------------------------------------ Typbeziehungen

    pub fn assignable(&self, from: &Ty, to: &Ty) -> bool {
        if from.is_error() || to.is_error() {
            return true;
        }
        if let Ty::Nullable(x) = to {
            if matches!(**x, Ty::Formed(Form::Weak, _)) {
                return match from {
                    Ty::Null => true,
                    Ty::Nullable(f) => self.assignable(f, x),
                    f => self.assignable(f, x),
                };
            }
        }
        if let Ty::Formed(Form::Weak, inner) = to {
            // V-6, V-13: weak entsteht nur aus shared (oder weak); ein own-Wert wäre sofort freigegeben.
            return match from {
                Ty::Formed(Form::Weak | Form::Shared, f) => self.assignable(f, inner),
                _ => false,
            };
        }
        if let Ty::Formed(Form::Weak, _) = from {
            return false;
        }
        let (f, t) = (from.unformed(), to.unformed());
        if f == t {
            return true;
        }
        match (f, t) {
            (Ty::Null, Ty::Nullable(_)) => true,
            (Ty::Null, _) => false,
            (Ty::Nullable(a), Ty::Nullable(b)) => self.assignable(a, b),
            (a, Ty::Nullable(b)) => self.assignable(a, b),
            (Ty::Prim(a), Ty::Prim(b)) => auto_converts(*a, *b),
            (Ty::Vector(a) | Ty::Array(a, _) | Ty::Slice(a), Ty::Slice(b)) => a.unformed() == b.unformed(),
            (Ty::Array(a, n), Ty::Array(b, m)) => a.unformed() == b.unformed() && (n.is_none() || m.is_none() || n == m),
            (Ty::Tuple(a), Ty::Tuple(b)) => a.len() == b.len() && a.iter().zip(b).all(|(x, y)| self.assignable(x, y)),
            (Ty::Named(..), Ty::Named(..)) => self.p.upcasts(f, t),
            (Ty::Param(_) | Ty::Prim(_), Ty::Named(..)) => self.p.satisfies(f, t),
            (Ty::Func(a, r), Ty::Func(b, s)) => a == b && r == s,
            _ => false,
        }
    }

    fn mismatch_rule(&self, from: &Ty, to: &Ty) -> &'static str {
        let weak_target = matches!(to, Ty::Formed(Form::Weak, _)) || matches!(to, Ty::Nullable(x) if matches!(**x, Ty::Formed(Form::Weak, _)));
        if weak_target && !matches!(from, Ty::Null | Ty::Nullable(_)) && !matches!(from, Ty::Formed(..)) {
            return "R-4.41";
        }
        match (from.unformed(), to.unformed()) {
            (Ty::Null, _) => "R-6.1",
            (Ty::Nullable(a), b) if !matches!(b, Ty::Nullable(_)) && self.assignable(a, b) => "R-6.1",
            (Ty::Void, _) => "R-4.45",
            (Ty::Prim(P::Bool), _) | (_, Ty::Prim(P::Bool)) => "R-4.3",
            (Ty::Prim(a), Ty::Prim(b)) if is_numeric(*a) && is_numeric(*b) => "R-4.6",
            (Ty::Vector(_) | Ty::Array(..), Ty::Vector(_) | Ty::Array(..) | Ty::Slice(_)) => "R-4.31",
            (Ty::Named(a, _), Ty::Named(b, _)) if a == b => "R-4.43",
            (Ty::Func(..), _) | (_, Ty::Func(..)) => "R-10.14",
            (Ty::Tuple(_), _) | (_, Ty::Tuple(_)) => "R-4.34",
            _ => "R-4.22",
        }
    }

    /// Meldet einen Fehler, wenn `from` nicht in `to` umgewandelt werden kann.
    fn require(&mut self, from: &Ty, to: &Ty, span: Span, e: Option<&ast::Expr>) -> bool {
        if let (Some(e), 0) = (e, self.quiet) {
            let key = self.key(e);
            self.p.rt.stores.insert(key, to.clone());
        }
        if self.assignable(from, to) {
            return true;
        }
        let rule = self.mismatch_rule(from, to);
        let also = match e {
            Some(e) if rule == "R-6.1" => self.null_reason(e),
            _ if rule == "R-4.41" => vec!["V-6", "V-13"],
            _ => Vec::new(),
        };
        let msg = format!("{} passt nicht zu {}", self.show(from), self.show(to));
        self.err_also(span, rule, also, msg);
        false
    }

    // ------------------------------------------------------------ Anweisungen

    /// Prüft einen Block; liefert, ob er normal enden kann.
    fn block(&mut self, b: &ast::Block) -> bool {
        self.push_scope();
        let c = self.stmts(&b.stmts);
        self.pop_scope();
        c
    }

    /// Anweisungsfolge; meldet unerreichbaren Code (R-8.27).
    fn stmts(&mut self, list: &[ast::Stmt]) -> bool {
        let mut completes = true;
        let mut warned = false;
        for s in list {
            if !completes && !warned {
                self.warn(s.span, "R-8.27", "unerreichbarer Code");
                warned = true;
            }
            if !self.stmt(s) {
                completes = false;
            }
        }
        completes
    }

    fn sub_stmt(&mut self, s: &ast::Stmt) -> bool {
        self.push_scope();
        let c = self.stmt(s);
        self.pop_scope();
        c
    }

    /// Prüft eine Bedingung und liefert ihre Fakten `W(c)`, `F(c)` (R-6.6).
    fn cond(&mut self, e: &ast::Expr) -> (BTreeSet<LocalId>, BTreeSet<LocalId>) {
        let t = self.expr(e, Some(&Ty::Prim(P::Bool)));
        if !t.is_error() && !t.is_bool() {
            let msg = format!("Bedingung hat den Typ {}, nicht bool", self.show(&t));
            self.err(e.span, "R-4.3", msg);
        }
        self.facts(e)
    }

    fn stmt(&mut self, s: &ast::Stmt) -> bool {
        self.stmt_depth += 1;
        let completes = self.stmt_kind(s);
        let depth = self.stmt_depth;
        self.loans.retain(|l| !matches!(l.until, Until::Stmt(d) if d >= depth));
        self.stmt_depth -= 1;
        if !completes {
            self.flow = Flow::dead();
        }
        self.note_try_state();
        completes
    }

    fn stmt_kind(&mut self, s: &ast::Stmt) -> bool {
        match &s.kind {
            S::Block(b) => self.block(b),
            S::LocalVar(d) => {
                self.local_var(d, s.span);
                true
            }
            S::Ref { ty, mutable, name, init } => {
                self.ref_decl(ty, *mutable, name, init);
                true
            }
            S::Tuple(d) => {
                self.tuple_decl(d);
                true
            }
            S::Expr(es) => self.expr_stmt(es, s.span),
            S::Discard(e) => {
                let is_call = matches!(e.kind, E::Call { .. });
                self.last_call = None;
                let t = self.expr(e, None);
                if !is_call {
                    self.err(e.span, "R-8.14", "discard verlangt einen Aufruf");
                } else if t == Ty::Void {
                    self.err(e.span, "R-8.14", "der Aufruf liefert keinen Wert, den discard verwerfen könnte");
                } else if self.last_call.is_some_and(|c| c.nodiscard) {
                    self.err(e.span, "R-8.15", "das Ergebnis einer nodiscard-Methode darf nicht verworfen werden");
                }
                true
            }
            S::If { cond, then, otherwise } => {
                let (w, f) = self.cond(cond);
                let base = self.flow.clone();
                self.flow.narrow(&w);
                let a = self.sub_stmt(then);
                let after_then = std::mem::replace(&mut self.flow, base);
                self.flow.narrow(&f);
                let b = match otherwise {
                    Some(o) => self.sub_stmt(o),
                    None => true,
                };
                self.flow = after_then.join(&self.flow, "R-6.8", "R-5.15");
                a || b
            }
            S::While { cond, body } => {
                let infinite = is_true(cond);
                self.looped(|c| {
                    let (w, f) = c.cond(cond);
                    let mut exit = if infinite { Flow::dead() } else { c.flow.clone() };
                    exit.narrow(&f);
                    c.flow.narrow(&w);
                    c.loop_body(body);
                    (c.flow.clone(), exit)
                });
                !infinite || contains_break(body)
            }
            S::DoWhile { body, cond } => {
                let infinite = is_true(cond);
                self.looped(|c| {
                    c.loop_body(body);
                    c.join_continues();
                    let (w, f) = c.cond(cond);
                    let mut exit = if infinite { Flow::dead() } else { c.flow.clone() };
                    exit.narrow(&f);
                    let mut back = c.flow.clone();
                    back.narrow(&w);
                    (back, exit)
                });
                !infinite || contains_break(body)
            }
            S::For { init, cond, update, body } => {
                self.push_scope();
                match init {
                    Some(ast::ForInit::Var(d)) => self.local_var(d, s.span),
                    Some(ast::ForInit::Tuple(d)) => self.tuple_decl(d),
                    Some(ast::ForInit::Exprs(list)) => {
                        for e in list {
                            self.expr_stmt(e, s.span);
                        }
                    }
                    None => {}
                }
                self.looped(|c| {
                    let (w, f) = match cond {
                        Some(x) => c.cond(x),
                        None => (BTreeSet::new(), BTreeSet::new()),
                    };
                    let mut exit = if cond.is_some() { c.flow.clone() } else { Flow::dead() };
                    exit.narrow(&f);
                    c.flow.narrow(&w);
                    c.loop_body(body);
                    c.join_continues();
                    for u in update {
                        c.expr_stmt(u, s.span);
                    }
                    (c.flow.clone(), exit)
                });
                self.pop_scope();
                cond.is_some() || contains_break(body)
            }
            S::ForEach { ty, name, iter, body, mutable, borrow_mut } => {
                let it = self.expr(iter, None);
                if Self::nullable(&it) && !it.is_error() {
                    let also = self.null_reason(iter);
                    let msg = format!("{} ist nullbar und nicht geprüft", self.show(&it));
                    self.err_also(iter.span, "R-6.2", also, msg);
                }
                let it = Self::strip_null(&it);
                let el = self.element_type(&it, *mutable, iter.span);
                self.push_scope();
                // R-5.17: Die Schleife leiht den Behälter bis zu ihrem Ende.
                if let Some(pl) = self.place_of(iter) {
                    if *borrow_mut {
                        self.writable(&pl, iter.span, "R-5.17");
                        self.access(&pl, true, iter.span);
                    }
                    let rule = if matches!(it.unformed(), Ty::Named(..)) { "R-13.10" } else { "R-5.17" };
                    let depth = self.scopes.len();
                    self.push_loan(pl, *borrow_mut, Until::Block(depth), rule);
                }
                let vt = match ty {
                    ast::VarType::Var => el,
                    ast::VarType::Type(t) => {
                        let t = self.resolve(t);
                        if !el.is_error() && !self.assignable(&el, &t) {
                            let msg = format!("Elemente vom Typ {} passen nicht zu {}", self.show(&el), self.show(&t));
                            self.err(name.span, "R-8.23", msg);
                        }
                        t
                    }
                };
                self.declare(name, vt, LocalKind::Bound(*mutable));
                self.looped(|c| {
                    let exit = c.flow.clone();
                    c.loop_body(body);
                    (c.flow.clone(), exit)
                });
                self.pop_scope();
                true
            }
            S::Switch { subject, cases } => self.switch(subject, cases, s.span),
            S::Return { value, borrow_mut } => {
                if self.frame().in_finally > 0 {
                    self.err(s.span, "R-9.10", "return verlässt einen finally-Block");
                }
                let ret = self.frame().ret.clone();
                match (ret, value) {
                    (Some(Ret::Void), Some(v)) => {
                        self.expr(v, None);
                        self.err(v.span, "R-8.26", "eine void-Methode gibt keinen Wert zurück");
                    }
                    (Some(Ret::Void), None) => {}
                    (Some(r), None) => {
                        let msg = format!("return braucht einen Wert vom Typ {}", self.show(&r.ty()));
                        self.err(s.span, "R-8.26", msg);
                    }
                    (Some(Ret::Ref(rt, m)), Some(v)) => {
                        // R-5.23: Der Borrow geht von this aus.
                        let t = self.expr(v, Some(&rt));
                        self.require(&t, &rt, v.span, Some(v));
                        // V-2: Ein Pfad endet nicht in einem shared-Feld; der Borrow ginge auf eine Hilfsvariable.
                        let through_shared = !matches!(v.kind, E::This | E::Name(_)) && Self::is_shared(&t) && !Self::is_shared(&rt);
                        match self.place_of(v) {
                            _ if through_shared => {
                                self.err_also(v.span, "R-5.23", vec!["V-2"], "ein Borrow auf ein shared-Feld kann nicht zurückgegeben werden");
                            }
                            Some(pl) if pl.root == THIS => {
                                if m || *borrow_mut {
                                    self.writable(&pl, v.span, "R-5.23");
                                }
                            }
                            _ => self.err(v.span, "R-5.23", "eine Methode gibt nur Borrows zurück, die von this ausgehen"),
                        }
                    }
                    (Some(r), Some(v)) => {
                        let rt = r.ty();
                        let t = self.expr(v, Some(&rt));
                        self.require(&t, &rt, v.span, Some(v));
                        self.consume(v, &t, &rt, "R-5.3", &["R-5.20"]);
                    }
                    (None, v) => {
                        let t = match v {
                            Some(v) => self.expr(v, None),
                            None => Ty::Void,
                        };
                        if let Some(v) = v {
                            self.consume(v, &t, &t, "R-5.3", &[]);
                        }
                        self.frame().inferred.push(t);
                    }
                }
                false
            }
            S::Break | S::Continue => {
                let is_break = matches!(s.kind, S::Break);
                let word = if is_break { "break" } else { "continue" };
                let f = self.frames.last().unwrap();
                let ok = if is_break { f.loops + f.switches > 0 } else { f.loops > 0 };
                if !ok {
                    let rule = if self.is_lambda_frame() {
                        "R-10.5"
                    } else if f.in_finally > 0 {
                        "R-9.10"
                    } else {
                        "R-8.25"
                    };
                    self.err(s.span, rule, format!("{word} außerhalb einer Schleife"));
                }
                let state = self.flow.clone();
                if is_break {
                    if let Some(j) = self.frame().jumps.last_mut() {
                        j.breaks.push(state);
                    }
                } else if let Some(j) = self.frame().jumps.iter_mut().rev().find(|j| j.is_loop) {
                    j.continues.push(state);
                }
                false
            }
            S::Throw(e) => {
                if self.frame().in_finally > 0 {
                    self.err(s.span, "R-9.10", "throw verlässt einen finally-Block");
                }
                let t = self.expr(e, None);
                self.require_exception(&t, e.span, "R-9.5");
                if Self::is_shared(&t) || Self::nullable(&t) {
                    let msg = format!("throw verlangt einen own-Wert, nicht {}", self.show(&t));
                    self.err(e.span, "R-9.5", msg);
                } else {
                    let reason = match &e.kind {
                        E::Name(n) if self.find_local(&n.name).is_some_and(|l| l.kind == LocalKind::Catch) => "R-9.8",
                        _ => "R-5.3",
                    };
                    let dest = Self::strip_null(&t);
                    self.consume(e, &t, &dest, reason, &["R-9.5"]);
                }
                false
            }
            S::Try { body, catches, finally } => self.try_stmt(body, catches, finally.as_ref()),
        }
    }

    fn try_stmt(&mut self, body: &ast::Block, catches: &[ast::CatchClause], finally: Option<&ast::Block>) -> bool {
        let pre = self.flow.clone();
        // R-6.12, R-9.17: catch und finally sehen jeden Stand des try-Blocks.
        let relabel = |acc: &Flow, pre: &Flow| {
            let mut e = acc.clone();
            for x in &pre.nonnull {
                if !acc.nonnull.contains(x) {
                    e.lost.insert(*x, "R-6.12");
                }
            }
            for (k, v) in e.moved.iter_mut() {
                if !pre.moved.contains_key(k) {
                    *v = "R-9.17";
                }
            }
            e
        };
        self.frame().tries.push(pre.clone());
        let mut completes = self.block(body);
        let acc = self.frame().tries.pop().unwrap().join(&self.flow, "R-6.12", "R-9.17");
        let entry = if pre.dead { Flow::dead() } else { relabel(&acc, &pre) };
        let mut out = self.flow.clone();
        let mut fin = entry.clone();
        let mut seen: Vec<Ty> = Vec::new();
        for c in catches {
            let t = self.resolve(&c.ty);
            self.require_exception(&t, c.ty.span, "R-9.6");
            // R-9.7: catch hinter dem catch einer Oberklasse ist unerreichbar.
            if !t.is_error() && seen.iter().any(|s| self.p.upcasts(t.unformed(), s.unformed())) {
                self.warn(c.ty.span, "R-8.27", "dieser catch ist unerreichbar: ein catch für eine Oberklasse steht davor");
            }
            seen.push(t.clone());
            self.flow = entry.clone();
            self.frame().tries.push(entry.clone());
            self.push_scope();
            self.declare(&c.name, t, LocalKind::Catch);
            if self.block(&c.body) {
                completes = true;
            }
            self.pop_scope();
            let cacc = self.frame().tries.pop().unwrap();
            fin = fin.join(&cacc, "R-6.12", "R-9.17").join(&self.flow, "R-6.12", "R-9.17");
            out = out.join(&self.flow, "R-6.8", "R-5.15");
        }
        self.flow = out;
        if let Some(f) = finally {
            let normal = self.flow.clone();
            self.flow = if pre.dead { Flow::dead() } else { relabel(&fin, &pre) };
            self.frame().in_finally += 1;
            let saved = (self.frame().loops, self.frame().switches);
            self.frame().loops = 0;
            self.frame().switches = 0;
            let fc = self.block(f);
            let fr = self.frame();
            fr.loops = saved.0;
            fr.switches = saved.1;
            fr.in_finally -= 1;
            if !fc {
                completes = false;
            }
            if normal.dead {
                self.flow = Flow::dead();
            }
        }
        completes
    }

    fn is_lambda_frame(&self) -> bool {
        self.frames.len() > 1
    }

    fn loop_body(&mut self, body: &ast::Stmt) {
        self.frame().loops += 1;
        self.sub_stmt(body);
        self.frame().loops -= 1;
    }

    fn require_exception(&mut self, t: &Ty, span: Span, rule: &'static str) {
        if t.is_error() {
            return;
        }
        let ok = match (t.unformed().core(), self.p.std_def("rpp.core.Exception")) {
            (c @ Ty::Named(..), Some(ex)) => self.p.upcasts(c, &Ty::Named(ex, Vec::new())),
            (_, None) => true,
            _ => false,
        };
        if !ok {
            let msg = format!("{} ist keine Exception", self.show(t));
            self.err(span, rule, msg);
        }
    }

    /// Initialisierer einer neuen lokalen Variablen: Move, Verengung, Herkunft.
    fn init_local(&mut self, id: LocalId, init: &ast::Expr, vt: &Ty, declared: &Ty) {
        self.consume(init, vt, declared, "R-5.3", &[]);
        self.assigned(id, vt);
        if Self::nullable(declared) {
            let origin = self.null_reason(init);
            if let Some(l) = self.scopes.last_mut().unwrap().iter_mut().rev().find(|l| l.id == id) {
                l.origin = origin;
            }
        }
    }

    /// Loans, die der Initialisierer gerade erzeugt hat (Slice, Borrow-Rückgabe), gelten bis zum Blockende.
    fn keep_loans(&mut self, mark: usize) {
        let depth = self.scopes.len();
        let sd = self.stmt_depth;
        for l in &mut self.loans[mark..] {
            if l.until == Until::Stmt(sd) {
                l.until = Until::Block(depth);
            }
        }
    }

    fn local_var(&mut self, d: &ast::LocalVarDecl, span: Span) {
        let kind = if d.is_const { LocalKind::Const } else { LocalKind::Var };
        let mark = self.loans.len();
        let (ty, vt) = match &d.ty {
            ast::VarType::Type(t) => {
                let t = self.resolve(t);
                let vt = match &d.init {
                    Some(init) => {
                        let it = self.expr(init, Some(&t));
                        self.require(&it, &t, init.span, Some(init));
                        Some(it)
                    }
                    None => None,
                };
                (t, vt)
            }
            ast::VarType::Var => match &d.init {
                None => {
                    self.err(span, "R-4.48", "var verlangt einen Initialisierer");
                    (Ty::Error, Some(Ty::Error))
                }
                Some(init) => {
                    if let E::Lambda { own: false, .. } = init.kind {
                        self.err(init.span, "R-10.4", "ein normales Lambda braucht einen Funktionstyp aus dem Zusammenhang");
                        (Ty::Error, Some(Ty::Error))
                    } else {
                        let t = self.expr(init, None);
                        match t {
                            Ty::Null => {
                                self.err(init.span, "R-4.40", "null braucht einen nullbaren Typ aus dem Zusammenhang");
                                (Ty::Error, Some(Ty::Error))
                            }
                            Ty::Void => {
                                let also = if has_safe_access(init) { vec!["R-6.17"] } else { Vec::new() };
                                self.err_also(init.span, "R-4.45", also, "eine Variable kann nicht den Typ void haben");
                                (Ty::Error, Some(Ty::Error))
                            }
                            t => (t.clone(), Some(t)),
                        }
                    }
                }
            },
        };
        if matches!(ty.unformed(), Ty::Slice(_)) {
            self.keep_loans(mark);
        }
        let id = self.declare(&d.name, ty.clone(), kind);
        match (&d.init, vt) {
            (Some(init), Some(vt)) => self.init_local(id, init, &vt, &ty),
            _ => {
                if !self.flow.dead {
                    self.flow.unassigned.insert(id);
                }
            }
        }
    }

    /// `ref T x = ort;` (Kapitel 5, Abschnitt 7): Borrow bis zum Blockende (R-5.17, R-5.24).
    fn ref_decl(&mut self, ty: &ast::Type, mutable: bool, name: &ast::Ident, init: &ast::Expr) {
        let t = self.resolve(ty);
        let mark = self.loans.len();
        let it = self.expr(init, Some(&t));
        self.require(&it, &t, init.span, Some(init));
        let mut target = init;
        while let E::Paren(x) | E::As { expr: x, .. } = &target.kind {
            target = x;
        }
        let depth = self.scopes.len();
        match self.place_of(target) {
            Some(pl) => {
                if mutable {
                    self.writable(&pl, init.span, "R-5.17");
                    self.access(&pl, true, init.span);
                }
                self.push_loan(pl, mutable, Until::Block(depth), "R-5.17");
            }
            None if matches!(target.kind, E::Call { .. }) && self.loans[mark..].iter().any(|l| l.until == Until::Stmt(self.stmt_depth)) => {
                self.keep_loans(mark);
            }
            None if it.is_error() || matches!(target.kind, E::Member { safe: true, .. }) => {}
            None => self.err(init.span, "R-5.17", "ref verlangt einen Ort (Variable, Feld oder Element), keinen temporären Wert"),
        }
        let id = self.declare(name, t.clone(), LocalKind::Ref(mutable));
        self.assigned(id, &it);
        if Self::nullable(&t) {
            let origin = self.null_reason(init);
            if let Some(l) = self.scopes.last_mut().unwrap().iter_mut().rev().find(|l| l.id == id) {
                l.origin = origin;
            }
        }
    }

    /// `(T a, U b) = e;`: Die Elemente gehen an die neuen Variablen über (R-4.36).
    fn tuple_decl(&mut self, d: &ast::TupleDecl) {
        let types: Vec<Ty> = d.vars.iter().map(|(t, _)| self.resolve(t)).collect();
        let expected = Ty::Tuple(types.clone());
        let it = self.expr(&d.init, Some(&expected));
        if matches!(it.core(), Ty::Named(..)) {
            self.unpack(d, types, it);
            return;
        }
        self.require(&it, &expected, d.init.span, None);
        self.consume(&d.init, &it, &expected, "R-4.36", &[]);
        let elems: Vec<Ty> = match &it {
            Ty::Tuple(ts) if ts.len() == types.len() => ts.clone(),
            _ => types.clone(),
        };
        for (((_, name), t), vt) in d.vars.iter().zip(types).zip(elems) {
            let id = self.declare(name, t, LocalKind::Var);
            self.assigned(id, &vt);
        }
    }

    /// R-5.25 (V-15): `(T₁ a₁, …, Tₙ aₙ) = e;` zerlegt ein own-Objekt einer Klasse ohne
    /// Destruktor in seine Felder (geerbte zuerst). Das Objekt wird verschoben und ohne
    /// Destruktor freigegeben; die Felder gehen an die neuen Variablen über.
    fn unpack(&mut self, d: &ast::TupleDecl, types: Vec<Ty>, it: Ty) {
        let span = d.init.span;
        let it = self.non_null_operand(it, &d.init, span);
        let mut fields: Vec<(String, Ty, Access, DefId)> = Vec::new();
        let mut ok = true;
        if Self::is_shared(&it) || weak_inner(&it).is_some() {
            let msg = format!("zerlegen lässt sich nur ein own-Objekt, nicht {}", self.show(&it));
            self.err(span, "R-5.25", msg);
            ok = false;
        }
        // Kette von der Klasse zur Basis, mit eingesetzten Typargumenten
        let mut chain: Vec<(DefId, Vec<Ty>)> = Vec::new();
        let mut cur = it.core().clone();
        while let Ty::Named(c, args) = cur {
            if self.p.defs[c].kind != DefKind::Class {
                let msg = format!("{} ist keine Klasse", self.p.defs[c].name);
                self.err(span, "R-5.25", msg);
                ok = false;
                break;
            }
            let map = self.p.subst_map(c, &args);
            let base = self.p.defs[c].base.as_ref().map(|b| b.subst(&map));
            chain.push((c, args));
            cur = base.unwrap_or(Ty::Error);
        }
        chain.reverse();
        for (c, args) in &chain {
            if self.p.defs[*c].has_dtor {
                let msg = format!("{} hat einen Destruktor und lässt sich nicht zerlegen", self.p.defs[*c].name);
                self.err(span, "R-5.25", msg);
                ok = false;
            }
            let map = self.p.subst_map(*c, args);
            for f in self.p.defs[*c].fields.iter().filter(|f| !f.is_static) {
                fields.push((f.name.clone(), f.ty.subst(&map), f.access, *c));
            }
        }
        if ok && fields.len() != types.len() {
            let msg = format!("{} hat {} Felder, zerlegt wird in {} Variablen", self.show(&it), fields.len(), types.len());
            self.err(span, "R-5.25", msg);
            ok = false;
        }
        if ok {
            for ((name, fty, access, owner), (t, (_, var))) in fields.iter().zip(types.iter().zip(&d.vars)) {
                self.check_access(*access, *owner, var.span);
                if !self.assignable(fty, t) {
                    let msg = format!("das Feld {name} hat den Typ {}, nicht {}", self.show(fty), self.show(t));
                    self.err(var.span, "R-5.25", msg);
                }
            }
        }
        let dest = it.unformed().clone();
        self.consume(&d.init, &it, &dest, "R-5.25", &[]);
        for (i, ((_, name), t)) in d.vars.iter().zip(types).enumerate() {
            let vt = fields.get(i).map(|f| f.1.clone()).unwrap_or(Ty::Error);
            let id = self.declare(name, t, LocalKind::Var);
            self.assigned(id, &vt);
        }
    }

    fn expr_stmt(&mut self, es: &ast::ExprStmt, span: Span) -> bool {
        match es {
            ast::ExprStmt::Assign { target, op, value } => {
                self.assign(target, *op, value);
                true
            }
            ast::ExprStmt::Expr(e) => {
                self.last_call = None;
                let t = self.expr(e, None);
                if let E::Call { .. } = e.kind {
                    if let Some(c) = self.last_call {
                        if c.never_returns {
                            return false;
                        }
                        if t != Ty::Void && !t.is_error() {
                            let rule = if c.func_value { "R-10.17" } else { "R-8.13" };
                            self.err(span, rule, "das Ergebnis des Aufrufs wird nicht verwendet (discard verwirft es)");
                        }
                    }
                }
                true
            }
        }
    }

    fn assign(&mut self, target: &ast::Expr, op: ast::AssignOp, value: &ast::Expr) {
        if let E::Tuple(items) = &target.kind {
            let vt = self.expr(value, None);
            let types: Vec<Ty> = items.iter().map(|t| self.lvalue(t)).collect();
            let expected = Ty::Tuple(types);
            self.require(&vt, &expected, value.span, None);
            self.consume(value, &vt, &expected, "R-4.36", &[]);
            let elems: Vec<Ty> = match &vt {
                Ty::Tuple(ts) if ts.len() == items.len() => ts.clone(),
                _ => vec![Ty::Error; items.len()],
            };
            for (x, et) in items.iter().zip(elems) {
                if let Some(id) = self.local_id_of(x) {
                    self.assigned(id, &et);
                }
            }
            return;
        }
        if op != ast::AssignOp::Assign {
            // Verbundzuweisung liest das Ziel.
            if let Some(n) = match &target.kind {
                E::Name(n) => Some(n),
                _ => None,
            } {
                if self.find_local(&n.name).is_some() {
                    self.name(n);
                }
            }
        }
        let tt = self.lvalue(target);
        if op == ast::AssignOp::Assign {
            if let E::Lambda { own: false, .. } = value.kind {
                self.err(value.span, "R-10.7", "ein normales Lambda kann nicht gespeichert werden");
                return;
            }
            let vt = self.expr(value, Some(&tt));
            self.require(&vt, &tt, value.span, Some(value));
            let also: &[&'static str] = if matches!(target.kind, E::Member { .. }) { &["R-5.13"] } else { &[] };
            self.consume(value, &vt, &tt, "R-5.3", also);
            // R-5.7, R-5.13: Erst der Wert, dann der Ort; x.f = g(x) verschiebt x vor dem Schreiben.
            if !matches!(target.kind, E::Name(_)) {
                if let Some(pl) = self.place_of(target).filter(|p| p.root != THIS) {
                    if let Some(reason) = self.flow.moved.remove(&pl.root) {
                        let name = self.place_name(&Place::var(pl.root));
                        self.err_also(target.span, "R-5.3", vec![reason, "R-5.7", "R-5.13"], format!("{name} wird im Wert verschoben und ist danach kein Ziel mehr"));
                    }
                }
            }
            if let Some(id) = self.local_id_of(target) {
                self.assigned(id, &vt);
            }
            return;
        }
        // Verbundzuweisung (R-8.20)
        let bop = match op {
            ast::AssignOp::Add => B::Add,
            ast::AssignOp::Sub => B::Sub,
            ast::AssignOp::Mul => B::Mul,
            ast::AssignOp::Div => B::Div,
            _ => B::Rem,
        };
        let tt = self.non_null_operand(tt, target, target.span);
        if tt.is_string() && bop == B::Add {
            let vt = self.expr(value, Some(&tt));
            self.require(&vt, &tt, value.span, None);
            return;
        }
        let vt = if is_literal(value) { self.expr(value, Some(&tt)) } else { self.expr(value, None) };
        let vt = self.non_null_operand(vt, value, value.span);
        let result = self.arith(bop, &tt, &vt, target.span);
        if !result.is_error() && !tt.is_error() && !self.assignable(&result, &tt) {
            let msg = format!("{} {}= {} ergibt {}, das nicht in {} passt", self.show(&tt), op_str(bop), self.show(&vt), self.show(&result), self.show(&tt));
            self.err(target.span, "R-8.20", msg);
        }
    }

    /// Typ eines Zuweisungsziels; prüft R-8.18, R-8.19, R-4.19, das Schreibrecht und die Loans.
    fn lvalue(&mut self, e: &ast::Expr) -> Ty {
        match &e.kind {
            E::Name(n) => {
                if let Some(l) = self.find_local(&n.name).cloned() {
                    match l.kind {
                        LocalKind::Const => self.err(e.span, "R-8.18", format!("{} ist const", n.name)),
                        LocalKind::Param(Mode::Read) => {
                            self.err(e.span, "R-8.19", format!("der Parameter {} ist nur lesbar", n.name))
                        }
                        // Über einen veränderbaren Borrow setzt man Felder, ersetzt aber nie das Objekt (Kapitel 5, 4.4).
                        LocalKind::Param(Mode::Mut) | LocalKind::Bound(true) if !matches!(l.ty.unformed(), Ty::Prim(_)) => {
                            self.err_also(e.span, "R-8.19", vec!["R-5.12"], format!("{} ist ein Borrow; das Objekt kann nicht ersetzt werden", n.name))
                        }
                        LocalKind::Ref(_) => self.err(e.span, "R-8.19", format!("{} ist eine ref-Variable", n.name)),
                        _ => {
                            if self.captured_by_own_lambda(l.id) {
                                self.err(e.span, "R-10.11", format!("{} ist im own-Lambda erfasst und unveränderlich", n.name));
                            }
                        }
                    }
                    self.access(&Place::var(l.id), true, e.span);
                    return l.ty;
                }
                let t = self.expr(e, None);
                if let Some(pl) = self.place_of(e) {
                    self.writable(&pl, e.span, "R-5.13");
                    self.access(&pl, true, e.span);
                }
                t
            }
            E::Index { object, .. } => {
                let ot = self.probe(|c| c.expr(object, None));
                if ot.is_string() {
                    self.err(e.span, "R-4.19", "Strings sind unveränderlich; s[i] kann nicht zugewiesen werden");
                    return Ty::Error;
                }
                let t = self.expr(e, None);
                if let Some(pl) = self.place_of(e) {
                    self.writable(&pl, e.span, "R-5.13");
                    self.access(&pl, true, e.span);
                }
                t
            }
            E::Member { .. } => {
                let t = self.expr(e, None);
                if let Some(pl) = self.place_of(e) {
                    self.writable(&pl, e.span, "R-5.13");
                    self.access(&pl, true, e.span);
                }
                // Das Ziel einer Zuweisung ist das Feld selbst, nicht sein verengter Wert.
                t
            }
            E::Paren(x) => self.lvalue(x),
            _ => {
                self.err(e.span, "R-8.19", "kein Zuweisungsziel");
                Ty::Error
            }
        }
    }

    fn element_type(&mut self, t: &Ty, mutable: bool, span: Span) -> Ty {
        match t.unformed() {
            Ty::Error => Ty::Error,
            Ty::Vector(e) | Ty::Array(e, _) | Ty::Slice(e) => (**e).clone(),
            Ty::Prim(P::String) => Ty::Prim(P::Char),
            Ty::Nullable(x) => self.element_type(&x.clone(), mutable, span),
            other @ Ty::Named(..) => {
                let name = if mutable { "rpp.core.MutIterable" } else { "rpp.core.Iterable" };
                if let Some(it) = self.p.std_def(name) {
                    let found = std::iter::once(other.clone())
                        .chain(self.p.supertypes(other))
                        .find_map(|s| match s {
                            Ty::Named(d, args) if d == it => args.first().cloned(),
                            _ => None,
                        });
                    if let Some(el) = found {
                        return el;
                    }
                }
                let msg = format!("{} ist nicht iterierbar", self.show(t));
                self.err(span, "R-8.23", msg);
                Ty::Error
            }
            _ => {
                let msg = format!("{} ist nicht iterierbar", self.show(t));
                self.err(span, "R-8.23", msg);
                Ty::Error
            }
        }
    }

    fn switch(&mut self, subject: &ast::Expr, cases: &[ast::CaseClause], span: Span) -> bool {
        let st = self.expr(subject, None);
        if Self::nullable(&st) && !st.is_error() {
            let also = self.null_reason(subject);
            let msg = format!("switch über den nullbaren Wert {}", self.show(&st));
            self.err_also(subject.span, "R-6.2", also, msg);
        }
        let st = Self::strip_null(&st);
        let enum_def = match st.unformed() {
            Ty::Named(d, _) if self.p.defs[*d].kind == DefKind::Enum => Some(*d),
            _ => None,
        };
        let allowed = st.is_error() || enum_def.is_some() || st.is_integer() || matches!(st.prim(), Some(P::Char | P::String));
        if !allowed {
            let msg = format!("switch über {} ist nicht erlaubt", self.show(&st));
            self.err(subject.span, "R-8.24", msg);
        }
        let base = self.flow.clone();
        let mut out = Flow::dead();
        let mut seen: Vec<String> = Vec::new();
        let mut has_default = false;
        let mut any_completes = false;
        for c in cases {
            has_default |= c.is_default;
            self.flow = base.clone();
            self.push_scope();
            for pat in &c.patterns {
                let key = self.pattern(pat, &st, enum_def);
                if let Some(k) = key {
                    if seen.contains(&k) {
                        self.err(c.span, "R-8.24", format!("die Marke {k} kommt doppelt vor"));
                    }
                    seen.push(k);
                }
            }
            self.frame().switches += 1;
            self.frame().jumps.push(Jump { is_loop: false, breaks: Vec::new(), continues: Vec::new() });
            let mut completes = self.stmts(&c.body);
            let j = self.frame().jumps.pop().unwrap();
            if c.body.iter().any(contains_break_stmt) {
                completes = true;
            }
            self.frame().switches -= 1;
            any_completes |= completes;
            out = out.join(&self.flow, "R-6.8", "R-5.15");
            for b in &j.breaks {
                out = out.join(b, "R-6.8", "R-5.15");
            }
            self.pop_scope();
        }
        let exhaustive = match enum_def {
            Some(d) => self.p.defs[d].variants.iter().all(|v| seen.contains(&v.name)),
            None => false,
        };
        if !has_default && !exhaustive && !st.is_error() {
            let msg = if enum_def.is_some() { "switch über ein Enum nennt nicht alle Varianten und hat kein default" } else { "switch braucht ein default" };
            self.err(span, "R-8.24", msg);
        }
        if !(has_default || exhaustive) || cases.is_empty() {
            out = out.join(&base, "R-6.8", "R-5.15");
        }
        self.flow = out;
        if has_default || exhaustive {
            any_completes || cases.is_empty()
        } else {
            true
        }
    }

    /// Prüft eine Case-Marke und liefert ihren Schlüssel für die Prüfung auf Doppelte.
    fn pattern(&mut self, pat: &ast::Pattern, st: &Ty, enum_def: Option<DefId>) -> Option<String> {
        match pat {
            ast::Pattern::Literal { negative, lit } => {
                let lt = match lit {
                    ast::Literal::Int(_) if st.is_integer() => st.clone(),
                    ast::Literal::Int(_) => Ty::Prim(P::I32),
                    ast::Literal::Char(_) => Ty::Prim(P::Char),
                    ast::Literal::Str(_) => Ty::Prim(P::String),
                    ast::Literal::Bool(_) => Ty::Prim(P::Bool),
                    ast::Literal::Float(_) => Ty::Prim(P::F64),
                    ast::Literal::Null => Ty::Null,
                };
                if !st.is_error() && !self.assignable(&lt, st) {
                    let msg = format!("die Marke passt nicht zu {}", self.show(st));
                    self.err(Span::default(), "R-8.24", msg);
                }
                let text = match lit {
                    ast::Literal::Int(v) => int_value(v).map(|v| v.to_string()).unwrap_or_default(),
                    ast::Literal::Str(s) | ast::Literal::Char(s) | ast::Literal::Float(s) => s.clone(),
                    other => format!("{other:?}"),
                };
                Some(if *negative { format!("-{text}") } else { text })
            }
            ast::Pattern::Name { name, bindings } => {
                let (variant, owner_ok) = match name {
                    ast::PatternName::Simple(id) => (id.name.clone(), true),
                    ast::PatternName::Static(sr) => {
                        let owner = match &sr.owner {
                            ast::StaticOwner::Named(tn) => {
                                let t = self.resolve(&ast::Type { ownership: None, base: ast::BaseType::Named(tn.clone()), suffixes: vec![], span: sr.span });
                                match t {
                                    Ty::Named(d, _) => Some(d),
                                    _ => None,
                                }
                            }
                            _ => None,
                        };
                        (sr.member.name.clone(), owner.is_some() && owner == enum_def || enum_def.is_none())
                    }
                };
                if let Some(d) = enum_def {
                    let v = self.p.defs[d].variants.iter().find(|v| v.name == variant).cloned();
                    match v {
                        Some(v) if owner_ok => {
                            if let Some(bs) = bindings {
                                if bs.len() != v.fields.len() {
                                    self.err(bs[0].span, "R-8.24", format!("{} hat {} Daten", v.name, v.fields.len()));
                                }
                                for (b, f) in bs.iter().zip(&v.fields) {
                                    self.declare(b, f.ty.clone(), LocalKind::Bound(false));
                                }
                            }
                        }
                        _ => {
                            let msg = format!("{} ist keine Variante von {}", variant, self.p.defs[d].name);
                            self.err(Span::default(), "R-8.24", msg);
                        }
                    }
                    return Some(variant);
                }
                // Konstante
                Some(variant)
            }
        }
    }

    // ------------------------------------------------------------ Ausdrücke

    /// Prüft einen Ausdruck. Für einen Ort (Variable, Feld, Element) wird hier der
    /// lesende Zugriff gegen die Loans geprüft, einmal für den ganzen Ort.
    pub fn expr(&mut self, e: &ast::Expr, expected: Option<&Ty>) -> Ty {
        let is_place = matches!(e.kind, E::Name(_) | E::Member { .. } | E::Index { .. } | E::This | E::Paren(_));
        let saved = self.place_quiet;
        if !is_place {
            self.place_quiet = 0;
        } else if saved == 0 && !matches!(e.kind, E::Paren(_)) {
            if let Some(pl) = self.place_of(e) {
                self.access(&pl, false, e.span);
            }
        }
        let t = self.expr_inner(e, expected);
        self.place_quiet = saved;
        if self.quiet == 0 {
            let key = self.key(e);
            self.p.rt.types.insert(key, t.clone());
        }
        t
    }

    fn expr_inner(&mut self, e: &ast::Expr, expected: Option<&Ty>) -> Ty {
        let as_arg = std::mem::replace(&mut self.as_argument, false);
        match &e.kind {
            E::Literal(lit) => self.literal(lit, expected, e.span, false),
            E::Name(n) => self.name(n),
            E::This => {
                if self.ctx.is_static || self.ctx.class.is_none() {
                    self.err(e.span, "R-7.4", "statische Methoden haben kein this");
                    return Ty::Error;
                }
                if self.frames.iter().any(|f| f.own_lambda) {
                    self.err(e.span, "R-10.10", "ein own-Lambda kann this nicht erfassen");
                    return Ty::Error;
                }
                self.self_ty().unwrap_or(Ty::Error)
            }
            E::Base => {
                self.err(e.span, "R-7.15", "base steht nur vor einem Methodenaufruf");
                Ty::Error
            }
            E::Paren(x) => {
                self.as_argument = as_arg;
                self.expr(x, expected)
            }
            E::Tuple(items) => {
                let exp: Vec<Option<Ty>> = match expected.map(|t| t.unformed()) {
                    Some(Ty::Tuple(ts)) if ts.len() == items.len() => ts.iter().cloned().map(Some).collect(),
                    _ => vec![None; items.len()],
                };
                let mut ts = Vec::new();
                for (x, et) in items.iter().zip(exp) {
                    let xt = self.expr(x, et.as_ref());
                    // R-4.36: Ein Tupel besitzt seine Elemente.
                    self.consume(x, &xt, et.as_ref().unwrap_or(&xt), "R-5.3", &["R-4.36"]);
                    ts.push(xt);
                }
                Ty::Tuple(ts)
            }
            E::Array(items) => self.array_literal(items, expected, e.span),
            E::New { ty, args } => {
                let t = self.resolve(&ast::Type { ownership: None, base: ast::BaseType::Named(ty.clone()), suffixes: vec![], span: e.span });
                self.last_target = None;
                let r = self.new_object(&t, args, e.span);
                self.record_call(e);
                r
            }
            E::Cast { ty, expr } => self.cast(ty, expr, e.span),
            E::Copy(x) => {
                let t = self.expr(x, None);
                let t = self.non_null_operand(t, x, x.span);
                match t.unformed().core() {
                    Ty::Func(..) => {
                        self.err(e.span, "R-10.15", "Funktionswerte können nicht kopiert werden");
                        Ty::Error
                    }
                    Ty::Named(d, _) if self.p.defs[*d].kind == DefKind::Class => {
                        let mut cur = Some(*d);
                        let mut has = false;
                        while let Some(c) = cur {
                            if self.p.defs[c].copy.is_some() {
                                has = true;
                                break;
                            }
                            cur = match &self.p.defs[c].base {
                                Some(Ty::Named(b, _)) => Some(*b),
                                _ => None,
                            };
                        }
                        if !has {
                            let name = self.p.defs[*d].name.clone();
                            self.err(e.span, "R-7.11", format!("{name} hat kein copy"));
                        }
                        t.unformed().clone()
                    }
                    _ => t.unformed().clone(),
                }
            }
            E::Static(sr) => {
                self.last_target = None;
                let t = self.static_value(sr);
                self.record_call(e);
                t
            }
            E::Lambda { own, params, body } => {
                self.last_captures.clear();
                let t = self.lambda(*own, params, body, expected, as_arg, e.span);
                if *own && self.quiet == 0 {
                    let key = self.key(e);
                    let caps = std::mem::take(&mut self.last_captures);
                    self.p.rt.captures.insert(key, caps);
                }
                t
            }
            E::Unary { op, expr } => self.unary(*op, expr, expected, e.span),
            E::Binary { op, left, right } => self.binary(*op, left, right, expected, e.span),
            E::As { expr, ty } => {
                let st = self.expr(expr, None);
                let st = self.non_null_operand(st, expr, expr.span);
                let tt = self.resolve(ty);
                if st.is_error() || tt.is_error() {
                    return Ty::Error;
                }
                let ok = match (st.core(), tt.unformed()) {
                    (s @ Ty::Named(..), t @ Ty::Named(td, _)) => {
                        self.p.defs[*td].kind != DefKind::Enum && s != t && self.p.upcasts(t, s)
                    }
                    _ => false,
                };
                if !ok || matches!(tt, Ty::Nullable(_)) {
                    let msg = format!("{} ist kein Untertyp von {}", self.show(&tt), self.show(&st));
                    self.err(ty.span, "R-4.23", msg);
                    return Ty::Error;
                }
                match st.unformed() {
                    Ty::Formed(Form::Shared, _) => Ty::Formed(Form::Shared, Box::new(tt)).nullable(),
                    _ => tt.nullable(),
                }
            }
            E::Coalesce { left, right } => {
                let lt = self.expr(left, expected.map(|t| t.clone().nullable()).as_ref());
                let inner = if lt.is_error() {
                    Ty::Error
                } else if self.declared_nullable(left, &lt) {
                    Self::strip_null(&lt)
                } else {
                    let msg = format!("?? verlangt links einen nullbaren Wert, nicht {}", self.show(&lt));
                    self.err(left.span, "R-6.18", msg);
                    lt.clone()
                };
                // b wird nur ausgewertet, wenn a null ist (R-6.18).
                let base = self.flow.clone();
                let rt = self.expr(right, Some(expected.unwrap_or(&inner)));
                self.flow = base.join(&self.flow, "R-6.18", "R-5.15");
                self.join(&inner, &rt, right.span).unwrap_or(Ty::Error)
            }
            E::Conditional { cond, then, otherwise } => {
                let (w, f) = self.cond(cond);
                let base = self.flow.clone();
                self.flow.narrow(&w);
                let a = self.expr(then, expected);
                let after_then = std::mem::replace(&mut self.flow, base);
                self.flow.narrow(&f);
                let b = self.expr(otherwise, expected);
                self.flow = after_then.join(&self.flow, "R-6.7", "R-5.15");
                match self.join(&a, &b, e.span) {
                    Some(t) => t,
                    None => {
                        let msg = format!("{} und {} haben keinen gemeinsamen Typ", self.show(&a), self.show(&b));
                        self.err(e.span, "R-8.9", msg);
                        Ty::Error
                    }
                }
            }
            E::Member { object, name, safe } => self.member(object, name, *safe, e.span),
            E::Call { callee, args } => {
                self.last_target = None;
                let t = self.call(callee, args, e.span);
                self.record_call(e);
                t
            }
            E::Index { object, index } => {
                self.place_quiet += 1;
                let ot = self.expr(object, None);
                self.place_quiet -= 1;
                let ot = self.non_null_operand(ot, object, object.span);
                let saved = std::mem::replace(&mut self.place_quiet, 0);
                let it = self.expr(index, Some(&Ty::Prim(P::I64)));
                self.place_quiet = saved;
                if !it.is_error() && !self.assignable(&it, &Ty::Prim(P::I64)) {
                    let msg = format!("ein Index ist eine Ganzzahl, nicht {}", self.show(&it));
                    self.err(index.span, "R-4.33", msg);
                }
                match ot.unformed().core() {
                    Ty::Error => Ty::Error,
                    Ty::Prim(P::String) => Ty::Prim(P::Char),
                    Ty::Vector(t) | Ty::Array(t, _) | Ty::Slice(t) => (**t).clone(),
                    other => {
                        let msg = format!("{} hat keinen Index", self.show(other));
                        self.err(e.span, "R-8.7", msg);
                        Ty::Error
                    }
                }
            }
            E::Slice { object, from, to } => {
                let ot = self.expr(object, None);
                let ot = self.non_null_operand(ot, object, object.span);
                // Der Slice leiht den Behälter (Kapitel 5, Abschnitt 7); bis zum Ende der
                // Anweisung oder, an eine lokale Variable gebunden, bis zum Blockende.
                if let Some(pl) = self.place_of(object) {
                    let sd = self.stmt_depth;
                    self.push_loan(pl, false, Until::Stmt(sd), "R-5.17");
                }
                for b in [from, to].into_iter().flatten() {
                    let bt = self.expr(b, Some(&Ty::Prim(P::I64)));
                    if !bt.is_error() && !self.assignable(&bt, &Ty::Prim(P::I64)) {
                        self.err(b.span, "R-8.7", "Bereichsgrenzen sind Ganzzahlen");
                    }
                }
                match ot.unformed().core() {
                    Ty::Error => Ty::Error,
                    Ty::Vector(t) | Ty::Array(t, _) | Ty::Slice(t) => Ty::Slice(t.clone()),
                    other => {
                        let msg = format!("Bereiche gibt es nicht für {}", self.show(other));
                        self.err(e.span, "R-8.7", msg);
                        Ty::Error
                    }
                }
            }
            E::PostInc(x) | E::PostDec(x) => self.inc_dec(x, e.span),
        }
    }

    fn literal(&mut self, lit: &ast::Literal, expected: Option<&Ty>, span: Span, negative: bool) -> Ty {
        let target = expected.map(|t| t.unformed().core().clone()).and_then(|t| t.prim());
        match lit {
            ast::Literal::Int(v) => {
                let Some(mut value) = int_value(v) else {
                    self.err(span, "R-4.7", "ungültiges Literal");
                    return Ty::Error;
                };
                if negative {
                    value = -value;
                }
                match target {
                    Some(p) if is_numeric(p) => {
                        if !int_fits(value, p) {
                            self.err(span, "R-4.7", format!("{value} passt nicht in {}", prim_name(p)));
                        }
                        Ty::Prim(p)
                    }
                    _ => Ty::Prim(default_int(value)),
                }
            }
            ast::Literal::Float(v) => match target {
                Some(p) if is_float(p) => {
                    if !float_fits(v, p) {
                        self.err(span, "R-4.7", format!("{v} ist in {} nicht endlich darstellbar", prim_name(p)));
                    }
                    Ty::Prim(p)
                }
                Some(p) if is_integer(p) => {
                    self.err(span, "R-4.7", format!("ein Gleitkommaliteral passt nicht in {}", prim_name(p)));
                    Ty::Error
                }
                _ => Ty::Prim(P::F64),
            },
            ast::Literal::Str(_) => Ty::Prim(P::String),
            ast::Literal::Char(_) => Ty::Prim(P::Char),
            ast::Literal::Bool(_) => Ty::Prim(P::Bool),
            ast::Literal::Null => Ty::Null,
        }
    }

    fn array_literal(&mut self, items: &[ast::Expr], expected: Option<&Ty>, span: Span) -> Ty {
        let exp = expected.map(|t| t.unformed().clone());
        let el = match &exp {
            Some(Ty::Vector(t)) | Some(Ty::Array(t, _)) | Some(Ty::Slice(t)) => Some((**t).clone()),
            Some(Ty::Nullable(x)) => match x.unformed() {
                Ty::Vector(t) | Ty::Array(t, _) => Some((**t).clone()),
                _ => None,
            },
            _ => None,
        };
        if let Some(Ty::Array(_, Some(n))) = &exp {
            if *n != items.len() as u64 {
                self.err(span, "R-4.32", format!("das Array hat {n} Elemente, das Literal {}", items.len()));
            }
        }
        match el {
            Some(t) => {
                for x in items {
                    let xt = self.expr(x, Some(&t));
                    self.require(&xt, &t, x.span, Some(x));
                }
                match exp {
                    Some(Ty::Array(..)) => exp.unwrap(),
                    _ => Ty::Vector(Box::new(t)),
                }
            }
            None => {
                if items.is_empty() {
                    self.err(span, "R-4.32", "[] braucht einen Typ aus dem Zusammenhang");
                    return Ty::Error;
                }
                let first = self.expr(&items[0], None);
                for x in &items[1..] {
                    let xt = self.expr(x, Some(&first));
                    self.require(&xt, &first, x.span, Some(x));
                }
                Ty::Vector(Box::new(first))
            }
        }
    }

    fn name(&mut self, n: &ast::Ident) -> Ty {
        if let Some(l) = self.find_local(&n.name).cloned() {
            if !self.flow.dead {
                if self.flow.unassigned.remove(&l.id) {
                    self.err(n.span, "R-4.46", format!("{} ist nicht auf jedem Weg zugewiesen", n.name));
                }
                if let Some(reason) = self.flow.moved.remove(&l.id) {
                    let mut also = vec![reason];
                    if self.call_moved.iter().any(|c| c.contains(&l.id)) {
                        also.push("R-5.10");
                    }
                    if matches!(l.ty.core(), Ty::Named(d, _) if self.p.defs[*d].kind == DefKind::Enum) {
                        also.push("R-4.29");
                    }
                    if l.kind == LocalKind::Catch {
                        also.push("R-9.8");
                    }
                    self.err_also(n.span, "R-5.3", also, format!("{} wurde verschoben und ist nicht mehr verfügbar", n.name));
                }
            }
            // Erfassung durch Lambdas (R-10.7 bis R-10.9)
            let frames = self.capturing_frames(l.id);
            if !frames.is_empty() {
                for f in frames.clone() {
                    if !self.frames[f].captures.contains(&l.id) {
                        self.frames[f].captures.push(l.id);
                    }
                }
                let by_own = frames.clone().any(|f| self.frames[f].own_lambda);
                let plain = matches!(l.ty.unformed().core(), Ty::Prim(_)) || Self::is_shared(&l.ty);
                if by_own && l.kind.is_borrow() && !plain {
                    self.err(n.span, "R-10.9", format!("{} ist nur geliehen und kann nicht per Wert erfasst werden (copy verwenden)", n.name));
                }
            }
            return self.local_type(&l);
        }
        if let Some(st) = self.self_ty() {
            if let Some((f, owner)) = self.p.field_of(&st, &n.name) {
                if !f.is_static && self.ctx.is_static {
                    self.err(n.span, "R-7.4", format!("{} ist ein Feld; statische Methoden haben kein this", n.name));
                    return Ty::Error;
                }
                if !f.is_static && self.frames.iter().any(|f| f.own_lambda) {
                    self.err(n.span, "R-10.10", "ein own-Lambda kann this nicht erfassen");
                    return Ty::Error;
                }
                self.check_access(f.access, owner, n.span);
                return f.ty;
            }
        }
        let rule = if self.in_mixin() { "R-7.19" } else { "R-8.16" };
        self.err(n.span, rule, format!("{} ist hier nicht bekannt", n.name));
        Ty::Error
    }

    fn check_access(&mut self, access: Access, owner: DefId, span: Span) {
        let ok = match access {
            Access::Public => true,
            Access::Private => self.ctx.class == Some(owner),
            Access::Protected => self.ctx.class.is_some_and(|c| self.p.is_subclass(c, owner)),
        };
        if !ok {
            let name = self.p.defs[owner].name.clone();
            self.err(span, "R-7.2", format!("der Member ist in {name} nicht sichtbar"));
        }
    }

    fn join(&self, a: &Ty, b: &Ty, _span: Span) -> Option<Ty> {
        if a.is_error() {
            return Some(b.clone());
        }
        if b.is_error() {
            return Some(a.clone());
        }
        if let (Some(x), Some(y)) = (a.prim(), b.prim()) {
            if is_numeric(x) && is_numeric(y) {
                return common_numeric(x, y).map(Ty::Prim);
            }
        }
        match (a, b) {
            (Ty::Null, t) | (t, Ty::Null) => return Some(t.clone().nullable()),
            _ => {}
        }
        if self.assignable(a, b) {
            Some(b.clone())
        } else if self.assignable(b, a) {
            Some(a.clone())
        } else {
            None
        }
    }

    fn cast(&mut self, ty: &ast::Type, x: &ast::Expr, span: Span) -> Ty {
        let target = self.resolve(ty);
        let src = self.expr(x, None);
        if src.is_error() || target.is_error() {
            return target;
        }
        let (s, t) = (src.unformed(), target.unformed());
        let is_plain_enum = |p: &Program, t: &Ty| match t {
            Ty::Named(d, _) => p.defs[*d].kind == DefKind::Enum && p.defs[*d].variants.iter().all(|v| v.fields.is_empty()),
            _ => false,
        };
        let ok = match (s.prim(), t.prim()) {
            (Some(P::Bool), _) | (_, Some(P::Bool)) => {
                self.err(span, "R-4.3", "zwischen bool und anderen Typen gibt es keine Umwandlung");
                return Ty::Error;
            }
            (Some(a), Some(b)) if is_numeric(a) && is_numeric(b) => true,
            (Some(P::Char), Some(b)) | (Some(b), Some(P::Char)) if is_integer(b) => true,
            (Some(P::Byte), Some(P::Ui8)) | (Some(P::Ui8), Some(P::Byte)) => true,
            (Some(a), Some(b)) if a == b => true,
            (Some(a), None) if is_integer(a) => is_plain_enum(self.p, t),
            (None, Some(b)) if is_integer(b) => is_plain_enum(self.p, s),
            _ => false,
        };
        if !ok {
            let msg = format!("cast von {} nach {} gibt es nicht", self.show(&src), self.show(&target));
            self.err(span, "R-4.8", msg);
            return Ty::Error;
        }
        target
    }

    fn inc_dec(&mut self, x: &ast::Expr, span: Span) -> Ty {
        let t = self.lvalue(x);
        if !t.is_error() && !t.is_integer() {
            let msg = format!("++ und -- gibt es nur für Ganzzahlen, nicht für {}", self.show(&t));
            self.err(span, "R-8.8", msg);
            return Ty::Error;
        }
        t
    }

    fn unary(&mut self, op: U, x: &ast::Expr, expected: Option<&Ty>, span: Span) -> Ty {
        match op {
            U::Not => {
                let t = self.expr(x, Some(&Ty::Prim(P::Bool)));
                let t = self.non_null_operand(t, x, x.span);
                if !t.is_error() && !t.is_bool() {
                    let msg = format!("! verlangt bool, nicht {}", self.show(&t));
                    self.err(span, "R-4.3", msg);
                }
                Ty::Prim(P::Bool)
            }
            U::Neg => {
                if let E::Literal(lit @ (ast::Literal::Int(_) | ast::Literal::Float(_))) = &x.kind {
                    let t = self.literal(lit, expected, span, true);
                    if let Some(p) = t.prim() {
                        if is_unsigned(p) {
                            self.err(span, "R-4.13", format!("einstelliges - auf {}", prim_name(p)));
                        }
                    }
                    return t;
                }
                let t = self.expr(x, expected);
                let t = self.non_null_operand(t, x, x.span);
                match t.prim() {
                    _ if t.is_error() => Ty::Error,
                    Some(p) if is_unsigned(p) => {
                        self.err(span, "R-4.13", format!("einstelliges - auf dem vorzeichenlosen Typ {}", prim_name(p)));
                        Ty::Error
                    }
                    Some(p) if is_numeric(p) => t,
                    _ => {
                        let msg = format!("- verlangt eine Zahl, nicht {}", self.show(&t));
                        self.err(span, "R-4.13", msg);
                        Ty::Error
                    }
                }
            }
            U::BitNot => {
                let t = self.expr(x, expected);
                let t = self.non_null_operand(t, x, x.span);
                if !t.is_error() && !t.is_integer() && t.prim() != Some(P::Byte) {
                    let msg = format!("~ verlangt eine Ganzzahl oder byte, nicht {}", self.show(&t));
                    self.err(span, "R-4.16", msg);
                    return Ty::Error;
                }
                t
            }
            U::PreInc | U::PreDec => self.inc_dec(x, span),
        }
    }

    /// Typ von `a op b` für arithmetische Operatoren (R-4.4, R-4.5, R-4.12, R-4.13, R-4.20).
    fn arith(&mut self, op: B, a: &Ty, b: &Ty, span: Span) -> Ty {
        if a.is_error() || b.is_error() {
            return Ty::Error;
        }
        if op == B::Add && (a.is_string() || b.is_string()) {
            if a.is_string() && b.is_string() {
                return Ty::Prim(P::String);
            }
            self.err(span, "R-4.20", "+ verbindet nur zwei Strings; Zahlen brauchen toString()");
            return Ty::Error;
        }
        let (Some(x), Some(y)) = (a.prim(), b.prim()) else {
            let msg = format!("{} gibt es nicht für {} und {}", op_str(op), self.show(a), self.show(b));
            self.err(span, "R-4.13", msg);
            return Ty::Error;
        };
        for p in [x, y] {
            let rule = match p {
                P::Byte => Some("R-4.5"),
                P::Char => Some("R-4.4"),
                P::Bool => Some("R-4.3"),
                P::String => Some("R-4.20"),
                _ => None,
            };
            if let Some(rule) = rule {
                self.err(span, rule, format!("keine Arithmetik mit {}", prim_name(p)));
                return Ty::Error;
            }
        }
        match common_numeric(x, y) {
            Some(c) => Ty::Prim(c),
            None => {
                self.err(span, "R-4.12", format!("{} und {} haben keinen gemeinsamen Typ; nötig ist cast", prim_name(x), prim_name(y)));
                Ty::Error
            }
        }
    }

    /// Ein Operand der Arithmetik, eines Vergleichs oder einer Dereferenzierung muss
    /// verengt sein (R-6.2, R-6.23). Liefert den Typ ohne `?`.
    fn non_null_operand(&mut self, t: Ty, e: &ast::Expr, span: Span) -> Ty {
        if t.is_error() || !Self::nullable(&t) || matches!(t, Ty::Null) {
            return t;
        }
        let mut also = Vec::new();
        let rule = if matches!(Self::strip_null(&t).unformed(), Ty::Prim(_)) {
            also.push("R-6.2");
            "R-6.23"
        } else {
            "R-6.2"
        };
        also.extend(self.null_reason(e));
        let msg = format!("{} hat den nullbaren Typ {} und ist hier nicht geprüft", expr_text(e), self.show(&t));
        self.err_also(span, rule, also, msg);
        Self::strip_null(&t)
    }

    /// Prüft beide Operanden; ein Literal bekommt den Typ des anderen Operanden (R-4.7).
    fn operands(&mut self, l: &ast::Expr, r: &ast::Expr) -> (Ty, Ty) {
        let (a, b) = self.operands_raw(l, r);
        (self.non_null_operand(a, l, l.span), self.non_null_operand(b, r, r.span))
    }

    fn operands_raw(&mut self, l: &ast::Expr, r: &ast::Expr) -> (Ty, Ty) {
        match (is_literal(l), is_literal(r)) {
            (true, false) => {
                let rt = self.expr(r, None);
                let ctx = if rt.is_numeric() { Some(rt.clone()) } else { None };
                (self.expr(l, ctx.as_ref()), rt)
            }
            (false, true) => {
                let lt = self.expr(l, None);
                let ctx = if lt.is_numeric() { Some(lt.clone()) } else { None };
                let rt = self.expr(r, ctx.as_ref());
                (lt, rt)
            }
            _ => (self.expr(l, None), self.expr(r, None)),
        }
    }

    fn binary(&mut self, op: B, l: &ast::Expr, r: &ast::Expr, _expected: Option<&Ty>, span: Span) -> Ty {
        match op {
            B::And | B::Or => {
                // R-6.7: b wird unter W(a) bzw. F(a) geprüft und nur bedingt ausgewertet.
                let mut base = Flow::default();
                for (i, x) in [l, r].into_iter().enumerate() {
                    if i == 1 {
                        let (w, f) = self.facts(l);
                        base = self.flow.clone();
                        let (add, drop) = if op == B::And { (w, f) } else { (f, w) };
                        self.flow.narrow(&add);
                        for v in drop.difference(&add) {
                            self.flow.unnarrow(*v, "R-6.7");
                        }
                    }
                    let t = self.expr(x, Some(&Ty::Prim(P::Bool)));
                    if !t.is_error() && !t.is_bool() {
                        let msg = format!("{} verlangt bool, nicht {}", op_str(op), self.show(&t));
                        self.err(x.span, "R-4.3", msg);
                    }
                }
                self.flow = base.join(&self.flow, "R-6.7", "R-5.15");
                Ty::Prim(P::Bool)
            }
            B::Add | B::Sub | B::Mul | B::Div | B::Rem => {
                let (a, b) = self.operands(l, r);
                self.arith(op, &a, &b, span)
            }
            B::Shl | B::Shr => {
                let a = self.expr(l, None);
                let a = self.non_null_operand(a, l, l.span);
                let b = self.expr(r, Some(&Ty::Prim(P::I32)));
                let b = self.non_null_operand(b, r, r.span);
                if a.is_error() || b.is_error() {
                    return Ty::Error;
                }
                if !(a.is_integer() || a.prim() == Some(P::Byte)) || !b.is_integer() {
                    self.err(span, "R-4.16", "Schiebeoperatoren verlangen Ganzzahlen");
                    return Ty::Error;
                }
                a
            }
            B::BitAnd | B::BitOr | B::BitXor => {
                let (a, b) = self.operands(l, r);
                if a.is_error() || b.is_error() {
                    return Ty::Error;
                }
                if a.prim() == Some(P::Byte) && b.prim() == Some(P::Byte) {
                    return a;
                }
                if a.is_integer() && b.is_integer() {
                    return match common_numeric(a.prim().unwrap(), b.prim().unwrap()) {
                        Some(c) => Ty::Prim(c),
                        None => {
                            self.err(span, "R-4.12", "kein gemeinsamer Typ");
                            Ty::Error
                        }
                    };
                }
                let msg = format!("{} gibt es für Ganzzahlen und byte, nicht für {} und {}", op_str(op), self.show(&a), self.show(&b));
                self.err(span, "R-4.16", msg);
                Ty::Error
            }
            B::Lt | B::Gt | B::Le | B::Ge => {
                let (a, b) = self.operands(l, r);
                self.compare(&a, &b, span);
                Ty::Prim(P::Bool)
            }
            B::Eq | B::Ne => {
                let with_null = matches!(r.kind, E::Literal(ast::Literal::Null)) || matches!(l.kind, E::Literal(ast::Literal::Null));
                if with_null {
                    // R-6.3: Vergleich mit null nur für nullbare Werte
                    let (a, b) = (self.expr(l, None), self.expr(r, None));
                    let (other, oe) = if matches!(r.kind, E::Literal(ast::Literal::Null)) { (&a, l) } else { (&b, r) };
                    if matches!(other, Ty::Null) {
                        self.err(span, "R-6.3", "null == null steht schon fest");
                    } else if !other.is_error() && !self.declared_nullable(oe, other) {
                        let msg = format!("{} ist nicht nullbar; der Vergleich mit null steht schon fest", self.show(other));
                        self.err(span, "R-6.3", msg);
                    }
                    return Ty::Prim(P::Bool);
                }
                let (a, b) = self.operands(l, r);
                self.equality(&a, &b, span);
                Ty::Prim(P::Bool)
            }
        }
    }

    fn compare(&mut self, a: &Ty, b: &Ty, span: Span) {
        if a.is_error() || b.is_error() {
            return;
        }
        if let (Some(x), Some(y)) = (a.prim(), b.prim()) {
            if is_numeric(x) && is_numeric(y) {
                if common_numeric(x, y).is_none() {
                    self.err(span, "R-4.12", format!("{} und {} haben keinen gemeinsamen Typ", prim_name(x), prim_name(y)));
                }
                return;
            }
            if x == y && matches!(x, P::Char | P::Byte | P::String) {
                return;
            }
        }
        if let Some(cmp) = self.p.std_def("rpp.core.Comparable") {
            let m = Ty::Named(cmp, vec![b.unformed().clone()]);
            if self.p.satisfies(a.unformed(), &m) {
                return;
            }
        }
        let rule = if matches!(a.unformed(), Ty::Param(_)) { "R-4.42" } else { "R-4.17" };
        let msg = format!("{} und {} lassen sich nicht mit < vergleichen", self.show(a), self.show(b));
        self.err(span, rule, msg);
    }

    fn equality(&mut self, a: &Ty, b: &Ty, span: Span) {
        if a.is_error() || b.is_error() {
            return;
        }
        if matches!(a, Ty::Null) || matches!(b, Ty::Null) {
            return;
        }
        let (x, y) = (a.unformed().core(), b.unformed().core());
        let rule_msg: Option<(&'static str, String)> = match (x, y) {
            (Ty::Prim(p), Ty::Prim(q)) if is_numeric(*p) && is_numeric(*q) => {
                if common_numeric(*p, *q).is_none() {
                    Some(("R-4.12", format!("{} und {} haben keinen gemeinsamen Typ", prim_name(*p), prim_name(*q))))
                } else {
                    None
                }
            }
            (Ty::Prim(p), Ty::Prim(q)) if p == q => None,
            (Ty::Tuple(_), _) | (_, Ty::Tuple(_)) => Some(("R-4.35", "Tupel haben kein ==".into())),
            (Ty::Func(..), _) | (_, Ty::Func(..)) => Some(("R-4.38", "Funktionstypen haben kein ==".into())),
            (Ty::Vector(_) | Ty::Array(..) | Ty::Slice(_), _) => Some(("R-4.33", "Arrays haben kein ==".into())),
            (Ty::Named(d, _), _) if self.p.defs[*d].kind == DefKind::Enum => {
                if self.p.defs[*d].variants.iter().any(|v| !v.fields.is_empty()) {
                    Some(("R-4.28", "Enums mit Daten vergleicht man mit switch".into()))
                } else if x != y {
                    Some(("R-4.28", "verschiedene Enums".into()))
                } else {
                    None
                }
            }
            (Ty::Named(..) | Ty::Param(_), _) => {
                let eq = self.p.std_def("rpp.core.Equatable");
                let ok = eq.is_some_and(|eq| {
                    let m = Ty::Named(eq, vec![y.clone()]);
                    self.p.satisfies(x, &m) || {
                        // Equatable<T> mit einer Oberklasse T
                        std::iter::once(x.clone()).chain(self.p.supertypes(x)).any(|s| {
                            self.p.supertypes(&s).iter().chain(std::iter::once(&s)).any(|t| match t {
                                Ty::Named(d, args) if *d == eq => args.first().is_some_and(|a| self.assignable(y, a)),
                                _ => false,
                            })
                        })
                    }
                });
                if ok {
                    None
                } else if matches!(x, Ty::Param(_)) {
                    Some(("R-4.42", format!("== verlangt {} : Equatable", self.show(x))))
                } else {
                    Some(("R-4.24", format!("{} importiert Equatable nicht", self.show(x))))
                }
            }
            _ => Some(("R-4.17", format!("{} und {} lassen sich nicht vergleichen", self.show(a), self.show(b)))),
        };
        if let Some((rule, msg)) = rule_msg {
            self.err(span, rule, msg);
        }
    }

    // ------------------------------------------------------------ Member

    /// Hat der Ausdruck einen nullbaren Typ? Für Variablen zählt der deklarierte Typ,
    /// auch wenn sie an dieser Stelle verengt sind (R-6.3, R-6.16, R-6.18).
    fn declared_nullable(&self, e: &ast::Expr, t: &Ty) -> bool {
        match &e.kind {
            E::Paren(x) => self.declared_nullable(x, t),
            E::Name(n) => match self.find_local(&n.name) {
                Some(l) => Self::nullable(&l.ty),
                None => Self::nullable(t),
            },
            _ => Self::nullable(t),
        }
    }

    /// Empfänger eines Zugriffs `x.f`, `x.m()` (R-6.2) bzw. `x?.f` (R-6.16): ohne `?`.
    fn receiver(&mut self, ot: Ty, object: &ast::Expr, safe: bool, span: Span) -> Ty {
        if ot.is_error() {
            return ot;
        }
        if safe {
            if !self.declared_nullable(object, &ot) {
                let msg = format!("?. auf dem nicht nullbaren Wert {}", self.show(&ot));
                self.err(span, "R-6.16", msg);
            }
            return Self::strip_null(&ot);
        }
        self.non_null_operand(ot, object, object.span)
    }

    fn member(&mut self, object: &ast::Expr, name: &ast::Ident, safe: bool, span: Span) -> Ty {
        self.place_quiet += 1;
        let ot = self.expr(object, None);
        self.place_quiet -= 1;
        let ot = self.receiver(ot, object, safe, span);
        let t = self.field_type(&ot, name, span);
        if safe {
            t.nullable()
        } else {
            t
        }
    }

    fn field_type(&mut self, ot: &Ty, name: &ast::Ident, span: Span) -> Ty {
        let core = ot.unformed().core().clone();
        if ot.is_error() {
            return Ty::Error;
        }
        if weak_inner(ot).is_some() {
            self.err(span, "R-4.41", "über weak greift man nur mit lock() zu");
            return Ty::Error;
        }
        match (&core, name.name.as_str()) {
            (Ty::Prim(P::String), "length" | "byteLength") => return Ty::Prim(P::I64),
            (Ty::Vector(_) | Ty::Array(..) | Ty::Slice(_), "length") => return Ty::Prim(P::I64),
            _ => {}
        }
        if let Ty::Named(..) = core {
            if let Some((f, owner)) = self.p.field_of(&core, &name.name) {
                if f.is_static {
                    self.err(span, "R-7.27", format!("{} ist statisch und wird mit :: angesprochen", name.name));
                }
                self.check_access(f.access, owner, name.span);
                return f.ty;
            }
        }
        let rule = self.unknown_member_rule(&core);
        let msg = format!("{} hat kein Feld {}", self.show(&core), name.name);
        self.err(name.span, rule, msg);
        Ty::Error
    }

    fn unknown_member_rule(&self, t: &Ty) -> &'static str {
        match t {
            Ty::Named(d, _) if self.p.defs[*d].kind == DefKind::Mixin => "R-7.22",
            Ty::Param(_) => "R-7.24",
            _ => "R-7.1",
        }
    }

    // ------------------------------------------------------------ Aufrufe

    fn call(&mut self, callee: &ast::Expr, args: &[ast::Arg], span: Span) -> Ty {
        self.last_call = None;
        let (t, facts) = self.call_inner(callee, args, span);
        self.last_call = Some(facts);
        t
    }

    fn call_inner(&mut self, callee: &ast::Expr, args: &[ast::Arg], span: Span) -> (Ty, CallFacts) {
        let plain = CallFacts { nodiscard: false, func_value: false, never_returns: false };
        match &callee.kind {
            E::Name(n) => {
                if self.find_local(&n.name).is_some() {
                    let t = self.expr(callee, None);
                    let t = self.non_null_operand(t, callee, callee.span);
                    return (self.call_value(&t, args, callee.span), CallFacts { func_value: true, ..plain });
                }
                let own_method = self.self_ty().is_some_and(|st| !self.p.methods_of(&st, &n.name).is_empty());
                if !own_method {
                    if let Some(r) = self.builtin_function(&n.name, args, span) {
                        self.last_target = Some(Target::Builtin(n.name.clone()));
                        return r;
                    }
                }
                let Some(st) = self.self_ty() else {
                    self.err(n.span, "R-8.16", format!("{} ist hier nicht bekannt", n.name));
                    return (Ty::Error, plain);
                };
                let cands = self.p.methods_of(&st, &n.name);
                if cands.is_empty() {
                    if self.p.field_of(&st, &n.name).is_some() {
                        let t = self.expr(callee, None);
                        let t = self.non_null_operand(t, callee, callee.span);
                        return (self.call_value(&t, args, callee.span), CallFacts { func_value: true, ..plain });
                    }
                    let rule = if self.in_mixin() { "R-7.19" } else { "R-8.16" };
                    self.err(n.span, rule, format!("{} ist hier nicht bekannt", n.name));
                    return (Ty::Error, plain);
                }
                self.pending_recv = Some(Place::var(THIS));
                self.invoke(cands, args, span, Receiver::This, &n.name)
            }
            E::Member { object, name, safe } => {
                if let E::Base = object.kind {
                    let base = self.ctx.class.and_then(|d| self.p.defs[d].base.clone());
                    if self.ctx.is_static || base.is_none() {
                        self.err(callee.span, "R-7.15", "base.m() gibt es nur in Methoden einer Unterklasse");
                        return (Ty::Error, plain);
                    }
                    let cands = self.p.methods_of(&base.unwrap(), &name.name);
                    self.pending_recv = Some(Place::var(THIS));
                    let r = self.invoke(cands, args, span, Receiver::This, &name.name);
                    if let Some(Target::Method(m)) = self.last_target.take() {
                        self.last_target = Some(Target::Exact(m));
                    }
                    return r;
                }
                let ot = self.expr(object, None);
                if ot.is_error() {
                    for a in args {
                        self.probe(|c| c.expr(&a.expr, None));
                    }
                    return (Ty::Error, plain);
                }
                let ot = self.receiver(ot, object, *safe, callee.span);
                // R-6.16: Bei null wird der Rest der Kette übersprungen, auch die Argumente.
                let base = if *safe { Some(self.flow.clone()) } else { None };
                self.pending_recv = self.place_of(object);
                let (t, facts) = self.method_call(&ot, name, args, span);
                self.pending_recv = None;
                if let Some(b) = base {
                    self.flow = b.join(&self.flow, "R-6.16", "R-5.15");
                }
                (if *safe { t.nullable() } else { t }, facts)
            }
            E::Static(sr) => self.static_call(sr, args, span),
            E::Paren(inner) if matches!(inner.kind, E::Lambda { .. }) => {
                let E::Lambda { own, params, body } = &inner.kind else { unreachable!() };
                let ps: Vec<PInfo> = params.iter().map(|p| self.param_info(p)).collect();
                for (a, p) in args.iter().zip(&ps) {
                    let at = self.expr(&a.expr, Some(&p.ty));
                    self.require(&at, &p.ty, a.expr.span, Some(&a.expr));
                    if p.mode == Mode::Own {
                        self.consume(&a.expr, &at, &p.ty, "R-5.3", &[]);
                    }
                }
                if args.len() != ps.len() {
                    self.err(span, "R-10.17", "falsche Anzahl von Argumenten");
                }
                let t = self.lambda_body(*own, &ps, body, None, inner.span);
                (t, CallFacts { func_value: true, ..plain })
            }
            _ => {
                let t = self.expr(callee, None);
                (self.call_value(&t, args, callee.span), CallFacts { func_value: true, ..plain })
            }
        }
    }

    /// Aufruf eines Funktionswerts (R-10.17).
    fn call_value(&mut self, t: &Ty, args: &[ast::Arg], span: Span) -> Ty {
        match t.unformed().core() {
            Ty::Error => {
                for a in args {
                    self.expr(&a.expr, None);
                }
                Ty::Error
            }
            Ty::Func(ps, r) => {
                if ps.len() != args.len() {
                    self.err(span, "R-10.17", format!("die Funktion erwartet {} Argumente", ps.len()));
                }
                let m = MethodInfo {
                    name: String::new(),
                    owner: usize::MAX,
                    params: ps.iter().enumerate().map(|(i, p)| PInfo { name: format!("p{i}"), ty: p.ty.clone(), mode: p.mode }).collect(),
                    ret: Ret::Void,
                    tparams: Vec::new(),
                    targs: Vec::new(),
                    is_static: true,
                    modifier: None,
                    nodiscard: false,
                    mut_this: false,
                    access: Access::Public,
                    has_body: true,
                    span: Span::default(),
                    ast: (0, 0, 0),
                };
                let id = self.begin_call();
                self.check_args(&m, args, id);
                self.end_call(id);
                self.last_target = Some(Target::FuncValue);
                (**r).clone()
            }
            other => {
                let msg = format!("{} ist keine Funktion", self.show(other));
                self.err(span, "R-10.17", msg);
                Ty::Error
            }
        }
    }

    fn param_info(&mut self, p: &ast::Param) -> PInfo {
        let ty = self.resolve(&p.ty);
        let mode = if p.mutable {
            Mode::Mut
        } else if p.ty.ownership == Some(ast::Ownership::Own) {
            Mode::Own
        } else {
            Mode::Read
        };
        PInfo { name: p.name.name.clone(), ty, mode }
    }

    fn method_call(&mut self, ot: &Ty, name: &ast::Ident, args: &[ast::Arg], span: Span) -> (Ty, CallFacts) {
        let plain = CallFacts { nodiscard: false, func_value: false, never_returns: false };
        let core = ot.unformed().core().clone();
        if let Some(inner) = weak_inner(ot) {
            if name.name == "lock" && args.is_empty() {
                self.last_target = Some(Target::Lock);
                return (Ty::Formed(Form::Shared, Box::new(inner)).nullable(), CallFacts { nodiscard: true, ..plain });
            }
            self.err(name.span, "R-4.41", "über weak greift man nur mit lock() zu");
            return (Ty::Error, plain);
        }
        if let Some(cands) = self.builtin_methods(&core, &name.name) {
            return self.invoke(cands, args, span, Receiver::Builtin(core), &name.name);
        }
        let recv = self.pending_recv.take();
        let cands = match &core {
            Ty::Named(..) => self.p.methods_of(&core, &name.name),
            Ty::Param(p) => {
                let cons = self.p.tparams[*p].constraints.clone();
                cons.iter().flat_map(|c| self.p.methods_of(c, &name.name)).collect()
            }
            _ => Vec::new(),
        };
        if cands.is_empty() {
            if let Ty::Named(..) = core {
                if let Some((f, owner)) = self.p.field_of(&core, &name.name) {
                    self.check_access(f.access, owner, name.span);
                    return (self.call_value(&f.ty, args, name.span), CallFacts { func_value: true, ..plain });
                }
            }
            let rule = self.unknown_member_rule(&core);
            let msg = format!("{} hat keine Methode {}", self.show(&core), name.name);
            self.err(name.span, rule, msg);
            for a in args {
                self.probe(|c| c.expr(&a.expr, None));
            }
            return (Ty::Error, plain);
        }
        self.pending_recv = recv;
        self.invoke(cands, args, span, Receiver::Object, &name.name)
    }

    fn static_call(&mut self, sr: &ast::StaticRef, args: &[ast::Arg], span: Span) -> (Ty, CallFacts) {
        let plain = CallFacts { nodiscard: false, func_value: false, never_returns: false };
        let owner = match &sr.owner {
            ast::StaticOwner::Primitive(p) => {
                self.pending_recv = None;
                if let Some(cands) = self.builtin_statics(*p, &sr.member.name) {
                    let r = self.invoke(cands, args, span, Receiver::Static, &sr.member.name);
                    if self.last_target.is_some() {
                        self.last_target = Some(Target::BuiltinStatic(*p, sr.member.name.clone()));
                    }
                    return r;
                }
                self.err(sr.member.span, "R-13.3", format!("{}::{} gibt es nicht", prim_name(*p), sr.member.name));
                return (Ty::Error, plain);
            }
            ast::StaticOwner::Named(tn) => self.resolve(&ast::Type { ownership: None, base: ast::BaseType::Named(tn.clone()), suffixes: vec![], span: sr.span }),
        };
        let Ty::Named(d, _) = &owner else {
            if !owner.is_error() {
                self.err(sr.span, "R-7.27", "vor :: steht ein Typ");
            }
            return (Ty::Error, plain);
        };
        let d = *d;
        match self.p.defs[d].kind {
            DefKind::Enum => {
                let v = self.p.defs[d].variants.iter().find(|v| v.name == sr.member.name).cloned();
                match v {
                    Some(v) if !v.fields.is_empty() => {
                        if v.fields.len() != args.len() {
                            self.err(span, "R-4.25", format!("{} hat {} Daten", v.name, v.fields.len()));
                        }
                        for (a, f) in args.iter().zip(&v.fields) {
                            let at = self.expr(&a.expr, Some(&f.ty));
                            self.require(&at, &f.ty, a.expr.span, Some(&a.expr));
                            self.consume(&a.expr, &at, &f.ty, "R-5.3", &["R-4.29"]);
                        }
                        let idx = self.p.defs[d].variants.iter().position(|x| x.name == v.name).unwrap_or(0);
                        self.last_target = Some(Target::Variant(d, idx));
                        (owner.clone(), plain)
                    }
                    _ => {
                        self.err(sr.member.span, "R-4.25", format!("{} ist keine Variante mit Daten", sr.member.name));
                        (Ty::Error, plain)
                    }
                }
            }
            DefKind::Mixin => {
                // M::m() ruft die Fassung aus dem importierten Mixin für this auf (R-7.15).
                let imported = self.self_ty().is_some_and(|st| self.p.supertypes(&st).iter().any(|s| matches!(s, Ty::Named(x, _) if *x == d)));
                if !imported || self.ctx.is_static {
                    self.err(sr.span, "R-7.15", "M::m() gibt es nur in einer Klasse, die M importiert");
                    return (Ty::Error, plain);
                }
                let st = self.self_ty().unwrap();
                let mt = self.p.supertypes(&st).into_iter().find(|s| matches!(s, Ty::Named(x, _) if *x == d)).unwrap();
                let cands: Vec<MethodInfo> = self.p.methods_of(&mt, &sr.member.name).into_iter().filter(|m| m.owner == d).collect();
                self.pending_recv = Some(Place::var(THIS));
                let r = self.invoke(cands, args, span, Receiver::This, &sr.member.name);
                if let Some(Target::Method(m)) = self.last_target.take() {
                    self.last_target = Some(Target::Exact(m));
                }
                r
            }
            DefKind::Class => {
                let cands: Vec<MethodInfo> = self.p.methods_of(&owner, &sr.member.name).into_iter().filter(|m| m.is_static).collect();
                if cands.is_empty() {
                    let name = self.p.defs[d].name.clone();
                    self.err(sr.member.span, "R-7.27", format!("{name} hat keine statische Methode {}", sr.member.name));
                    return (Ty::Error, plain);
                }
                self.invoke(cands, args, span, Receiver::Static, &sr.member.name)
            }
        }
    }

    fn static_value(&mut self, sr: &ast::StaticRef) -> Ty {
        match &sr.owner {
            ast::StaticOwner::Primitive(p) => {
                if matches!(sr.member.name.as_str(), "MIN" | "MAX") && is_numeric(*p) {
                    return Ty::Prim(*p);
                }
                self.err(sr.member.span, "R-13.3", format!("{}::{} ist kein Wert", prim_name(*p), sr.member.name));
                Ty::Error
            }
            ast::StaticOwner::Named(tn) => {
                let owner = self.resolve(&ast::Type { ownership: None, base: ast::BaseType::Named(tn.clone()), suffixes: vec![], span: sr.span });
                let Ty::Named(d, _) = &owner else { return Ty::Error };
                let d = *d;
                if self.p.defs[d].kind == DefKind::Enum {
                    match self.p.defs[d].variants.iter().find(|v| v.name == sr.member.name) {
                        Some(v) if v.fields.is_empty() => {
                            let idx = self.p.defs[d].variants.iter().position(|x| x.name == v.name).unwrap_or(0);
                            self.last_target = Some(Target::Variant(d, idx));
                            return owner.clone();
                        }
                        Some(_) => {
                            self.err(sr.member.span, "R-4.25", format!("{} trägt Daten: {}(…)", sr.member.name, sr.member.name));
                            return Ty::Error;
                        }
                        None => {}
                    }
                }
                if let Some((f, fowner)) = self.p.field_of(&owner, &sr.member.name) {
                    if f.is_static {
                        self.check_access(f.access, fowner, sr.member.span);
                        self.last_target = Some(Target::StaticField(fowner, sr.member.name.clone()));
                        return f.ty;
                    }
                }
                let name = self.p.defs[d].name.clone();
                self.err(sr.member.span, "R-7.27", format!("{name}::{} ist kein statischer Wert", sr.member.name));
                Ty::Error
            }
        }
    }

    /// Typ eines Arguments ohne Zusammenhang, für die Auflösung von Überladungen.
    fn arg_shape(&mut self, a: &ast::Arg) -> ArgShape {
        match &a.expr.kind {
            E::Lambda { params, .. } => {
                let ps: Vec<PInfo> = self.probe(|c| params.iter().map(|p| c.param_info(p)).collect());
                ArgShape::Lambda(ps)
            }
            E::Literal(ast::Literal::Null) => ArgShape::Null,
            _ => match literal_info(&a.expr) {
                Some((false, v)) => ArgShape::Int(v),
                Some((true, _)) => ArgShape::Float,
                None => ArgShape::Ty(self.probe(|c| c.expr(&a.expr, None))),
            },
        }
    }

    /// Rang eines Arguments für einen Parametertyp (R-7.17); `None`: passt nicht.
    fn rank(&self, a: &ArgShape, p: &Ty) -> Option<u8> {
        let (pt, nullable) = match p.unformed() {
            Ty::Nullable(x) => (x.unformed().clone(), true),
            other => (other.clone(), false),
        };
        let r = match a {
            ArgShape::Null => return if nullable || p.is_error() { Some(0) } else { None },
            ArgShape::Int(v) => match pt.prim() {
                Some(q) if q == default_int(*v) => 0,
                Some(q) if is_numeric(q) && int_fits(*v, q) => 1,
                _ if pt.is_error() || matches!(pt, Ty::Param(_)) => 0,
                _ => return None,
            },
            ArgShape::Float => match pt.prim() {
                Some(P::F64) => 0,
                Some(P::F32) => 1,
                _ if pt.is_error() || matches!(pt, Ty::Param(_)) => 0,
                _ => return None,
            },
            ArgShape::Lambda(ps) => match &pt {
                Ty::Func(fps, _) => {
                    let ok = fps.len() == ps.len() && fps.iter().zip(ps).all(|(f, l)| f.mode == l.mode && (f.ty.unformed() == l.ty.unformed() || l.ty.is_error() || matches!(f.ty, Ty::Param(_))));
                    if ok {
                        0
                    } else {
                        return None;
                    }
                }
                Ty::Error | Ty::Param(_) => 0,
                _ => return None,
            },
            ArgShape::Ty(t) => {
                let t = match t.unformed() {
                    Ty::Nullable(x) => x.unformed().clone(),
                    other => other.clone(),
                };
                if t.is_error() || pt.is_error() || matches!(pt, Ty::Param(_)) || t == pt {
                    0
                } else if let (Some(x), Some(y)) = (t.prim(), pt.prim()) {
                    if auto_converts(x, y) {
                        1
                    } else {
                        return None;
                    }
                } else if matches!((&t, &pt), (Ty::Vector(a) | Ty::Array(a, _) | Ty::Slice(a), Ty::Slice(b)) if a.unformed() == b.unformed()) {
                    0
                } else if self.assignable(&t, &pt) {
                    2
                } else {
                    return None;
                }
            }
        };
        let arg_nullable = matches!(a, ArgShape::Ty(t) if matches!(t.unformed(), Ty::Nullable(_)));
        Some(if nullable && !arg_nullable { r.max(3) } else { r })
    }

    /// Wählt unter den Varianten aus und prüft die Argumente (R-7.17, R-4.44).
    fn invoke(&mut self, cands: Vec<MethodInfo>, args: &[ast::Arg], span: Span, recv: Receiver, name: &str) -> (Ty, CallFacts) {
        let recv_place = match recv {
            Receiver::Static => None,
            _ => self.pending_recv.take(),
        };
        let shapes: Vec<ArgShape> = args.iter().map(|a| self.arg_shape(a)).collect();
        let mut viable: Vec<(MethodInfo, Vec<u8>)> = Vec::new();
        for m in &cands {
            if m.params.len() != args.len() {
                continue;
            }
            let m = self.instantiate(m, &shapes);
            let Some(m) = m else { continue };
            let ranks: Option<Vec<u8>> = shapes.iter().zip(&m.params).map(|(a, p)| self.rank(a, &p.ty)).collect();
            if let Some(r) = ranks {
                viable.push((m, r));
            }
        }
        let chosen = match viable.len() {
            0 => {
                let same_arity: Vec<&MethodInfo> = cands.iter().filter(|m| m.params.len() == args.len()).collect();
                if same_arity.len() == 1 {
                    // genau eine Variante: den ersten unpassenden Parameter melden
                    let m = same_arity[0].clone();
                    if !m.tparams.is_empty() && self.instantiate(&m, &shapes).is_none() {
                        self.err(span, "R-4.44", format!("die Typargumente von {name} lassen sich nicht ableiten"));
                        return (Ty::Error, self.facts_of(&m));
                    }
                    let m = self.instantiate(&m, &shapes).unwrap_or(m);
                    let id = self.begin_call();
                    self.check_args(&m, args, id);
                    self.end_call(id);
                    return (m.ret.ty(), self.facts_of(&m));
                }
                if same_arity.is_empty() && cands.len() == 1 {
                    let msg = format!("{name} erwartet {} Argumente, nicht {}", cands[0].params.len(), args.len());
                    self.err(span, "R-7.17", msg);
                } else {
                    let rule = if args.iter().any(|a| matches!(a.expr.kind, E::Lambda { .. })) { "R-10.18" } else { "R-7.17" };
                    self.err(span, rule, format!("keine Variante von {name} passt zu den Argumenten"));
                }
                for a in args {
                    self.probe(|c| c.expr(&a.expr, None));
                }
                return (Ty::Error, CallFacts { nodiscard: false, func_value: false, never_returns: false });
            }
            1 => viable.pop().unwrap().0,
            _ => match self.best(&viable) {
                Some(i) => viable.swap_remove(i).0,
                None => {
                    self.err(span, "R-7.17", format!("der Aufruf von {name} ist mehrdeutig"));
                    return (Ty::Error, CallFacts { nodiscard: false, func_value: false, never_returns: false });
                }
            },
        };
        match recv {
            Receiver::This => {
                if self.ctx.is_static && !chosen.is_static {
                    self.err(span, "R-7.4", format!("{name} braucht this; statische Methoden haben kein this"));
                }
                if !chosen.is_static && self.frames.iter().any(|f| f.own_lambda) {
                    self.err(span, "R-10.10", "ein own-Lambda kann this nicht erfassen");
                }
            }
            Receiver::Object => {
                if chosen.is_static {
                    self.err(span, "R-7.27", format!("{name} ist statisch und wird mit :: aufgerufen"));
                }
            }
            _ => {}
        }
        if chosen.owner != usize::MAX {
            self.check_access(chosen.access, chosen.owner, span);
        }
        if let Receiver::Builtin(ref t) = recv {
            self.builtin_constraints(t, name, span);
        }
        let id = self.begin_call();
        // R-5.10: Der Empfänger ist für die Dauer des Aufrufs geliehen.
        let recv_place = recv_place.filter(|_| !chosen.is_static);
        if let Some(pl) = &recv_place {
            if chosen.mut_this {
                self.writable(pl, span, "R-5.10");
                self.access(pl, true, span);
            }
            self.push_loan(pl.clone(), chosen.mut_this, Until::Call(id), "R-5.10");
        }
        self.check_args(&chosen, args, id);
        self.end_call(id);
        // R-5.24: Eine Borrow-Rückgabe gilt als Loan auf den Empfänger.
        if matches!(chosen.ret, Ret::Ref(..)) {
            self.ref_calls.insert(span);
        }
        if let (Ret::Ref(_, m), Some(pl)) = (&chosen.ret, recv_place) {
            let sd = self.stmt_depth;
            self.push_loan(pl, *m || chosen.mut_this, Until::Stmt(sd), "R-5.24");
        }
        let facts = self.facts_of(&chosen);
        self.last_target = Some(match recv {
            Receiver::Builtin(_) => Target::BuiltinMethod(name.to_string()),
            _ => Target::Method(chosen.clone()),
        });
        let ret = chosen.ret.ty();
        self.last_chosen = Some(chosen);
        (ret, facts)
    }

    fn facts_of(&self, m: &MethodInfo) -> CallFacts {
        CallFacts { nodiscard: m.nodiscard, func_value: false, never_returns: false }
    }

    fn begin_call(&mut self) -> usize {
        self.call_seq += 1;
        self.call_moved.push(Vec::new());
        self.call_seq
    }

    fn end_call(&mut self, id: usize) {
        self.loans.retain(|l| l.until != Until::Call(id));
        self.call_moved.pop();
    }

    /// Argumente nach der Markierung der Parameter (R-5.8, R-5.9): `own` verschiebt,
    /// sonst wird der Ort für die Dauer des Aufrufs geliehen.
    fn check_args(&mut self, m: &MethodInfo, args: &[ast::Arg], call: usize) {
        for (a, p) in args.iter().zip(&m.params) {
            self.as_argument = true;
            let at = self.expr(&a.expr, Some(&p.ty));
            self.as_argument = false;
            if !matches!(a.expr.kind, E::Lambda { .. }) {
                self.require(&at, &p.ty, a.expr.span, Some(&a.expr));
            }
            let is_lambda = matches!(a.expr.kind, E::Lambda { .. });
            if p.mode == Mode::Mut && !a.borrow_mut && !is_lambda && !at.is_error() {
                self.err(a.expr.span, "R-5.9", format!("der Parameter {} ist veränderbar geliehen; das Argument braucht mut", p.name));
            } else if p.mode != Mode::Mut && a.borrow_mut {
                self.err(a.expr.span, "R-5.9", format!("der Parameter {} ist nicht veränderbar geliehen; mut ist hier falsch", p.name));
            }
            // V-9: Ein Parameter vom Typ shared T oder weak T speichert den Wert; ein own-Wert
            // wird dabei verschoben (bei weak wie share mit anschließendem demote).
            let stores = p.mode == Mode::Own || (p.mode == Mode::Read && (Self::is_shared(&p.ty) || weak_inner(&p.ty).is_some()));
            match p.mode {
                _ if stores => self.consume(&a.expr, &at, &p.ty, "R-5.3", &["V-9"]),
                Mode::Own => {}
                Mode::Mut | Mode::Read => {
                    let mutable = p.mode == Mode::Mut;
                    let Some(pl) = self.place_of(&a.expr) else { continue };
                    if mutable {
                        self.writable(&pl, a.expr.span, "R-5.9");
                        self.access(&pl, true, a.expr.span);
                    }
                    if mutable || !matches!(at.unformed().core(), Ty::Prim(_)) {
                        self.push_loan(pl, mutable, Until::Call(call), "R-5.9");
                    }
                }
            }
        }
    }

    /// Setzt die Typargumente einer generischen Methode aus den Argumenten ein (R-4.44, R-7.28).
    fn instantiate(&self, m: &MethodInfo, shapes: &[ArgShape]) -> Option<MethodInfo> {
        if m.tparams.is_empty() {
            return Some(m.clone());
        }
        let mut map: HashMap<ParamId, Ty> = HashMap::new();
        for (a, p) in shapes.iter().zip(&m.params) {
            let at = match a {
                ArgShape::Ty(t) => t.clone(),
                ArgShape::Int(v) => Ty::Prim(default_int(*v)),
                ArgShape::Float => Ty::Prim(P::F64),
                _ => continue,
            };
            unify(self.p, &p.ty, &at, &m.tparams, &mut map);
        }
        if m.tparams.iter().any(|t| !map.contains_key(t)) {
            return None;
        }
        let mut c = m.clone();
        c.targs = m.tparams.iter().map(|t| map[t].clone()).collect();
        for p in &mut c.params {
            p.ty = p.ty.subst(&map);
        }
        c.ret = match &c.ret {
            Ret::Void => Ret::Void,
            Ret::Val(t) => Ret::Val(t.subst(&map)),
            Ret::Ref(t, b) => Ret::Ref(t.subst(&map), *b),
        };
        Some(c)
    }

    fn best(&self, viable: &[(MethodInfo, Vec<u8>)]) -> Option<usize> {
        let better = |a: &(MethodInfo, Vec<u8>), b: &(MethodInfo, Vec<u8>)| -> bool {
            let mut strictly = false;
            for i in 0..a.1.len() {
                let (ra, rb) = (a.1[i], b.1[i]);
                let (pa, pb) = (&a.0.params[i].ty, &b.0.params[i].ty);
                let cmp = if ra != rb {
                    rb.cmp(&ra)
                } else if pa.unformed() != pb.unformed() {
                    // speziellerer Typ: lässt sich in den anderen umwandeln
                    match (self.assignable(pa, pb), self.assignable(pb, pa)) {
                        (true, false) => std::cmp::Ordering::Greater,
                        (false, true) => std::cmp::Ordering::Less,
                        _ => std::cmp::Ordering::Equal,
                    }
                } else {
                    std::cmp::Ordering::Equal
                };
                match cmp {
                    std::cmp::Ordering::Less => return false,
                    std::cmp::Ordering::Greater => strictly = true,
                    _ => {}
                }
            }
            strictly || (a.0.tparams.is_empty() && !b.0.tparams.is_empty())
        };
        (0..viable.len()).find(|&i| (0..viable.len()).all(|j| i == j || better(&viable[i], &viable[j])))
    }

    // ------------------------------------------------------------ new

    fn new_object(&mut self, t: &Ty, args: &[ast::Arg], span: Span) -> Ty {
        match t {
            Ty::Error => {
                for a in args {
                    self.probe(|c| c.expr(&a.expr, None));
                }
                Ty::Error
            }
            Ty::Param(p) => {
                if !self.p.tparams[*p].needs_new {
                    let msg = format!("new {}() verlangt den Constraint new()", self.p.tparams[*p].name);
                    self.err(span, "R-7.24", msg);
                }
                self.last_target = Some(Target::NewParam(*p));
                t.clone()
            }
            Ty::Named(d, targs) => {
                let def = self.p.defs[*d].clone();
                match def.kind {
                    DefKind::Class => {}
                    _ => {
                        self.err(span, "R-7.5", format!("{} ist keine Klasse", def.name));
                        return Ty::Error;
                    }
                }
                if def.is_abstract && !matches!(self.frames.last(), Some(f) if f.ret == Some(Ret::Void) && self.ctx.class == Some(*d)) {
                    self.err(span, "R-7.14", format!("{} ist abstrakt", def.name));
                    return Ty::Error;
                }
                let map = self.p.subst_map(*d, targs);
                if def.ctors.is_empty() {
                    if !args.is_empty() {
                        self.err(span, "R-7.5", format!("{} hat keinen Konstruktor mit Parametern", def.name));
                    } else if !self.p.has_default_ctor(*d) {
                        self.err(span, "R-7.5", format!("{} hat keinen parameterlosen Konstruktor", def.name));
                    }
                    self.last_target = Some(Target::Ctor(*d, None));
                    return t.clone();
                }
                let cands: Vec<MethodInfo> = def
                    .ctors
                    .iter()
                    .map(|c| MethodInfo {
                        name: def.name.clone(),
                        owner: *d,
                        params: c.params.iter().map(|p| PInfo { name: p.name.clone(), ty: p.ty.subst(&map), mode: p.mode }).collect(),
                        ret: Ret::Val(t.clone()),
                        tparams: Vec::new(),
                        targs: Vec::new(),
                        is_static: true,
                        modifier: None,
                        nodiscard: false,
                        mut_this: false,
                        access: c.access,
                        has_body: true,
                        span: c.span,
                        ast: c.ast,
                    })
                    .collect();
                if args.is_empty() && !cands.iter().any(|c| c.params.is_empty()) {
                    self.err(span, "R-7.5", format!("{} hat keinen parameterlosen Konstruktor", def.name));
                    return t.clone();
                }
                self.invoke(cands, args, span, Receiver::Static, &def.name);
                self.last_target = self.last_chosen.clone().map(|m| Target::Ctor(*d, Some(m)));
                t.clone()
            }
            _ => {
                let msg = format!("new {} ist nicht möglich", self.show(t));
                self.err(span, "R-7.5", msg);
                Ty::Error
            }
        }
    }

    // ------------------------------------------------------------ Lambdas

    fn lambda(&mut self, own: bool, params: &[ast::Param], body: &ast::LambdaBody, expected: Option<&Ty>, as_arg: bool, span: Span) -> Ty {
        let ps: Vec<PInfo> = params.iter().map(|p| self.param_info(p)).collect();
        let exp = expected.map(|t| t.unformed().core().clone());
        match exp {
            Some(Ty::Func(fps, r)) => {
                if !own && !as_arg {
                    self.err(span, "R-10.7", "ein normales Lambda steht nur als Argument oder wird direkt aufgerufen");
                    return Ty::Error;
                }
                let ok = fps.len() == ps.len() && fps.iter().zip(&ps).all(|(f, l)| f.mode == l.mode && (f.ty.unformed() == l.ty.unformed() || l.ty.is_error()));
                if !ok {
                    self.err(span, "R-10.14", "die Parameter des Lambdas passen nicht zum erwarteten Funktionstyp");
                    return Ty::Error;
                }
                let ret = if *r == Ty::Void { Ret::Void } else { Ret::Val((*r).clone()) };
                self.lambda_body(own, &ps, body, Some(ret), span);
                Ty::Func(fps, r)
            }
            Some(Ty::Error) => {
                self.lambda_body(own, &ps, body, None, span);
                Ty::Error
            }
            Some(Ty::Param(_)) | None => {
                if !own && !as_arg {
                    self.err(span, "R-10.4", "ein normales Lambda braucht einen Funktionstyp aus dem Zusammenhang");
                    return Ty::Error;
                }
                let r = self.lambda_body(own, &ps, body, None, span);
                Ty::Func(ps.iter().map(|p| FParam { ty: p.ty.clone(), mode: p.mode }).collect(), Box::new(r))
            }
            Some(other) => {
                let msg = format!("ein Lambda passt nicht zu {}", self.show(&other));
                self.err(span, "R-10.14", msg);
                Ty::Error
            }
        }
    }

    /// Prüft den Rumpf eines Lambdas; liefert den (abgeleiteten) Rückgabetyp.
    fn lambda_body(&mut self, own: bool, ps: &[PInfo], body: &ast::LambdaBody, ret: Option<Ret>, span: Span) -> Ty {
        // R-6.13: Das Lambda sieht die Fakten an seiner Stelle; sein Rumpf ändert den Stand außen nicht.
        let outer = self.flow.clone();
        let outer_loans = std::mem::take(&mut self.loans);
        self.frames.push(Frame::new(ret.clone(), own));
        self.push_scope();
        for p in ps {
            let id = ast::Ident { name: p.name.clone(), span: Span::default() };
            if self.find_local(&p.name).is_some() {
                self.err(id.span, "R-8.17", format!("{} verdeckt eine sichtbare Variable", p.name));
            }
            self.new_local(&p.name, p.ty.clone(), LocalKind::Param(p.mode));
        }
        let result = match body {
            ast::LambdaBody::Expr(x) => {
                let exp = match &ret {
                    Some(Ret::Val(t)) => Some(t.clone()),
                    _ => None,
                };
                let t = self.expr(x, exp.as_ref());
                match &ret {
                    Some(Ret::Val(r)) => {
                        let r = r.clone();
                        if !self.assignable(&t, &r) {
                            let msg = format!("das Lambda liefert {}, erwartet ist {}", self.show(&t), self.show(&r));
                            self.err(x.span, "R-10.14", msg);
                        }
                        r
                    }
                    Some(Ret::Void) => {
                        if t != Ty::Void && !t.is_error() {
                            let msg = format!("das Lambda liefert {}, erwartet ist void", self.show(&t));
                            self.err(x.span, "R-10.14", msg);
                        }
                        Ty::Void
                    }
                    _ => t,
                }
            }
            ast::LambdaBody::Block(b) => {
                let completes = self.block(b);
                match &ret {
                    Some(Ret::Val(r)) => {
                        if completes {
                            self.err(b.end, "R-8.26", "das Lambda endet ohne return");
                        }
                        r.clone()
                    }
                    Some(Ret::Void) => Ty::Void,
                    _ => {
                        let inferred = self.frame().inferred.clone();
                        let mut t = Ty::Void;
                        for x in inferred {
                            t = if t == Ty::Void { x } else { self.join(&t, &x, b.end).unwrap_or(Ty::Error) };
                        }
                        t
                    }
                }
            }
        };
        self.pop_scope();
        let frame = self.frames.pop().unwrap();
        self.last_captures = frame.captures.iter().filter_map(|id| self.local_by_id(*id).map(|l| l.name.clone())).collect();
        if frame.capture_access.iter().any(|(id, _)| *id == THIS) {
            self.last_captures.push("this".into());
        }
        self.loans = outer_loans;
        self.flow = outer;
        // R-10.7: Ein Lambda leiht die erfassten Variablen, solange es läuft; das darf keinem
        // bestehenden Loan widersprechen (etwa dem Empfänger des Aufrufs, dem es übergeben wird).
        for (id, mutable) in &frame.capture_access {
            let place = Place::var(*id);
            let mutable = *mutable || (own && *id != THIS && self.local_by_id(*id).is_some_and(|l| self.movable(&l.ty) && !l.kind.is_borrow()));
            if let Some(l) = self.loans.iter().rev().find(|l| l.conflicts(mutable, &place)).cloned() {
                let how = if l.mutable { "veränderbar" } else { "lesend" };
                self.err_also(span, "R-10.7", vec![l.rule], format!("das Lambda verwendet {}, das gerade {how} geliehen ist", l.what));
            }
        }
        // R-10.8: Ein own-Lambda verschiebt erfasste own-Werte in sich.
        if own {
            for id in frame.captures {
                let Some(l) = self.local_by_id(id).cloned() else { continue };
                if l.frame <= self.frames.len() && self.movable(&l.ty) && !l.kind.is_borrow() && !self.flow.dead {
                    self.flow.moved.insert(id, "R-10.8");
                }
            }
        }
        result
    }

    // ------------------------------------------------------------ Eingebautes

    fn builtin(&self, name: &str, params: Vec<(Ty, Mode)>, ret: Ty, nodiscard: bool) -> MethodInfo {
        let mut_this = matches!(name, "push" | "insert" | "pop" | "remove" | "clear" | "sort" | "sortBy");
        MethodInfo {
            name: name.into(),
            owner: usize::MAX,
            params: params.into_iter().enumerate().map(|(i, (ty, mode))| PInfo { name: format!("p{i}"), ty, mode }).collect(),
            ret: if ret == Ty::Void { Ret::Void } else { Ret::Val(ret) },
            tparams: Vec::new(),
            targs: Vec::new(),
            is_static: false,
            modifier: None,
            nodiscard,
            mut_this,
            access: Access::Public,
            has_body: true,
            span: Span::default(),
            ast: (0, 0, 0),
        }
    }

    /// Eingebaute Methoden der primitiven Typen, von string und der Arrays (R-13.11 bis R-13.13, R-13.16).
    fn builtin_methods(&self, t: &Ty, name: &str) -> Option<Vec<MethodInfo>> {
        let s = Ty::Prim(P::String);
        let b = Ty::Prim(P::Bool);
        let i64t = Ty::Prim(P::I64);
        let r = Mode::Read;
        let m = |n: &str, ps: Vec<(Ty, Mode)>, ret: Ty| vec![self.builtin(n, ps, ret, false)];
        Some(match (t, name) {
            (Ty::Prim(_), "toString") => m(name, vec![], s),
            (Ty::Prim(p), "abs") if is_numeric(*p) => m(name, vec![], t.clone()),
            (Ty::Prim(p), "min" | "max") if is_numeric(*p) => m(name, vec![(t.clone(), r)], t.clone()),
            (Ty::Prim(P::Char), "isLetter" | "isDigit" | "isWhitespace" | "isUpper" | "isLower") => m(name, vec![], b),
            (Ty::Prim(P::Char), "toUpper" | "toLower") => m(name, vec![], t.clone()),
            (Ty::Prim(P::String), "byteAt") => m(name, vec![(i64t, r)], Ty::Prim(P::Byte)),
            (Ty::Prim(P::String), "substring") => m(name, vec![(i64t.clone(), r), (i64t, r)], s),
            (Ty::Prim(P::String), "indexOf") => m(name, vec![(s, r)], i64t.nullable()),
            (Ty::Prim(P::String), "startsWith" | "endsWith" | "contains") => m(name, vec![(s, r)], b),
            (Ty::Prim(P::String), "split") => m(name, vec![(s.clone(), r)], Ty::Vector(Box::new(s))),
            (Ty::Prim(P::String), "trim" | "toUpper" | "toLower") => m(name, vec![], s),
            (Ty::Prim(P::String), "replace") => m(name, vec![(s.clone(), r), (s.clone(), r)], s),
            (Ty::Prim(P::String), "bytes") => m(name, vec![], Ty::Vector(Box::new(Ty::Prim(P::Byte)))),
            (Ty::Vector(e), _) => {
                let e = (**e).clone();
                match name {
                    "push" => m(name, vec![(e, Mode::Own)], Ty::Void),
                    "insert" => m(name, vec![(i64t, r), (e, Mode::Own)], Ty::Void),
                    "pop" => m(name, vec![], e),
                    "remove" => m(name, vec![(i64t, r)], e),
                    "clear" | "sort" => m(name, vec![], Ty::Void),
                    "first" | "last" => {
                        let mut f = self.builtin(name, vec![], e.clone().nullable(), false);
                        f.ret = Ret::Ref(e.nullable(), false);
                        vec![f]
                    }
                    "contains" => m(name, vec![(e, r)], b),
                    "indexOf" => m(name, vec![(e, r)], i64t.nullable()),
                    "sortBy" => {
                        let f = Ty::Func(vec![FParam { ty: e.clone(), mode: r }, FParam { ty: e, mode: r }], Box::new(Ty::Prim(P::I32)));
                        m(name, vec![(f, r)], Ty::Void)
                    }
                    _ => return None,
                }
            }
            _ => return None,
        })
    }

    /// Constraints eingebauter Methoden: `sort` verlangt Comparable, `contains`/`indexOf` Equatable.
    fn builtin_constraints(&mut self, t: &Ty, name: &str, span: Span) {
        let Ty::Vector(e) = t else { return };
        let (mixin, rule) = match name {
            "sort" => ("rpp.core.Comparable", "R-13.16"),
            "contains" | "indexOf" => ("rpp.core.Equatable", "R-13.16"),
            _ => return,
        };
        if let Some(md) = self.p.std_def(mixin) {
            let m = Ty::Named(md, vec![e.unformed().clone()]);
            if !self.p.satisfies(e.unformed(), &m) {
                let msg = format!("{name} verlangt, dass {} {} erfüllt", self.show(e), mixin.trim_start_matches("rpp.core."));
                self.err(span, rule, msg);
            }
        }
    }

    fn builtin_statics(&self, p: P, name: &str) -> Option<Vec<MethodInfo>> {
        let s = Ty::Prim(P::String);
        let r = Mode::Read;
        Some(match (p, name) {
            (p, "parse") if is_numeric(p) => vec![self.builtin(name, vec![(s, r)], Ty::Prim(p), true)],
            (p, "tryParse") if is_numeric(p) => vec![self.builtin(name, vec![(s, r)], Ty::Prim(p).nullable(), true)],
            (P::String, "join") => vec![self.builtin(name, vec![(s.clone(), r), (Ty::Slice(Box::new(s.clone())), r)], s, false)],
            (P::String, "fromUtf8") => vec![self.builtin(name, vec![(Ty::Slice(Box::new(Ty::Prim(P::Byte))), r)], s, false)],
            _ => return None,
        })
    }

    /// print, assert, fail, take, replace, same (R-13.21, R-6.10, R-9.19, V-3, R-4.24).
    fn builtin_function(&mut self, name: &str, args: &[ast::Arg], span: Span) -> Option<(Ty, CallFacts)> {
        let plain = CallFacts { nodiscard: false, func_value: false, never_returns: false };
        let expect = |c: &mut Self, n: usize| {
            if args.len() != n {
                c.err(span, "R-8.16", format!("{name} erwartet {n} Argumente"));
            }
        };
        let s = Ty::Prim(P::String);
        match name {
            "print" => {
                expect(self, 1);
                for a in args {
                    let t = self.expr(&a.expr, Some(&s));
                    self.require(&t, &s, a.expr.span, Some(&a.expr));
                }
                Some((Ty::Void, plain))
            }
            "assert" => {
                expect(self, 2);
                let facts = args.first().map(|a| self.cond(&a.expr));
                if let Some(a) = args.get(1) {
                    let t = self.expr(&a.expr, Some(&s));
                    self.require(&t, &s, a.expr.span, Some(&a.expr));
                }
                // R-6.10: Nach assert gilt W(c).
                if let Some((w, _)) = facts {
                    self.flow.narrow(&w);
                }
                Some((Ty::Void, plain))
            }
            "fail" => {
                expect(self, 1);
                for a in args {
                    let t = self.expr(&a.expr, Some(&s));
                    self.require(&t, &s, a.expr.span, Some(&a.expr));
                }
                Some((Ty::Void, CallFacts { never_returns: true, ..plain }))
            }
            "take" => {
                expect(self, 1);
                let t = args.first().map(|a| self.field_arg(a)).unwrap_or(Ty::Error);
                if !t.is_error() && !matches!(t.unformed(), Ty::Nullable(_)) {
                    self.err(span, "R-5.7", "take verlangt ein nullbares Feld");
                }
                Some((t, CallFacts { nodiscard: false, ..plain }))
            }
            "replace" => {
                expect(self, 2);
                // R-5.7: zuerst der neue Wert, dann der Ort
                let t = args.first().map(|a| self.probe(|c| c.expr(&a.expr, None))).unwrap_or(Ty::Error);
                if let Some(a) = args.get(1) {
                    let v = self.expr(&a.expr, Some(&t));
                    self.require(&v, &t, a.expr.span, Some(&a.expr));
                    self.consume(&a.expr, &v, &t, "R-5.3", &["R-5.7"]);
                }
                let t = args.first().map(|a| self.field_arg(a)).unwrap_or(Ty::Error);
                Some((t, plain))
            }
            "same" => {
                expect(self, 2);
                for a in args {
                    self.expr(&a.expr, None);
                }
                Some((Ty::Prim(P::Bool), plain))
            }
            _ => None,
        }
    }
}

impl Checker<'_> {
    /// Das Feld in `take(mut x.f)` und `replace(mut x.f, v)`: ein Ort, der verändert wird (R-5.7).
    fn field_arg(&mut self, a: &ast::Arg) -> Ty {
        let t = self.expr(&a.expr, None);
        if let Some(pl) = self.place_of(&a.expr) {
            self.writable(&pl, a.expr.span, "R-5.7");
            self.access(&pl, true, a.expr.span);
        }
        t
    }
}

/// Text eines einfachen Ausdrucks für Meldungen.
fn expr_text(e: &ast::Expr) -> String {
    match &e.kind {
        E::Name(n) => n.name.clone(),
        E::This => "this".into(),
        E::Paren(x) => expr_text(x),
        E::Member { object, name, safe } => format!("{}{}{}", expr_text(object), if *safe { "?." } else { "." }, name.name),
        E::Index { object, .. } => format!("{}[…]", expr_text(object)),
        E::Call { callee, .. } => format!("{}(…)", expr_text(callee)),
        _ => "der Ausdruck".into(),
    }
}

/// Enthält die Kette einen sicheren Zugriff `?.` (R-6.17)?
fn has_safe_access(e: &ast::Expr) -> bool {
    match &e.kind {
        E::Member { safe: true, .. } => true,
        E::Member { object, .. } => has_safe_access(object),
        E::Call { callee, .. } => has_safe_access(callee),
        E::Paren(x) => has_safe_access(x),
        _ => false,
    }
}

enum ArgShape {
    Ty(Ty),
    Int(i128),
    Float,
    Null,
    Lambda(Vec<PInfo>),
}

enum Receiver {
    This,
    Object,
    Static,
    Builtin(Ty),
}

fn op_str(op: B) -> &'static str {
    match op {
        B::Mul => "*",
        B::Div => "/",
        B::Rem => "%",
        B::Add => "+",
        B::Sub => "-",
        B::Shl => "<<",
        B::Shr => ">>",
        B::Lt => "<",
        B::Gt => ">",
        B::Le => "<=",
        B::Ge => ">=",
        B::Eq => "==",
        B::Ne => "!=",
        B::BitAnd => "&",
        B::BitXor => "^",
        B::BitOr => "|",
        B::And => "&&",
        B::Or => "||",
    }
}

/// Bindet Typ-Parameter einer generischen Methode an die Typen der Argumente.
fn unify(p: &Program, param: &Ty, arg: &Ty, tps: &[ParamId], map: &mut HashMap<ParamId, Ty>) {
    let arg = arg.unformed();
    match (param.unformed(), arg) {
        (Ty::Param(x), a) if tps.contains(x) => {
            if !a.is_error() {
                map.entry(*x).or_insert_with(|| a.clone());
            }
        }
        (Ty::Nullable(x), Ty::Nullable(y)) => unify(p, x, y, tps, map),
        (Ty::Nullable(x), y) => unify(p, x, y, tps, map),
        (Ty::Vector(x) | Ty::Slice(x) | Ty::Array(x, _), Ty::Vector(y) | Ty::Slice(y) | Ty::Array(y, _)) => unify(p, x, y, tps, map),
        (Ty::Tuple(xs), Ty::Tuple(ys)) if xs.len() == ys.len() => {
            for (x, y) in xs.iter().zip(ys) {
                unify(p, x, y, tps, map);
            }
        }
        (Ty::Func(xs, xr), Ty::Func(ys, yr)) if xs.len() == ys.len() => {
            for (x, y) in xs.iter().zip(ys) {
                unify(p, &x.ty, &y.ty, tps, map);
            }
            unify(p, xr, yr, tps, map);
        }
        (Ty::Named(d, xs), a @ Ty::Named(..)) => {
            let found = std::iter::once(a.clone()).chain(p.supertypes(a)).find(|s| matches!(s, Ty::Named(e, _) if e == d));
            if let Some(Ty::Named(_, ys)) = found {
                for (x, y) in xs.iter().zip(ys.iter()) {
                    unify(p, x, y, tps, map);
                }
            }
        }
        _ => {}
    }
}

/// Ziel eines `weak`-Verweises (auch hinter `?`).
fn weak_inner(t: &Ty) -> Option<Ty> {
    match t {
        Ty::Formed(Form::Weak, x) => Some((**x).clone()),
        Ty::Nullable(x) => weak_inner(x),
        _ => None,
    }
}

fn is_true(e: &ast::Expr) -> bool {
    matches!(e.kind, E::Literal(ast::Literal::Bool(true)))
}

/// Enthält die Anweisung ein `break`, das diese Schleife verlässt?
fn contains_break(s: &ast::Stmt) -> bool {
    contains_break_stmt(s)
}

fn contains_break_stmt(s: &ast::Stmt) -> bool {
    match &s.kind {
        S::Break => true,
        S::Block(b) => b.stmts.iter().any(contains_break_stmt),
        S::If { then, otherwise, .. } => contains_break_stmt(then) || otherwise.as_ref().is_some_and(|o| contains_break_stmt(o)),
        S::Try { body, catches, finally } => {
            body.stmts.iter().any(contains_break_stmt)
                || catches.iter().any(|c| c.body.stmts.iter().any(contains_break_stmt))
                || finally.as_ref().is_some_and(|f| f.stmts.iter().any(contains_break_stmt))
        }
        // break in inneren Schleifen und switch gehört diesen
        _ => false,
    }
}

/// Felder, die eine Anweisungsfolge auf jedem Weg setzt (für R-7.7). `*` steht für
/// „endet nicht normal“ (dann gelten alle als gesetzt).
fn assigned_fields(stmts: &[ast::Stmt]) -> HashSet<String> {
    let mut set = HashSet::new();
    for s in stmts {
        match &s.kind {
            S::Expr(ast::ExprStmt::Assign { target, .. }) => {
                let name = match &target.kind {
                    E::Member { object, name, .. } if matches!(object.kind, E::This) => Some(name.name.clone()),
                    E::Name(n) => Some(n.name.clone()),
                    _ => None,
                };
                if let Some(n) = name {
                    set.insert(n);
                }
            }
            S::Block(b) => set.extend(assigned_fields(&b.stmts)),
            S::If { then, otherwise: Some(o), .. } => {
                let a = assigned_fields(std::slice::from_ref(then));
                let b = assigned_fields(std::slice::from_ref(o));
                if a.contains("*") {
                    set.extend(b);
                } else if b.contains("*") {
                    set.extend(a);
                } else {
                    set.extend(a.intersection(&b).cloned());
                }
            }
            S::Return { .. } | S::Throw(_) => {
                set.insert("*".into());
                return set;
            }
            S::Expr(ast::ExprStmt::Expr(e)) => {
                if let E::Call { callee, .. } = &e.kind {
                    if matches!(&callee.kind, E::Name(n) if n.name == "fail") {
                        set.insert("*".into());
                        return set;
                    }
                }
            }
            _ => {}
        }
    }
    set
}
