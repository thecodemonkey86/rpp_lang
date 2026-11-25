//! Textform der Deklarationstabellen und der Meldungen der Deklarationsphase
//! (`Program::build`), zum Vergleich mit `rppcc defs` (Meilenstein M6).

use rpp_sema::defs::{DefKind, PInfo, Program, Ret};
use rpp_sema::project::Workspace;
use crate::{display_path, input_dir};
use rpp_sema::ty::{Mode, ParamId};
use rpp_syntax::ast::{Access, MethodModifier};
use std::fmt::Write;

fn access(a: Access) -> &'static str {
    match a {
        Access::Public => "public",
        Access::Private => "private",
        Access::Protected => "protected",
    }
}

fn params(p: &Program, ps: &[PInfo]) -> String {
    ps.iter()
        .map(|x| match x.mode {
            Mode::Read => format!("{} {}", p.show(&x.ty), x.name),
            Mode::Mut => format!("mut {} {}", p.show(&x.ty), x.name),
            Mode::Own => format!("own {} {}", p.show(&x.ty), x.name),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn tparams(p: &Program, ids: &[ParamId]) -> String {
    ids.iter()
        .map(|&id| {
            let tp = &p.tparams[id];
            let mut s = tp.name.clone();
            if tp.needs_new {
                s.push_str(" new");
            }
            if !tp.constraints.is_empty() {
                s.push_str(" : ");
                s.push_str(&tp.constraints.iter().map(|c| p.show(c)).collect::<Vec<_>>().join(", "));
            }
            s
        })
        .collect::<Vec<_>>()
        .join("; ")
}

pub fn defs(input: &std::path::Path, std_dir: &std::path::Path, release: bool) -> String {
    let base = input_dir(input);
    let ws = Workspace::load_mode(input, std_dir, release);
    let mut p = Program::build(ws);
    p.diags.sort_by(|a, b| (&a.file, a.span).cmp(&(&b.file, b.span)));
    p.diags.dedup();
    let mut out = String::new();
    for (i, d) in p.defs.iter().enumerate() {
        let kind = match d.kind {
            DefKind::Class => "klasse",
            DefKind::Mixin => "mixin",
            DefKind::Enum => "enum",
        };
        let mut flags = String::new();
        if d.export {
            flags.push_str(" export");
        }
        if d.is_abstract {
            flags.push_str(" abstract");
        }
        let _ = writeln!(out, "typ {i} {} {kind}{flags} projekt={}", d.full_name(), d.project);
        if !d.tparams.is_empty() {
            let _ = writeln!(out, "  tparams {}", tparams(&p, &d.tparams));
        }
        if let Some(b) = &d.base {
            let _ = writeln!(out, "  basis {}", p.show(b));
        }
        for (t, s) in &d.imports {
            let _ = writeln!(out, "  import {} @{}:{}", p.show(t), s.line, s.col);
        }
        for f in &d.fields {
            let mut fl = String::new();
            if f.is_static {
                fl.push_str(" static");
            }
            if f.is_const {
                fl.push_str(" const");
            }
            if f.has_init {
                fl.push_str(" init");
            }
            let _ = writeln!(out, "  feld {}{fl} {}: {} @{}:{}", access(f.access), f.name, p.show(&f.ty), f.span.line, f.span.col);
        }
        for m in &d.methods {
            let mut fl = String::new();
            match m.modifier {
                Some(MethodModifier::Static) => fl.push_str(" static"),
                Some(MethodModifier::Virtual) => fl.push_str(" virtual"),
                Some(MethodModifier::Override) => fl.push_str(" override"),
                Some(MethodModifier::Abstract) => fl.push_str(" abstract"),
                None => {}
            }
            if m.nodiscard {
                fl.push_str(" nodiscard");
            }
            if m.mut_this {
                fl.push_str(" mut");
            }
            if !m.has_body {
                fl.push_str(" ohne-rumpf");
            }
            let ret = match &m.ret {
                Ret::Void => "void".to_string(),
                Ret::Val(t) => p.show(t),
                Ret::Ref(t, false) => format!("ref {}", p.show(t)),
                Ret::Ref(t, true) => format!("ref mut {}", p.show(t)),
            };
            let tps = if m.tparams.is_empty() { String::new() } else { format!("<{}>", tparams(&p, &m.tparams)) };
            let _ = writeln!(
                out,
                "  methode {}{fl} {}{tps}({}) -> {ret} @{}:{}",
                access(m.access),
                m.name,
                params(&p, &m.params),
                m.span.line,
                m.span.col
            );
        }
        for c in &d.ctors {
            let base = if c.calls_base { " base" } else { "" };
            let _ = writeln!(out, "  konstruktor {}({}){base} @{}:{}", access(c.access), params(&p, &c.params), c.span.line, c.span.col);
        }
        if d.has_dtor {
            let _ = writeln!(out, "  destruktor");
        }
        if let Some(s) = d.copy {
            let _ = writeln!(out, "  copy @{}:{}", s.line, s.col);
        }
        for v in &d.variants {
            let value = v.value.map(|x| format!(" = {x}")).unwrap_or_default();
            let _ = writeln!(out, "  variante {}({}){value}", v.name, params(&p, &v.fields));
        }
    }
    for d in &p.diags {
        let _ = writeln!(out, "error {}:{}:{} [{}] {}", display_path(&d.file, &base), d.span.line, d.span.col, d.rule, d.message);
    }
    out
}

/// Alle Meldungen von `analyze` (Deklarationen und Rümpfe), zum Vergleich mit `rppcc diagnose`.
pub fn pruefe(input: &std::path::Path, std_dir: &std::path::Path) -> String {
    let base = input_dir(input);
    let p = rpp_sema::analyze(input, std_dir, false);
    let mut out = String::new();
    for d in &p.diags {
        let kind = if d.warning { "warning" } else { "error" };
        let mut rules = d.rule.to_string();
        for a in &d.also {
            rules.push_str(", ");
            rules.push_str(a);
        }
        let _ = writeln!(out, "{kind} {}:{}:{} [{rules}] {}", display_path(&d.file, &base), d.span.line, d.span.col, d.message);
    }
    out
}
