use super::{make_int, prim_of, unescape, Value};
use rpp_sema::defs::Program;
use rpp_sema::rt::Key;
use rpp_sema::ty;
use rpp_syntax::ast::{Literal, LiteralContext, Primitive};
use std::rc::Rc;

pub(super) enum PreparedLiteral {
    Value(Value),
    // Case-Labels werden ohne Einengung auf einen R++-Ganzzahltyp verglichen.
    PatternInt(i128),
}

pub(super) struct FileLiterals {
    // Nur Literale benötigen einen Laufzeitwert. Die übrigen IDs belegen je einen Index.
    by_id: Vec<u32>,
    values: Vec<PreparedLiteral>,
}

pub(super) fn prepare(program: &Program) -> Vec<FileLiterals> {
    program.ws.files.iter().enumerate().map(|(file, source)| {
        let sites = &source.unit.literals;
        let len = sites.last().map_or(0, |site| site.id as usize + 1);
        let mut by_id = vec![u32::MAX; len];
        let mut values = Vec::with_capacity(sites.len());
        for site in sites {
            let key = (file, site.id);
            let value = match site.context {
                LiteralContext::Pattern { negative } => pattern(&site.literal, negative),
                context => {
                    let primitive = program.rt.types.get(&key).and_then(prim_of);
                    PreparedLiteral::Value(expression(
                        &site.literal, primitive, context == LiteralContext::NegatedNumber,
                    ))
                }
            };
            by_id[site.id as usize] = values.len() as u32;
            values.push(value);
        }
        FileLiterals { by_id, values }
    }).collect()
}

impl FileLiterals {
    pub(super) fn get(files: &[Self], (file, id): Key) -> &PreparedLiteral {
        let literals = &files[file];
        &literals.values[literals.by_id[id as usize] as usize]
    }

    pub(super) fn expression(files: &[Self], key: Key) -> Value {
        match Self::get(files, key) {
            PreparedLiteral::Value(value) => value.clone(),
            PreparedLiteral::PatternInt(_) => unreachable!("Ganzzahlmuster als Ausdruck"),
        }
    }
}

fn integer(text: &str, negative: bool) -> i128 {
    let value = ty::int_value(text).unwrap_or(0);
    if negative { -value } else { value }
}

fn expression(literal: &Literal, primitive: Option<Primitive>, negative: bool) -> Value {
    match literal {
        Literal::Int(text) => {
            let value = integer(text, negative);
            make_int(value, primitive.unwrap_or_else(|| ty::default_int(value)))
        }
        Literal::Float(text) => {
            let clean: String = text.chars().filter(|&c| c != '_').collect();
            if primitive == Some(Primitive::F32) {
                let value = clean.parse::<f32>().unwrap_or(0.0);
                Value::F32(if negative { -value } else { value })
            } else {
                let value = clean.parse::<f64>().unwrap_or(0.0);
                Value::F64(if negative { -value } else { value })
            }
        }
        Literal::Str(text) => Value::Str(Rc::from(unescape(text).as_str())),
        Literal::Char(text) => Value::Char(unescape(text).chars().next().unwrap_or('\0')),
        Literal::Bool(value) => Value::Bool(*value),
        Literal::Null => Value::Null,
    }
}

fn pattern(literal: &Literal, negative: bool) -> PreparedLiteral {
    match literal {
        Literal::Int(text) => PreparedLiteral::PatternInt(integer(text, negative)),
        Literal::Float(_) => PreparedLiteral::Value(Value::Null),
        _ => PreparedLiteral::Value(expression(literal, None, false)),
    }
}
