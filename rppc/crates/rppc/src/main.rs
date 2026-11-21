//! `rppc` – Kommandozeile der ersten R++-Implementierung.
//!
//! Meilenstein M1 (siehe „R++ Implementierungsplan“):
//!   rppc parse [--symbols a,b,…] DATEI…   eine oder mehrere Dateien parsen
//!   rppc syntaxtest [WURZEL]              alle Grammatik- und Konformitätstests auf Syntax prüfen
//!
//! WURZEL ist das Verzeichnis, das `rpp-grammatik/` und `konformitaet/` enthält.

mod syntaxtest;

use std::collections::HashSet;
use std::path::PathBuf;
use std::process::ExitCode;

fn usage() -> ExitCode {
    eprintln!("Aufruf:\n  rppc parse [--symbols a,b,…] DATEI…\n  rppc syntaxtest [WURZEL]");
    ExitCode::from(2)
}

fn run() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
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
                        eprintln!("{}: {e}", f.display());
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
        .expect("Thread starten")
        .join()
        .unwrap_or(ExitCode::from(101))
}
