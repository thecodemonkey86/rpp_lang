//! Projekte und Quelldateien laden (Kapitel 11): `rpp.yaml`, Quellordner,
//! Abhängigkeiten, die Standardbibliothek. Prüft R-11.2, R-11.13, R-11.15 bis R-11.18.

use crate::yaml::{self, Node, Value};
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
    /// Releasemodus: Symbol `release` statt `debug` (R-11.24)
    pub release: bool,
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
    std::fs::canonicalize(p).unwrap_or_else(|_| normalize(p))
}

/// Absoluter Pfad ohne `.` und `..`, rein textuell (ohne Verweise aufzulösen).
pub fn normalize(p: &Path) -> PathBuf {
    use std::path::Component;
    let abs = std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf());
    let mut out = PathBuf::new();
    for c in abs.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(out.components().next_back(), Some(Component::Normal(_))) {
                    out.pop();
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Pfad in der Form der Ausgaben: `/` als Trenner, ohne das Windows-Präfix `\\?\`.
pub fn slash_path(p: &Path) -> String {
    let s = p.display().to_string();
    if let Some(unc) = s.strip_prefix(r"\\?\UNC\") {
        return format!("//{}", unc.replace('\\', "/"));
    }
    s.strip_prefix(r"\\?\").unwrap_or(&s).replace('\\', "/")
}

/// Normalisiert einen R++-Pfad rein lexikalisch, ohne Arbeitsverzeichnis oder Dateizugriff.
pub fn lexical_path(text: &str) -> PathBuf {
    use std::path::Component;
    let text = text.replace('\\', "/");
    let mut path = PathBuf::new();
    for component in Path::new(&text).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir if matches!(path.components().next_back(), Some(Component::Normal(_))) => {
                path.pop();
            }
            Component::ParentDir if path.has_root() => {}
            other => path.push(other.as_os_str()),
        }
    }
    path
}

pub fn relative_to(target: &str, base: &str) -> Option<String> {
    use std::path::Component;
    let target = lexical_path(target);
    let base = lexical_path(base);
    let tail = target.strip_prefix(base).ok()?;
    // Auch bei leerer Basis muss das Ergebnis innerhalb der Basis liegen.
    if tail.components().any(|c| !matches!(c, Component::Normal(_))) {
        return None;
    }
    Some(if tail.as_os_str().is_empty() { ".".into() } else { slash_path(tail) })
}

impl Workspace {
    fn diag(&mut self, file: &Path, line: u32, rule: &'static str, message: impl Into<String>) {
        self.diags.push(Diag::error(file, Span { line, col: 0 }, rule, message));
    }

    /// Lädt einen Test oder ein Programm: eine einzelne `.rpp`-Datei oder ein
    /// Verzeichnis mit `rpp.yaml`. Die Standardbibliothek kommt aus `std_dir`.
    pub fn load(input: &Path, std_dir: &Path) -> Workspace {
        Workspace::load_mode(input, std_dir, false)
    }

    /// Wie `load`, im Debug- oder Releasemodus (bedingte Kompilierung, R-11.24).
    pub fn load_mode(input: &Path, std_dir: &Path, release: bool) -> Workspace {
        let mut ws = Workspace { release, ..Workspace::default() };
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
            dir: canonical(std_dir),
            deps: Vec::new(),
            is_std: true,
            single_file: false,
            main_file: None,
            yaml: None,
        });
        self.load_sources(&canonical(std_dir), idx, &[]);
    }

    fn parse_file(&mut self, path: &Path, project: usize, expected: Option<String>, symbols: &[String]) {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                self.diag(path, 1, "R-11.1", format!("Datei nicht lesbar: {e}"));
                return;
            }
        };
        let mode = if self.release { "release" } else { "debug" };
        let mut syms: HashSet<String> = [mode, host_os()].iter().map(|s| s.to_string()).collect();
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
                    self.diag(file, line, "R-11.15", format!("{} enthält keine rpp.yaml", slash_path(&dir)));
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
                self.diag(file, line, "R-11.15", format!("das Projekt in {} heißt '{name}', nicht '{want}'", slash_path(&dir)));
            }
        }
        let kind = match doc.get("type").and_then(Node::as_str) {
            Some("executable") => ProjectKind::Executable,
            Some("library") => ProjectKind::Library,
            _ => {
                if let Some(t) = doc.get("type") {
                    self.diag(&yaml_path, t.line, "R-11.13", "type ist executable oder library");
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
            if !is_list(list) {
                self.diag(&yaml_path, list.line, "R-11.13", "dependencies ist eine Liste");
            }
            for dep in list.as_list() {
                let dep_name = dep.get("name").and_then(Node::as_str).unwrap_or("").to_string();
                if let Some(path) = dep.get("path") {
                    let Some(rel) = path.as_str() else {
                        self.diag(&yaml_path, path.line, "R-11.15", "path ist ein Pfad");
                        continue;
                    };
                    let target = canonical(&dir.join(rel));
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

        let mode = if self.release { "release" } else { "debug" };
        let mut symbols: Vec<String> = Vec::new();
        if let Some(syms) = doc.get("symbols") {
            if !matches!(syms.value, Value::Map(_)) {
                self.diag(&yaml_path, syms.line, "R-11.13", "symbols ordnet den Modi debug und release Listen zu");
            }
            for (key, list) in syms.entries() {
                if key != "debug" && key != "release" {
                    self.diag(&yaml_path, list.line, "R-11.13", format!("unbekannter Modus '{key}'"));
                } else if !is_list(list) || list.as_list().iter().any(|s| s.as_str().is_none()) {
                    self.diag(&yaml_path, list.line, "R-11.13", format!("{key} ist eine Liste von Symbolen"));
                } else if key == mode {
                    symbols = list.as_list().iter().filter_map(Node::as_str).map(str::to_string).collect();
                }
            }
        }
        if let Some(sources) = doc.get("sources") {
            if !is_list(sources) {
                self.diag(&yaml_path, sources.line, "R-11.13", "sources ist eine Liste von Quellordnern");
            }
            for s in sources.as_list() {
                let Some(rel) = s.as_str() else {
                    self.diag(&yaml_path, s.line, "R-11.13", "ein Quellordner ist ein Pfad");
                    continue;
                };
                let src = dir.join(rel);
                if !src.is_dir() {
                    self.diag(&yaml_path, s.line, "R-11.13", format!("Quellordner {} fehlt", slash_path(&src)));
                    continue;
                }
                self.load_sources(&canonical(&src), idx, &symbols);
            }
        }
        Some(idx)
    }
}

fn is_list(n: &Node) -> bool {
    matches!(n.value, Value::List(_))
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
