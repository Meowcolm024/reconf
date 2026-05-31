use std::collections::BTreeMap;

use crate::error::{Error, Result};
use crate::eval::{Env, Value, env_with, eval};
use crate::syntax::surface::{Expr, Type};

pub fn validate_refinement(
    value: Value,
    binder: &str,
    pred: &Expr,
    env: &Env,
    aliases: &BTreeMap<String, Type>,
) -> Result<Value> {
    match eval(
        pred,
        &env_with(env, binder.to_string(), value.clone()),
        aliases,
    )? {
        Value::Bool(true) => Ok(value),
        Value::Bool(false) => Err(Error::new("refinement failed")),
        _ => Err(Error::new("unknown predicate")),
    }
}
