//! Angaben der Prüfung für die Ausführung (Meilenstein M4): Was der Interpreter über
//! einen Ausdruck wissen muss, ohne die Prüfung zu wiederholen. Schlüssel ist die
//! Datei und die Nummer des Ausdrucks (`ast::Expr::id`).

use crate::defs::{MethodInfo, PInfo};
use crate::ty::{DefId, Ty};
use rpp_syntax::ast::Primitive;
use rpp_syntax::Span;
use std::collections::HashMap;
use std::num::NonZeroU32;

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
struct ExprEntry {
    ty: Option<NonZeroU32>,
    store: Option<NonZeroU32>,
    call: Option<NonZeroU32>,
    captures: Option<NonZeroU32>,
    consumes: bool,
}

#[derive(Debug, Default)]
struct FileRuntimeInfo {
    expressions: Vec<ExprEntry>,
    types: Vec<Ty>,
    stores: Vec<Ty>,
    calls: Vec<CallInfo>,
    captures: Vec<Vec<String>>,
}

static EMPTY_FILE: FileRuntimeInfo = FileRuntimeInfo {
    expressions: Vec::new(), types: Vec::new(), stores: Vec::new(),
    calls: Vec::new(), captures: Vec::new(),
};

static EMPTY_EXPR: ExprEntry = ExprEntry {
    ty: None, store: None, call: None, captures: None, consumes: false,
};

/// Laufzeitdaten eines Ausdrucks. Seltene Einträge belegen im Index nur einen Platz.
pub struct ExpressionInfo<'a> {
    file: &'a FileRuntimeInfo,
    entry: &'a ExprEntry,
}

impl<'a> ExpressionInfo<'a> {
    /// Statischer Typ nach Verengung.
    #[inline]
    pub fn ty(&self) -> Option<&'a Ty> {
        self.entry.ty.map(|i| &self.file.types[i.get() as usize - 1])
    }

    /// Zieltyp eines gespeicherten Werts oder Arguments.
    #[inline]
    pub fn store(&self) -> Option<&'a Ty> {
        self.entry.store.map(|i| &self.file.stores[i.get() as usize - 1])
    }

    /// Der Wert geht an einen Besitzer (Move oder gezählte Kopie).
    #[inline]
    pub fn consumes(&self) -> bool {
        self.entry.consumes
    }

    #[inline]
    pub fn call(&self) -> Option<&'a CallInfo> {
        self.entry.call.map(|i| &self.file.calls[i.get() as usize - 1])
    }

    /// Von einem own-Lambda erfasste Variablen.
    #[inline]
    pub fn captures(&self) -> &'a [String] {
        self.entry.captures.map_or(&[], |i| &self.file.captures[i.get() as usize - 1])
    }
}

#[derive(Debug, Default)]
pub struct RuntimeInfo {
    files: Vec<FileRuntimeInfo>,
    /// Basiskonstruktor je Konstruktor (Datei, Position der Deklaration)
    pub base_ctors: HashMap<(usize, Span), Option<MethodInfo>>,
    /// Typen lokaler Deklarationen (Datei, Position des Namens)
    pub decls: HashMap<(usize, Span), Ty>,
}

impl RuntimeInfo {
    #[inline]
    pub fn expression(&self, (file, id): Key) -> ExpressionInfo<'_> {
        let file = self.files.get(file).unwrap_or(&EMPTY_FILE);
        let entry = file.expressions.get(id as usize).unwrap_or(&EMPTY_EXPR);
        ExpressionInfo { file, entry }
    }

    fn expression_slot(&mut self, (file, id): Key) -> (&mut FileRuntimeInfo, usize) {
        if file >= self.files.len() {
            self.files.resize_with(file + 1, FileRuntimeInfo::default);
        }
        let data = &mut self.files[file];
        let id = id as usize;
        if id >= data.expressions.len() {
            data.expressions.resize_with(id + 1, ExprEntry::default);
        }
        (data, id)
    }

    pub(crate) fn record_type(&mut self, key: Key, ty: Ty) {
        let (data, id) = self.expression_slot(key);
        save(&mut data.expressions[id].ty, &mut data.types, ty);
    }

    pub(crate) fn record_store(&mut self, key: Key, ty: Ty) {
        let (data, id) = self.expression_slot(key);
        save(&mut data.expressions[id].store, &mut data.stores, ty);
    }

    pub(crate) fn record_consume(&mut self, key: Key) {
        let (data, id) = self.expression_slot(key);
        data.expressions[id].consumes = true;
    }

    pub(crate) fn record_call(&mut self, key: Key, call: CallInfo) {
        let (data, id) = self.expression_slot(key);
        save(&mut data.expressions[id].call, &mut data.calls, call);
    }

    pub(crate) fn record_captures(&mut self, key: Key, captures: Vec<String>) {
        let (data, id) = self.expression_slot(key);
        save(&mut data.expressions[id].captures, &mut data.captures, captures);
    }
}

fn save<T>(index: &mut Option<NonZeroU32>, values: &mut Vec<T>, value: T) {
    if let Some(i) = index {
        values[i.get() as usize - 1] = value;
    } else {
        values.push(value);
        let i = u32::try_from(values.len()).expect("Zu viele Ausdruckseinträge");
        *index = NonZeroU32::new(i);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ausdrucks_ids_sind_dateilokal() {
        let mut rt = RuntimeInfo::default();
        let first = (0, 7);
        let second = (2, 7);
        rt.record_type(first, Ty::Prim(Primitive::I32));
        rt.record_type(second, Ty::Prim(Primitive::Bool));
        rt.record_store(first, Ty::Prim(Primitive::I64));
        rt.record_store(first, Ty::Prim(Primitive::I16));
        rt.record_consume(first);
        rt.record_call(second, CallInfo { target: Target::FuncValue, params: Vec::new() });
        rt.record_captures((2, 8), vec!["wert".into()]);

        let expr = rt.expression(first);
        assert_eq!(expr.ty(), Some(&Ty::Prim(Primitive::I32)));
        assert_eq!(expr.store(), Some(&Ty::Prim(Primitive::I16)));
        assert!(expr.consumes());
        assert!(expr.call().is_none());

        let expr = rt.expression(second);
        assert_eq!(expr.ty(), Some(&Ty::Prim(Primitive::Bool)));
        assert!(expr.store().is_none());
        assert!(!expr.consumes());
        assert!(matches!(expr.call().map(|call| &call.target), Some(Target::FuncValue)));

        assert_eq!(rt.expression((2, 8)).captures(), ["wert"]);
        assert!(rt.expression((2, 6)).captures().is_empty());
        assert!(rt.expression((1, 7)).ty().is_none());
        assert!(rt.expression((3, 7)).ty().is_none());
    }
}
