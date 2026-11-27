//! Projekte und Quelldateien laden (Kapitel 11): `rpp.yaml`, Quellordner,
//! Abhängigkeiten, die Standardbibliothek. Prüft R-11.2, R-11.13, R-11.15 bis R-11.20
//! und R-11.26 bis R-11.30 (Abhängigkeiten aus dem Verzeichnis, `rpp.lock`).

use crate::registry::{self, LockEntry, Version};
use crate::yaml::{self, Node, Value};
use crate::Diag;
use rpp_syntax::ast::CompilationUnit;
use rpp_syntax::Span;
use std::collections::{HashMap, HashSet};
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
    /// gewählte Version einer Bibliothek aus dem Verzeichnis (R-11.19)
    pub registry_version: Option<Version>,
    /// `native: true` (R-15.13): das Projekt darf extern-Klassen deklarieren
    pub native: bool,
    /// `nativePaths` (R-15.4): Ordner, in denen C-Bibliotheken zuerst gesucht werden
    pub native_paths: Vec<PathBuf>,
    /// je Eintrag in `deps`: Zeile der Abhängigkeit und ob `allowNative: true` gesetzt ist (R-15.14)
    pub dep_lines: Vec<(u32, bool)>,
    /// bringt C-Code mit (R-15.14, V-15.8): native oder eine Abhängigkeit mit C-Code
    pub brings_c: bool,
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
    /// Bibliotheken aus dem Verzeichnis mit Version und Prüfsumme, nach Namen sortiert
    /// (Name, Version, Prüfsumme, native nach R-15.15)
    pub resolved: Vec<(String, Version, String, bool)>,
    /// Anforderungen `version` aus dem Programm und seinen lokalen Abhängigkeiten
    reqs: Vec<Req>,
    /// Plätze in `Project::deps`, die auf eine Bibliothek aus dem Verzeichnis zeigen
    links: Vec<Link>,
    /// `registry` aus der `rpp.yaml` des Programms: Datei, Zeile, Pfad (None: kein Text)
    registry_entry: Option<(PathBuf, u32, Option<String>)>,
    /// lädt gerade eine Bibliothek aus dem Verzeichnis
    in_registry: bool,
    expect_version: Option<Version>,
    /// `lock`: ohne vorhandene Sperrdatei auflösen (R-11.30)
    ignore_lock: bool,
}

/// Eine Abhängigkeit mit `version` (R-11.15, R-11.19)
#[derive(Debug, Clone)]
struct Req {
    yaml: PathBuf,
    line: u32,
    name: String,
    min: Version,
}

#[derive(Debug)]
struct Link {
    project: usize,
    slot: usize,
    name: String,
    yaml: PathBuf,
    line: u32,
}

/// Platzhalter in `Project::deps`, bis die Bibliothek aus dem Verzeichnis geladen ist
const PENDING: usize = usize::MAX;

const KEYS: &[&str] = &["name", "version", "type", "main", "sources", "dependencies", "registry", "symbols", "native", "nativePaths"];
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
        Workspace::load_with(input, std_dir, release, false)
    }

    /// Für den Befehl `lock`: löst ohne die vorhandene `rpp.lock` auf (R-11.30).
    pub fn load_for_lock(input: &Path, std_dir: &Path) -> Workspace {
        Workspace::load_with(input, std_dir, false, true)
    }

    fn load_with(input: &Path, std_dir: &Path, release: bool, ignore_lock: bool) -> Workspace {
        let mut ws = Workspace { release, ignore_lock, ..Workspace::default() };
        ws.load_std(std_dir);
        if input.is_dir() {
            let mut stack = Vec::new();
            ws.load_project(input, &mut stack, None);
            ws.resolve();
            ws.check_native();
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
                registry_version: None,
                native: false,
                native_paths: Vec::new(),
                dep_lines: Vec::new(),
                brings_c: false,
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
            registry_version: None,
            native: false,
            native_paths: Vec::new(),
            dep_lines: Vec::new(),
            brings_c: false,
        });
        self.load_sources(&canonical(std_dir), idx, &[]);
    }

    fn parse_file(&mut self, path: &Path, project: usize, expected: Option<String>, symbols: &[String]) {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                self.diag(path, 1, "R-11.1", format!("Could not read file: {e}"));
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
                    self.diag(file, line, "R-11.15", format!("{} does not contain rpp.yaml", slash_path(&dir)));
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
                self.diag(&yaml_path, node.line, "R-11.13", format!("Unknown entry '{key}'"));
            }
        }
        for key in REQUIRED {
            if doc.get(key).is_none() {
                self.diag(&yaml_path, 1, "R-11.13", format!("Missing entry '{key}'"));
            }
        }
        if let Some(v) = doc.get("version") {
            match v.as_str().and_then(Version::parse) {
                None => self.diag(&yaml_path, v.line, "R-11.27", "version must have the form MAJOR.MINOR.PATCH"),
                Some(found) => {
                    if let Some(want) = self.expect_version.filter(|w| *w != found) {
                        self.diag(&yaml_path, v.line, "R-11.26", format!(
                            "The library in {} has version {found}, expected {want}", slash_path(&dir)));
                    }
                }
            }
        }
        let name_node = doc.get("name");
        let name = name_node.and_then(Node::as_str).unwrap_or("").to_string();
        let name_line = name_node.map_or(1, |n| n.line);
        if name == "rpp" || name.starts_with("rpp.") {
            self.diag(&yaml_path, name_line, "R-11.17", "The owner name rpp is reserved for the standard library");
        } else if name.split('.').any(|p| p.is_empty() || is_keyword(p)) {
            self.diag(&yaml_path, name_line, "R-11.17", format!("Invalid name '{name}'"));
        }
        if let Some((want, file, line)) = expected_name {
            if want != name {
                self.diag(file, line, "R-11.15", format!("The project in {} is named '{name}', expected '{want}'", slash_path(&dir)));
            }
        }
        let kind = match doc.get("type").and_then(Node::as_str) {
            Some("executable") => ProjectKind::Executable,
            Some("library") => ProjectKind::Library,
            _ => {
                if let Some(t) = doc.get("type") {
                    self.diag(&yaml_path, t.line, "R-11.13", "type must be executable or library");
                }
                ProjectKind::Library
            }
        };
        let main_file = doc.get("main").and_then(Node::as_str).map(|m| canonical(&dir.join(m)));
        if kind == ProjectKind::Executable && main_file.is_none() {
            self.diag(&yaml_path, 1, "R-11.13", "An executable requires a main entry");
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
            registry_version: None,
            native: false,
            native_paths: Vec::new(),
            dep_lines: Vec::new(),
            brings_c: false,
        });
        // R-15.13: native, R-15.4: nativePaths
        if let Some(n) = doc.get("native") {
            match n.as_str() {
                Some("true") => self.projects[idx].native = true,
                Some("false") => {}
                _ => self.diag(&yaml_path, n.line, "R-11.13", "native must be true or false"),
            }
        }
        if let Some(list) = doc.get("nativePaths") {
            if !is_list(list) {
                self.diag(&yaml_path, list.line, "R-11.13", "nativePaths must be a list of directories");
            }
            for p in list.as_list() {
                match p.as_str() {
                    Some(rel) => {
                        let path = normalize(&dir.join(rel));
                        self.projects[idx].native_paths.push(path);
                    }
                    None => self.diag(&yaml_path, p.line, "R-11.13", "An entry of nativePaths must be a path"),
                }
            }
        }
        if idx == 1 && !self.in_registry {
            self.registry_entry =
                doc.get("registry").map(|r| (yaml_path.clone(), r.line, r.as_str().map(str::to_string)));
        }

        stack.push(dir.clone());
        let mut deps = Vec::new();
        let mut dep_lines = Vec::new();
        if let Some(list) = doc.get("dependencies") {
            if !is_list(list) {
                self.diag(&yaml_path, list.line, "R-11.13", "dependencies must be a list");
            }
            for dep in list.as_list() {
                let dep_name = dep.get("name").and_then(Node::as_str).unwrap_or("").to_string();
                let allow = match dep.get("allowNative") {
                    None => false,
                    Some(a) => match a.as_str() {
                        Some("true") => true,
                        Some("false") => false,
                        _ => {
                            self.diag(&yaml_path, a.line, "R-11.13", "allowNative must be true or false");
                            false
                        }
                    },
                };
                let before = deps.len();
                if self.in_registry && dep.get("path").is_some() {
                    // schon bei der Auflösung gemeldet (R-11.27)
                    continue;
                }
                if let Some(path) = dep.get("path") {
                    let Some(rel) = path.as_str() else {
                        self.diag(&yaml_path, path.line, "R-11.15", "path must be a file path");
                        continue;
                    };
                    let target = canonical(&dir.join(rel));
                    if stack.contains(&target) {
                        self.diag(&yaml_path, path.line, "R-11.16", format!("Dependency cycle through '{dep_name}'"));
                        continue;
                    }
                    if let Some(d) = self.load_project(&target, stack, Some((&dep_name, &yaml_path, path.line))) {
                        if self.projects[d].kind != ProjectKind::Library {
                            self.diag(&yaml_path, path.line, "R-11.15", format!("'{dep_name}' is not a library"));
                        }
                        deps.push(d);
                    }
                } else if let Some(v) = dep.get("version") {
                    match v.as_str().and_then(Version::parse_min) {
                        Some(min) => {
                            if !self.in_registry {
                                self.reqs.push(Req { yaml: yaml_path.clone(), line: v.line, name: dep_name.clone(), min });
                            }
                            self.links.push(Link { project: idx, slot: deps.len(), name: dep_name, yaml: yaml_path.clone(), line: v.line });
                            deps.push(PENDING);
                        }
                        // im Verzeichnis schon bei der Auflösung gemeldet
                        None if self.in_registry => {}
                        None => self.diag(&yaml_path, v.line, "R-11.27", format!("Invalid version in dependency '{dep_name}'")),
                    }
                } else {
                    self.diag(&yaml_path, dep.line, "R-11.15", "A dependency requires path or version");
                }
                if deps.len() > before {
                    dep_lines.push((dep.line, allow));
                }
            }
        }
        stack.pop();
        self.projects[idx].deps = deps;
        self.projects[idx].dep_lines = dep_lines;

        let mode = if self.release { "release" } else { "debug" };
        let mut symbols: Vec<String> = Vec::new();
        if let Some(syms) = doc.get("symbols") {
            if !matches!(syms.value, Value::Map(_)) {
                self.diag(&yaml_path, syms.line, "R-11.13", "symbols must map debug and release to lists");
            }
            for (key, list) in syms.entries() {
                if key != "debug" && key != "release" {
                    self.diag(&yaml_path, list.line, "R-11.13", format!("Unknown mode '{key}'"));
                } else if !is_list(list) || list.as_list().iter().any(|s| s.as_str().is_none()) {
                    self.diag(&yaml_path, list.line, "R-11.13", format!("{key} must be a list of symbols"));
                } else if key == mode {
                    symbols = list.as_list().iter().filter_map(Node::as_str).map(str::to_string).collect();
                }
            }
        }
        if let Some(sources) = doc.get("sources") {
            if !is_list(sources) {
                self.diag(&yaml_path, sources.line, "R-11.13", "sources must be a list of source directories");
            }
            for s in sources.as_list() {
                let Some(rel) = s.as_str() else {
                    self.diag(&yaml_path, s.line, "R-11.13", "A source directory must be a path");
                    continue;
                };
                let src = dir.join(rel);
                if !src.is_dir() {
                    self.diag(&yaml_path, s.line, "R-11.13", format!("Source directory {} is missing", slash_path(&src)));
                    continue;
                }
                self.load_sources(&canonical(&src), idx, &symbols);
            }
        }
        Some(idx)
    }
}

impl Workspace {
    /// Das Verzeichnis des Programms (R-11.26); meldet einen Fehler, wenn es fehlt.
    fn registry_dir(&mut self, req: &Req) -> Option<PathBuf> {
        match self.registry_entry.clone() {
            Some((yaml, line, Some(text))) => {
                let d = self.projects[1].dir.join(&text);
                if d.is_dir() {
                    return Some(canonical(&d));
                }
                self.diag(&yaml, line, "R-11.26", format!("Registry directory {} does not exist", slash_path(&canonical(&d))));
                None
            }
            Some((yaml, line, None)) => {
                self.diag(&yaml, line, "R-11.26", "registry must be a directory path");
                None
            }
            None => match std::env::var("RPP_REGISTRY") {
                Ok(v) if !v.is_empty() => {
                    let d = PathBuf::from(&v);
                    if d.is_dir() {
                        return Some(canonical(&d));
                    }
                    self.diag(&req.yaml, req.line, "R-11.26", format!(
                        "Registry directory {} (RPP_REGISTRY) does not exist", slash_path(&canonical(&d))));
                    None
                }
                _ => {
                    self.diag(&req.yaml, req.line, "R-11.26",
                        "No registry: set registry in rpp.yaml or the environment variable RPP_REGISTRY");
                    None
                }
            },
        }
    }

    /// Wählt die Bibliotheken aus dem Verzeichnis (R-11.19, R-11.28), prüft `rpp.lock`
    /// (R-11.20, R-11.30), lädt die gewählten Versionen und verknüpft die Abhängigkeiten.
    fn resolve(&mut self) {
        if self.projects.len() < 2 {
            return;
        }
        let lock_path = self.projects[1].dir.join("rpp.lock");
        let mut lock: Option<Vec<LockEntry>> = None;
        if !self.ignore_lock && lock_path.is_file() {
            match std::fs::read_to_string(&lock_path) {
                Ok(text) => match registry::parse_lock(&text) {
                    Ok(entries) => lock = Some(entries),
                    Err((line, message)) => {
                        self.diag(&lock_path, line, "R-11.20", message);
                        self.drop_pending();
                        return;
                    }
                },
                Err(e) => {
                    self.diag(&lock_path, 1, "R-11.20", format!("Could not read rpp.lock: {e}"));
                    self.drop_pending();
                    return;
                }
            }
        }
        let mut queue = std::mem::take(&mut self.reqs);
        let local: HashSet<String> = self.projects.iter().filter(|p| !p.is_std).map(|p| p.name.clone()).collect();
        let mut reg: Option<PathBuf> = None;
        let mut reg_failed = false;
        let mut major: HashMap<String, u64> = HashMap::new();
        let mut first: HashMap<String, (PathBuf, u32)> = HashMap::new();
        let mut order: Vec<String> = Vec::new();
        let mut selected: HashMap<String, Version> = HashMap::new();
        let mut visited: HashSet<(String, Version)> = HashSet::new();
        let mut failed: HashSet<(String, Version)> = HashSet::new();
        // Bibliotheken, die irgendeine Anforderung nennt (auch wenn sie scheitert)
        let mut requested: HashSet<String> = HashSet::new();
        let mut qi = 0;
        while qi < queue.len() {
            let req = queue[qi].clone();
            qi += 1;
            if reg.is_none() {
                match self.registry_dir(&req) {
                    Some(d) => reg = Some(d),
                    None => {
                        reg_failed = true;
                        break;
                    }
                }
            }
            let reg_dir = reg.clone().unwrap();
            let name = req.name.clone();
            if name == "rpp" || name.starts_with("rpp.") {
                self.diag(&req.yaml, req.line, "R-11.17", "The owner name rpp is reserved for the standard library");
                continue;
            }
            if !registry::valid_registry_name(&name) {
                self.diag(&req.yaml, req.line, "R-11.17", format!("'{name}': a library from the registry is named owner.name"));
                continue;
            }
            if local.contains(&name) {
                self.diag(&req.yaml, req.line, "R-11.15", format!("'{name}' is both a local dependency and one from the registry"));
                continue;
            }
            requested.insert(name.clone());
            match major.get(&name) {
                Some(&m) if m != req.min.0 => {
                    self.diag(&req.yaml, req.line, "R-11.19", format!("'{name}' is required in major versions {m} and {}", req.min.0));
                    continue;
                }
                Some(_) => {}
                None => {
                    major.insert(name.clone(), req.min.0);
                }
            }
            let lib_dir = reg_dir.join(&name);
            let node = if let Some(entries) = &lock {
                let Some(e) = entries.iter().find(|e| e.name == name) else {
                    self.diag(&req.yaml, req.line, "R-11.20", format!("rpp.lock does not list '{name}'; run the lock command"));
                    continue;
                };
                let v = e.version;
                if v.0 != req.min.0 || v < req.min {
                    self.diag(&req.yaml, req.line, "R-11.20", format!(
                        "rpp.lock is out of date: '{name}' {v} does not satisfy {}; run the lock command", req.min));
                    continue;
                }
                if !lib_dir.join(v.to_string()).is_dir() {
                    self.diag(&req.yaml, req.line, "R-11.26", format!("'{name}' {v} is not in the registry"));
                    continue;
                }
                v
            } else {
                if !lib_dir.is_dir() {
                    self.diag(&req.yaml, req.line, "R-11.26", format!("Library '{name}' is not in the registry"));
                    continue;
                }
                let found = registry::versions(&lib_dir).into_iter().find(|(v, withdrawn)| !withdrawn && v.0 == req.min.0 && *v >= req.min);
                let Some((v, _)) = found else {
                    self.diag(&req.yaml, req.line, "R-11.19", format!(
                        "No version of '{name}' satisfies {} (same major version, not withdrawn)", req.min));
                    continue;
                };
                v
            };
            if !first.contains_key(&name) {
                first.insert(name.clone(), (req.yaml.clone(), req.line));
                order.push(name.clone());
            }
            let sel = selected.entry(name.clone()).or_insert(node);
            if node > *sel {
                *sel = node;
            }
            if !visited.insert((name.clone(), node)) {
                continue;
            }
            let node_dir = canonical(&lib_dir.join(node.to_string()));
            let yaml_path = node_dir.join("rpp.yaml");
            let Ok(text) = std::fs::read_to_string(&yaml_path) else {
                self.diag(&req.yaml, req.line, "R-11.26", format!("{} does not contain rpp.yaml", slash_path(&node_dir)));
                failed.insert((name, node));
                continue;
            };
            let doc = match yaml::parse(&text) {
                Ok(d) => d,
                Err(e) => {
                    self.diag(&yaml_path, e.line, "R-11.13", e.message);
                    failed.insert((name, node));
                    continue;
                }
            };
            if let Some(list) = doc.get("dependencies") {
                for dep in list.as_list() {
                    let dep_name = dep.get("name").and_then(Node::as_str).unwrap_or("").to_string();
                    if let Some(p) = dep.get("path") {
                        self.diag(&yaml_path, p.line, "R-11.27", "A library from the registry cannot have path dependencies");
                    } else if let Some(v) = dep.get("version") {
                        match v.as_str().and_then(Version::parse_min) {
                            Some(min) => queue.push(Req { yaml: yaml_path.clone(), line: v.line, name: dep_name, min }),
                            None => self.diag(&yaml_path, v.line, "R-11.27", format!("Invalid version in dependency '{dep_name}'")),
                        }
                    }
                }
            }
        }
        if let (Some(entries), false) = (&lock, reg_failed) {
            for e in entries {
                if !requested.contains(&e.name) {
                    self.diag(&lock_path, e.line, "R-11.20", format!(
                        "rpp.lock lists '{}', which is not needed; run the lock command", e.name));
                }
            }
        }
        let Some(reg_dir) = reg else {
            self.drop_pending();
            return;
        };
        let mut index: HashMap<String, usize> = HashMap::new();
        for name in &order {
            let v = selected[name];
            if failed.contains(&(name.clone(), v)) {
                continue;
            }
            let dir = reg_dir.join(name).join(v.to_string());
            let (fy, fl) = first[name].clone();
            self.in_registry = true;
            self.expect_version = Some(v);
            let mut stack = Vec::new();
            let loaded = self.load_project(&dir, &mut stack, Some((name, &fy, fl)));
            self.in_registry = false;
            self.expect_version = None;
            let Some(i) = loaded else { continue };
            self.projects[i].registry_version = Some(v);
            index.insert(name.clone(), i);
            let pdir = self.projects[i].dir.clone();
            match registry::checksum(&pdir) {
                Ok(sum) => {
                    let native = self.projects[i].native;
                    if let Some(e) = lock.as_ref().and_then(|l| l.iter().find(|e| &e.name == name)) {
                        if e.checksum != sum {
                            self.diag(&lock_path, e.checksum_line, "R-11.20", format!(
                                "The checksum of '{name}' {v} does not match rpp.lock: the library has changed"));
                        } else if e.native != native {
                            self.diag(&lock_path, e.line, "R-15.15", format!(
                                "rpp.lock and '{name}' {v} disagree about native; run the lock command"));
                        }
                    }
                    self.resolved.push((name.clone(), v, sum, native));
                }
                Err(e) => self.diag(&pdir.join("rpp.yaml"), 1, "R-11.29", format!("Could not compute the checksum: {e}")),
            }
        }
        self.resolved.sort_by(|a, b| a.0.cmp(&b.0));
        let links = std::mem::take(&mut self.links);
        for l in &links {
            if let Some(&d) = index.get(&l.name) {
                self.projects[l.project].deps[l.slot] = d;
                if self.projects[d].kind != ProjectKind::Library {
                    self.diag(&l.yaml, l.line, "R-11.15", format!("'{}' is not a library", l.name));
                }
            }
        }
        // R-11.16 im Verzeichnis: Zyklen über Abhängigkeiten mit version
        let mut state: HashMap<usize, u8> = HashMap::new();
        for name in &order {
            if let Some(&p) = index.get(name) {
                self.cycles(p, &links, &mut state);
            }
        }
        self.drop_pending();
    }

    fn cycles(&mut self, p: usize, links: &[Link], state: &mut HashMap<usize, u8>) {
        if state.get(&p).copied().unwrap_or(0) != 0 {
            return;
        }
        state.insert(p, 1);
        for slot in 0..self.projects[p].deps.len() {
            let d = self.projects[p].deps[slot];
            if d == PENDING || self.projects[d].registry_version.is_none() {
                continue;
            }
            match state.get(&d).copied().unwrap_or(0) {
                1 => {
                    if let Some(l) = links.iter().find(|l| l.project == p && l.slot == slot) {
                        let (yaml, line, name) = (l.yaml.clone(), l.line, l.name.clone());
                        self.diag(&yaml, line, "R-11.16", format!("Dependency cycle through '{name}'"));
                    }
                    self.projects[p].deps[slot] = PENDING;
                }
                0 => self.cycles(d, links, state),
                _ => {}
            }
        }
        state.insert(p, 2);
    }

    /// R-15.14, V-15.8: Welche Projekte bringen C-Code mit, und hat jede Abhängigkeit auf
    /// ein solches Projekt `allowNative: true`?
    fn check_native(&mut self) {
        let n = self.projects.len();
        // Abhängigkeiten zeigen auf Projekte, die früher oder später geladen wurden; bis zum Fixpunkt rechnen.
        let mut source: Vec<Option<usize>> = (0..n).map(|p| self.projects[p].native.then_some(p)).collect();
        loop {
            let mut changed = false;
            for p in 0..n {
                if source[p].is_some() {
                    continue;
                }
                if let Some(s) = self.projects[p].deps.iter().find_map(|&d| source.get(d).copied().flatten()) {
                    source[p] = Some(s);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        for p in 0..n {
            self.projects[p].brings_c = source[p].is_some();
        }
        for p in 0..n {
            let Some(yaml) = self.projects[p].yaml.clone() else { continue };
            for (k, &d) in self.projects[p].deps.clone().iter().enumerate() {
                let (line, allow) = self.projects[p].dep_lines.get(k).copied().unwrap_or((1, false));
                let name = self.projects[d].name.clone();
                match source[d] {
                    Some(s) if !allow => {
                        let origin = if s == d { String::new() } else { format!(" (through '{}')", self.projects[s].name) };
                        self.diag(&yaml, line, "R-15.14", format!(
                            "'{name}' brings C code{origin}; allow it with allowNative: true"));
                    }
                    None if allow => {
                        self.diags.push(Diag { warning: true, ..Diag::error(&yaml, rpp_syntax::Span { line, col: 0 }, "R-15.14", format!(
                            "allowNative is set, but '{name}' brings no C code")) });
                    }
                    _ => {}
                }
            }
        }
    }

    fn drop_pending(&mut self) {
        self.links.clear();
        for p in &mut self.projects {
            let mut lines = std::mem::take(&mut p.dep_lines).into_iter();
            let mut kept = Vec::new();
            p.deps.retain(|&d| {
                let line = lines.next().unwrap_or((0, false));
                if d != PENDING {
                    kept.push(line);
                }
                d != PENDING
            });
            p.dep_lines = kept;
        }
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
