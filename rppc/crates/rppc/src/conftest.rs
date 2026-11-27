//! Semantischer Konformitätstestlauf (Kapitel 14, R-14.3 bis R-14.7), soweit die
//! Implementierung reicht: übersetzen oder ablehnen. Laufzeittests werden außerdem in
//! beiden Modi ausgeführt; verglichen werden Ausgabe, Exit-Code und Fehlerausgabe (R-14.5).
//!
//! Ein abgelehnter Test ist bestanden, wenn alle Fehler in der markierten Zeile liegen
//! und einer davon die erste Regel aus dem Testkopf nennt, als Hauptregel oder als
//! Begründung (bei Syntaxfehlern zählt nur die Zeile). Ein akzeptierter Test ist
//! bestanden, wenn es keine Fehler gibt und Warnungen genau in den mit `WARNUNG`
//! markierten Zeilen.

use rpp_sema::Diag;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Laufzeittests ausführen (ab Meilenstein M4); `RPPC_NUR_PRUEFEN=1` schaltet das ab.
fn run_runtime_enabled() -> bool {
    std::env::var("RPPC_NUR_PRUEFEN").map(|v| v != "1").unwrap_or(true)
}

pub fn std_dir() -> PathBuf {
    let dir = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../std"));
    fs::canonicalize(&dir).unwrap_or(dir)
}

fn canonical(p: &Path) -> PathBuf {
    fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
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

struct Unit {
    path: PathBuf,
    chapter: String,
    kind: String,
    header: PathBuf,
    files: Vec<PathBuf>,
}

fn units(root: &Path) -> Vec<Unit> {
    let kinds = ["accepted", "rejected", "runtime"];
    let mut all = Vec::new();
    walk(root, &mut all);
    let kind_of = |p: &Path| p.parent().and_then(|d| d.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let chapter_of = |p: &Path| {
        p.strip_prefix(root).ok().and_then(|r| r.components().next()).map(|c| c.as_os_str().to_string_lossy().to_string()).unwrap_or_default()
    };
    let projects: Vec<PathBuf> = all
        .iter()
        .filter(|p| p.file_name().is_some_and(|n| n == "rpp.yaml"))
        .filter_map(|p| p.parent().map(Path::to_path_buf))
        .filter(|d| kinds.contains(&kind_of(d).as_str()))
        .collect();
    let mut out = Vec::new();
    for p in all.iter().filter(|p| p.extension().is_some_and(|e| e == "rpp")) {
        if !projects.iter().any(|d| p.starts_with(d)) {
            out.push(Unit { path: p.clone(), chapter: chapter_of(p), kind: kind_of(p), header: p.clone(), files: vec![p.clone()] });
        }
    }
    for d in &projects {
        let files = all.iter().filter(|p| p.starts_with(d)).cloned().collect();
        out.push(Unit { path: d.clone(), chapter: chapter_of(d), kind: kind_of(d), header: d.join("rpp.yaml"), files });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// Regeln aus der Kopfzeile `Regel:` (die erste ist die erwartete, R-14.4).
fn header_rules(header: &Path) -> (Option<String>, String) {
    let text = fs::read_to_string(header).unwrap_or_default();
    let line = text
        .lines()
        .take(6)
        .find_map(|l| l.strip_prefix("// Regel: ").or_else(|| l.strip_prefix("# Regel: ")))
        .unwrap_or("")
        .to_string();
    let first = line.split(',').next().map(str::trim).filter(|r| r.starts_with("R-")).map(|r| {
        r.split_whitespace().next().unwrap_or(r).to_string()
    });
    (first, line)
}

fn markers(files: &[PathBuf], word: &str) -> Vec<(PathBuf, u32)> {
    let mut out = Vec::new();
    for f in files {
        let text = fs::read_to_string(f).unwrap_or_default();
        for (i, line) in text.lines().enumerate() {
            let t = line.trim_end();
            if t.ends_with(&format!("// {word}")) || t.ends_with(&format!("# {word}")) {
                out.push((canonical(f), i as u32 + 1));
            }
        }
    }
    out
}

enum Outcome {
    Pass,
    Fail(String),
    /// braucht eingebaute Teile der Standardbibliothek, die noch fehlen
    Open(String),
}

fn judge(u: &Unit, diags: &[Diag]) -> Outcome {
    let errors: Vec<&Diag> = diags.iter().filter(|d| !d.warning).collect();
    let show = |ds: &[&Diag]| ds.iter().map(|d| format!("{d}")).collect::<Vec<_>>().join("\n         ");
    match u.kind.as_str() {
        "accepted" | "runtime" => {
            if !errors.is_empty() {
                return Outcome::Fail(show(&errors));
            }
            // R-14.7: genau die markierten Warnungen und keine weiteren
            let marks = markers(&u.files, "WARNUNG");
            let warnings: Vec<&Diag> = diags.iter().filter(|d| d.warning).collect();
            let unexpected: Vec<&Diag> = warnings.iter().copied().filter(|d| !marks.contains(&(canonical(&d.file), d.span.line))).collect();
            if !unexpected.is_empty() {
                return Outcome::Fail(format!("Unexpected warning:
         {}", show(&unexpected)));
            }
            let missing: Vec<&(PathBuf, u32)> = marks.iter().filter(|m| !warnings.iter().any(|d| (canonical(&d.file), d.span.line) == **m)).collect();
            if let Some((f, l)) = missing.first() {
                return Outcome::Fail(format!("Expected warning missing at {}:{l}", f.display()));
            }
            Outcome::Pass
        }
        _ => {
            let marks = markers(&u.files, "FEHLER");
            let (rule, _) = header_rules(&u.header);
            if errors.is_empty() {
                return Outcome::Fail("No error reported".into());
            }
            let elsewhere: Vec<&Diag> = errors.iter().copied().filter(|d| !marks.contains(&(canonical(&d.file), d.span.line))).collect();
            if !elsewhere.is_empty() {
                return Outcome::Fail(format!("Error outside the marked line:\n         {}", show(&errors)));
            }
            let syntax = errors.iter().any(|d| d.rule == "Syntax");
            let rule_ok = match &rule {
                Some(r) => syntax || errors.iter().any(|d| d.names(r)),
                None => true,
            };
            if rule_ok {
                Outcome::Pass
            } else {
                Outcome::Fail(format!("Expected {}, got:\n         {}", rule.unwrap_or_default(), show(&errors)))
            }
        }
    }
}

/// Übersetzt die C-Testbibliotheken `KAPITEL/native/NAME.c` nach `KAPITEL/native/build`
/// (Kapitel 15): `NAME.dll` und `NAME.lib` mit MSVC unter Windows, sonst `libNAME.so` bzw.
/// `libNAME.dylib` mit `cc`. Unveränderte Bibliotheken werden nicht neu übersetzt.
pub fn build_native_libs(conformance: &Path) -> Result<(), String> {
    use std::process::Command;
    let mut sources = Vec::new();
    for ch in fs::read_dir(conformance).map_err(|e| e.to_string())?.flatten() {
        let dir = ch.path().join("native");
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for f in rd.flatten() {
            let p = f.path();
            if p.extension().is_some_and(|e| e == "c") {
                sources.push(p);
            }
        }
    }
    sources.sort();
    for src in sources {
        let dir = src.parent().unwrap().join("build");
        let stem = src.file_stem().unwrap().to_string_lossy().to_string();
        let out = dir.join(rpp_interp::ffi::file_name(&stem));
        let modified = |p: &Path| fs::metadata(p).and_then(|m| m.modified()).ok();
        if let (Some(a), Some(b)) = (modified(&out), modified(&src)) {
            if a >= b {
                continue;
            }
        }
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let mut cmd = if cfg!(windows) {
            let mut c = Command::new("cl");
            c.arg("/nologo").arg("/LD").arg("/O2").arg(&src);
            c.arg(format!("/Fe:{}", out.display()));
            c.arg(format!("/Fo:{}\\", dir.display()));
            c
        } else {
            let mut c = Command::new("cc");
            c.arg("-shared").arg("-fPIC").arg("-O2").arg("-o").arg(&out).arg(&src);
            c
        };
        let res = cmd.output().map_err(|e| format!("cannot run the C compiler for {}: {e}", src.display()))?;
        if !res.status.success() {
            return Err(format!(
                "compiling {} failed:\n{}{}",
                src.display(),
                String::from_utf8_lossy(&res.stdout),
                String::from_utf8_lossy(&res.stderr)
            ));
        }
    }
    Ok(())
}

pub fn run(root: &Path, verbose: bool) -> ExitCode {
    let conformance = root.join("conformance");
    if run_runtime_enabled() {
        if let Err(e) = build_native_libs(&conformance) {
            eprintln!("C test libraries: {e}");
            return ExitCode::from(1);
        }
    }
    let std = std_dir();
    let mut stats: BTreeMap<String, [usize; 3]> = BTreeMap::new();
    let mut lines = Vec::new();
    let mut failures = 0;
    for u in units(&conformance) {
        let diags = rpp_sema::check(&u.path, &std);
        let outcome = match judge(&u, &diags) {
            Outcome::Pass if u.kind == "runtime" && run_runtime_enabled() => run_runtime(&u),
            other => other,
        };
        let label = u.path.strip_prefix(&conformance).unwrap_or(&u.path).to_string_lossy().replace('\\', "/");
        let entry = stats.entry(u.chapter.clone()).or_default();
        match outcome {
            Outcome::Pass => {
                entry[0] += 1;
                if verbose {
                    lines.push(format!("  ok     {label}"));
                }
            }
            Outcome::Fail(why) => {
                entry[1] += 1;
                failures += 1;
                lines.push(format!("  FAIL  {label}\n         {why}"));
            }
            Outcome::Open(why) => {
                entry[2] += 1;
                if verbose {
                    lines.push(format!("  pending {label} ({why})"));
                }
            }
        }
    }
    lines.push(String::new());
    lines.push("Chapter                   passed          failed     pending".to_string());
    let mut total = [0usize; 3];
    for (chapter, s) in &stats {
        lines.push(format!("  {chapter:<20} {:>9}  {:>14}  {:>10}", s[0], s[1], s[2]));
        for i in 0..3 {
            total[i] += s[i];
        }
    }
    lines.push(format!("  {:<20} {:>9}  {:>14}  {:>10}", "total", total[0], total[1], total[2]));
    println!("{}", lines.join("\n"));
    if failures == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

/// Prüft und führt ein Programm aus. Bei Fehlern der Prüfung: die Meldungen.
pub fn execute_stdio(input: &Path, release: bool, check: bool, args: Vec<String>) -> Result<i32, Vec<Diag>> {
    let program = checked_program(input, release)?;
    Ok(rpp_interp::run_stdio(program, release, check, args))
}

fn checked_program(input: &Path, release: bool) -> Result<&'static rpp_sema::defs::Program, Vec<Diag>> {
    let program = rpp_sema::analyze(input, &std_dir(), release);
    if program.diags.iter().any(|d| !d.warning) {
        return Err(program.diags);
    }
    // R-15.12: im Releasemodus sind fehlende C-Bibliotheken und Symbole Fehler beim Bauen
    if release && has_natives(&program) {
        if let Err((msg, def)) = rpp_interp::ffi::load(&program) {
            let d = &program.defs[def];
            let span = d.ext.as_ref().map(|e| e.lib_span).unwrap_or_default();
            let path = program.ws.files[d.file].path.clone();
            return Err(vec![Diag::error(&path, span, "R-15.12", msg)]);
        }
    }
    // Das Programm lebt bis zum Ende des Prozesses; der Interpreter verweist in seinen Text.
    Ok(Box::leak(Box::new(program)))
}

/// Deklariert das Programm C-Funktionen (Kapitel 15)?
fn has_natives(p: &rpp_sema::defs::Program) -> bool {
    p.defs.iter().any(|d| d.ext.is_some())
}

/// Führt ein Programm mit C-Anbindung in einem eigenen Prozess aus: Ausgaben von C gehen an
/// die Standardausgabe des Prozesses und lassen sich nur so mit denen von R++ zusammen erfassen.
fn execute_process(input: &Path, release: bool, args: &[String], stdin: &str) -> Result<rpp_interp::Outcome, String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut cmd = Command::new(exe);
    cmd.arg("run");
    if release {
        cmd.arg("--release");
    }
    cmd.arg("--check-invariants").arg(input).arg("--").args(args);
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| e.to_string())?;
    if let Some(mut s) = child.stdin.take() {
        let _ = s.write_all(stdin.as_bytes());
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let text = |b: &[u8]| String::from_utf8_lossy(b).replace("\r\n", "\n");
    Ok(rpp_interp::Outcome { stdout: text(&out.stdout), stderr: text(&out.stderr), exit: out.status.code().unwrap_or(-1) })
}

/// Erwartung eines Laufzeittests für einen Modus (R-14.5).
#[derive(Default, Debug)]
struct Expect {
    stdout: Option<String>,
    exit: Option<i32>,
    stderr_contains: Vec<String>,
    /// `Fehler beim Bauen`: das Programm lässt sich in diesem Modus nicht bauen (R-15.12)
    build_error: bool,
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    let inner = s.strip_prefix('"').and_then(|x| x.strip_suffix('"')).unwrap_or(s);
    let mut out = String::new();
    let mut it = inner.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(o) => out.push(o),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn parse_expect(spec: &str) -> Expect {
    let mut e = Expect::default();
    for part in spec.split(';') {
        let part = part.trim();
        if let Some(x) = part.strip_prefix("Ausgabe ") {
            e.stdout = Some(unquote(x));
        } else if let Some(x) = part.strip_prefix("Exit-Code ") {
            e.exit = x.trim().parse().ok();
        } else if let Some(x) = part.strip_prefix("Fehlerausgabe enthält ") {
            e.stderr_contains.push(unquote(x));
        } else if part == "Fehler beim Bauen" {
            e.build_error = true;
        }
    }
    e
}

/// Kopfzeilen eines Laufzeittests: Erwartung je Modus, Argumente, Eingabe.
fn runtime_header(header: &Path) -> ([Option<Expect>; 2], Vec<String>, String) {
    let text = fs::read_to_string(header).unwrap_or_default();
    let mut exp: [Option<Expect>; 2] = [None, None];
    let mut args = Vec::new();
    let mut input = String::new();
    for l in text.lines().take(12) {
        let l = l.trim_start_matches("//").trim_start_matches('#').trim();
        if let Some(x) = l.strip_prefix("Erwartet (debug):") {
            exp[0] = Some(parse_expect(x));
        } else if let Some(x) = l.strip_prefix("Erwartet (release):") {
            exp[1] = Some(parse_expect(x));
        } else if let Some(x) = l.strip_prefix("Erwartet:") {
            exp[0] = Some(parse_expect(x));
            exp[1] = Some(parse_expect(x));
        } else if let Some(x) = l.strip_prefix("Argumente:") {
            args = x.split_whitespace().map(str::to_string).collect();
        } else if let Some(x) = l.strip_prefix("Eingabe:") {
            input = unquote(x);
        }
    }
    (exp, args, input)
}

/// Führt einen Laufzeittest in beiden Modi aus (R-14.7).
fn run_runtime(u: &Unit) -> Outcome {
    let (exp, args, input) = runtime_header(&u.header);
    for (mode, e) in exp.iter().enumerate() {
        let Some(e) = e else { continue };
        let name = if mode == 0 { "debug" } else { "release" };
        // V-14.3: jeder Lauf in einem frischen, leeren Arbeitsverzeichnis, das danach gelöscht wird
        let path = canonical(&u.path);
        let work = std::env::temp_dir().join(format!("rppc-test-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&work);
        let previous = std::env::current_dir().ok();
        let ran = fs::create_dir_all(&work).and_then(|_| std::env::set_current_dir(&work));
        let result = match ran {
            Ok(()) => match checked_program(&path, mode == 1) {
                Ok(p) if has_natives(p) => execute_process(&path, mode == 1, &args, &input).map_err(|e| {
                    vec![Diag::error(&path, Default::default(), "Intern", format!("cannot start rppc: {e}"))]
                }),
                Ok(p) => Ok(rpp_interp::run(p, rpp_interp::Options { release: mode == 1, check: true, args: args.clone(), input: input.clone() })),
                Err(d) => Err(d),
            },
            Err(e) => return Outcome::Fail(format!("{name}: working directory {}: {e}", work.display())),
        };
        if let Some(prev) = previous {
            let _ = std::env::set_current_dir(prev);
        }
        let _ = fs::remove_dir_all(&work);
        let o = match result {
            Ok(_) if e.build_error => return Outcome::Fail(format!("{name}: expected a build error")),
            Ok(o) => o,
            Err(_) if e.build_error => continue,
            Err(diags) => return Outcome::Fail(format!("{name}: compilation failed: {}", diags.first().map(|d| d.to_string()).unwrap_or_default())),
        };
        let mut problems = Vec::new();
        if let Some(s) = &e.stdout {
            let got = o.stdout.strip_suffix('\n').unwrap_or(&o.stdout);
            if got != s {
                problems.push(format!("expected stdout {s:?}, got {got:?}"));
            }
        }
        if let Some(x) = e.exit {
            if o.exit != x {
                problems.push(format!("expected exit code {x}, got {}", o.exit));
            }
        }
        for c in &e.stderr_contains {
            if !o.stderr.contains(c.as_str()) {
                problems.push(format!("stderr does not contain {c:?}"));
            }
        }
        if !problems.is_empty() && o.stderr.contains("FailError: eingebaut") {
            return Outcome::Open(format!("{name}: requires standard library built-ins that are not available yet"));
        }
        if !problems.is_empty() {
            return Outcome::Fail(format!("{name}: {}; stderr: {:?}", problems.join("; "), o.stderr.lines().next().unwrap_or("")));
        }
    }
    Outcome::Pass
}

/// `rppc check DATEI|PROJEKT`: Meldungen ausgeben.
pub fn check(input: &Path) -> ExitCode {
    let diags = rpp_sema::check(input, &std_dir());
    let failed = diags.iter().any(|d| !d.warning);
    crate::print_diags(input, diags);
    if failed {
        ExitCode::from(1)
    } else {
        println!("{}: ok", input.display());
        ExitCode::SUCCESS
    }
}
