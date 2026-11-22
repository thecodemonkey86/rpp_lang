//! Semantische Typen und die Zahlregeln aus Kapitel 4 (R-4.6, R-4.7, R-4.12).

use rpp_syntax::ast::Primitive as P;
use std::collections::HashMap;

pub type DefId = usize;
pub type ParamId = usize;

/// Markierung eines Parameters (R-10.1): lesend geliehen, veränderbar geliehen, übernommen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    Read,
    Mut,
    Own,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FParam {
    pub ty: Ty,
    pub mode: Mode,
}

/// Besitzform eines Objekttyps (Kapitel 5). In M2 nur für `weak` und `shared` relevant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Form {
    Plain,
    Shared,
    Weak,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    /// nach einem gemeldeten Fehler; passt zu allem, damit keine Folgefehler entstehen
    Error,
    Void,
    /// Typ des Literals `null`
    Null,
    Prim(P),
    /// Klasse, Mixin oder Enum mit Typargumenten
    Named(DefId, Vec<Ty>),
    Param(ParamId),
    Vector(Box<Ty>),
    Array(Box<Ty>, Option<u64>),
    Slice(Box<Ty>),
    Tuple(Vec<Ty>),
    Func(Vec<FParam>, Box<Ty>),
    Nullable(Box<Ty>),
    /// `shared T` oder `weak T`
    Formed(Form, Box<Ty>),
}

impl Ty {
    pub fn nullable(self) -> Ty {
        match self {
            Ty::Nullable(_) | Ty::Error | Ty::Null | Ty::Void => self,
            other => Ty::Nullable(Box::new(other)),
        }
    }

    /// Ohne `?` und ohne Besitzform.
    pub fn core(&self) -> &Ty {
        match self {
            Ty::Nullable(t) | Ty::Formed(_, t) => t.core(),
            t => t,
        }
    }

    /// Ohne Besitzform (aber mit `?`).
    pub fn unformed(&self) -> &Ty {
        match self {
            Ty::Formed(_, t) => t.unformed(),
            t => t,
        }
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Ty::Error)
    }

    pub fn prim(&self) -> Option<P> {
        match self.unformed() {
            Ty::Prim(p) => Some(*p),
            _ => None,
        }
    }

    pub fn is_bool(&self) -> bool {
        self.prim() == Some(P::Bool)
    }

    pub fn is_numeric(&self) -> bool {
        self.prim().is_some_and(is_numeric)
    }

    pub fn is_integer(&self) -> bool {
        self.prim().is_some_and(is_integer)
    }

    pub fn is_string(&self) -> bool {
        self.prim() == Some(P::String)
    }

    pub fn subst(&self, map: &HashMap<ParamId, Ty>) -> Ty {
        if map.is_empty() {
            return self.clone();
        }
        match self {
            Ty::Param(p) => match map.get(p) {
                Some(t) => t.clone(),
                None => self.clone(),
            },
            Ty::Named(d, args) => Ty::Named(*d, args.iter().map(|a| a.subst(map)).collect()),
            Ty::Vector(t) => Ty::Vector(Box::new(t.subst(map))),
            Ty::Array(t, n) => Ty::Array(Box::new(t.subst(map)), *n),
            Ty::Slice(t) => Ty::Slice(Box::new(t.subst(map))),
            Ty::Tuple(ts) => Ty::Tuple(ts.iter().map(|t| t.subst(map)).collect()),
            Ty::Func(ps, r) => Ty::Func(
                ps.iter().map(|p| FParam { ty: p.ty.subst(map), mode: p.mode }).collect(),
                Box::new(r.subst(map)),
            ),
            Ty::Nullable(t) => t.subst(map).nullable(),
            Ty::Formed(f, t) => Ty::Formed(*f, Box::new(t.subst(map))),
            other => other.clone(),
        }
    }

    pub fn mentions_param(&self) -> bool {
        match self {
            Ty::Param(_) => true,
            Ty::Named(_, a) | Ty::Tuple(a) => a.iter().any(Ty::mentions_param),
            Ty::Vector(t) | Ty::Array(t, _) | Ty::Slice(t) | Ty::Nullable(t) | Ty::Formed(_, t) => t.mentions_param(),
            Ty::Func(ps, r) => ps.iter().any(|p| p.ty.mentions_param()) || r.mentions_param(),
            _ => false,
        }
    }
}

pub fn is_integer(p: P) -> bool {
    matches!(p, P::I8 | P::I16 | P::I32 | P::I64 | P::Ui8 | P::Ui16 | P::Ui32 | P::Ui64)
}

pub fn is_float(p: P) -> bool {
    matches!(p, P::F32 | P::F64)
}

pub fn is_numeric(p: P) -> bool {
    is_integer(p) || is_float(p)
}

pub fn is_unsigned(p: P) -> bool {
    matches!(p, P::Ui8 | P::Ui16 | P::Ui32 | P::Ui64)
}

pub const NUMERIC: [P; 10] = [P::I8, P::I16, P::I32, P::I64, P::Ui8, P::Ui16, P::Ui32, P::Ui64, P::F32, P::F64];

/// Automatische Umwandlung zwischen Zahltypen (Tabelle in R-4.6), ohne die Identität.
pub fn auto_converts(from: P, to: P) -> bool {
    use P::*;
    let targets: &[P] = match from {
        I8 => &[I16, I32, I64, F32, F64],
        I16 => &[I32, I64, F32, F64],
        I32 => &[I64, F64],
        Ui8 => &[Ui16, Ui32, Ui64, I16, I32, I64, F32, F64],
        Ui16 => &[Ui32, Ui64, I32, I64, F32, F64],
        Ui32 => &[Ui64, I64, F64],
        F32 => &[F64],
        _ => &[],
    };
    targets.contains(&to)
}

fn converts_or_same(from: P, to: P) -> bool {
    from == to || auto_converts(from, to)
}

/// Gemeinsamer Typ zweier Zahltypen (R-4.12).
pub fn common_numeric(a: P, b: P) -> Option<P> {
    // Bei zwei Ganzzahlen ist der gemeinsame Typ eine Ganzzahl (i32, ui32 ergibt i64, nicht f64).
    let both_int = is_integer(a) && is_integer(b);
    let candidates: Vec<P> = NUMERIC
        .iter()
        .copied()
        .filter(|&t| converts_or_same(a, t) && converts_or_same(b, t) && (!both_int || is_integer(t)))
        .collect();
    candidates.iter().copied().find(|&t| candidates.iter().all(|&o| converts_or_same(t, o)))
}

/// Wert eines Ganzzahlliterals (ohne Trenner, auch hexadezimal).
pub fn int_value(text: &str) -> Option<i128> {
    let clean: String = text.chars().filter(|&c| c != '_').collect();
    match clean.strip_prefix("0x") {
        Some(hex) => i128::from_str_radix(hex, 16).ok(),
        None => clean.parse().ok(),
    }
}

pub fn int_range(p: P) -> Option<(i128, i128)> {
    Some(match p {
        P::I8 => (i8::MIN as i128, i8::MAX as i128),
        P::I16 => (i16::MIN as i128, i16::MAX as i128),
        P::I32 => (i32::MIN as i128, i32::MAX as i128),
        P::I64 => (i64::MIN as i128, i64::MAX as i128),
        P::Ui8 => (0, u8::MAX as i128),
        P::Ui16 => (0, u16::MAX as i128),
        P::Ui32 => (0, u32::MAX as i128),
        P::Ui64 => (0, u64::MAX as i128),
        _ => return None,
    })
}

/// Passt ein Ganzzahlliteral mit Wert `v` in den Zahltyp `p` (R-4.7)?
pub fn int_fits(v: i128, p: P) -> bool {
    match int_range(p) {
        Some((lo, hi)) => lo <= v && v <= hi,
        None => is_float(p),
    }
}

/// Ist ein Gleitkommaliteral im Typ `p` endlich darstellbar (R-4.7)?
pub fn float_fits(text: &str, p: P) -> bool {
    let clean: String = text.chars().filter(|&c| c != '_').collect();
    let Ok(v) = clean.parse::<f64>() else { return false };
    match p {
        P::F64 => v.is_finite(),
        P::F32 => (v as f32).is_finite(),
        _ => false,
    }
}

/// Typ eines Ganzzahlliterals ohne Kontext (R-4.7).
pub fn default_int(v: i128) -> P {
    if int_fits(v, P::I32) {
        P::I32
    } else {
        P::I64
    }
}

pub fn prim_name(p: P) -> &'static str {
    match p {
        P::Bool => "bool",
        P::Char => "char",
        P::Byte => "byte",
        P::String => "string",
        P::I8 => "i8",
        P::I16 => "i16",
        P::I32 => "i32",
        P::I64 => "i64",
        P::Ui8 => "ui8",
        P::Ui16 => "ui16",
        P::Ui32 => "ui32",
        P::Ui64 => "ui64",
        P::F32 => "f32",
        P::F64 => "f64",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemeinsamer_typ() {
        assert_eq!(common_numeric(P::I8, P::I32), Some(P::I32));
        assert_eq!(common_numeric(P::I8, P::Ui8), Some(P::I16));
        assert_eq!(common_numeric(P::I32, P::Ui32), Some(P::I64));
        assert_eq!(common_numeric(P::I32, P::F32), Some(P::F64));
        assert_eq!(common_numeric(P::I64, P::Ui64), None);
        assert_eq!(common_numeric(P::I64, P::F64), None);
        assert_eq!(common_numeric(P::Ui64, P::F32), None);
    }
}
