use std::collections::BTreeMap;

use crate::error::{Error, ErrorCode, Result};
use crate::eval::{Env, Value, eval};
use crate::refine::validate::validate_refinement_with_code;
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
                        return Err(Error::with_code(
                            ErrorCode::RecordUnknownField,
                            format!("unknown field `{name}`"),
                        ));
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
                        None => {
                            return Err(Error::with_code(
                                ErrorCode::RecordMissingField,
                                format!("missing field `{name}`"),
                            ));
                        }
                    }
                }
                Ok(Value::Record(out))
            } else {
                let value = eval(expr, env, aliases)?;
                check_value_against(value, &Type::Record(fields), env, aliases)
            }
        }
        Type::LiteralUnion(choices) => {
            let value = check_expr(expr, &Type::String, env, aliases)?;
            validate_literal_union(value, &choices, env, aliases)
        }
        Type::Refinement { binder, base, pred } => {
            let value = check_expr(expr, &base, env, aliases)?;
            validate_refinement_with_code(
                value,
                &binder,
                &pred,
                env,
                aliases,
                ErrorCode::RefineFailed,
            )
        }
        other => {
            let value = eval(expr, env, aliases)?;
            check_value_against(value, &other, env, aliases)
        }
    }
}

pub fn synth_expr(expr: &Expr, env: &Env, aliases: &BTreeMap<String, Type>) -> Result<Value> {
    match expr {
        Expr::None => Err(Error::with_code(
            ErrorCode::TypeNoneNeedsExpected,
            "`none` requires an expected option type",
        )),
        Expr::List(items) if items.is_empty() => Err(Error::with_code(
            ErrorCode::TypeNoneNeedsExpected,
            "empty lists require an expected list type",
        )),
        Expr::Interp(_) => Err(Error::with_code(
            ErrorCode::TypeBadInterpolation,
            "cannot interpolate value",
        )),
        Expr::String(_) => eval(expr, env, aliases),
        Expr::Binary(op, left, right) if op == "++" => {
            let left = synth_expr(left, env, aliases)?;
            let right = synth_expr(right, env, aliases)?;
            match (left, right) {
                (Value::String(left), Value::String(right)) => Ok(Value::String(left + &right)),
                _ => Err(Error::with_code(
                    ErrorCode::TypeBadInterpolation,
                    "cannot interpolate value",
                )),
            }
        }
        Expr::Apply(function, arg) if is_show(function) => {
            let value = synth_expr(arg, env, aliases)?;
            match value {
                Value::Int(_) | Value::Float(_) | Value::Bool(_) | Value::String(_) => {
                    eval(expr, env, aliases)
                }
                _ => Err(Error::with_code(
                    ErrorCode::TypeBadInterpolation,
                    "cannot interpolate value",
                )),
            }
        }
        expr => eval(expr, env, aliases),
    }
}

fn is_show(expr: &Expr) -> bool {
    matches!(expr, Expr::Var(name) if name == "show")
}

pub fn check_value_against(
    value: Value,
    expected: &Type,
    env: &Env,
    aliases: &BTreeMap<String, Type>,
) -> Result<Value> {
    let expected = expand_type(expected, aliases)?;
    match &expected {
        Type::LiteralUnion(choices) => {
            let value = check_value_against(value, &Type::String, env, aliases)?;
            validate_literal_union(value, choices, env, aliases)
        }
        Type::Refinement { binder, base, pred } => {
            let value = check_value_against(value, base, env, aliases)?;
            validate_refinement_with_code(
                value,
                binder,
                pred,
                env,
                aliases,
                ErrorCode::RefineFailed,
            )
        }
        Type::Record(fields) => {
            let Value::Record(values) = value else {
                return Err(Error::new("type mismatch: expected record"));
            };
            for name in values.keys() {
                if !fields.contains_key(name) {
                    return Err(Error::with_code(
                        ErrorCode::RecordUnknownField,
                        format!("unknown field `{name}`"),
                    ));
                }
            }
            let mut out = BTreeMap::new();
            for (name, ty) in fields {
                let Some(value) = values.get(name) else {
                    if is_option_type(ty, aliases)? {
                        out.insert(name.clone(), Value::None);
                        continue;
                    }
                    return Err(Error::with_code(
                        ErrorCode::RecordMissingField,
                        format!("missing field `{name}`"),
                    ));
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
        _ => Err(Error::with_code(
            ErrorCode::TypeMismatch,
            format!(
                "type mismatch: expected {}, got {}",
                type_name(&expected),
                value_name(&value)
            ),
        )),
    }
}

fn validate_literal_union(
    value: Value,
    choices: &[String],
    _: &Env,
    _: &BTreeMap<String, Type>,
) -> Result<Value> {
    match value {
        Value::String(value) if choices.iter().any(|choice| choice == &value) => {
            Ok(Value::String(value))
        }
        Value::String(_) => Err(Error::with_code(
            ErrorCode::RefineLiteralUnion,
            "literal union refinement failed",
        )),
        value => Err(Error::with_code(
            ErrorCode::TypeMismatch,
            format!("type mismatch: expected String, got {}", value_name(&value)),
        )),
    }
}
