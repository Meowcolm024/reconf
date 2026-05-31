use std::collections::BTreeMap;

use crate::error::{Error, ErrorCode, Result};
use crate::eval::Value;
use crate::syntax::surface::Type;

pub fn expand_type(ty: &Type, aliases: &BTreeMap<String, Type>) -> Result<Type> {
    match ty {
        Type::Alias(name) => {
            let ty = aliases
                .get(name)
                .ok_or_else(|| Error::new(format!("unknown type `{name}`")))?;
            if matches!(ty, Type::Alias(alias) if alias == name) {
                return Err(Error::with_code(
                    ErrorCode::TypeRecursiveAlias,
                    format!("recursive type alias `{name}`"),
                ));
            }
            expand_type(ty, aliases)
        }
        Type::Option(inner) => Ok(Type::Option(Box::new(expand_type(inner, aliases)?))),
        Type::List(inner) => Ok(Type::List(Box::new(expand_type(inner, aliases)?))),
        Type::Record(fields) => fields
            .iter()
            .map(|(name, ty)| Ok((name.clone(), expand_type(ty, aliases)?)))
            .collect::<Result<BTreeMap<_, _>>>()
            .map(Type::Record),
        Type::Refinement { binder, base, pred } => Ok(Type::Refinement {
            binder: binder.clone(),
            base: Box::new(expand_type(base, aliases)?),
            pred: pred.clone(),
        }),
        Type::Function(a, b) => Ok(Type::Function(
            Box::new(expand_type(a, aliases)?),
            Box::new(expand_type(b, aliases)?),
        )),
        ty => Ok(ty.clone()),
    }
}

pub fn is_option_type(ty: &Type, aliases: &BTreeMap<String, Type>) -> Result<bool> {
    Ok(matches!(expand_type(ty, aliases)?, Type::Option(_)))
}

pub fn matches_type(value: &Value, ty: &Type, aliases: &BTreeMap<String, Type>) -> Result<bool> {
    Ok(match expand_type(ty, aliases)? {
        Type::Int => matches!(value, Value::Int(_)),
        Type::Float => matches!(value, Value::Float(_) | Value::Int(_)),
        Type::Bool => matches!(value, Value::Bool(_)),
        Type::String => matches!(value, Value::String(_)),
        Type::Option(inner) => match value {
            Value::None => true,
            Value::Some(value) => matches_type(value, &inner, aliases)?,
            _ => false,
        },
        Type::List(inner) => match value {
            Value::List(items) => {
                for item in items {
                    if !matches_type(item, &inner, aliases)? {
                        return Ok(false);
                    }
                }
                true
            }
            _ => false,
        },
        Type::Record(fields) => match value {
            Value::Record(values) => {
                values.len() == fields.len()
                    && fields.iter().all(|(name, ty)| {
                        values
                            .get(name)
                            .map(|value| matches_type(value, ty, aliases).unwrap_or(false))
                            .unwrap_or(false)
                    })
            }
            _ => false,
        },
        Type::Refinement { .. } => false,
        Type::Function(_, _) => matches!(value, Value::Closure { .. } | Value::Native(_)),
        Type::Alias(_) => unreachable!(),
    })
}

pub fn type_name(ty: &Type) -> &'static str {
    match ty {
        Type::Int => "Int",
        Type::Float => "Float",
        Type::Bool => "Bool",
        Type::String => "String",
        Type::Option(_) => "option",
        Type::List(_) => "list",
        Type::Record(_) => "record",
        Type::Refinement { .. } => "refinement",
        Type::Function(_, _) => "function",
        Type::Alias(_) => "alias",
    }
}

pub fn value_name(value: &Value) -> &'static str {
    match value {
        Value::Int(_) => "Int",
        Value::Float(_) => "Float",
        Value::Bool(_) => "Bool",
        Value::String(_) => "String",
        Value::None | Value::Some(_) => "option",
        Value::List(_) => "list",
        Value::Record(_) => "record",
        Value::Closure { .. } | Value::Native(_) => "function",
    }
}
