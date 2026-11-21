//! Projekte und Quelldateien laden (Kapitel 11): `rpp.yaml`, Quellordner,
//! Abhängigkeiten, die Standardbibliothek. Prüft R-11.2, R-11.13, R-11.15 bis R-11.18.

use crate::yaml::{self, Node};
use crate::Diag;
use rpp_syntax::ast::CompilationUnit;
use rpp_syntax::Span;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectKind {
    Executable,
    Library,
}

#[derive(Debug)]
pub struct Project {
    pub name: String,
    pub kind: ProjectKind,
    pub dir: PathBuf,
    /// direkte Abhängigkeiten (Indizes in `Workspace::projects`); die Standardbibliothek zählt nicht
    pub deps: Vec<usize>,
    pub is_std: bool,
    /// Einzeldatei-Test ohne `rpp.yaml`: keine Prüfung von Paket und Verzeichnis
    pub single_file: bool,
    /// Datei mit `main` laut `rpp.yaml`
    pub main_file: Option<PathBuf>,
    pub yaml: Option<PathBuf>,
}

#[derive(Debug)]
pub struct SourceFile {
    pub path: PathBuf,
    pub project: usize,
    pub unit: CompilationUnit,
    /// Paket laut Verzeichnis (R-11.2); `None` bei Einzeldateien
    pub expected_package: Option<String>,
}

impl SourceFile {
    pub fn package(&self) -> String {
        self.unit.package.as_ref().map(|q| q.to_dotted()).unwrap_or_default()
    }
}

#[derive(Debug, Default)]
pub struct Workspace {
    pub projects: Vec<Project>,
    pub files: Vec<SourceFile>,
    pub diags: Vec<Diag>,
}

const KEYS: &[&str] = &["name", "version", "type", "main", "sources", "dependencies", "registry", "symbols"];
const REQUIRED: &[&str] = &["name", "version", "type", "sources"];

fn host_os() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

fn is_keyword(s: &str) -> bool {
    rpp_syntax::lexer::Keyword::lookup(s).is_some()
}

fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

impl Workspace {
    fn diag(&mut self, file: &Path, line: u32, rule: &'static str, message: impl Into<String>) {
        self.diags.push(Diag::error(file, Span { line, col: 0 }, rule, message));
    }

    /// Lädt einen Test oder ein Programm: eine einzelne `.rpp`-Datei oder ein
    /// Verzeichnis mit `rpp.yaml`. Die Standardbibliothek kommt aus `std_dir`.
    pub fn load(input: &Path, std_dir: &Path) -> Workspace {
        let mut ws = Workspace::default();
        ws.load_std(std_dir);
        if input.is_dir() {
            let mut stack = Vec::new();
            ws.load_project(input, &mut stack, None);
        } else {
            let idx = ws.projects.len();
            ws.projects.push(Project {
                name: input.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
                kind: ProjectKind::Library,
                dir: input.parent().map(Path::to_path_buf).unwrap_or_default(),
                deps: Vec::new(),
                is_std: false,
                single_file: true,
                main_file: None,
                yaml: None,
            });
            ws.parse_file(input, idx, None, &[]);
            let has_main = ws.files.last().is_some_and(|f| {
                f.project == idx && f.unit.decls.iter().any(|d| matches!(d, rpp_syntax::ast::TopDecl::Main(_)))
            });
            if has_main {
                ws.projects[idx].kind = ProjectKind::Executable;
                ws.projects[idx].main_file = Some(input.to_path_buf());
            }
        }
        ws
    }

    fn load_std(&mut self, std_dir: &Path) {
        let idx = self.projects.len();
        self.projects.push(Project {
            name: "rpp".into(),
            kind: ProjectKind::Library,
            dir: std_dir.to_path_buf(),
            deps: Vec::new(),
            is_std: true,
            single_file: false,
            main_file: None,
            yaml: None,
        });
        self.load_sources(std_dir, idx, &[]);
    }

    fn parse_file(&mut self, path: &Path, project: usize, expected: Option<String>, symbols: &[String]) {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                self.diag(path, 1, "R-11.1", format!("Datei nicht lesbar: {e}"));
                return;
            }
        };
        let mut syms: HashSet<String> = ["debug", host_os()].iter().map(|s| s.to_string()).collect();
        syms.extend(symbols.iter().cloned());
        match rpp_syntax::parse(&source, &syms) {
            Ok(unit) => self.files.push(SourceFile { path: path.to_path_buf(), project, unit, expected_package: expected }),
            Err(e) => self.diags.push(Diag::error(path, e.span, "Syntax", e.message)),
        }
    }

    fn load_sources(&mut self, src: &Path, project: usize, symbols: &[String]) {
        let mut files = Vec::new();
        collect_rpp(src, &mut files);
        for f in files {
            let rel = f.parent().and_then(|p| p.strip_prefix(src).ok()).map(Path::to_path_buf).unwrap_or_default();
            let package = rel.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect::<Vec<_>>().join(".");
            self.parse_file(&f, project, Some(package), symbols);
        }
    }

    /// Lädt ein Projekt mit seinen lokalen Abhängigkeiten. `stack` enthält die Projekte,
    /// deren Abhängigkeiten gerade geladen werden (für R-11.16).
    fn load_project(&mut self, dir: &Path, stack: &mut Vec<PathBuf>, expected_name: Option<(&str, &Path, u32)>) -> Option<usize> {
        let dir = canonical(dir);
        if let Some(i) = self.projects.iter().position(|p| !p.is_std && p.dir == dir) {
            return Some(i);
        }
        let yaml_path = dir.join("rpp.yaml");
        let text = match std::fs::read_to_string(&yaml_path) {
            Ok(t) => t,
            Err(_) => {
                if let Some((_, file, line)) = expected_name {
                    self.diag(file, line, "R-11.15", format!("{} enthält keine rpp.yaml", dir.display()));
                }
                return None;
            }
        };
        let doc = match yaml::parse(&text) {
            Ok(d) => d,
            Err(e) => {
                self.diag(&yaml_path, e.line, "R-11.13", e.message);
                return None;
            }
        };
        for (key, node) in doc.entries() {
            if !KEYS.contains(&key.as_str()) {
                self.diag(&yaml_path, node.line, "R-11.13", format!("unbekannter Eintrag '{key}'"));
            }
        }
        for key in REQUIRED {
            if doc.get(key).is_none() {
                self.diag(&yaml_path, 1, "R-11.13", format!("Eintrag '{key}' fehlt"));
            }
        }
        let name_node = doc.get("name");
        let name = name_node.and_then(Node::as_str).unwrap_or("").to_string();
        let name_line = name_node.map_or(1, |n| n.line);
        if name == "rpp" || name.starts_with("rpp.") {
            self.diag(&yaml_path, name_line, "R-11.17", "der Besitzer rpp ist für die Standardbibliothek reserviert");
        } else if name.split('.').any(|p| p.is_empty() || is_keyword(p)) {
            self.diag(&yaml_path, name_line, "R-11.17", format!("ungültiger Name '{name}'"));
        }
        if let Some((want, file, line)) = expected_name {
            if want != name {
                self.diag(file, line, "R-11.15", format!("das Projekt in {} heißt '{name}', nicht '{want}'", dir.display()));
            }
        }
        let kind = match doc.get("type").and_then(Node::as_str) {
            Some("executable") => ProjectKind::Executable,
            Some("library") => ProjectKind::Library,
            other => {
                let line = doc.get("type").map_or(1, |n| n.line);
                if other.is_some() {
                    self.diag(&yaml_path, line, "R-11.13", "type ist executable oder library");
                }
                ProjectKind::Library
            }
        };
        let main_file = doc.get("main").and_then(Node::as_str).map(|m| canonical(&dir.join(m)));
        if kind == ProjectKind::Executable && main_file.is_none() {
            self.diag(&yaml_path, 1, "R-11.13", "ein Programm braucht den Eintrag main");
        }
        let idx = self.projects.len();
        self.projects.push(Project {
            name: name.clone(),
            kind,
            dir: dir.clone(),
            deps: Vec::new(),
            is_std: false,
            single_file: false,
            main_file,
            yaml: Some(yaml_path.clone()),
        });

        stack.push(dir.clone());
        let mut deps = Vec::new();
        if let Some(list) = doc.get("dependencies") {
            for dep in list.as_list() {
                let dep_name = dep.get("name").and_then(Node::as_str).unwrap_or("").to_string();
                if let Some(path) = dep.get("path") {
                    let target = canonical(&dir.join(path.as_str().unwrap_or("")));
                    if stack.contains(&target) {
                        self.diag(&yaml_path, path.line, "R-11.16", format!("Zyklus in den Abhängigkeiten über '{dep_name}'"));
                        continue;
                    }
                    if let Some(d) = self.load_project(&target, stack, Some((&dep_name, &yaml_path, path.line))) {
                        if self.projects[d].kind != ProjectKind::Library {
                            self.diag(&yaml_path, path.line, "R-11.15", format!("'{dep_name}' ist keine Bibliothek"));
                        }
                        deps.push(d);
                    }
                } else if dep.get("version").is_some() {
                    self.diag(&yaml_path, dep.line, "R-11.15", format!(
                        "'{dep_name}': Bibliotheken aus dem Verzeichnis werden von dieser Implementierung noch nicht geladen"));
                } else {
                    self.diag(&yaml_path, dep.line, "R-11.15", "eine Abhängigkeit braucht path oder version");
                }
            }
        }
        stack.pop();
        self.projects[idx].deps = deps;

        let symbols: Vec<String> = doc
            .get("symbols")
            .and_then(|s| s.get("debug"))
            .map(|l| l.as_list().iter().filter_map(Node::as_str).map(str::to_string).collect())
            .unwrap_or_default();
        if let Some(sources) = doc.get("sources") {
            for s in sources.as_list() {
                let src = dir.join(s.as_str().unwrap_or(""));
                if !src.is_dir() {
                    self.diag(&yaml_path, s.line, "R-11.13", format!("Quellordner {} fehlt", src.display()));
                    continue;
                }
                self.load_sources(&canonical(&src), idx, &symbols);
            }
        }
        Some(idx)
    }
}

fn collect_rpp(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            if !p.join("rpp.yaml").exists() {
                collect_rpp(&p, out);
            }
        } else if p.extension().is_some_and(|e| e == "rpp") {
            out.push(canonical(&p));
        }
    }
}
