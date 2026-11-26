//! Eingebaute Teile der Standardbibliothek (Meilenstein M5, Kapitel 13).
//!
//! Die Klassen stehen als R++-Quelltext in `std/rpp/…`; Methoden, deren Rumpf dort
//! `fail("eingebaut")` ist, führt der Interpreter hier aus. Ihr Zustand liegt in den
//! Feldern des Objekts (`Obj::slots`) hinter den deklarierten Feldern:
//! - `Map`: je Eintrag ein `Entry`-Objekt (besitzt Schlüssel und Wert), in Einfügereihenfolge;
//! - `Set`: die Elemente in Einfügereihenfolge;
//! - `StringBuilder`: ein String;
//! - `File`: der Pfad.

use super::*;
use std::cmp::Ordering;
use std::io::{BufRead, BufReader};

/// Klassen und Methoden, die eingebaut sind.
const NATIVE_CLASSES: &[(&str, &str)] = &[
    ("rpp.io", "Console"),
    ("rpp.io", "File"),
    ("rpp.io", "TextReader"),
    ("rpp.io", "SaveFile"),
    ("rpp.io", "Dir"),
    ("rpp.io", "Path"),
    ("rpp.text", "StringBuilder"),
    ("rpp.text", "StringView"),
    ("rpp.collections", "Map"),
    ("rpp.collections", "Set"),
    ("rpp.fmt", "Fmt"),
    ("rpp.math", "Math"),
    ("rpp.env", "Env"),
    ("rpp.env", "ElapsedTimer"),
];

fn str_value(s: impl AsRef<str>) -> Value {
    Value::Str(Rc::from(s.as_ref()))
}

fn file_data(value: &Value) -> Vec<u8> {
    let bytes = |xs: &[Value]| xs.iter().map(|x| if let Value::Byte(b) = x { *b } else { 0 }).collect();
    match value {
        Value::Str(s) => s.as_bytes().to_vec(),
        Value::Slice(v, a, b) => bytes(&v.slots.borrow()[*a..*b]),
        Value::Obj(v, _) => bytes(&v.slots.borrow()),
        _ => Vec::new(),
    }
}

/// Pfade verwenden `/` als Trenner (R-13.20).
fn slash(p: &str) -> String {
    p.replace('\\', "/")
}

impl Interp {
    fn string_view(&mut self, method: &str, class: DefId, object: &ORef, args: &[Value]) -> R<Value> {
        let fields = object.slots.borrow();
        let [Value::Str(source), Value::I64(start), Value::I64(end), ..] = fields.as_slice() else {
            return Err(self.internal("ungültige StringView"));
        };
        let text = &source[*start as usize..*end as usize];
        let view = |it: &mut Self, offset: usize, len: usize| {
            let begin = *start + offset as i64;
            let fields = vec![Value::Str(source.clone()), Value::I64(begin), Value::I64(begin + len as i64)];
            Value::Obj(it.raw_object(class, fields), Form::Own)
        };
        let part = || args[0].as_str().unwrap();
        let index = |n: usize| usize::try_from(args[n].as_i128().unwrap()).ok();
        match method {
            "length" => Ok(Value::I64(text.chars().count() as i64)),
            "charAt" => index(0).and_then(|i| text.chars().nth(i)).map(Value::Char)
                .ok_or_else(|| self.fatal("IndexError", "Index is outside the StringView")),
            "slice" => {
                let boundary = |i| text.char_indices().map(|(b, _)| b).chain(std::iter::once(text.len())).nth(i);
                match (index(0).and_then(boundary), index(1).and_then(boundary)) {
                    (Some(a), Some(b)) if a <= b => Ok(view(self, a, b - a)),
                    _ => Err(self.fatal("IndexError", "Range is outside the StringView")),
                }
            }
            "trim" | "trimEnd" => {
                let left = if method == "trim" { text.trim_start() } else { text };
                Ok(view(self, text.len() - left.len(), left.trim_end().len()))
            }
            "startsWith" => Ok(Value::Bool(text.starts_with(part()))),
            "endsWith" => Ok(Value::Bool(text.ends_with(part()))),
            "indexOf" => Ok(text.find(part()).map_or(Value::Null, |b| Value::I64(text[..b].chars().count() as i64))),
            "splitOnce" => match text.split_once(part()) {
                Some((left, right)) => Ok(Value::Tuple(vec![
                    view(self, 0, left.len()),
                    view(self, text.len() - right.len(), right.len()),
                ])),
                None => Ok(Value::Null),
            },
            "split" => {
                let mut parts = Vec::new();
                let mut begin = 0;
                for (end, separator) in text.match_indices(part()) {
                    parts.push(view(self, begin, end - begin));
                    begin = end + separator.len();
                }
                parts.push(view(self, begin, text.len() - begin));
                Ok(Value::Obj(self.new_vec(parts), Form::Own))
            }
            "toString" => Ok(if text.len() == source.len() { Value::Str(source.clone()) } else { str_value(text) }),
            _ => Err(self.internal(format!("StringView.{method}"))),
        }
    }

    /// Voller Name einer eingebauten Methode, sonst `None`.
    pub(crate) fn native_name(&self, m: &MethodInfo, method: &ast::MethodDecl) -> Option<String> {
        if m.owner == usize::MAX {
            return None;
        }
        let def = &self.p.defs[m.owner];
        if !NATIVE_CLASSES.contains(&(def.package.as_str(), def.name.as_str())) {
            return None;
        }
        // Konstruktoren und Methoden mit echtem Rumpf (etwa Entry) laufen normal.
        let body_is_native = method.body.as_ref().is_some_and(|b| {
            matches!(b.stmts.first().map(|s| &s.kind), Some(S::Expr(ast::ExprStmt::Expr(e))) if matches!(&e.kind, E::Call { callee, .. } if matches!(&callee.kind, E::Name(n) if n.name == "fail")))
        });
        body_is_native.then(|| format!("{}.{}.{}", def.package, def.name, m.name))
    }

    fn std_throw(&mut self, class: &str, args: Vec<Value>) -> R<Value> {
        let Some(d) = self.p.std_def(class) else { return Err(self.internal(format!("{class} fehlt"))) };
        let ctor = self.p.defs[d].ctors.iter().find(|c| c.params.len() == args.len()).map(|c| Ctor { ast: c.ast, params: c.params.clone() });
        let o = self.new_object(d, ctor, args)?;
        Err(Flow::Throw(Value::Obj(o, Form::Own), self.trace()))
    }

    fn io_error(&mut self, msg: String, path: &str) -> R<Value> {
        self.std_throw("rpp.core.IOException", vec![str_value(msg), str_value(path)])
    }

    /// Objekt ohne Konstruktor anlegen (für eingebaute Rückgabewerte).
    fn raw_object(&mut self, d: DefId, slots: Vec<Value>) -> ORef {
        let o = Rc::new(Obj { class: Some(d), slots: RefCell::new(slots), rc: Cell::new(0), alive: Cell::new(true), dying: Cell::new(false), lock: Cell::new(0), targs: RefCell::new(Vec::new()), index: Default::default() });
        self.register(&o);
        o
    }

    fn sb_text(o: &ORef) -> String {
        o.slots.borrow().first().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
    }

    /// Hashwert eines Schlüssels: eingebaut für primitive Werte und Strings, sonst `hash()` (R-13.8).
    fn hash_value(&mut self, v: &Value) -> R<i64> {
        if prim_of_value(v).is_some() {
            return Ok(self.prim_mixin("hash", v, &[]).and_then(|r| r.ok()).and_then(|x| x.as_i128()).unwrap_or(0) as i64);
        }
        let Some(d) = v.obj().and_then(|o| o.class) else { return Ok(0) };
        let m = self.p.methods_of(&Ty::Named(d, Vec::new()), "hash").into_iter().find(|m| m.params.is_empty() && m.has_body);
        match m {
            Some(m) => Ok(self.call_method(&m, Some(v.view()), Vec::new())?.as_i128().unwrap_or(0) as i64),
            None => Ok(0),
        }
    }

    /// Schlüssel des Eintrags an Position `i` (Map) bzw. das Element (Set).
    fn key_at(o: &ORef, i: usize, map: bool) -> Value {
        let it = o.slots.borrow()[i].view();
        // Map-Einträge und die geteilten Set-Einträge für nicht kopierbare Werte
        // besitzen den Schlüssel im ersten Feld. Einfache Set-Werte liegen direkt im Slot.
        if map || matches!(it, Value::Obj(..)) {
            it.obj().map(|e| e.slots.borrow()[0].view()).unwrap_or(Value::Null)
        } else {
            it
        }
    }

    /// Hash-Index neu aufbauen (nach dem Entfernen verschieben sich die Positionen).
    fn reindex(&mut self, o: &ORef, map: bool) -> R<()> {
        let n = o.slots.borrow().len();
        let mut idx: HashMap<i64, Vec<usize>> = HashMap::new();
        for i in 0..n {
            let k = Self::key_at(o, i, map);
            let h = self.hash_value(&k)?;
            idx.entry(h).or_default().push(i);
        }
        *o.index.borrow_mut() = idx;
        Ok(())
    }

    /// Position des Eintrags mit dem Schlüssel (Map) bzw. des Elements (Set), über den Hash-Index.
    fn find(&mut self, o: &ORef, key: &Value, map: bool) -> R<Option<usize>> {
        let h = self.hash_value(key)?;
        let cands = o.index.borrow().get(&h).cloned().unwrap_or_default();
        for i in cands {
            let k = Self::key_at(o, i, map);
            if self.equal(&k, key)? {
                return Ok(Some(i));
            }
        }
        Ok(None)
    }

    fn add_to_index(&mut self, o: &ORef, key: &Value, pos: usize) -> R<()> {
        let h = self.hash_value(key)?;
        o.index.borrow_mut().entry(h).or_default().push(pos);
        Ok(())
    }

    pub(crate) fn native(&mut self, name: &str, m: &MethodInfo, this: Option<Value>, mut args: Vec<Value>) -> R<Value> {
        let text = |args: &Vec<Value>, i: usize| args.get(i).map(to_text).unwrap_or_default();
        let o = this.as_ref().and_then(|t| t.obj().cloned());
        let int = |args: &Vec<Value>, i: usize| args.get(i).and_then(Value::as_i128).unwrap_or(0);
        if let Some(method) = name.strip_prefix("rpp.text.StringView.") {
            return self.string_view(method, m.owner, &o.unwrap(), &args);
        }
        match name {
            // ---------------------------------------------------------------- rpp.io
            "rpp.io.Console.write" => {
                self.console.write_stdout(&text(&args, 0));
                Ok(Value::Void)
            }
            "rpp.io.Console.error" => {
                self.console.write_stderr(&format!("{}\n", text(&args, 0)));
                Ok(Value::Void)
            }
            "rpp.io.Console.readLine" => {
                Ok(self.console.read_line().map_or(Value::Null, str_value))
            }
            "rpp.io.File.open" => {
                let path = text(&args, 0);
                if !std::path::Path::new(&path).is_file() {
                    return self.io_error(format!("File not found: {path}"), &path);
                }
                let d = m.owner;
                let f = self.raw_object(d, vec![str_value(&path)]);
                Ok(Value::Obj(f, Form::Own))
            }
            "rpp.io.File.openTextReader" => {
                let path = text(&args, 0);
                let file = match std::fs::File::open(&path) {
                    Ok(file) => file,
                    Err(e) => return self.io_error(format!("{path}: {e}"), &path),
                };
                match file.metadata() {
                    Ok(meta) if meta.is_dir() => return self.io_error(format!("{path}: ist ein Verzeichnis"), &path),
                    Err(e) => return self.io_error(format!("{path}: {e}"), &path),
                    _ => {}
                }
                let Some(class) = self.p.std_def("rpp.io.TextReader") else { return Err(self.internal("TextReader fehlt")) };
                let object = self.raw_object(class, vec![str_value(&path)]);
                self.text_readers.insert(Rc::as_ptr(&object), BufReader::new(file));
                Ok(Value::Obj(object, Form::Own))
            }
            "rpp.io.TextReader.close" => {
                self.text_readers.remove(&Rc::as_ptr(&o.unwrap()));
                Ok(Value::Void)
            }
            "rpp.io.TextReader.readLine" => {
                let object = o.unwrap();
                let Some(reader) = self.text_readers.get_mut(&Rc::as_ptr(&object)) else {
                    return Err(self.internal("TextReader ohne Datei"));
                };
                let mut line = Vec::new();
                match reader.read_until(b'\n', &mut line) {
                    Ok(0) => return Ok(Value::Null),
                    Ok(_) => {}
                    Err(e) => {
                        let path = Self::sb_text(&object);
                        return self.io_error(format!("{path}: {e}"), &path);
                    }
                }
                if line.last() == Some(&b'\n') {
                    line.pop();
                    if line.last() == Some(&b'\r') { line.pop(); }
                }
                match String::from_utf8(line) {
                    Ok(s) => Ok(str_value(s)),
                    Err(_) => self.std_throw("rpp.core.FormatException", vec![str_value(format!("{}: kein gültiges UTF-8", Self::sb_text(&object)))]),
                }
            }
            "rpp.io.File.readAllText" | "rpp.io.File.readAllBytes" => {
                let path = o.as_ref().map(Self::sb_text).unwrap_or_default();
                let bytes = match std::fs::read(&path) {
                    Ok(b) => b,
                    Err(e) => return self.io_error(format!("{path}: {e}"), &path),
                };
                if name.ends_with("Bytes") {
                    let v: Vec<Value> = bytes.into_iter().map(Value::Byte).collect();
                    return Ok(Value::Obj(self.new_vec(v), Form::Own));
                }
                match String::from_utf8(bytes) {
                    Ok(s) => Ok(str_value(s)),
                    Err(_) => self.std_throw("rpp.core.FormatException", vec![str_value(format!("{path}: kein gültiges UTF-8"))]),
                }
            }
            "rpp.io.File.writeText" | "rpp.io.File.writeBytes" => {
                let path = text(&args, 0);
                let data = file_data(&args[1]);
                match std::fs::write(&path, data) {
                    Ok(()) => Ok(Value::Void),
                    Err(e) => self.io_error(format!("{path}: {e}"), &path),
                }
            }
            // ---------------------------------------------------------------- rpp.io Datei-Operationen (R-13.31)
            "rpp.io.File.appendText" => {
                let path = text(&args, 0);
                let data = text(&args, 1);
                let r = std::fs::OpenOptions::new().append(true).create(true).open(&path)
                    .and_then(|mut f| std::io::Write::write_all(&mut f, data.as_bytes()));
                match r {
                    Ok(()) => Ok(Value::Void),
                    Err(e) => self.io_error(format!("{path}: {e}"), &path),
                }
            }
            "rpp.io.File.copyFile" => {
                let from = text(&args, 0);
                let to = text(&args, 1);
                match std::fs::copy(&from, &to) {
                    Ok(n) => Ok(Value::I64(n as i64)),
                    Err(e) => self.io_error(format!("{from}: {e}"), &from),
                }
            }
            "rpp.io.File.moveFile" => {
                let from = text(&args, 0);
                let to = text(&args, 1);
                match std::fs::rename(&from, &to) {
                    Ok(()) => Ok(Value::Void),
                    // Auf Windows schlägt rename fehl, wenn das Ziel existiert: ersetzen.
                    Err(_) if std::path::Path::new(&to).is_file() => {
                        match std::fs::remove_file(&to).and_then(|()| std::fs::rename(&from, &to)) {
                            Ok(()) => Ok(Value::Void),
                            Err(e) => self.io_error(format!("{from}: {e}"), &from),
                        }
                    }
                    Err(e) => self.io_error(format!("{from}: {e}"), &from),
                }
            }
            "rpp.io.File.delete" => {
                let path = text(&args, 0);
                match std::fs::remove_file(&path) {
                    Ok(()) => Ok(Value::Void),
                    Err(e) => self.io_error(format!("{path}: {e}"), &path),
                }
            }
            "rpp.io.File.size" => {
                let path = text(&args, 0);
                match std::fs::metadata(&path) {
                    Ok(md) => Ok(Value::I64(md.len() as i64)),
                    Err(e) => self.io_error(format!("{path}: {e}"), &path),
                }
            }
            "rpp.io.SaveFile.open" => {
                let path = text(&args, 0);
                match save_file::SaveFile::open(std::path::Path::new(&path)) {
                    Ok(save) => {
                        let object = self.raw_object(m.owner, vec![str_value(&path)]);
                        self.save_files.insert(Rc::as_ptr(&object), save);
                        Ok(Value::Obj(object, Form::Own))
                    }
                    Err(e) => self.io_error(format!("{path}: {e}"), &path),
                }
            }
            "rpp.io.SaveFile.writeText" | "rpp.io.SaveFile.writeBytes" | "rpp.io.SaveFile.commit" | "rpp.io.SaveFile.cancel" => {
                let object = o.unwrap();
                let key = Rc::as_ptr(&object);
                if name.ends_with(".cancel") {
                    self.save_files.remove(&key);
                    return Ok(Value::Void);
                }
                let path = Self::sb_text(&object);
                let result = if name.ends_with(".commit") {
                    self.save_files.remove(&key).map(|save| save.commit())
                } else {
                    self.save_files.get_mut(&key).map(|save| save.write(&file_data(&args[0])))
                };
                match result {
                    Some(Ok(())) => Ok(Value::Void),
                    Some(Err(e)) => {
                        self.save_files.remove(&key);
                        self.io_error(format!("{path}: {e}"), &path)
                    }
                    None => self.io_error(format!("{path}: Schreibvorgang ist beendet"), &path),
                }
            }
            "rpp.io.Dir.list" => {
                let path = text(&args, 0);
                let rd = match std::fs::read_dir(&path) {
                    Ok(rd) => rd,
                    Err(e) => return self.io_error(format!("{path}: {e}"), &path),
                };
                // R-13.20: sortiert
                let mut names: Vec<String> = rd.filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().to_string())).collect();
                names.sort();
                let v = names.into_iter().map(str_value).collect();
                Ok(Value::Obj(self.new_vec(v), Form::Own))
            }
            "rpp.io.Dir.entries" => {
                let dir = text(&args, 0);
                let Some(entry_class) = self.p.std_def("rpp.io.DirEntry") else { return Err(self.internal("DirEntry fehlt")) };
                let Some(kind_enum) = self.p.std_def("rpp.io.FileType") else { return Err(self.internal("FileType fehlt")) };
                let read = std::fs::read_dir(&dir).and_then(|rd| rd.map(|entry| {
                    let entry = entry?;
                    let kind = entry.file_type()?;
                    let variant = if kind.is_dir() { "Directory" } else if kind.is_file() { "File" }
                        else if kind.is_symlink() { "Symlink" } else { "Other" };
                    Ok((entry.file_name().to_string_lossy().into_owned(),
                        rpp_sema::project::slash_path(&entry.path()), variant))
                }).collect::<std::io::Result<Vec<_>>>());
                let mut entries = match read {
                    Ok(entries) => entries,
                    Err(e) => return self.io_error(format!("{dir}: {e}"), &dir),
                };
                entries.sort_by(|a, b| a.0.cmp(&b.0));
                let mut values = Vec::with_capacity(entries.len());
                for (name, path, variant) in entries {
                    let Some(i) = self.p.defs[kind_enum].variants.iter().position(|v| v.name == variant) else {
                        return Err(self.internal(format!("FileType.{variant} fehlt")));
                    };
                    let fields = vec![str_value(name), str_value(path), Value::Enum(kind_enum, i, Vec::new())];
                    values.push(Value::Obj(self.raw_object(entry_class, fields), Form::Own));
                }
                Ok(Value::Obj(self.new_vec(values), Form::Own))
            }
            // V-13.8
            "rpp.io.File.exists" => Ok(Value::Bool(std::path::Path::new(&text(&args, 0)).is_file())),
            "rpp.io.Path.absolute" => {
                let p = rpp_sema::project::normalize(std::path::Path::new(&text(&args, 0)));
                Ok(str_value(rpp_sema::project::slash_path(&p)))
            }
            "rpp.io.Path.normalizedText" => {
                let p = rpp_sema::project::lexical_path(&text(&args, 0));
                Ok(str_value(if p.as_os_str().is_empty() { ".".into() } else { rpp_sema::project::slash_path(&p) }))
            }
            "rpp.io.Path.relativeText" => {
                Ok(rpp_sema::project::relative_to(&text(&args, 0), &text(&args, 1)).map(str_value).unwrap_or(Value::Null))
            }
            "rpp.io.Dir.exists" => Ok(Value::Bool(std::path::Path::new(&text(&args, 0)).is_dir())),
            "rpp.io.Dir.create" => {
                let path = text(&args, 0);
                match std::fs::create_dir_all(&path) {
                    Ok(()) => Ok(Value::Void),
                    Err(e) => self.io_error(format!("{path}: {e}"), &path),
                }
            }
            "rpp.io.Path.join" => {
                let (a, b) = (slash(&text(&args, 0)), slash(&text(&args, 1)));
                Ok(str_value(if a.is_empty() {
                    b
                } else if a.ends_with('/') {
                    format!("{a}{b}")
                } else {
                    format!("{a}/{b}")
                }))
            }
            "rpp.io.Path.fileName" => {
                let p = slash(&text(&args, 0));
                Ok(str_value(p.rsplit('/').next().unwrap_or("")))
            }
            "rpp.io.Path.extension" => {
                let p = slash(&text(&args, 0));
                let file = p.rsplit('/').next().unwrap_or("");
                Ok(str_value(match file.rfind('.') {
                    Some(i) if i > 0 => &file[i + 1..],
                    _ => "",
                }))
            }
            "rpp.io.Path.parent" => {
                let p = slash(&text(&args, 0));
                Ok(str_value(match p.rfind('/') {
                    Some(i) => &p[..i],
                    None => "",
                }))
            }
            // ---------------------------------------------------------------- rpp.env
            "rpp.env.Env.get" => Ok(std::env::var(text(&args, 0)).map(str_value).unwrap_or(Value::Null)),
            "rpp.env.ElapsedTimer.monotonicNanos" => {
                static ORIGIN: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
                let elapsed = ORIGIN.get_or_init(std::time::Instant::now).elapsed();
                Ok(Value::I64(i64::try_from(elapsed.as_nanos()).unwrap_or(i64::MAX)))
            }
            "rpp.env.Env.os" => {
                let Some(d) = self.p.std_def("rpp.env.Os") else { return Err(self.internal("Os fehlt")) };
                let name = if cfg!(windows) {
                    "Windows"
                } else if cfg!(target_os = "macos") {
                    "MacOS"
                } else {
                    "Linux"
                };
                let i = self.p.defs[d].variants.iter().position(|v| v.name == name).unwrap_or(0);
                Ok(Value::Enum(d, i, Vec::new()))
            }
            // ---------------------------------------------------------------- rpp.fmt
            "rpp.fmt.Fmt.padLeft" | "rpp.fmt.Fmt.padRight" => {
                let s = text(&args, 0);
                let w = int(&args, 1).max(0) as usize;
                let n = s.chars().count();
                let pad = " ".repeat(w.saturating_sub(n));
                Ok(str_value(if name.ends_with("Left") { format!("{pad}{s}") } else { format!("{s}{pad}") }))
            }
            "rpp.fmt.Fmt.hex" => Ok(str_value(match args.first() {
                Some(Value::U64(x)) => format!("{x:x}"),
                Some(Value::I64(x)) => format!("{x:x}"),
                Some(v) => format!("{:x}", v.as_i128().unwrap_or(0) as i64),
                None => String::new(),
            })),
            "rpp.fmt.Fmt.fixed" => {
                let x = args.first().and_then(Value::as_f64).unwrap_or(0.0);
                let d = int(&args, 1).clamp(0, 300) as usize;
                Ok(str_value(format!("{x:.d$}")))
            }
            // ---------------------------------------------------------------- rpp.math
            "rpp.math.Math.sqrt" => {
                let x = args.first().and_then(Value::as_f64).unwrap_or(0.0);
                Ok(Value::F64(x.sqrt()))
            }
            // ---------------------------------------------------------------- rpp.text
            "rpp.text.StringBuilder.length" => Ok(Value::I64(o.as_ref().map(|o| Self::sb_text(o).chars().count()).unwrap_or(0) as i64)),
            "rpp.text.StringBuilder.toString" => Ok(str_value(o.as_ref().map(Self::sb_text).unwrap_or_default())),
            "rpp.text.StringBuilder.append" | "rpp.text.StringBuilder.appendChar" | "rpp.text.StringBuilder.appendLine" | "rpp.text.StringBuilder.clear" => {
                let Some(o) = o else { return Err(self.internal("StringBuilder ohne Objekt")) };
                let mut s = Self::sb_text(&o);
                match name.rsplit('.').next().unwrap_or("") {
                    "clear" => s.clear(),
                    "appendLine" => {
                        s.push_str(&text(&args, 0));
                        s.push('\n');
                    }
                    _ => s.push_str(&text(&args, 0)),
                }
                let mut slots = o.slots.borrow_mut();
                if slots.is_empty() {
                    slots.push(str_value(s));
                } else {
                    slots[0] = str_value(s);
                }
                Ok(Value::Void)
            }
            // ---------------------------------------------------------------- rpp.collections
            _ if name.starts_with("rpp.collections.") => {
                let Some(o) = o else { return Err(self.internal(format!("{name} ohne Objekt"))) };
                let map = name.starts_with("rpp.collections.Map.");
                let method = name.rsplit('.').next().unwrap_or("");
                let len = o.slots.borrow().len();
                match method {
                    "length" => Ok(Value::I64(len as i64)),
                    "begin" => Ok(Value::I64(0)),
                    "done" => Ok(Value::Bool(int(&args, 0) >= len as i128)),
                    "next" => Ok(Value::I64(int(&args, 0) as i64 + 1)),
                    "at" if !m.mut_this && m.params.first().is_some_and(|p| matches!(p.ty, Ty::Prim(P::I64))) => {
                        let i = int(&args, 0);
                        if i < 0 || i as usize >= len {
                            return Err(self.fatal("IndexError", format!("Cursor {i} is outside 0..{len}")));
                        }
                        Ok(if map { o.slots.borrow()[i as usize].view() } else { Self::key_at(&o, i as usize, false) })
                    }
                    "contains" => Ok(Value::Bool(self.find(&o, &args[0], map)?.is_some())),
                    "getOrPut" => {
                        // R-13.33: Wert zum Schlüssel, sonst factory() einsetzen.
                        let factory = args.pop().unwrap_or(Value::Null);
                        let key = args.pop().unwrap_or(Value::Null);
                        let i = match self.find(&o, &key, true)? {
                            Some(i) => { self.drop_value(key)?; i }
                            None => {
                                let value = match &factory {
                                    Value::Func(c) => self.call_closure(c, vec![])?,
                                    _ => return Err(self.internal("getOrPut ohne Factory")),
                                };
                                let Some(d) = self.p.std_def("rpp.collections.Entry") else { return Err(self.internal("Entry fehlt")) };
                                let pos = o.slots.borrow().len();
                                self.add_to_index(&o, &key, pos)?;
                                let e = self.raw_object(d, vec![key, value]);
                                o.slots.borrow_mut().push(Value::Obj(e, Form::Own));
                                pos
                            }
                        };
                        let e = o.slots.borrow()[i].obj().cloned();
                        Ok(e.map(|e| e.slots.borrow()[1].view()).unwrap_or(Value::Null))
                    }
                    "get" | "at" => {
                        let found = self.find(&o, &args[0], true)?;
                        match found {
                            Some(i) => {
                                let e = o.slots.borrow()[i].obj().cloned();
                                Ok(e.map(|e| e.slots.borrow()[1].view()).unwrap_or(Value::Null))
                            }
                            None if method == "get" => Ok(Value::Null),
                            None => Err(self.fatal("IndexError", "The key is missing from the map")),
                        }
                    }
                    "put" => {
                        let value = args.pop().unwrap_or(Value::Null);
                        let key = args.pop().unwrap_or(Value::Null);
                        match self.find(&o, &key, true)? {
                            // R-13.17: ersetzt den Wert, behält die Position; der alte Wert wird freigegeben
                            Some(i) => {
                                let e = o.slots.borrow()[i].obj().cloned();
                                if let Some(e) = e {
                                    let old = std::mem::replace(&mut e.slots.borrow_mut()[1], value);
                                    self.drop_value(old)?;
                                }
                                self.drop_value(key)?;
                            }
                            None => {
                                let Some(d) = self.p.std_def("rpp.collections.Entry") else { return Err(self.internal("Entry fehlt")) };
                                let pos = o.slots.borrow().len();
                                self.add_to_index(&o, &key, pos)?;
                                let e = self.raw_object(d, vec![key, value]);
                                o.slots.borrow_mut().push(Value::Obj(e, Form::Own));
                            }
                        }
                        Ok(Value::Void)
                    }
                    "add" => {
                        let x = args.pop().unwrap_or(Value::Null);
                        if self.find(&o, &x, false)?.is_some() {
                            self.drop_value(x)?;
                        } else {
                            let pos = o.slots.borrow().len();
                            self.add_to_index(&o, &x, pos)?;
                            let entry = if self.plain_value(&x) {
                                x
                            } else {
                                let holder = self.new_vec(vec![x]);
                                holder.rc.set(1);
                                Value::Obj(holder, Form::Shared)
                            };
                            o.slots.borrow_mut().push(entry);
                        }
                        Ok(Value::Void)
                    }
                    "appendEntry" => {
                        let source = args[0].obj().unwrap();
                        let i = int(&args, 1) as usize;
                        let key = Self::key_at(source, i, false);
                        self.add_to_index(&o, &key, len)?;
                        let entry = self.own_copy(source.slots.borrow()[i].clone());
                        o.slots.borrow_mut().push(entry);
                        Ok(Value::Void)
                    }
                    "remove" => {
                        let found = self.find(&o, &args[0], map)?;
                        let Some(i) = found else { return Ok(if map { Value::Null } else { Value::Void }) };
                        let item = o.slots.borrow_mut().remove(i);
                        self.reindex(&o, map)?;
                        if !map {
                            self.drop_value(item)?;
                            return Ok(Value::Void);
                        }
                        // den Wert herausnehmen, den Eintrag (mit dem Schlüssel) freigeben
                        let value = match item.obj() {
                            Some(e) => std::mem::replace(&mut e.slots.borrow_mut()[1], Value::Moved),
                            None => Value::Null,
                        };
                        self.drop_value(item)?;
                        Ok(value)
                    }
                    "clear" => {
                        o.index.borrow_mut().clear();
                        let items = std::mem::take(&mut *o.slots.borrow_mut());
                        for x in items {
                            self.drop_value(x)?;
                        }
                        Ok(Value::Void)
                    }
                    _ => Err(self.internal(format!("{name} ist nicht eingebaut"))),
                }
            }
            _ => Err(self.internal(format!("{name} ist nicht eingebaut"))),
        }
    }

    /// `hash`, `equals`, `compareTo` und `less` für primitive Werte und Strings (R-13.6 bis R-13.8).
    pub(crate) fn prim_mixin(&mut self, name: &str, v: &Value, args: &[Value]) -> Option<R<Value>> {
        Some(match name {
            "hash" => Ok(Value::I64(match v {
                Value::Str(s) => {
                    // FNV-1a über die UTF-8-Bytes
                    let mut h: u64 = 0xcbf29ce484222325;
                    for b in s.bytes() {
                        h ^= b as u64;
                        h = h.wrapping_mul(0x100000001b3);
                    }
                    h as i64
                }
                Value::Bool(b) => *b as i64,
                Value::Char(c) => *c as i64,
                Value::Byte(b) => *b as i64,
                Value::F32(x) => x.to_bits() as i64,
                Value::F64(x) => x.to_bits() as i64,
                other => other.as_i128().unwrap_or(0) as i64,
            })),
            "equals" => self.equal(v, args.first()?).map(Value::Bool),
            "compareTo" | "less" => self.compare(v, args.first()?).map(|o| {
                if name == "less" {
                    Value::Bool(o == Ordering::Less)
                } else {
                    Value::I32(match o {
                        Ordering::Less => -1,
                        Ordering::Equal => 0,
                        Ordering::Greater => 1,
                    })
                }
            }),
            _ => return None,
        })
    }
}
