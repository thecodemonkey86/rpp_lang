//! Angaben der Prüfung für die Ausführung (Meilenstein M4): Was der Interpreter über
//! einen Ausdruck wissen muss, ohne die Prüfung zu wiederholen. Schlüssel ist die
//! Datei und die Nummer des Ausdrucks (`ast::Expr::id`).

use crate::defs::{MethodInfo, PInfo};
use crate::ty::{DefId, Ty};
use rpp_syntax::ast::Primitive;
use rpp_syntax::Span;
use std::collections::{HashMap, HashSet};

pub type Key = (usize, u32);

/// Was ein Aufruf (oder `new`, `E::V(…)`, `T::x`) zur Laufzeit tut.
#[derive(Debug, Clone)]
pub enum Target {
    /// Methode einer Klasse oder eines Mixins; bei Instanzmethoden virtuell nach der Klasse des Objekts
    Method(MethodInfo),
    /// `base.m()` bzw. `M::m()`: genau diese Fassung, nicht virtuell
    Exact(MethodInfo),
    /// Konstruktor (`None`: der implizite parameterlose)
    Ctor(DefId, Option<MethodInfo>),
    /// `new T()` mit einem Typ-Parameter (R-7.24, R-7.26): Klasse erst zur Laufzeit bekannt
    NewParam(crate::ty::ParamId),
    /// `print`, `assert`, `fail`, `take`, `replace`, `same`
    Builtin(String),
    /// Methode eines primitiven Typs, eines Strings oder eines Vektors
    BuiltinMethod(String),
    /// `i32::parse(…)` usw.
    BuiltinStatic(Primitive, String),
    /// Aufruf eines Funktionswerts
    FuncValue,
    /// Enum-Wert mit Daten `E::V(…)` bzw. ohne Daten `E::V`
    Variant(DefId, usize),
    /// statisches Feld `T::x`
    StaticField(DefId, String),
    /// `w.lock()`
    Lock,
}

#[derive(Debug, Clone)]
pub struct CallInfo {
    pub target: Target,
    /// Parameter nach dem Einsetzen der Typargumente (für Umwandlung und Moves)
    pub params: Vec<PInfo>,
}

#[derive(Debug, Default)]
pub struct RuntimeInfo {
    /// statischer Typ jedes Ausdrucks (nach Verengung)
    pub types: HashMap<Key, Ty>,
    /// Zieltyp eines Werts, der gespeichert oder übergeben wird (für Zahlumwandlungen)
    pub stores: HashMap<Key, Ty>,
    /// Ausdrücke, deren Wert an einen Besitzer geht: own-Variablen werden verschoben, shared kopiert
    pub consumes: HashSet<Key>,
    pub calls: HashMap<Key, CallInfo>,
    /// von einem own-Lambda erfasste Variablen
    pub captures: HashMap<Key, Vec<String>>,
    /// Basiskonstruktor je Konstruktor (Datei, Position der Deklaration)
    pub base_ctors: HashMap<(usize, Span), Option<MethodInfo>>,
    /// Typen lokaler Deklarationen (Datei, Position des Namens)
    pub decls: HashMap<(usize, Span), Ty>,
}
