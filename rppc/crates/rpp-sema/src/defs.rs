//! Deklarationen: Typen sammeln, Namen auflösen, Membertabellen bauen und die
//! Regeln auf Ebene von Dateien, Klassen, Mixins und Enums prüfen
//! (Kapitel 4 Abschnitt 7, Kapitel 7, Kapitel 11).

use crate::project::{ProjectKind, Workspace};
use crate::ty::*;
use crate::Diag;
use rpp_syntax::ast::{self, Access, ClassMember, MethodModifier, TopDecl, TypeArg, TypeSuffix};
use rpp_syntax::Span;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefKind {
    Class,
    Mixin,
    Enum,
}

#[derive(Debug, Clone)]
pub struct TParam {
    pub name: String,
    pub constraints: Vec<Ty>,
    pub needs_new: bool,
}

#[derive(Debug, Clone)]
pub struct PInfo {
    pub name: String,
    pub ty: Ty,
    pub mode: Mode,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Ret {
    Void,
    Val(Ty),
    /// zurückgegebener Borrow `ref T` bzw. `ref T&`
    Ref(Ty, bool),
}

impl Ret {
    pub fn ty(&self) -> Ty {
        match self {
            Ret::Void => Ty::Void,
            Ret::Val(t) | Ret::Ref(t, _) => t.clone(),
        }
    }
    fn subst(&self, m: &HashMap<ParamId, Ty>) -> Ret {
        match self {
            Ret::Void => Ret::Void,
            Ret::Val(t) => Ret::Val(t.subst(m)),
            Ret::Ref(t, b) => Ret::Ref(t.subst(m), *b),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub name: String,
    pub ty: Ty,
    pub is_static: bool,
    pub is_const: bool,
    pub access: Access,
    pub has_init: bool,
    pub span: Span,
}

/// Ort einer Methode, eines Konstruktors usw. im AST: (Datei, Deklaration, Member).
pub type AstRef = (usize, usize, usize);

#[derive(Debug, Clone)]
pub struct MethodInfo {
    pub name: String,
    pub owner: DefId,
    pub params: Vec<PInfo>,
    pub ret: Ret,
    pub tparams: Vec<ParamId>,
    pub is_static: bool,
    pub modifier: Option<MethodModifier>,
    pub nodiscard: bool,
    pub mut_this: bool,
    pub access: Access,
    pub has_body: bool,
    pub span: Span,
    pub ast: AstRef,
}

impl MethodInfo {
    pub fn overridable(&self) -> bool {
        matches!(self.modifier, Some(MethodModifier::Virtual | MethodModifier::Abstract | MethodModifier::Override))
            || (!self.has_body && !self.is_static)
    }
    fn subst(&self, m: &HashMap<ParamId, Ty>) -> MethodInfo {
        let mut c = self.clone();
        for p in &mut c.params {
            p.ty = p.ty.subst(m);
        }
        c.ret = c.ret.subst(m);
        c
    }
}

#[derive(Debug, Clone)]
pub struct CtorInfo {
    pub params: Vec<PInfo>,
    pub access: Access,
    pub span: Span,
    pub calls_base: bool,
    pub ast: AstRef,
}

#[derive(Debug, Clone)]
pub struct VariantInfo {
    pub name: String,
    pub fields: Vec<PInfo>,
    pub value: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct TypeDef {
    pub kind: DefKind,
    pub name: String,
    pub package: String,
    pub project: usize,
    pub file: usize,
    pub decl: usize,
    pub export: bool,
    pub is_abstract: bool,
    pub tparams: Vec<ParamId>,
    pub base: Option<Ty>,
    /// direkte Mixin-Importe mit Zeile
    pub imports: Vec<(Ty, Span)>,
    pub fields: Vec<FieldInfo>,
    pub methods: Vec<MethodInfo>,
    pub ctors: Vec<CtorInfo>,
    pub variants: Vec<VariantInfo>,
    pub copy: Option<Span>,
    pub has_dtor: bool,
    pub span: Span,
}

impl TypeDef {
    pub fn full_name(&self) -> String {
        if self.package.is_empty() {
            self.name.clone()
        } else {
            format!("{}.{}", self.package, self.name)
        }
    }
}

/// Eine noch zu prüfende Verwendung eines generischen Typs (R-7.25, R-7.26).
struct PendingArgs {
    file: usize,
    span: Span,
    def: DefId,
    args: Vec<Ty>,
}

pub struct Program {
    pub ws: Workspace,
    pub defs: Vec<TypeDef>,
    pub tparams: Vec<TParam>,
    pub diags: Vec<Diag>,
    /// Angaben für die Ausführung (Meilenstein M4)
    pub rt: crate::rt::RuntimeInfo,
    by_full: HashMap<String, DefId>,
    pending: Vec<PendingArgs>,
}

/// Was im Rumpf einer Methode als Typ-Parameter sichtbar ist.
pub type TScope = Vec<ParamId>;

impl Program {
    pub fn error(&mut self, file: usize, span: Span, rule: &'static str, message: impl Into<String>) {
        let path = self.ws.files[file].path.clone();
        self.diags.push(Diag::error(&path, span, rule, message));
    }

    /// Fehler mit weiteren begründenden Regeln.
    pub fn error_also(&mut self, file: usize, span: Span, rule: &'static str, also: Vec<&'static str>, message: impl Into<String>) {
        let path = self.ws.files[file].path.clone();
        let mut d = Diag::error(&path, span, rule, message);
        for a in also {
            if a != rule && !d.also.contains(&a) {
                d.also.push(a);
            }
        }
        self.diags.push(d);
    }

    /// Warnung (R-8.1).
    pub fn warning(&mut self, file: usize, span: Span, rule: &'static str, message: impl Into<String>) {
        let path = self.ws.files[file].path.clone();
        let mut d = Diag::error(&path, span, rule, message);
        d.warning = true;
        self.diags.push(d);
    }

    pub fn is_std_file(&self, file: usize) -> bool {
        self.ws.projects[self.ws.files[file].project].is_std
    }

    pub fn def_by_full(&self, full: &str) -> Option<DefId> {
        self.by_full.get(full).copied()
    }

    /// Typ aus `rpp.core` (oder einem anderen Paket der Standardbibliothek).
    pub fn std_def(&self, full: &str) -> Option<DefId> {
        self.def_by_full(full)
    }

    pub fn show(&self, t: &Ty) -> String {
        match t {
            Ty::Error => "?".into(),
            Ty::Void => "void".into(),
            Ty::Null => "null".into(),
            Ty::Prim(p) => prim_name(*p).into(),
            Ty::Named(d, args) => {
                let mut s = self.defs[*d].name.clone();
                if !args.is_empty() {
                    s.push('<');
                    s.push_str(&args.iter().map(|a| self.show(a)).collect::<Vec<_>>().join(", "));
                    s.push('>');
                }
                s
            }
            Ty::Param(p) => self.tparams[*p].name.clone(),
            Ty::Vector(t) => format!("{}[]", self.show(t)),
            Ty::Array(t, n) => format!("{}[{}]", self.show(t), n.map(|n| n.to_string()).unwrap_or("N".into())),
            Ty::Slice(t) => format!("Slice<{}>", self.show(t)),
            Ty::Tuple(ts) => format!("({})", ts.iter().map(|t| self.show(t)).collect::<Vec<_>>().join(", ")),
            Ty::Func(ps, r) => {
                let mut parts: Vec<String> = ps
                    .iter()
                    .map(|p| match p.mode {
                        Mode::Read => self.show(&p.ty),
                        Mode::Mut => format!("{}&", self.show(&p.ty)),
                        Mode::Own => format!("own {}", self.show(&p.ty)),
                    })
                    .collect();
                parts.push(self.show(r));
                format!("Func<{}>", parts.join(", "))
            }
            Ty::Nullable(t) => format!("{}?", self.show(t)),
            Ty::Formed(Form::Shared, t) => format!("shared {}", self.show(t)),
            Ty::Formed(Form::Weak, t) => format!("weak {}", self.show(t)),
            Ty::Formed(Form::Plain, t) => self.show(t),
        }
    }

    // ------------------------------------------------------------ Aufbau

    pub fn build(ws: Workspace) -> Program {
        let mut p = Program { ws, defs: Vec::new(), tparams: Vec::new(), diags: Vec::new(), rt: Default::default(), by_full: HashMap::new(), pending: Vec::new() };
        p.diags.append(&mut p.ws.diags);
        p.collect();
        p.resolve_headers();
        p.resolve_members();
        p.check_pending();
        p.check_files();
        p.check_defs();
        p
    }

    fn new_tparam(&mut self, name: &str) -> ParamId {
        self.tparams.push(TParam { name: name.to_string(), constraints: Vec::new(), needs_new: false });
        self.tparams.len() - 1
    }

    /// Phase 1: alle Typdeklarationen mit Namen erfassen.
    fn collect(&mut self) {
        for f in 0..self.ws.files.len() {
            let package = self.ws.files[f].package();
            let project = self.ws.files[f].project;
            let decls = self.ws.files[f].unit.decls.clone();
            for (d, decl) in decls.iter().enumerate() {
                let (kind, name, export, is_abstract, tps, span) = match decl {
                    TopDecl::Class(c) => (DefKind::Class, &c.name, c.export, c.is_abstract, &c.type_params[..], c.span),
                    TopDecl::Mixin(m) => (DefKind::Mixin, &m.name, m.export, false, &m.type_params[..], m.span),
                    TopDecl::Enum(e) => (DefKind::Enum, &e.name, e.export, false, &[][..], e.span),
                    TopDecl::Main(_) => continue,
                };
                let tparams = tps.iter().map(|t| self.new_tparam(&t.name.name)).collect();
                let def = TypeDef {
                    kind,
                    name: name.name.clone(),
                    package: package.clone(),
                    project,
                    file: f,
                    decl: d,
                    export,
                    is_abstract,
                    tparams,
                    base: None,
                    imports: Vec::new(),
                    fields: Vec::new(),
                    methods: Vec::new(),
                    ctors: Vec::new(),
                    variants: Vec::new(),
                    copy: None,
                    has_dtor: false,
                    span,
                };
                let full = def.full_name();
                if let Some(&other) = self.by_full.get(&full) {
                    if self.defs[other].project == project {
                        self.error(f, name.span, "R-11.4", format!("Typ {full} ist schon deklariert"));
                        continue;
                    }
                }
                self.by_full.insert(full, self.defs.len());
                self.defs.push(def);
            }
        }
    }

    // ------------------------------------------------------------ Namen

    /// Ist `def` aus Projekt `from` verwendbar (R-11.9, R-11.10, R-11.12)?
    /// Liefert die verletzte Regel.
    fn visibility(&self, def: DefId, from: usize) -> Result<(), &'static str> {
        let d = &self.defs[def];
        if d.project == from {
            return Ok(());
        }
        let proj = &self.ws.projects[d.project];
        if proj.is_std || self.ws.projects[from].deps.contains(&d.project) {
            return if d.export { Ok(()) } else { Err("R-11.10") };
        }
        Err("R-11.12")
    }

    /// Sucht einen einfachen Typnamen in einer Datei (R-11.8): eigenes Paket, using, rpp.core.
    fn lookup_simple(&mut self, file: usize, name: &str, span: Span) -> Option<DefId> {
        let project = self.ws.files[file].project;
        let package = self.ws.files[file].package();
        if let Some(d) = self.defs.iter().position(|d| d.project == project && d.package == package && d.name == name) {
            return Some(d);
        }
        let usings = self.ws.files[file].unit.usings.clone();
        let mut found: Vec<String> = Vec::new();
        for u in &usings {
            let simple = u.alias.as_ref().map(|a| a.name.as_str()).unwrap_or_else(|| &u.target.parts.last().unwrap().name);
            if simple == name {
                let full = u.target.to_dotted();
                if !found.contains(&full) {
                    found.push(full);
                }
            }
        }
        if found.len() > 1 {
            self.error(file, span, "R-11.8", format!("{name} ist mehrdeutig: {}", found.join(", ")));
            return None;
        }
        if let Some(full) = found.first() {
            return self.by_full.get(full).copied();
        }
        let core = format!("rpp.core.{name}");
        self.by_full.get(&core).copied()
    }

    /// Löst einen Typ aus dem AST auf. Fehler werden gemeldet; das Ergebnis ist dann `Ty::Error`.
    pub fn resolve_type(&mut self, file: usize, scope: &[ParamId], t: &ast::Type) -> Ty {
        let mut ty = match &t.base {
            ast::BaseType::Primitive(p) => Ty::Prim(*p),
            ast::BaseType::Tuple(ts) => Ty::Tuple(ts.iter().map(|x| self.resolve_type(file, scope, x)).collect()),
            ast::BaseType::Named(tn) => self.resolve_named(file, scope, tn, t.span),
        };
        match t.ownership {
            Some(ast::Ownership::Shared) => ty = Ty::Formed(Form::Shared, Box::new(ty)),
            Some(ast::Ownership::Weak) => ty = Ty::Formed(Form::Weak, Box::new(ty)),
            _ => {}
        }
        for s in &t.suffixes {
            ty = match s {
                TypeSuffix::Nullable => ty.nullable(),
                TypeSuffix::Vector => Ty::Vector(Box::new(ty)),
                TypeSuffix::Array(size) => {
                    let n = match size {
                        ast::ArraySize::Int(v) => int_value(v).map(|v| v as u64),
                        _ => None,
                    };
                    Ty::Array(Box::new(ty), n)
                }
            };
        }
        ty
    }

    fn resolve_type_arg(&mut self, file: usize, scope: &[ParamId], a: &TypeArg, span: Span) -> Ty {
        match a {
            TypeArg::Void => {
                self.error(file, span, "R-4.45", "void ist hier kein Typargument");
                Ty::Error
            }
            TypeArg::Type { ty, mutable } => {
                if *mutable {
                    self.error(file, ty.span, "R-10.13", "& ist nur an Parametern von Func erlaubt");
                }
                self.resolve_type(file, scope, ty)
            }
        }
    }

    fn resolve_named(&mut self, file: usize, scope: &[ParamId], tn: &ast::TypeName, span: Span) -> Ty {
        let parts = &tn.name.parts;
        if parts.len() == 1 {
            let name = parts[0].name.as_str();
            if let Some(&p) = scope.iter().rev().find(|&&p| self.tparams[p].name == name) {
                return Ty::Param(p);
            }
            if name == "Func" && !tn.args.is_empty() && self.lookup_simple_silent(file, name).is_none() {
                return self.resolve_func(file, scope, &tn.args, span);
            }
            if name == "Slice" && tn.args.len() == 1 && self.lookup_simple_silent(file, name).is_none() {
                let el = self.resolve_type_arg(file, scope, &tn.args[0], span);
                return Ty::Slice(Box::new(el));
            }
        }
        let def = if parts.len() == 1 {
            self.lookup_simple(file, &parts[0].name, parts[0].span)
        } else {
            let full = tn.name.to_dotted();
            let d = self.by_full.get(&full).copied();
            if let Some(d) = d {
                let from = self.ws.files[file].project;
                if let Err(rule) = self.visibility(d, from) {
                    self.error(file, tn.name.span(), rule, format!("{full} ist hier nicht verwendbar"));
                    return Ty::Error;
                }
            }
            d
        };
        let Some(def) = def else {
            if !self.diags.iter().any(|d| d.span == parts[0].span) {
                self.error(file, tn.name.span(), "R-11.8", format!("unbekannter Typ {}", tn.name.to_dotted()));
            }
            return Ty::Error;
        };
        let args: Vec<Ty> = tn.args.iter().map(|a| self.resolve_type_arg(file, scope, a, span)).collect();
        let want = self.defs[def].tparams.len();
        if args.len() != want {
            self.error(file, tn.name.span(), "R-7.23", format!(
                "{} erwartet {want} Typargumente, nicht {}", self.defs[def].name, args.len()));
            return Ty::Error;
        }
        if !args.is_empty() {
            self.pending.push(PendingArgs { file, span: tn.name.span(), def, args: args.clone() });
        }
        Ty::Named(def, args)
    }

    fn lookup_simple_silent(&self, file: usize, name: &str) -> Option<DefId> {
        let project = self.ws.files[file].project;
        let package = self.ws.files[file].package();
        self.defs.iter().position(|d| d.project == project && d.package == package && d.name == name)
    }

    /// `Func<P₁, …, Pₙ, R>` (R-4.37, R-10.13)
    fn resolve_func(&mut self, file: usize, scope: &[ParamId], args: &[TypeArg], span: Span) -> Ty {
        let n = args.len();
        let mut params = Vec::new();
        for (i, a) in args.iter().enumerate() {
            let last = i + 1 == n;
            match a {
                TypeArg::Void if !last => {
                    self.error(file, span, "R-4.37", "void steht in Func nur an letzter Stelle");
                    return Ty::Error;
                }
                TypeArg::Void => return Ty::Func(params, Box::new(Ty::Void)),
                TypeArg::Type { ty, mutable } => {
                    let t = self.resolve_type(file, scope, ty);
                    if last {
                        if *mutable {
                            self.error(file, ty.span, "R-10.13", "& ist am Rückgabetyp von Func nicht erlaubt");
                            return Ty::Error;
                        }
                        return Ty::Func(params, Box::new(t));
                    }
                    let mode = if *mutable {
                        Mode::Mut
                    } else if ty.ownership == Some(ast::Ownership::Own) {
                        Mode::Own
                    } else {
                        Mode::Read
                    };
                    params.push(FParam { ty: t, mode });
                }
            }
        }
        Ty::Error
    }

    // ------------------------------------------------------------ Phase 2

    fn decl(&self, def: DefId) -> TopDecl {
        let d = &self.defs[def];
        self.ws.files[d.file].unit.decls[d.decl].clone()
    }

    /// Basisklasse, Importe und Constraints.
    fn resolve_headers(&mut self) {
        for def in 0..self.defs.len() {
            let file = self.defs[def].file;
            let scope = self.defs[def].tparams.clone();
            let (tps, base, members): (Vec<ast::TypeParam>, Option<ast::TypeName>, Vec<ClassMember>) = match self.decl(def) {
                TopDecl::Class(c) => (c.type_params, c.base, c.members),
                TopDecl::Mixin(m) => (m.type_params, None, m.members),
                _ => continue,
            };
            for (i, tp) in tps.iter().enumerate() {
                let id = scope[i];
                for c in &tp.constraints {
                    match c {
                        ast::Constraint::New => self.tparams[id].needs_new = true,
                        ast::Constraint::Type(tn) => {
                            let t = self.resolve_named(file, &scope, tn, tp.name.span);
                            self.tparams[id].constraints.push(t);
                        }
                    }
                }
            }
            if let Some(b) = base {
                let t = self.resolve_named(file, &scope, &b, b.name.span());
                if let Ty::Named(bd, _) = &t {
                    if self.defs[*bd].kind != DefKind::Class {
                        self.error(file, b.name.span(), "R-7.12", format!("{} ist keine Klasse", self.defs[*bd].name));
                    } else {
                        self.defs[def].base = Some(t);
                    }
                }
            }
            for m in &members {
                if let ClassMember::Import(tn) = m {
                    let t = self.resolve_named(file, &scope, tn, tn.name.span());
                    if let Ty::Named(md, _) = &t {
                        if self.defs[*md].kind != DefKind::Mixin {
                            self.error(file, tn.name.span(), "R-7.20", format!("{} ist kein Mixin", self.defs[*md].name));
                            continue;
                        }
                        self.defs[def].imports.push((t, tn.name.span()));
                    }
                }
            }
        }
        // Vererbungszyklen (R-7.12): Basis auflösen, bis ein Typ doppelt vorkommt.
        for def in 0..self.defs.len() {
            let mut seen = vec![def];
            let mut cur = def;
            while let Some(Ty::Named(b, _)) = self.defs[cur].base.clone() {
                if seen.contains(&b) {
                    let cycle: Vec<DefId> = seen[seen.iter().position(|&x| x == b).unwrap()..].to_vec();
                    let last = *cycle.iter().max_by_key(|&&d| (self.defs[d].file, self.defs[d].span)).unwrap();
                    if last == def {
                        let (f, s) = (self.defs[def].file, self.defs[def].span);
                        self.error(f, s, "R-7.12", format!("Vererbungszyklus über {}", self.defs[def].name));
                    }
                    break;
                }
                seen.push(b);
                cur = b;
            }
        }
        for def in 0..self.defs.len() {
            // Zyklen auflösen, damit spätere Durchläufe enden
            let mut seen = HashSet::new();
            let mut cur = def;
            while let Some(Ty::Named(b, _)) = self.defs[cur].base.clone() {
                if !seen.insert(cur) {
                    self.defs[cur].base = None;
                    break;
                }
                cur = b;
            }
        }
    }

    fn params(&mut self, file: usize, scope: &[ParamId], ps: &[ast::Param]) -> Vec<PInfo> {
        ps.iter()
            .map(|p| {
                let ty = self.resolve_type(file, scope, &p.ty);
                let mode = if p.mutable {
                    Mode::Mut
                } else if p.ty.ownership == Some(ast::Ownership::Own) {
                    Mode::Own
                } else {
                    Mode::Read
                };
                PInfo { name: p.name.name.clone(), ty, mode }
            })
            .collect()
    }

    fn resolve_ret(&mut self, file: usize, scope: &[ParamId], r: &ast::ReturnType) -> Ret {
        match r {
            ast::ReturnType::Void => Ret::Void,
            ast::ReturnType::Type(t) => Ret::Val(self.resolve_type(file, scope, t)),
            ast::ReturnType::Ref { ty, mutable } => Ret::Ref(self.resolve_type(file, scope, ty), *mutable),
        }
    }

    /// Felder, Methoden, Konstruktoren und Varianten.
    fn resolve_members(&mut self) {
        for def in 0..self.defs.len() {
            let file = self.defs[def].file;
            let decl_idx = self.defs[def].decl;
            let class_scope = self.defs[def].tparams.clone();
            match self.decl(def) {
                TopDecl::Enum(e) => {
                    let mut next: i64 = 0;
                    let mut seen: HashMap<i64, String> = HashMap::new();
                    let has_data = e.variants.iter().any(|v| matches!(v.payload, ast::VariantPayload::Data(_)));
                    let has_value = e.variants.iter().any(|v| matches!(v.payload, ast::VariantPayload::Value(_)));
                    let mut names = HashSet::new();
                    for v in &e.variants {
                        if !names.insert(v.name.name.clone()) {
                            self.error(file, v.name.span, "R-4.25", format!("Variante {} ist doppelt", v.name.name));
                        }
                        let (fields, value) = match &v.payload {
                            ast::VariantPayload::Data(ps) => {
                                if has_value {
                                    self.error(file, v.name.span, "R-4.25", "ein Enum mit Zahlwerten hat keine Varianten mit Daten");
                                }
                                (self.params(file, &[], ps), None)
                            }
                            ast::VariantPayload::Value(text) => {
                                let val = int_value(text.trim_start_matches('-')).map(|x| if text.starts_with('-') { -x } else { x });
                                match val {
                                    Some(x) if x >= i32::MIN as i128 && x <= i32::MAX as i128 => next = x as i64,
                                    _ => self.error(file, v.name.span, "R-4.26", "Zahlwert außerhalb von i32"),
                                }
                                (Vec::new(), Some(next))
                            }
                            ast::VariantPayload::None => (Vec::new(), if has_data { None } else { Some(next) }),
                        };
                        if let Some(x) = value {
                            if let Some(other) = seen.insert(x, v.name.name.clone()) {
                                self.error(file, v.name.span, "R-4.26", format!(
                                    "{} hat denselben Wert {x} wie {other}", v.name.name));
                            }
                            next = x + 1;
                        }
                        self.defs[def].variants.push(VariantInfo { name: v.name.name.clone(), fields, value });
                    }
                }
                TopDecl::Class(ast::ClassDecl { members, .. }) | TopDecl::Mixin(ast::MixinDecl { members, .. }) => {
                    let is_mixin = self.defs[def].kind == DefKind::Mixin;
                    let mut access = if is_mixin { Access::Public } else { Access::Private };
                    for (mi, m) in members.iter().enumerate() {
                        let ast_ref = (file, decl_idx, mi);
                        match m {
                            ClassMember::Access(a, _) => access = *a,
                            ClassMember::Field(f) => {
                                let ty = self.resolve_type(file, &class_scope, &f.ty);
                                if f.is_static && ty.mentions_param() {
                                    self.error(file, f.name.span, "R-7.27", "eine statische Konstante darf nicht von einem Typ-Parameter abhängen");
                                }
                                self.defs[def].fields.push(FieldInfo {
                                    name: f.name.name.clone(),
                                    ty,
                                    is_static: f.is_static,
                                    is_const: f.is_const,
                                    access,
                                    has_init: f.init.is_some(),
                                    span: f.name.span,
                                });
                            }
                            ClassMember::Method(md) => {
                                let mut scope = class_scope.clone();
                                let tps: Vec<ParamId> = md.type_params.iter().map(|t| self.new_tparam(&t.name.name)).collect();
                                scope.extend(&tps);
                                for (i, tp) in md.type_params.iter().enumerate() {
                                    for c in &tp.constraints {
                                        match c {
                                            ast::Constraint::New => self.tparams[tps[i]].needs_new = true,
                                            ast::Constraint::Type(tn) => {
                                                let t = self.resolve_named(file, &scope, tn, tp.name.span);
                                                self.tparams[tps[i]].constraints.push(t);
                                            }
                                        }
                                    }
                                }
                                let params = self.params(file, &scope, &md.params);
                                let ret = self.resolve_ret(file, &scope, &md.ret);
                                self.defs[def].methods.push(MethodInfo {
                                    name: md.name.name.clone(),
                                    owner: def,
                                    params,
                                    ret,
                                    tparams: tps,
                                    is_static: md.modifier == Some(MethodModifier::Static),
                                    modifier: md.modifier,
                                    nodiscard: md.nodiscard,
                                    mut_this: md.mut_this,
                                    access: if is_mixin { Access::Public } else { access },
                                    has_body: md.body.is_some(),
                                    span: md.name.span,
                                    ast: ast_ref,
                                });
                            }
                            ClassMember::Constructor(c) => {
                                let params = self.params(file, &class_scope, &c.params);
                                self.defs[def].ctors.push(CtorInfo {
                                    params,
                                    access,
                                    span: c.span,
                                    calls_base: c.base_args.is_some(),
                                    ast: ast_ref,
                                });
                            }
                            ClassMember::Destructor(..) => self.defs[def].has_dtor = true,
                            ClassMember::Copy(_, _, span) => self.defs[def].copy = Some(*span),
                            ClassMember::Import(_) => {}
                        }
                    }
                }
                TopDecl::Main(_) => {}
            }
        }
    }

    // ------------------------------------------------------------ Constraints

    /// Erfüllt `t` das Mixin `m` (R-4.42, R-7.26)?
    pub fn satisfies(&self, t: &Ty, m: &Ty) -> bool {
        let Ty::Named(md, margs) = m else { return true };
        let mname = self.defs[*md].full_name();
        if t.is_error() {
            return true;
        }
        match t.unformed() {
            Ty::Prim(p) => {
                let self_arg = margs.first().is_none_or(|a| a.is_error() || a == &Ty::Prim(*p));
                match mname.as_str() {
                    "rpp.core.Equatable" | "rpp.core.Hashable" => self_arg,
                    "rpp.core.Comparable" => self_arg && *p != rpp_syntax::ast::Primitive::Bool,
                    _ => false,
                }
            }
            Ty::Named(d, _) if self.defs[*d].kind == DefKind::Enum => {
                let plain = self.defs[*d].variants.iter().all(|v| v.fields.is_empty());
                let self_arg = margs.first().is_none_or(|a| a.is_error() || a == t.unformed());
                plain && self_arg && matches!(mname.as_str(), "rpp.core.Equatable" | "rpp.core.Hashable")
            }
            Ty::Param(p) => self.tparams[*p].constraints.iter().any(|c| self.mixin_includes(c, m)),
            Ty::Named(..) => self.upcasts(t, m),
            _ => false,
        }
    }

    /// Bringt das Mixin `a` (mit seinen Importen) das Mixin `b` mit?
    fn mixin_includes(&self, a: &Ty, b: &Ty) -> bool {
        if a == b {
            return true;
        }
        let Ty::Named(ad, aargs) = a else { return false };
        let map = self.subst_map(*ad, aargs);
        self.defs[*ad].imports.iter().any(|(i, _)| self.mixin_includes(&i.subst(&map), b))
    }

    pub fn subst_map(&self, def: DefId, args: &[Ty]) -> HashMap<ParamId, Ty> {
        self.defs[def].tparams.iter().copied().zip(args.iter().cloned()).collect()
    }

    /// Alle Obertypen (Basisklassen und Mixins) eines Klassentyps, mit eingesetzten Typargumenten.
    pub fn supertypes(&self, t: &Ty) -> Vec<Ty> {
        let mut out = Vec::new();
        let mut stack = vec![t.clone()];
        while let Some(cur) = stack.pop() {
            let Ty::Named(d, args) = &cur else { continue };
            let map = self.subst_map(*d, args);
            if let Some(b) = &self.defs[*d].base {
                let bt = b.subst(&map);
                if !out.contains(&bt) {
                    out.push(bt.clone());
                    stack.push(bt);
                }
            }
            for (i, _) in &self.defs[*d].imports {
                let it = i.subst(&map);
                if !out.contains(&it) {
                    out.push(it.clone());
                    stack.push(it);
                }
            }
        }
        out
    }

    /// `C <: D` für Klassen- und Mixin-Typen (R-4.22, R-4.43).
    pub fn upcasts(&self, from: &Ty, to: &Ty) -> bool {
        from == to || self.supertypes(from).contains(to)
    }

    pub fn has_default_ctor(&self, def: DefId) -> bool {
        let d = &self.defs[def];
        if d.ctors.is_empty() {
            d.fields.iter().filter(|f| !f.is_static).all(|f| f.has_init || matches!(f.ty, Ty::Nullable(_)))
        } else {
            d.ctors.iter().any(|c| c.params.is_empty())
        }
    }

    /// Verwirft noch nicht geprüfte Verwendungen (nach einer Probeprüfung).
    pub fn drop_pending(&mut self) {
        self.pending.clear();
    }

    /// Prüft die Constraints aller seit dem letzten Aufruf aufgelösten generischen Typen.
    pub fn check_pending(&mut self) {
        let pending = std::mem::take(&mut self.pending);
        for p in pending {
            let tps = self.defs[p.def].tparams.clone();
            let map = self.subst_map(p.def, &p.args);
            for (i, tp) in tps.iter().enumerate() {
                let arg = &p.args[i];
                if arg.is_error() || matches!(arg, Ty::Param(_)) && self.tparams[*tp].constraints.is_empty() {
                    continue;
                }
                if self.tparams[*tp].needs_new && !self.satisfies_new(arg) {
                    self.error(p.file, p.span, "R-7.25", format!(
                        "{} hat keinen sichtbaren parameterlosen Konstruktor (new())", self.show(arg)));
                }
                for c in self.tparams[*tp].constraints.clone() {
                    let c = c.subst(&map);
                    if !self.satisfies(arg, &c) {
                        self.error(p.file, p.span, "R-7.26", format!("{} erfüllt {} nicht", self.show(arg), self.show(&c)));
                    }
                }
            }
        }
    }

    pub fn satisfies_new(&self, t: &Ty) -> bool {
        match t {
            Ty::Named(d, _) => {
                let def = &self.defs[*d];
                def.kind == DefKind::Class && !def.is_abstract && self.has_default_ctor(*d)
            }
            Ty::Param(p) => self.tparams[*p].needs_new,
            Ty::Error => true,
            _ => false,
        }
    }

    // ------------------------------------------------------------ Dateien

    fn check_files(&mut self) {
        let mut mains: HashMap<usize, Vec<(usize, Span)>> = HashMap::new();
        for f in 0..self.ws.files.len() {
            let project = self.ws.files[f].project;
            let proj_single = self.ws.projects[project].single_file;
            let is_std = self.ws.projects[project].is_std;
            let lib = self.ws.projects[project].kind == ProjectKind::Library;
            let pname = self.ws.projects[project].name.clone();
            let unit = self.ws.files[f].unit.clone();
            let package = self.ws.files[f].package();
            let pspan = unit.package.as_ref().map_or(Span { line: 1, col: 0 }, |q| q.span());
            if let Some(expected) = self.ws.files[f].expected_package.clone() {
                if expected != package {
                    let shown = if expected.is_empty() { "keine package-Zeile (Wurzelpaket)".to_string() } else { format!("package {expected};") };
                    self.error(f, pspan, "R-11.2", format!("die Datei verlangt {shown}"));
                } else if lib && !is_std && !(package == pname || package.starts_with(&format!("{pname}."))) {
                    self.error(f, pspan, "R-11.18", format!("Pakete der Bibliothek {pname} beginnen mit {pname}"));
                }
            } else if lib && !proj_single && !is_std && package != pname && !package.starts_with(&format!("{pname}.")) {
                self.error(f, pspan, "R-11.18", format!("Pakete der Bibliothek {pname} beginnen mit {pname}"));
            }
            // using (R-11.7, R-11.8, R-11.12)
            for u in &unit.usings {
                let full = u.target.to_dotted();
                match self.by_full.get(&full).copied() {
                    None => {
                        self.error(f, u.span, "R-11.7", format!("unbekannter Typ {full}"));
                    }
                    Some(d) => {
                        if let Err(rule) = self.visibility(d, project) {
                            self.error(f, u.span, rule, format!("{full} ist hier nicht verwendbar"));
                        }
                        if let Some(alias) = &u.alias {
                            let own = self.defs.iter().any(|x| x.project == project && x.package == package && x.name == alias.name);
                            if own {
                                self.error(f, u.span, "R-11.8", format!(
                                    "der Name {} verdeckt einen Typ des eigenen Pakets", alias.name));
                            }
                        }
                    }
                }
            }
            for d in &unit.decls {
                if let TopDecl::Main(m) = d {
                    mains.entry(project).or_default().push((f, m.span));
                }
            }
        }
        // main (R-11.14)
        for (pi, proj) in self.ws.projects.iter().enumerate() {
            let list = mains.get(&pi).cloned().unwrap_or_default();
            let _ = proj;
            let kind = self.ws.projects[pi].kind;
            let main_file = self.ws.projects[pi].main_file.clone();
            if kind == ProjectKind::Library {
                for (f, s) in &list {
                    self.diags.push(Diag::error(&self.ws.files[*f].path.clone(), *s, "R-11.14", "eine Bibliothek hat keine main"));
                }
                continue;
            }
            for (k, (f, s)) in list.iter().enumerate() {
                let path = self.ws.files[*f].path.clone();
                if k > 0 {
                    self.diags.push(Diag::error(&path, *s, "R-11.14", "ein Programm hat genau eine main"));
                } else if let Some(mf) = &main_file {
                    if !self.ws.projects[pi].single_file && *mf != path {
                        self.diags.push(Diag::error(&path, *s, "R-11.14", "main steht nicht in der Datei, die rpp.yaml nennt"));
                    }
                }
            }
            if list.is_empty() && !self.ws.projects[pi].single_file {
                if let Some(y) = self.ws.projects[pi].yaml.clone() {
                    self.diags.push(Diag::error(&y, Span { line: 1, col: 0 }, "R-11.14", "das Programm hat keine main"));
                }
            }
        }
    }

    // ------------------------------------------------------------ Klassen

    /// Alle Methoden mit diesem Namen, die ein Typ hat: eigene, aus Mixins, geerbte
    /// (überschriebene nur einmal), mit eingesetzten Typargumenten.
    pub fn methods_of(&self, t: &Ty, name: &str) -> Vec<MethodInfo> {
        let mut out: Vec<MethodInfo> = Vec::new();
        let mut types = vec![t.clone()];
        types.extend(self.supertypes(t));
        for ty in types {
            let Ty::Named(d, args) = &ty else { continue };
            let map = self.subst_map(*d, args);
            for m in self.defs[*d].methods.iter().filter(|m| m.name == name) {
                let m = m.subst(&map);
                let dup = out.iter().any(|o| same_params(&o.params, &m.params));
                if !dup {
                    out.push(m);
                } else if let Some(o) = out.iter_mut().find(|o| same_params(&o.params, &m.params)) {
                    // nodiscard einer erfüllten Anforderung oder überschriebenen Methode gilt weiter (V-8.2)
                    let nodiscard = o.nodiscard || m.nodiscard;
                    // eine Mixin-Methode mit Rumpf erfüllt eine Anforderung
                    if !o.has_body && m.has_body && self.defs[o.owner].kind == DefKind::Mixin {
                        *o = m;
                    }
                    o.nodiscard = nodiscard;
                }
            }
        }
        out
    }

    pub fn field_of(&self, t: &Ty, name: &str) -> Option<(FieldInfo, DefId)> {
        let mut types = vec![t.clone()];
        types.extend(self.supertypes(t));
        for ty in types {
            let Ty::Named(d, args) = &ty else { continue };
            let map = self.subst_map(*d, args);
            if let Some(f) = self.defs[*d].fields.iter().find(|f| f.name == name) {
                let mut f = f.clone();
                f.ty = f.ty.subst(&map);
                return Some((f, *d));
            }
        }
        None
    }

    pub fn is_subclass(&self, sub: DefId, sup: DefId) -> bool {
        let mut cur = Some(sub);
        while let Some(c) = cur {
            if c == sup {
                return true;
            }
            cur = match &self.defs[c].base {
                Some(Ty::Named(b, _)) => Some(*b),
                _ => None,
            };
        }
        false
    }

    fn check_defs(&mut self) {
        for def in 0..self.defs.len() {
            if self.is_std_file(self.defs[def].file) {
                continue;
            }
            match self.defs[def].kind {
                DefKind::Class => self.check_class(def),
                DefKind::Mixin => self.check_mixin(def),
                DefKind::Enum => {}
            }
            if self.defs[def].export {
                self.check_export(def);
            }
        }
    }

    fn check_mixin(&mut self, def: DefId) {
        let file = self.defs[def].file;
        for m in self.defs[def].methods.clone() {
            if m.is_static {
                self.error(file, m.span, "R-7.18", "ein Mixin hat keine statischen Methoden");
            }
            if m.nodiscard && m.ret == Ret::Void {
                self.error(file, m.span, "R-8.15", "nodiscard an einer void-Methode");
            }
        }
    }

    fn check_class(&mut self, def: DefId) {
        let file = self.defs[def].file;
        let d = self.defs[def].clone();
        let self_ty = Ty::Named(def, d.tparams.iter().map(|&p| Ty::Param(p)).collect());

        // R-7.1: eindeutige Namen
        let mut seen_fields: HashMap<String, Span> = HashMap::new();
        if let Some(b) = &d.base {
            let mut types = vec![b.clone()];
            types.extend(self.supertypes(b));
            for t in types {
                if let Ty::Named(bd, _) = t {
                    for f in &self.defs[bd].fields {
                        seen_fields.entry(f.name.clone()).or_insert(f.span);
                    }
                }
            }
        }
        for f in &d.fields {
            if seen_fields.contains_key(&f.name) {
                self.error(file, f.span, "R-7.1", format!("Feld {} ist schon deklariert", f.name));
            }
            seen_fields.insert(f.name.clone(), f.span);
        }
        for m in &d.methods {
            if let Some(fs) = d.fields.iter().find(|f| f.name == m.name) {
                let at = if fs.span > m.span { fs.span } else { m.span };
                self.error(file, at, "R-7.1", format!("Feld und Methode heißen beide {}", m.name));
            }
        }

        // V-8.2: nodiscard an void
        for m in &d.methods {
            if m.nodiscard && m.ret == Ret::Void {
                self.error(file, m.span, "R-8.15", "nodiscard an einer void-Methode");
            }
        }

        // R-7.16: Überladen
        for (i, m) in d.methods.iter().enumerate() {
            for o in &d.methods[..i] {
                if o.name != m.name || o.is_static != m.is_static || !same_param_types(&o.params, &m.params) {
                    continue;
                }
                let msg = if !same_params(&o.params, &m.params) {
                    "unterscheidet sich nur in Besitzform oder Borrow-Markierung der Parameter"
                } else if o.ret != m.ret {
                    "unterscheidet sich nur im Rückgabetyp"
                } else {
                    "ist doppelt deklariert"
                };
                self.error(file, m.span, "R-7.16", format!("Methode {} {msg}", m.name));
                break;
            }
        }
        for (i, c) in d.ctors.iter().enumerate() {
            if d.ctors[..i].iter().any(|o| same_param_types(&o.params, &c.params)) {
                self.error(file, c.span, "R-7.16", "Konstruktor mit denselben Parametertypen ist doppelt");
            }
        }

        // R-7.13: Überschreiben
        let inherited: Vec<MethodInfo> = match &d.base {
            Some(b) => d.methods.iter().flat_map(|m| self.methods_of(b, &m.name)).collect(),
            None => Vec::new(),
        };
        let mixin_methods: Vec<MethodInfo> = d
            .imports
            .iter()
            .flat_map(|(t, _)| {
                let mut ts = vec![t.clone()];
                ts.extend(self.supertypes(t));
                ts
            })
            .flat_map(|t| match &t {
                Ty::Named(md, args) => {
                    let map = self.subst_map(*md, args);
                    self.defs[*md].methods.iter().map(|m| m.subst(&map)).collect::<Vec<_>>()
                }
                _ => Vec::new(),
            })
            .collect();
        for m in d.methods.iter().filter(|m| !m.is_static) {
            let base_m = inherited.iter().find(|b| b.name == m.name && same_param_types(&b.params, &m.params));
            let mix_m = mixin_methods.iter().find(|b| b.name == m.name && same_param_types(&b.params, &m.params));
            let is_override = m.modifier == Some(MethodModifier::Override);
            match base_m {
                Some(b) => {
                    if b.nodiscard && !m.nodiscard {
                        self.error(file, m.span, "R-8.15", format!("{} muss nodiscard übernehmen", m.name));
                    } else if !b.overridable() {
                        self.error(file, m.span, "R-7.13", format!("{} ist in der Basisklasse nicht virtual", m.name));
                    } else if !is_override {
                        self.error(file, m.span, "R-7.13", format!("{} überschreibt eine geerbte Methode und braucht override", m.name));
                    } else if !same_params(&b.params, &m.params) || b.mut_this != m.mut_this {
                        self.error(file, m.span, "R-7.13", format!("{} überschreibt mit anderer Borrow-Markierung", m.name));
                    } else if access_rank(m.access) < access_rank(b.access) {
                        self.error(file, m.span, "R-7.13", format!("{} darf beim Überschreiben nicht weniger sichtbar werden", m.name));
                    }
                }
                None => match mix_m {
                    Some(b) if b.has_body && !is_override => {
                        self.error(file, m.span, "R-7.21", format!("{} kommt auch aus einem Mixin; nötig ist override", m.name));
                    }
                    Some(_) => {}
                    None if is_override => {
                        self.error(file, m.span, "R-7.13", format!("{} überschreibt nichts", m.name));
                    }
                    None => {}
                },
            }
        }

        // R-7.21: Konflikte zwischen Mixins; R-7.20: Anforderungen
        let mut bodies: Vec<(MethodInfo, Span)> = Vec::new();
        for (it, ispan) in &d.imports {
            let mut ts = vec![it.clone()];
            ts.extend(self.supertypes(it));
            for t in ts {
                let Ty::Named(md, args) = &t else { continue };
                let map = self.subst_map(*md, args);
                for m in self.defs[*md].methods.clone().into_iter().filter(|m| m.has_body) {
                    let m = m.subst(&map);
                    let conflict = bodies.iter().any(|(o, _)| o.owner != m.owner && o.name == m.name && same_param_types(&o.params, &m.params));
                    let resolved = d.methods.iter().any(|c| c.name == m.name && same_param_types(&c.params, &m.params) && c.modifier == Some(MethodModifier::Override));
                    if conflict && !resolved {
                        self.error(file, *ispan, "R-7.21", format!("{} kommt aus zwei Mixins", m.name));
                    }
                    if !bodies.iter().any(|(o, _)| o.owner == m.owner && o.name == m.name && same_param_types(&o.params, &m.params)) {
                        bodies.push((m, *ispan));
                    }
                }
            }
        }
        for (it, ispan) in &d.imports {
            let mut ts = vec![it.clone()];
            ts.extend(self.supertypes(it));
            for t in ts {
                let Ty::Named(md, args) = &t else { continue };
                let map = self.subst_map(*md, args);
                for r in self.defs[*md].methods.clone().into_iter().filter(|m| !m.has_body) {
                    let r = r.subst(&map);
                    let ok = self.methods_of(&self_ty, &r.name).iter().any(|c| {
                        c.has_body && same_param_types(&c.params, &r.params) && (c.ret == r.ret || c.ret.ty().is_error())
                    }) || d.is_abstract;
                    if !ok {
                        self.error(file, *ispan, "R-7.20", format!(
                            "die Anforderung {} von {} ist nicht erfüllt", r.name, self.defs[*md].name));
                    }
                }
            }
        }

        // R-7.14: abstrakte Methoden nur in abstrakten Klassen
        if !d.is_abstract {
            let own_abstract = d.methods.iter().find(|m| m.modifier == Some(MethodModifier::Abstract));
            if let Some(m) = own_abstract {
                self.error(file, m.span, "R-7.14", format!("{} ist abstrakt, die Klasse aber nicht", m.name));
            } else if let Some(b) = &d.base {
                let mut names: Vec<String> = Vec::new();
                for t in std::iter::once(b.clone()).chain(self.supertypes(b)) {
                    if let Ty::Named(bd, _) = t {
                        names.extend(self.defs[bd].methods.iter().filter(|m| m.modifier == Some(MethodModifier::Abstract)).map(|m| m.name.clone()));
                    }
                }
                for n in names {
                    let implemented = self.methods_of(&self_ty, &n).iter().all(|m| m.has_body);
                    if !implemented {
                        self.error(file, d.span, "R-7.14", format!("{n} ist abstrakt; die Klasse muss abstract sein"));
                        break;
                    }
                }
            }
        }

        // R-7.10, R-7.11: copy
        let base_def = match &d.base {
            Some(Ty::Named(b, _)) => Some(*b),
            _ => None,
        };
        let ancestor_copy = {
            let mut cur = base_def;
            let mut found = false;
            while let Some(c) = cur {
                if self.defs[c].copy.is_some() {
                    found = true;
                    break;
                }
                cur = match &self.defs[c].base {
                    Some(Ty::Named(b, _)) => Some(*b),
                    _ => None,
                };
            }
            found
        };
        if ancestor_copy && d.copy.is_none() {
            self.error(file, d.span, "R-7.11", format!("{} braucht ein eigenes copy, weil die Basisklasse eines hat", d.name));
        }
        if let (Some(cs), Some(b)) = (d.copy, base_def) {
            if self.defs[b].copy.is_none() {
                self.error(file, cs, "R-7.10", "copy in der Unterklasse verlangt copy in der Basisklasse");
            }
        }

        // R-7.5, R-7.6: Konstruktoren und Basiskonstruktor
        if let Some(b) = base_def {
            let base_default = self.has_default_ctor(b);
            if !base_default {
                for c in d.ctors.iter().filter(|c| !c.calls_base) {
                    self.error(file, c.span, "R-7.6", format!(
                        "{} hat keinen parameterlosen Konstruktor; nötig ist : base(…)", self.defs[b].name));
                }
                if d.ctors.is_empty() {
                    self.error(file, d.span, "R-7.6", format!(
                        "{} hat keinen parameterlosen Konstruktor; nötig ist ein Konstruktor mit : base(…)", self.defs[b].name));
                }
            }
        }
    }

    /// R-11.11: Die öffentliche Schnittstelle eines exportierten Typs nennt nur exportierte Typen.
    fn check_export(&mut self, def: DefId) {
        let d = self.defs[def].clone();
        let file = d.file;
        let mut uses: Vec<(Ty, Span)> = Vec::new();
        if let Some(b) = &d.base {
            uses.push((b.clone(), d.span));
        }
        for f in d.fields.iter().filter(|f| f.access != Access::Private) {
            uses.push((f.ty.clone(), f.span));
        }
        for m in d.methods.iter().filter(|m| m.access != Access::Private) {
            uses.push((m.ret.ty(), m.span));
            uses.extend(m.params.iter().map(|p| (p.ty.clone(), m.span)));
        }
        for c in d.ctors.iter().filter(|c| c.access != Access::Private) {
            uses.extend(c.params.iter().map(|p| (p.ty.clone(), c.span)));
        }
        for v in &d.variants {
            uses.extend(v.fields.iter().map(|p| (p.ty.clone(), d.span)));
        }
        let mut reported = HashSet::new();
        for (t, span) in uses {
            let mut named = Vec::new();
            collect_named(&t, &mut named);
            for n in named {
                let nd = &self.defs[n];
                if nd.project == d.project && !nd.export && reported.insert(span) {
                    let msg = format!("{} ist nicht exportiert, steht aber in der Schnittstelle von {}", nd.name, d.name);
                    self.error(file, span, "R-11.11", msg);
                }
            }
        }
    }
}

fn collect_named(t: &Ty, out: &mut Vec<DefId>) {
    match t {
        Ty::Named(d, args) => {
            out.push(*d);
            args.iter().for_each(|a| collect_named(a, out));
        }
        Ty::Vector(x) | Ty::Array(x, _) | Ty::Slice(x) | Ty::Nullable(x) | Ty::Formed(_, x) => collect_named(x, out),
        Ty::Tuple(ts) => ts.iter().for_each(|x| collect_named(x, out)),
        Ty::Func(ps, r) => {
            ps.iter().for_each(|p| collect_named(&p.ty, out));
            collect_named(r, out);
        }
        _ => {}
    }
}

pub fn access_rank(a: Access) -> u8 {
    match a {
        Access::Private => 0,
        Access::Protected => 1,
        Access::Public => 2,
    }
}

/// Gleiche Parametertypen (ohne Markierungen).
pub fn same_param_types(a: &[PInfo], b: &[PInfo]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.ty.unformed() == y.ty.unformed() || x.ty.is_error() || y.ty.is_error())
}

/// Gleiche Parametertypen und Markierungen.
pub fn same_params(a: &[PInfo], b: &[PInfo]) -> bool {
    same_param_types(a, b) && a.iter().zip(b).all(|(x, y)| x.mode == y.mode)
}
