use std::collections::BTreeMap;

use crate::error::{Error, ErrorCode, Result};
use crate::eval::{Env, Value, env_with, eval};
use crate::syntax::surface::{Expr, Type};

pub fn validate_refinement(
    value: Value,
    binder: &str,
    pred: &Expr,
    env: &Env,
    aliases: &BTreeMap<String, Type>,
) -> Result<Value> {
    validate_refinement_with_code(value, binder, pred, env, aliases, ErrorCode::RefineFailed)
}

pub fn validate_refinement_with_code(
    value: Value,
    binder: &str,
    pred: &Expr,
    env: &Env,
    aliases: &BTreeMap<String, Type>,
    code: ErrorCode,
) -> Result<Value> {
    match eval(
        pred,
        &env_with(env, binder.to_string(), value.clone()),
        aliases,
    )? {
        Value::Bool(true) => Ok(value),
        Value::Bool(false) => Err(Error::with_code(code, "refinement failed")),
        _ => Err(Error::new("unknown predicate")),
    }
}
