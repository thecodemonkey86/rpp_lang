//! `rppc` – Kommandozeile der ersten R++-Implementierung.
//!
//! Meilenstein M1 (siehe „R++ Implementierungsplan“):
//!   rppc parse [--symbols a,b,…] DATEI…   eine oder mehrere Dateien parsen
//!   rppc syntaxtest [WURZEL]              alle Grammatik- und Konformitätstests auf Syntax prüfen
//!
//! Meilenstein M2:
//!   rppc check DATEI|PROJEKT              Namen und Typen prüfen
//!   rppc test [-v] [WURZEL]               Konformitätstests semantisch prüfen
//!
//! Meilenstein M6 (Vergleich mit dem Lexer in R++):
//!   rppc tokens DATEI                     Tokens ausgeben, eine Zeile je Token
//!   rppc pre SYMBOLE DATEI                bedingte Kompilierung anwenden (Symbole mit Komma)
//!   rppc ast SYMBOLE DATEI                Syntaxbaum in Textform (oder der erste Fehler)
//!
//! Meilenstein M4:
//!   rppc run [--release] [--check-invariants] PATH [-- ARGS…]   Programm ausführen (Debug-Interpreter)
//!
//! Meilenstein M9:
//!   rppc lock [--stdout] PROJEKT          Abhängigkeiten aus dem Verzeichnis auflösen, rpp.lock schreiben
//!
//! WURZEL ist das Verzeichnis, das `rpp-grammar/` und `conformance/` enthält.

mod conftest;
mod defsdump;
mod syntaxtest;

use std::collections::HashSet;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
Usage:
  rppc check PATH
  rppc run [--release] [--check-invariants] PATH [-- ARGS...]
  rppc lock [--stdout] PROJECT
  rppc parse [--symbols a,b,...] FILE...
  rppc tokens FILE
  rppc pre SYMBOLS FILE
  rppc ast SYMBOLS FILE
  rppc project [--release] STD PATH
  rppc defs [--release] STD PATH
  rppc diagnose STD PATH
  rppc syntaxtest [ROOT]
  rppc test [-v] [ROOT]";

fn usage() -> ExitCode {
    eprintln!("{USAGE}");
    ExitCode::from(2)
}

fn run() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("-h" | "--help") => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some("parse") => {
            let mut symbols = rpp_syntax::preprocess::default_symbols();
            let mut files = Vec::new();
            let mut rest = args[1..].iter();
            while let Some(a) = rest.next() {
                if a == "--symbols" {
                    let Some(list) = rest.next() else { return usage() };
                    symbols = list.split(',').filter(|s| !s.is_empty()).map(str::to_string).collect::<HashSet<_>>();
                } else {
                    files.push(PathBuf::from(a));
                }
            }
            if files.is_empty() {
                return usage();
            }
            let mut failed = false;
            for f in files {
                let source = match std::fs::read_to_string(&f) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("{}: could not read file: {e}", f.display());
                        failed = true;
                        continue;
                    }
                };
                match rpp_syntax::parse(&source, &symbols) {
                    Ok(_) => println!("{}: ok", f.display()),
                    Err(e) => {
                        println!("{}:{}:{}: {}", f.display(), e.span.line, e.span.col, e.message);
                        failed = true;
                    }
                }
            }
            if failed {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        Some("run") => {
            let mut release = false;
            let mut check = false;
            let mut input = None;
            let mut prog_args = Vec::new();
            let mut rest = args[1..].iter();
            while let Some(a) = rest.next() {
                match a.as_str() {
                    "--release" => release = true,
                    "--check-invariants" | "--pruefen" => check = true,
                    "--" => {
                        prog_args.extend(rest.by_ref().cloned());
                    }
                    _ if input.is_none() => input = Some(PathBuf::from(a)),
                    _ => prog_args.push(a.clone()),
                }
            }
            let Some(input) = input else { return usage() };
            match conftest::execute_stdio(&input, release, check, prog_args) {
                Ok(exit) => ExitCode::from(exit as u8),
                Err(diags) => {
                    print_diags(&input, diags);
                    ExitCode::from(1)
                }
            }
        }
        Some("pre") => match (args.get(1), args.get(2)) {
            (Some(list), Some(p)) => {
                let symbols: HashSet<String> = list.split(',').filter(|s| !s.is_empty()).map(str::to_string).collect();
                let source = match std::fs::read_to_string(p) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("{p}: could not read file: {e}");
                        return ExitCode::from(1);
                    }
                };
                // dieselbe Form wie rpp-compiler (rppcc pre): Text oder "Line N: message"
                match rpp_syntax::preprocess::preprocess(&source, &symbols) {
                    Ok(text) => println!("{text}"),
                    Err(e) => println!("Line {}: {}", e.line, e.message),
                }
                ExitCode::SUCCESS
            }
            _ => usage(),
        },
        Some("project" | "projekt") => {
            // geladene Projekte, Dateien und Meldungen; dieselbe Form wie rpp-compiler (rppcc project)
            let release = args.get(1).is_some_and(|a| a == "--release");
            let rest = &args[if release { 2 } else { 1 }..];
            let (Some(std_dir), Some(input)) = (rest.first(), rest.get(1)) else { return usage() };
            print!("{}", projekt(std::path::Path::new(input), std::path::Path::new(std_dir), release));
            ExitCode::SUCCESS
        }
        Some("diagnose" | "pruefe") => {
            // alle Meldungen der Prüfung (rppcc diagnose)
            let (Some(std_dir), Some(input)) = (args.get(1), args.get(2)) else { return usage() };
            print!("{}", defsdump::pruefe(std::path::Path::new(input), std::path::Path::new(std_dir)));
            ExitCode::SUCCESS
        }
        Some("defs") => {
            // Deklarationstabellen und Meldungen der Deklarationsphase (rppcc defs)
            let release = args.get(1).is_some_and(|a| a == "--release");
            let rest = &args[if release { 2 } else { 1 }..];
            let (Some(std_dir), Some(input)) = (rest.first(), rest.get(1)) else { return usage() };
            print!("{}", defsdump::defs(std::path::Path::new(input), std::path::Path::new(std_dir), release));
            ExitCode::SUCCESS
        }
        Some("ast") => match (args.get(1), args.get(2)) {
            (Some(list), Some(p)) => {
                let symbols: HashSet<String> = list.split(',').filter(|s| !s.is_empty()).map(str::to_string).collect();
                let source = match std::fs::read_to_string(p) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("{p}: could not read file: {e}");
                        return ExitCode::from(1);
                    }
                };
                // dieselbe Form wie rpp-compiler (rppcc ast): Syntaxbaum oder "Line l:c message"
                match rpp_syntax::parse(&source, &symbols) {
                    Ok(u) => print!("{}", rpp_syntax::dump::unit(&u)),
                    Err(e) => println!("{e}"),
                }
                ExitCode::SUCCESS
            }
            _ => usage(),
        },
        Some("tokens") => match args.get(1) {
            Some(p) => {
                let source = match std::fs::read_to_string(p) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("{p}: could not read file: {e}");
                        return ExitCode::from(1);
                    }
                };
                // eine Zeile je Token, dieselbe Form wie rpp-compiler (rppcc tokens)
                for t in rpp_syntax::lexer::lex(&source) {
                    use rpp_syntax::lexer::TokenKind as K;
                    let pos = format!("{}:{}", t.span.line, t.span.col);
                    match t.kind {
                        K::Keyword(k) => println!("{pos} kw {}", k.as_str()),
                        K::Ident(s) => println!("{pos} id {s}"),
                        K::Int(s) => println!("{pos} int {s}"),
                        K::Float(s) => println!("{pos} float {s}"),
                        K::Str(s) => println!("{pos} str {s}"),
                        K::Char(s) => println!("{pos} char {s}"),
                        K::Punct(p) => println!("{pos} punct {}", p.as_str()),
                        K::Error(s) => println!("{pos} error {s}"),
                        K::Eof => println!("{pos} eof"),
                    }
                }
                ExitCode::SUCCESS
            }
            None => usage(),
        },
        Some("lock") => {
            let stdout = args.iter().any(|a| a == "--stdout");
            match args.iter().skip(1).find(|a| *a != "--stdout") {
                Some(p) => lock(std::path::Path::new(p), stdout),
                None => usage(),
            }
        }
        Some("check") => match args.get(1) {
            Some(p) => conftest::check(&PathBuf::from(p)),
            None => usage(),
        },
        Some("test") => {
            let verbose = args.iter().any(|a| a == "-v");
            let root = args.iter().skip(1).find(|a| *a != "-v").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".."));
            conftest::run(&root, verbose)
        }
        Some("syntaxtest") => {
            let root = args.get(1).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".."));
            syntaxtest::run(&root)
        }
        _ => usage(),
    }
}

fn main() -> ExitCode {
    // Großzügiger Stack für tief verschachtelte Ausdrücke im rekursiven Abstieg.
    std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(run)
        .expect("could not start the compiler thread")
        .join()
        .unwrap_or(ExitCode::from(101))
}

fn input_dir(input: &std::path::Path) -> std::path::PathBuf {
    let mut dir = rpp_sema::project::normalize(input);
    if !input.is_dir() {
        dir.pop();
    }
    dir
}

fn display_path(file: &std::path::Path, base: &std::path::Path) -> String {
    use rpp_sema::project::{normalize, relative_to, slash_path};
    let file = slash_path(&normalize(file));
    let base = if base.as_os_str().is_empty() { std::path::Path::new(".") } else { base };
    relative_to(&file, &slash_path(&normalize(base))).unwrap_or(file)
}

fn print_diags(input: &std::path::Path, diags: Vec<rpp_sema::Diag>) {
    let base = input_dir(input);
    for mut diag in diags {
        diag.file = PathBuf::from(display_path(&diag.file, &base));
        println!("{diag}");
    }
}

/// `lock`: löst die Abhängigkeiten aus dem Verzeichnis ohne die vorhandene Sperrdatei auf
/// und schreibt `rpp.lock` neu (R-11.30); mit `--stdout` nur ausgeben.
fn lock(input: &std::path::Path, stdout: bool) -> ExitCode {
    use rpp_sema::project::Workspace;
    if !input.join("rpp.yaml").is_file() {
        eprintln!("lock requires a project directory with rpp.yaml");
        return ExitCode::from(2);
    }
    let ws = Workspace::load_for_lock(input, &conftest::std_dir());
    if !ws.diags.is_empty() {
        print_diags(input, ws.diags);
        return ExitCode::from(1);
    }
    let text = rpp_sema::registry::format_lock(&ws.resolved);
    if stdout {
        print!("{text}");
        return ExitCode::SUCCESS;
    }
    let path = input.join("rpp.lock");
    if let Err(e) = std::fs::write(&path, text) {
        eprintln!("Could not write {}: {e}", path.display());
        return ExitCode::from(1);
    }
    println!("rpp.lock: {} libraries from the registry", ws.resolved.len());
    ExitCode::SUCCESS
}

/// Textform eines geladenen Arbeitsbereichs (Vergleich mit `rppcc project`, Meilenstein M6).
fn projekt(input: &std::path::Path, std_dir: &std::path::Path, release: bool) -> String {
    use rpp_sema::project::{slash_path, ProjectKind, Workspace};
    use std::fmt::Write;
    let ws = Workspace::load_mode(input, std_dir, release);
    let mut out = String::new();
    for (i, p) in ws.projects.iter().enumerate() {
        let kind = if p.kind == ProjectKind::Executable { "programm" } else { "bibliothek" };
        let mut flags = String::new();
        if p.is_std {
            flags.push_str(" std");
        }
        if p.single_file {
            flags.push_str(" einzeldatei");
        }
        if p.registry_version.is_some() {
            flags.push_str(" verzeichnis");
        }
        // R-15.15: native und ob das Projekt C-Code mitbringt
        if p.native {
            flags.push_str(" native");
        }
        if p.brings_c {
            flags.push_str(" c-code");
        }
        let dir = if p.single_file { input_dir(input) } else { p.dir.clone() };
        let _ = writeln!(out, "projekt {i} {} {kind}{flags} {}", p.name, slash_path(&dir));
        if let Some(m) = &p.main_file {
            let _ = writeln!(out, "  main {}", display_path(m, &dir));
        }
        if let Some(v) = &p.registry_version {
            let _ = writeln!(out, "  version {v}");
        }
        if !p.deps.is_empty() {
            let deps: Vec<String> = p.deps.iter().map(|d| d.to_string()).collect();
            let _ = writeln!(out, "  abhängig {}", deps.join(" "));
        }
        let allowed: Vec<String> = p.deps.iter().zip(&p.dep_lines).filter(|(_, l)| l.1).map(|(d, _)| d.to_string()).collect();
        if !allowed.is_empty() {
            let _ = writeln!(out, "  allowNative {}", allowed.join(" "));
        }
        for np in &p.native_paths {
            let _ = writeln!(out, "  nativePath {}", display_path(np, &dir));
        }
    }
    for f in &ws.files {
        let pkg = f.package();
        use rpp_syntax::ast::TopDecl;
        let has_main = f.unit.decls.iter().any(|d| matches!(d, TopDecl::Main(_)));
        let types: Vec<&str> = f
            .unit
            .decls
            .iter()
            .filter_map(|d| match d {
                TopDecl::Class(c) => Some(c.name.name.as_str()),
                TopDecl::Mixin(m) => Some(m.name.name.as_str()),
                TopDecl::Enum(e) => Some(e.name.name.as_str()),
                TopDecl::Main(_) => None,
            })
            .collect();
        let _ = writeln!(
            out,
            "datei {} projekt={} paket={} erwartet={} typen={}{}",
            display_path(&f.path, &ws.projects[f.project].dir),
            f.project,
            if pkg.is_empty() { "-" } else { &pkg },
            f.expected_package.as_deref().map_or("-", |e| if e.is_empty() { "." } else { e }),
            if types.is_empty() { "-".to_string() } else { types.join(",") },
            if has_main { " main" } else { "" }
        );
    }
    for (name, version, sum, native) in &ws.resolved {
        let _ = writeln!(out, "gewählt {name} {version} {sum}{}", if *native { " native" } else { "" });
    }
    let base = input_dir(input);
    for d in &ws.diags {
        let _ = writeln!(out, "error {}:{}:{} [{}] {}", display_path(&d.file, &base), d.span.line, d.span.col, d.rule, d.message);
    }
    out
}
