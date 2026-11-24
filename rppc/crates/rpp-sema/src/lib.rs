//! Semantische Prüfung von R++ (Meilensteine M2 und M3): Projekte und Pakete (Kapitel 11),
//! Namensauflösung, Typen (Kapitel 4), Besitz und Borrows (Kapitel 5), Null-Sicherheit
//! (Kapitel 6), Klassen und Generics (Kapitel 7), Anweisungen (Kapitel 8), Exceptions
//! (Kapitel 9), Lambdas (Kapitel 10) und die Signaturen der Standardbibliothek (Kapitel 13).
//!
//! Einstieg: [`check`]. Jede Meldung nennt die Regel, gegen die verstoßen wird (R-1.5).

pub mod body;
pub mod defs;
pub mod flow;
pub mod project;
pub mod rt;
pub mod ty;
pub mod yaml;

use rpp_syntax::ast::{ClassMember, TopDecl};
use rpp_syntax::Span;
use std::path::{Path, PathBuf};

/// Eine Meldung der Implementierung: Fehler oder Warnung mit Regelnummer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diag {
    pub file: PathBuf,
    pub span: Span,
    /// Regelnummer wie `R-4.3`; `Syntax` für Syntaxfehler
    pub rule: &'static str,
    /// weitere Regeln, die den Fehler genauer begründen (etwa warum ein Wert nicht verengt ist)
    pub also: Vec<&'static str>,
    pub message: String,
    pub warning: bool,
}

impl Diag {
    pub fn error(file: &Path, span: Span, rule: &'static str, message: impl Into<String>) -> Diag {
        Diag { file: file.to_path_buf(), span, rule, also: Vec::new(), message: message.into(), warning: false }
    }

    /// Nennt der Fehler die Regel `r` (als Hauptregel oder als Begründung)?
    pub fn names(&self, r: &str) -> bool {
        self.rule == r || self.also.iter().any(|a| *a == r)
    }
}

impl std::fmt::Display for Diag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = if self.warning { "Warnung" } else { "Fehler" };
        let mut rules = self.rule.to_string();
        for a in &self.also {
            rules.push_str(", ");
            rules.push_str(a);
        }
        write!(f, "{}:{}:{}: {kind} [{rules}] {}", self.file.display(), self.span.line, self.span.col, self.message)
    }
}

/// Prüft eine Datei oder ein Projektverzeichnis. `std_dir` enthält die Quellen der
/// Standardbibliothek (`rpp/core/…`). Liefert alle Meldungen, nach Position sortiert.
pub fn check(input: &Path, std_dir: &Path) -> Vec<Diag> {
    analyze(input, std_dir, false).diags
}

/// Prüft wie `check` und liefert das geprüfte Programm mit den Angaben für die
/// Ausführung (`Program::rt`). `release` wählt die bedingte Kompilierung (R-11.24).
pub fn analyze(input: &Path, std_dir: &Path, release: bool) -> defs::Program {
    let ws = project::Workspace::load_mode(input, std_dir, release);
    let mut program = defs::Program::build(ws);
    check_bodies(&mut program);
    program.diags.sort_by(|a, b| (&a.file, a.span).cmp(&(&b.file, b.span)));
    program.diags.dedup();
    program
}

fn check_bodies(p: &mut defs::Program) {
    use body::{Checker, Ctx};
    for def in 0..p.defs.len() {
        let file = p.defs[def].file;
        // Die Standardbibliothek wird für die Ausführung mitgeprüft; ihre Meldungen zählen nicht.
        let std_mark = if p.is_std_file(file) { Some(p.diags.len()) } else { None };
        check_def(p, def, file);
        if let Some(n) = std_mark {
            p.diags.truncate(n);
        }
    }
    for file in 0..p.ws.files.len() {
        if p.is_std_file(file) {
            continue;
        }
        let decls = p.ws.files[file].unit.decls.clone();
        for d in &decls {
            if let TopDecl::Main(m) = d {
                Checker::new(p, Ctx { file, class: None, is_static: true, tparams: Vec::new() }).main(m);
            }
        }
    }
}

fn check_def(p: &mut defs::Program, def: usize, file: usize) {
    use body::{Checker, Ctx};
    {
        let class_tps = p.defs[def].tparams.clone();
        let methods = p.defs[def].methods.clone();
        let fields = p.defs[def].fields.clone();
        let ctors = p.defs[def].ctors.clone();
        let decl = p.ws.files[file].unit.decls[p.defs[def].decl].clone();
        let members: Vec<ClassMember> = match &decl {
            TopDecl::Class(c) => c.members.clone(),
            TopDecl::Mixin(m) => m.members.clone(),
            _ => return,
        };
        let ctx = |is_static: bool, extra: &[usize]| {
            let mut tparams = class_tps.clone();
            tparams.extend_from_slice(extra);
            Ctx { file, class: Some(def), is_static, tparams }
        };
        for m in &methods {
            let ClassMember::Method(md) = &members[m.ast.2] else { continue };
            if let Some(body) = &md.body {
                Checker::new(p, ctx(m.is_static, &m.tparams)).method(m, body);
            }
        }
        for c in &ctors {
            let ClassMember::Constructor(cd) = &members[c.ast.2] else { continue };
            Checker::new(p, ctx(false, &[])).ctor_like(&c.params, cd.base_args.as_deref(), &cd.body, true, cd.span);
        }
        for m in &members {
            match m {
                ClassMember::Destructor(b, span) => Checker::new(p, ctx(false, &[])).ctor_like(&[], None, b, false, *span),
                ClassMember::Copy(param, b, span) => {
                    let ty = p.resolve_type(file, &class_tps, &param.ty);
                    let info = defs::PInfo { name: param.name.name.clone(), ty, mode: ty::Mode::Read };
                    Checker::new(p, ctx(false, &[])).ctor_like(&[info], None, b, false, *span);
                }
                ClassMember::Field(f) => {
                    if let Some(init) = &f.init {
                        let fty = fields.iter().find(|x| x.name == f.name.name).map(|x| x.ty.clone()).unwrap_or(ty::Ty::Error);
                        Checker::new(p, ctx(f.is_static, &[])).field_init(&fty, init);
                    }
                }
                _ => {}
            }
        }
    }
}
