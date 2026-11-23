//! Laufzeitwerte (Kapitel 5, Abschnitt 5.1, für die Oberflächensprache).
//!
//! Ein Objekt liegt in einem `Rc<Obj>`; die Besitzform steht am Verweis: `Own` (der
//! einzige Besitzer), `Shared` (ein gezählter Verweis), `Weak` und `Borrow` (nicht
//! besitzend). Freigegeben wird nicht von Rust, sondern ausdrücklich durch den
//! Interpreter, wenn der Besitzer den Wert aufgibt (V-7, R-12.11). `rc` zählt die
//! `Shared`-Verweise; `alive` ist falsch, sobald das Objekt freigegeben ist.

use rpp_sema::ty::DefId;
use rpp_syntax::ast;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    Own,
    Shared,
    Weak,
    Borrow,
}

pub struct Obj {
    /// Klasse; `None` bei einem Vektor
    pub class: Option<DefId>,
    /// Felder (geerbte zuerst) bzw. Elemente
    pub slots: RefCell<Vec<Value>>,
    pub rc: Cell<usize>,
    pub alive: Cell<bool>,
    pub dying: Cell<bool>,
    /// Sperre eines shared-Objekts (V-1): 0 frei, n > 0 lesend, -1 schreibend
    pub lock: Cell<i32>,
    /// Typargumente der Klasse (R-7.26: im Interpreter zur Laufzeit erhalten)
    pub targs: RefCell<Vec<rpp_sema::ty::Ty>>,
    /// Hash-Index eingebauter Sammlungen (Map, Set): Hashwert → Positionen
    pub index: RefCell<std::collections::HashMap<i64, Vec<usize>>>,
}

pub type ORef = Rc<Obj>;

/// Variable: eine Zelle, die auch ein normales Lambda erfassen kann.
pub type Cellv = Rc<RefCell<Value>>;

pub struct Closure {
    pub own: bool,
    pub params: Vec<(String, rpp_sema::ty::Mode)>,
    pub body: &'static ast::LambdaBody,
    pub file: usize,
    pub class: Option<DefId>,
    pub env: Vec<(String, Cellv)>,
    pub this: Option<Value>,
    /// Typargumente der generischen Methode, in der das Lambda steht
    pub mtargs: Vec<(usize, rpp_sema::ty::Ty)>,
}

#[derive(Clone)]
pub enum Value {
    Void,
    Null,
    /// verschoben (Drop-Flag gelöscht, V-5)
    Moved,
    /// noch nicht gesetzt (Feld während der Konstruktion)
    Uninit,
    Bool(bool),
    Char(char),
    Byte(u8),
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
    Str(Rc<str>),
    Obj(ORef, Form),
    Tuple(Vec<Value>),
    Enum(DefId, usize, Vec<Value>),
    Func(Rc<Closure>),
    /// Bereich eines Vektors (Borrow)
    Slice(ORef, usize, usize),
}

impl Value {
    pub fn obj(&self) -> Option<&ORef> {
        match self {
            Value::Obj(o, _) | Value::Slice(o, _, _) => Some(o),
            _ => None,
        }
    }

    /// Nicht besitzende Sicht auf denselben Wert.
    pub fn view(&self) -> Value {
        match self {
            Value::Obj(o, Form::Own | Form::Shared) => Value::Obj(o.clone(), Form::Borrow),
            Value::Tuple(vs) => Value::Tuple(vs.iter().map(Value::view).collect()),
            Value::Enum(d, i, vs) => Value::Enum(*d, *i, vs.iter().map(Value::view).collect()),
            other => other.clone(),
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    pub fn as_bool(&self) -> bool {
        matches!(self, Value::Bool(true))
    }

    pub fn as_i128(&self) -> Option<i128> {
        Some(match *self {
            Value::I8(v) => v as i128,
            Value::I16(v) => v as i128,
            Value::I32(v) => v as i128,
            Value::I64(v) => v as i128,
            Value::U8(v) => v as i128,
            Value::U16(v) => v as i128,
            Value::U32(v) => v as i128,
            Value::U64(v) => v as i128,
            _ => return None,
        })
    }

    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            Value::F32(v) => Some(v as f64),
            Value::F64(v) => Some(v),
            _ => self.as_i128().map(|v| v as f64),
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }
}
