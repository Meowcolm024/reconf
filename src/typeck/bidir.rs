use std::collections::BTreeMap;

use crate::error::{Error, Result};
use crate::eval::{Env, Value, eval};
use crate::refine::validate::validate_refinement;
use crate::syntax::surface::{Expr, Type};
use crate::typeck::unify::{expand_type, is_option_type, matches_type, type_name, value_name};

pub fn check_expr(
    expr: &Expr,
    expected: &Type,
    env: &Env,
    aliases: &BTreeMap<String, Type>,
) -> Result<Value> {
    let expected = expand_type(expected, aliases)?;
    match expected {
        Type::Option(inner) => match expr {
            Expr::None => Ok(Value::None),
            Expr::Some(expr) => Ok(Value::Some(Box::new(check_expr(
                expr, &inner, env, aliases,
            )?))),
            _ => {
                let value = eval(expr, env, aliases)?;
                if matches_type(&value, &Type::Option(inner.clone()), aliases)? {
                    Ok(value)
                } else {
                    Ok(Value::Some(Box::new(check_value_against(
                        value, &inner, env, aliases,
                    )?)))
                }
            }
        },
        Type::Record(fields) => {
            if let Expr::Record(expr_fields) = expr {
                let mut out = BTreeMap::new();
                for name in expr_fields.keys() {
                    if !fields.contains_key(name) {
                        return Err(Error::new(format!("unknown field `{name}`")));
                    }
                }
                for (name, ty) in fields {
                    match expr_fields.get(&name) {
                        Some(expr) => {
                            out.insert(name, check_expr(expr, &ty, env, aliases)?);
                        }
                        None if is_option_type(&ty, aliases)? => {
                            out.insert(name, Value::None);
                        }
                        None => return Err(Error::new(format!("missing field `{name}`"))),
                    }
                }
                Ok(Value::Record(out))
            } else {
                let value = eval(expr, env, aliases)?;
                check_value_against(value, &Type::Record(fields), env, aliases)
            }
        }
        Type::Refinement { binder, base, pred } => {
            let value = check_expr(expr, &base, env, aliases)?;
            validate_refinement(value, &binder, &pred, env, aliases)
        }
        other => {
            let value = eval(expr, env, aliases)?;
            check_value_against(value, &other, env, aliases)
        }
    }
}

pub fn synth_expr(expr: &Expr, env: &Env, aliases: &BTreeMap<String, Type>) -> Result<Value> {
    match expr {
        Expr::None => Err(Error::new("`none` requires an expected option type")),
        Expr::List(items) if items.is_empty() => {
            Err(Error::new("empty lists require an expected list type"))
        }
        expr => eval(expr, env, aliases),
    }
}

pub fn check_value_against(
    value: Value,
    expected: &Type,
    env: &Env,
    aliases: &BTreeMap<String, Type>,
) -> Result<Value> {
    let expected = expand_type(expected, aliases)?;
    match &expected {
        Type::Refinement { binder, base, pred } => {
            let value = check_value_against(value, base, env, aliases)?;
            validate_refinement(value, binder, pred, env, aliases)
        }
        Type::Record(fields) => {
            let Value::Record(values) = value else {
                return Err(Error::new("type mismatch: expected record"));
            };
            for name in values.keys() {
                if !fields.contains_key(name) {
                    return Err(Error::new(format!("unknown field `{name}`")));
                }
            }
            let mut out = BTreeMap::new();
            for (name, ty) in fields {
                let Some(value) = values.get(name) else {
                    if is_option_type(ty, aliases)? {
                        out.insert(name.clone(), Value::None);
                        continue;
                    }
                    return Err(Error::new(format!("missing field `{name}`")));
                };
                out.insert(
                    name.clone(),
                    check_value_against(value.clone(), ty, env, aliases)?,
                );
            }
            Ok(Value::Record(out))
        }
        Type::Option(inner) => match value {
            Value::None => Ok(Value::None),
            Value::Some(value) => Ok(Value::Some(Box::new(check_value_against(
                *value, inner, env, aliases,
            )?))),
            value => Ok(Value::Some(Box::new(check_value_against(
                value, inner, env, aliases,
            )?))),
        },
        _ if matches_type(&value, &expected, aliases)? => Ok(value),
        _ => Err(Error::new(format!(
            "type mismatch: expected {}, got {}",
            type_name(&expected),
            value_name(&value)
        ))),
    }
}
