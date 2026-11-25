//! Syntax-Testlauf: dieselben Prüfungen wie `rpp-grammatik/tools/run_tests.py`,
//! aber mit dem Rust-Parser. So wird gezeigt, dass beide Parser dieselbe Sprache lesen.

use rpp_syntax::ast::{ClassMember, TopDecl};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn symbols(list: &[&str]) -> HashSet<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn rpp_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    files.retain(|p| p.extension().is_some_and(|e| e == "rpp"));
    files.sort();
    files
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn read(p: &Path) -> String {
    fs::read_to_string(p).unwrap_or_default()
}

fn header_value(text: &str, key: &str) -> Option<String> {
    text.lines().take(3).find_map(|l| l.strip_prefix(key).map(|v| v.trim().to_string()))
}

/// Ein Konformitätstest: einzelne Datei oder Projektverzeichnis (R-14.1).
struct Unit {
    path: PathBuf,
    kind: String,
    sources: Vec<PathBuf>,
    header: PathBuf,
    files: Vec<PathBuf>,
}

fn conformance_units(root: &Path) -> Vec<Unit> {
    let kinds = ["akzeptiert", "abgelehnt", "laufzeit"];
    let mut all = Vec::new();
    walk(root, &mut all);
    let kind_of = |p: &Path| p.parent().and_then(|d| d.file_name()).map(|n| n.to_string_lossy().to_string());
    let projects: Vec<PathBuf> = all
        .iter()
        .filter(|p| p.file_name().is_some_and(|n| n == "rpp.yaml"))
        .filter_map(|p| p.parent().map(Path::to_path_buf))
        .filter(|d| kind_of(d).is_some_and(|k| kinds.contains(&k.as_str())))
        .collect();
    let mut units = Vec::new();
    for p in all.iter().filter(|p| p.extension().is_some_and(|e| e == "rpp")) {
        if !projects.iter().any(|d| p.starts_with(d)) {
            units.push(Unit {
                path: p.clone(),
                kind: kind_of(p).unwrap_or_default(),
                sources: vec![p.clone()],
                header: p.clone(),
                files: vec![p.clone()],
            });
        }
    }
    for d in &projects {
        let files: Vec<PathBuf> = all.iter().filter(|p| p.starts_with(d)).cloned().collect();
        units.push(Unit {
            path: d.clone(),
            kind: kind_of(d).unwrap_or_default(),
            sources: files.iter().filter(|p| p.extension().is_some_and(|e| e == "rpp")).cloned().collect(),
            header: d.join("rpp.yaml"),
            files,
        });
    }
    units.sort_by(|a, b| a.path.cmp(&b.path));
    units
}

fn is_marker(line: &str) -> bool {
    let t = line.trim_end();
    t.ends_with("// FEHLER") || t.ends_with("# FEHLER")
}

fn check_unit(u: &Unit) -> Vec<String> {
    let mut problems = Vec::new();
    let mut markers = Vec::new();
    for f in &u.files {
        for (i, line) in read(f).lines().enumerate() {
            if is_marker(line) {
                markers.push((f.clone(), i as u32 + 1));
            }
        }
    }
    let defaults = rpp_syntax::preprocess::default_symbols();
    for src in &u.sources {
        if let Err(e) = rpp_syntax::parse(&read(src), &defaults) {
            let at_marker = u.kind == "abgelehnt" && markers.contains(&(src.clone(), e.span.line));
            if !at_marker {
                let name = src.file_name().unwrap().to_string_lossy();
                problems.push(format!("{name}: {e}"));
            }
        }
    }
    if u.kind == "abgelehnt" && markers.len() != 1 {
        problems.push(format!("expected one 'FEHLER' marker, found {}", markers.len()));
    }
    if u.kind != "abgelehnt" && !markers.is_empty() {
        problems.push(format!("test in '{}' contains a 'FEHLER' marker", u.kind));
    }
    if u.kind == "laufzeit" {
        let head: Vec<String> = read(&u.header).lines().take(8).map(str::to_string).collect();
        let ok = head.iter().any(|l| {
            let l = l.strip_prefix("// ").or_else(|| l.strip_prefix("# ")).unwrap_or("");
            l.starts_with("Erwartet: ") || l.starts_with("Erwartet (debug): ") || l.starts_with("Erwartet (release): ")
        });
        if !ok {
            problems.push("runtime test is missing the 'Erwartet: …' header".into());
        }
    }
    problems
}

pub fn run(root: &Path) -> ExitCode {
    let grammar = root.join("rpp-grammatik").join("tests");
    let conformance = root.join("konformitaet");
    if !grammar.is_dir() {
        eprintln!("{} was not found. ROOT must contain rpp-grammatik/.", grammar.display());
        return ExitCode::from(2);
    }
    let mut failures = 0;
    let mut lines = Vec::new();

    lines.push("== Positive tests (must be accepted) ==".to_string());
    let pos = rpp_files(&grammar.join("pos"));
    for path in &pos {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let variants: Vec<HashSet<String>> = if name.contains("bedingte") {
            vec![symbols(&["debug"]), symbols(&["release"])]
        } else if name.contains("bedingungen") {
            vec![
                symbols(&["debug", "linux"]),
                symbols(&["debug", "windows"]),
                symbols(&["release", "macos", "TRACE"]),
                symbols(&["release", "windows"]),
            ]
        } else {
            vec![symbols(&["debug", "linux"])]
        };
        for syms in &variants {
            let result = rpp_syntax::parse(&read(path), syms);
            let expect_ok = !(name.contains("bedingte") && *syms == symbols(&["release"]));
            let mut sorted: Vec<&String> = syms.iter().collect();
            sorted.sort();
            let label = if variants.len() > 1 {
                format!("{name} [{}]", sorted.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("/"))
            } else {
                name.clone()
            };
            if result.is_ok() == expect_ok {
                lines.push(format!("  ok    {label}"));
            } else {
                failures += 1;
                match result {
                    Err(e) => lines.push(format!("  FAIL  {label}: {e}")),
                    Ok(_) => lines.push(format!("  FAIL  {label}: unexpectedly accepted")),
                }
            }
        }
    }

    lines.push(String::new());
    lines.push("== Negative tests (must be rejected) ==".to_string());
    let neg = rpp_files(&grammar.join("neg"));
    let defaults = rpp_syntax::preprocess::default_symbols();
    for path in &neg {
        let text = read(path);
        let name = header_value(&text, "// Name: ").unwrap_or_default();
        match rpp_syntax::parse(&text, &defaults) {
            Err(_) => lines.push(format!("  ok    {name}")),
            Ok(_) => {
                failures += 1;
                lines.push(format!("  FAIL  {name}: unexpectedly accepted"));
            }
        }
    }

    lines.push(String::new());
    lines.push("== Parse interpretations ==".to_string());
    let probes = rpp_files(&grammar.join("lesarten"));
    for path in &probes {
        let text = read(path);
        let name = header_value(&text, "// Name: ").unwrap_or_default();
        let expected = header_value(&text, "// Lesart: ").unwrap_or_default();
        match rpp_syntax::parse(&text, &defaults) {
            Err(e) => {
                failures += 1;
                lines.push(format!("  FAIL  {name}: {e}"));
            }
            Ok(unit) => {
                let actual = first_statement_rule(&unit).unwrap_or("?");
                if actual == expected {
                    lines.push(format!("  ok    {name}  ->  {actual}"));
                } else {
                    failures += 1;
                    lines.push(format!("  FAIL  {name}: expected {expected}, got {actual}"));
                }
            }
        }
    }

    lines.push(String::new());
    lines.push("== Conformance tests (syntax only) ==".to_string());
    let units = conformance_units(&conformance);
    for u in &units {
        let mut label = u.path.strip_prefix(&conformance).unwrap_or(&u.path).to_string_lossy().replace('\\', "/");
        if u.path.is_dir() {
            label.push('/');
        }
        let problems = check_unit(u);
        if problems.is_empty() {
            lines.push(format!("  ok    {label}"));
        } else {
            failures += 1;
            lines.push(format!("  FAIL  {label}"));
            lines.extend(problems.iter().map(|p| format!("         {p}")));
        }
    }

    lines.push(String::new());
    lines.push(format!(
        "Scope: {} positive files, {} negative tests, {} parse interpretations, {} conformance tests",
        pos.len(),
        neg.len(),
        probes.len(),
        units.len()
    ));
    lines.push(format!("Result: {failures} failures"));
    println!("{}", lines.join("\n"));
    if failures == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

/// Regel der ersten Anweisung in der ersten Methode der ersten Klasse.
fn first_statement_rule(unit: &rpp_syntax::ast::CompilationUnit) -> Option<&'static str> {
    let TopDecl::Class(c) = unit.decls.first()? else { return None };
    let ClassMember::Method(m) = c.members.first()? else { return None };
    Some(m.body.as_ref()?.stmts.first()?.rule_name())
}
