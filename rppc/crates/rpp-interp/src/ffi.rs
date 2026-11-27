//! C-Anbindung im Interpreter (Kapitel 15): lädt die C-Bibliotheken eines Programms vor
//! `main` (R-15.12), ruft Methoden von extern-Klassen über libffi auf (R-15.6 bis R-15.11)
//! und gibt Handles mit ihrer Freigabefunktion frei (R-15.10).

use libffi::middle::{Arg, Cif, CodePtr, Type};
use libloading::Library;
use rpp_sema::defs::Program;
use rpp_sema::ty::DefId;
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::PathBuf;

/// Dateiname einer C-Bibliothek auf dieser Plattform (R-15.4).
pub fn file_name(lib: &str) -> String {
    if cfg!(windows) {
        format!("{lib}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{lib}.dylib")
    } else {
        format!("lib{lib}.so")
    }
}

/// Geladene Bibliotheken und aufgelöste Symbole eines Programms.
pub struct Natives {
    _libs: Vec<Library>,
    /// (extern-Klasse, Methode) → Adresse der C-Funktion
    funcs: HashMap<(DefId, String), usize>,
    /// Handle-Typ → Adresse der Freigabefunktion
    frees: HashMap<DefId, usize>,
}

/// Ordner, in denen die Bibliothek einer Deklaration zuerst gesucht wird: `nativePaths` des
/// Projekts mit der Deklaration, dann die des Programms (R-15.4).
fn search_dirs(p: &Program, def: DefId) -> Vec<PathBuf> {
    let mut dirs = p.ws.projects[p.defs[def].project].native_paths.clone();
    if let Some(prog) = p.ws.projects.iter().find(|x| !x.is_std) {
        for d in &prog.native_paths {
            if !dirs.contains(d) {
                dirs.push(d.clone());
            }
        }
    }
    dirs
}

/// Lädt alle Bibliotheken und löst jedes deklarierte Symbol auf (R-15.12, V-15.5).
/// Der Fehler ist die Meldung für `NativeError` bzw. den Fehler beim Bauen, mit der
/// Deklaration, an der er auftrat.
pub fn load(p: &Program) -> Result<Natives, (String, DefId)> {
    let mut libs: Vec<Library> = Vec::new();
    let mut by_name: HashMap<String, usize> = HashMap::new();
    let mut funcs = HashMap::new();
    let mut frees = HashMap::new();
    for (def, d) in p.defs.iter().enumerate() {
        let Some(ext) = &d.ext else { continue };
        let li = match by_name.get(&ext.lib) {
            Some(&i) => i,
            None => {
                let file = file_name(&ext.lib);
                let mut found = None;
                for dir in search_dirs(p, def) {
                    let path = dir.join(&file);
                    if path.is_file() {
                        found = Some(path);
                        break;
                    }
                }
                // SAFETY: Das Laden führt Initialisierungscode der Bibliothek aus; das Projekt
                // hat C-Code ausdrücklich zugelassen (R-15.13, R-15.14, R-15.16).
                let loaded = match &found {
                    Some(path) => unsafe { Library::new(path) },
                    None => unsafe { Library::new(&file) },
                };
                let lib = loaded.map_err(|e| (format!("Cannot load C library \"{}\" ({file}): {e}", ext.lib), def))?;
                libs.push(lib);
                by_name.insert(ext.lib.clone(), libs.len() - 1);
                libs.len() - 1
            }
        };
        let lookup = |name: &str| -> Result<usize, String> {
            let mut sym = name.as_bytes().to_vec();
            sym.push(0);
            // SAFETY: Es wird nur die Adresse gelesen, nicht aufgerufen.
            unsafe {
                libs[li]
                    .get::<*mut c_void>(&sym)
                    .map(|s| *s as usize)
                    .map_err(|_| format!("Symbol {name} not found in C library \"{}\"", ext.lib))
            }
        };
        match &ext.handle_free {
            Some(free) => {
                frees.insert(def, lookup(free).map_err(|e| (e, def))?);
            }
            None => {
                for m in &d.methods {
                    funcs.insert((def, m.name.clone()), lookup(&m.name).map_err(|e| (e, def))?);
                }
            }
        }
    }
    Ok(Natives { _libs: libs, funcs, frees })
}

impl Natives {
    pub fn func(&self, def: DefId, name: &str) -> Option<usize> {
        self.funcs.get(&(def, name.to_string())).copied()
    }

    /// Ruft die Freigabefunktion eines Handle-Typs mit `ptr` auf.
    pub fn free(&self, def: DefId, ptr: usize) {
        let Some(&f) = self.frees.get(&def) else { return };
        let cif = Cif::new([Type::pointer()], Type::void());
        let p = ptr as *mut c_void;
        // SAFETY: Signatur void f(void*) nach R-15.5; der Zeiger stammt aus C (R-15.10).
        unsafe { cif.call::<()>(CodePtr::from_ptr(f as *const c_void), &[Arg::new(&p)]) }
    }
}

/// Ein Argument in seiner C-Form.
pub enum CVal {
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    F32(f32),
    F64(f64),
    Ptr(*mut c_void),
    Size(usize),
}

impl CVal {
    fn ty(&self) -> Type {
        match self {
            CVal::I8(_) => Type::i8(),
            CVal::I16(_) => Type::i16(),
            CVal::I32(_) => Type::i32(),
            CVal::I64(_) => Type::i64(),
            CVal::U8(_) => Type::u8(),
            CVal::U16(_) => Type::u16(),
            CVal::U32(_) => Type::u32(),
            CVal::U64(_) => Type::u64(),
            CVal::F32(_) => Type::f32(),
            CVal::F64(_) => Type::f64(),
            CVal::Ptr(_) => Type::pointer(),
            CVal::Size(_) => Type::usize(),
        }
    }

    fn arg(&self) -> Arg {
        match self {
            CVal::I8(v) => Arg::new(v),
            CVal::I16(v) => Arg::new(v),
            CVal::I32(v) => Arg::new(v),
            CVal::I64(v) => Arg::new(v),
            CVal::U8(v) => Arg::new(v),
            CVal::U16(v) => Arg::new(v),
            CVal::U32(v) => Arg::new(v),
            CVal::U64(v) => Arg::new(v),
            CVal::F32(v) => Arg::new(v),
            CVal::F64(v) => Arg::new(v),
            CVal::Ptr(v) => Arg::new(v),
            CVal::Size(v) => Arg::new(v),
        }
    }
}

/// Art des Rückgabewerts in C.
#[derive(Clone, Copy)]
pub enum CRet {
    Void,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    Ptr,
}

/// Rückgabewert in seiner C-Form.
pub enum CResult {
    Void,
    I(i64),
    U(u64),
    F32(f32),
    F64(f64),
    Ptr(usize),
}

/// Ruft die C-Funktion an `addr` auf.
///
/// # Safety
/// Die Funktion muss die Signatur haben, die sich aus `args` und `ret` ergibt (R-15.16).
pub unsafe fn call(addr: usize, args: &[CVal], ret: CRet) -> CResult {
    let rtype = match ret {
        CRet::Void => Type::void(),
        CRet::I8 => Type::i8(),
        CRet::I16 => Type::i16(),
        CRet::I32 => Type::i32(),
        CRet::I64 => Type::i64(),
        CRet::U8 => Type::u8(),
        CRet::U16 => Type::u16(),
        CRet::U32 => Type::u32(),
        CRet::U64 => Type::u64(),
        CRet::F32 => Type::f32(),
        CRet::F64 => Type::f64(),
        CRet::Ptr => Type::pointer(),
    };
    let cif = Cif::new(args.iter().map(CVal::ty), rtype);
    let argv: Vec<Arg> = args.iter().map(CVal::arg).collect();
    let f = CodePtr::from_ptr(addr as *const c_void);
    // SAFETY: siehe oben
    unsafe {
        match ret {
            CRet::Void => {
                cif.call::<()>(f, &argv);
                CResult::Void
            }
            CRet::I8 => CResult::I(cif.call::<i8>(f, &argv) as i64),
            CRet::I16 => CResult::I(cif.call::<i16>(f, &argv) as i64),
            CRet::I32 => CResult::I(cif.call::<i32>(f, &argv) as i64),
            CRet::I64 => CResult::I(cif.call::<i64>(f, &argv)),
            CRet::U8 => CResult::U(cif.call::<u8>(f, &argv) as u64),
            CRet::U16 => CResult::U(cif.call::<u16>(f, &argv) as u64),
            CRet::U32 => CResult::U(cif.call::<u32>(f, &argv) as u64),
            CRet::U64 => CResult::U(cif.call::<u64>(f, &argv)),
            CRet::F32 => CResult::F32(cif.call::<f32>(f, &argv)),
            CRet::F64 => CResult::F64(cif.call::<f64>(f, &argv)),
            CRet::Ptr => CResult::Ptr(cif.call::<*mut c_void>(f, &argv) as usize),
        }
    }
}
