use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

pub mod builtins;
pub mod prelude;

use crate::error::{Error, Result};
use crate::eval::builtins::NativeFunction;
use crate::syntax::surface::{Expr, Type};
use crate::typeck::bidir::check_expr;

#[derive(Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    None,
    Some(Box<Value>),
    List(Vec<Value>),
    Record(BTreeMap<String, Value>),
    Closure { param: String, body: Expr, env: Env },
    Native(NativeFunction),
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(x) => write!(f, "{x}"),
            Value::Float(x) => write!(f, "{x}"),
            Value::Bool(x) => write!(f, "{x}"),
            Value::String(x) => write!(f, "{x:?}"),
            Value::None => write!(f, "none"),
            Value::Some(x) => write!(f, "some {x:?}"),
            Value::List(xs) => f.debug_list().entries(xs).finish(),
            Value::Record(fields) => f.debug_map().entries(fields).finish(),
            Value::Closure { .. } => write!(f, "<function>"),
            Value::Native(function) => write!(f, "<native {}>", function.name),
        }
    }
}

pub type Env = Rc<BTreeMap<String, Value>>;

pub fn env_with(env: &Env, name: String, value: Value) -> Env {
    let mut next = (**env).clone();
    next.insert(name, value);
    Rc::new(next)
}

pub fn eval(expr: &Expr, env: &Env, aliases: &BTreeMap<String, Type>) -> Result<Value> {
    match expr {
        Expr::Int(x) => Ok(Value::Int(*x)),
        Expr::Float(x) => Ok(Value::Float(*x)),
        Expr::Bool(x) => Ok(Value::Bool(*x)),
        Expr::String(x) => Ok(Value::String(x.clone())),
        Expr::Interp(_) => Err(Error::new("internal error: unlowered interpolation")),
        Expr::None => Ok(Value::None),
        Expr::Some(expr) => Ok(Value::Some(Box::new(eval(expr, env, aliases)?))),
        Expr::Var(name) => env
            .get(name)
            .cloned()
            .ok_or_else(|| Error::new(format!("unknown identifier `{name}`"))),
        Expr::List(items) => items
            .iter()
            .map(|item| eval(item, env, aliases))
            .collect::<Result<Vec<_>>>()
            .map(Value::List),
        Expr::Record(fields) => fields
            .iter()
            .map(|(name, expr)| Ok((name.clone(), eval(expr, env, aliases)?)))
            .collect::<Result<BTreeMap<_, _>>>()
            .map(Value::Record),
        Expr::Field(expr, name) => match eval(expr, env, aliases)? {
            Value::Record(fields) => fields
                .get(name)
                .cloned()
                .ok_or_else(|| Error::new(format!("unknown field `{name}`"))),
            _ => Err(Error::new(format!("unknown field `{name}`"))),
        },
        Expr::Dot(expr, name) => {
            let receiver = eval(expr, env, aliases)?;
            if let Value::Record(fields) = &receiver
                && let Some(value) = fields.get(name)
            {
                return Ok(value.clone());
            }
            let method = env
                .get(name)
                .cloned()
                .ok_or_else(|| Error::new(format!("unknown field `{name}`")))?;
            apply(method, receiver, aliases)
        }
        Expr::If(cond, then_expr, else_expr) => match eval(cond, env, aliases)? {
            Value::Bool(true) => eval(then_expr, env, aliases),
            Value::Bool(false) => eval(else_expr, env, aliases),
            _ => Err(Error::new("type mismatch: if condition must be Bool")),
        },
        Expr::Let(name, annotation, value, body) => {
            let value = if let Some(ty) = annotation {
                check_expr(value, ty, env, aliases)?
            } else {
                eval(value, env, aliases)?
            };
            eval(body, &env_with(env, name.clone(), value), aliases)
        }
        Expr::Lambda(param, _ty, body) => Ok(Value::Closure {
            param: param.clone(),
            body: *body.clone(),
            env: env.clone(),
        }),
        Expr::Apply(function, arg) => {
            let function = eval(function, env, aliases)?;
            let arg = eval(arg, env, aliases)?;
            apply(function, arg, aliases)
        }
        Expr::Ascribe(expr, ty) => check_expr(expr, ty, env, aliases),
        Expr::Unary(op, expr) => {
            let value = eval(expr, env, aliases)?;
            match (op.as_str(), value) {
                ("!", Value::Bool(x)) => Ok(Value::Bool(!x)),
                ("-", Value::Int(x)) => Ok(Value::Int(-x)),
                ("-", Value::Float(x)) => Ok(Value::Float(-x)),
                _ => Err(Error::new(format!("type mismatch: invalid unary `{op}`"))),
            }
        }
        Expr::Binary(op, a, b) => {
            if op == "&&" {
                return match eval(a, env, aliases)? {
                    Value::Bool(false) => Ok(Value::Bool(false)),
                    Value::Bool(true) => match eval(b, env, aliases)? {
                        Value::Bool(x) => Ok(Value::Bool(x)),
                        _ => Err(Error::new("type mismatch: && expects Bool")),
                    },
                    _ => Err(Error::new("type mismatch: && expects Bool")),
                };
            }
            if op == "||" {
                return match eval(a, env, aliases)? {
                    Value::Bool(true) => Ok(Value::Bool(true)),
                    Value::Bool(false) => match eval(b, env, aliases)? {
                        Value::Bool(x) => Ok(Value::Bool(x)),
                        _ => Err(Error::new("type mismatch: || expects Bool")),
                    },
                    _ => Err(Error::new("type mismatch: || expects Bool")),
                };
            }
            binary(op, eval(a, env, aliases)?, eval(b, env, aliases)?)
        }
    }
}

pub fn apply_value(function: Value, arg: Value) -> Result<Value> {
    apply(function, arg, &BTreeMap::new())
}

fn apply(function: Value, arg: Value, aliases: &BTreeMap<String, Type>) -> Result<Value> {
    match function {
        Value::Closure { param, body, env } => eval(&body, &env_with(&env, param, arg), aliases),
        Value::Native(function) => function.apply(arg),
        _ => Err(Error::new("type mismatch: applying non-function")),
    }
}

fn binary(op: &str, a: Value, b: Value) -> Result<Value> {
    match (op, a, b) {
        ("+", Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
        ("-", Value::Int(a), Value::Int(b)) => Ok(Value::Int(a - b)),
        ("*", Value::Int(a), Value::Int(b)) => Ok(Value::Int(a * b)),
        ("/", Value::Int(_), Value::Int(0)) | ("%", Value::Int(_), Value::Int(0)) => {
            Err(Error::new("division by zero"))
        }
        ("/", Value::Int(a), Value::Int(b)) => Ok(Value::Int(a / b)),
        ("%", Value::Int(a), Value::Int(b)) => Ok(Value::Int(a % b)),
        ("+", Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
        ("-", Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
        ("*", Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
        ("/", Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),
        ("++", Value::String(a), Value::String(b)) => Ok(Value::String(format!("{a}{b}"))),
        ("==", a, b) => Ok(Value::Bool(value_eq(&a, &b))),
        ("!=", a, b) => Ok(Value::Bool(!value_eq(&a, &b))),
        ("<", Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a < b)),
        ("<=", Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a <= b)),
        (">", Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a > b)),
        (">=", Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a >= b)),
        ("<", Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a < b)),
        ("<=", Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a <= b)),
        (">", Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a > b)),
        (">=", Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a >= b)),
        _ => Err(Error::new(format!("type mismatch: invalid binary `{op}`"))),
    }
}

fn value_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => a == b,
        (Value::Float(a), Value::Float(b)) => a == b,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::String(a), Value::String(b)) => a == b,
        (Value::None, Value::None) => true,
        (Value::Some(a), Value::Some(b)) => value_eq(a, b),
        (Value::List(a), Value::List(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| value_eq(a, b))
        }
        (Value::Record(a), Value::Record(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(name, a)| b.get(name).is_some_and(|b| value_eq(a, b)))
        }
        _ => false,
    }
}

pub fn contains_function(value: &Value) -> bool {
    match value {
        Value::Closure { .. } | Value::Native(_) => true,
        Value::Some(value) => contains_function(value),
        Value::List(items) => items.iter().any(contains_function),
        Value::Record(fields) => fields.values().any(contains_function),
        _ => false,
    }
}

pub fn emit(value: &Value) -> Result<String> {
    Ok(match value {
        Value::Int(x) => x.to_string(),
        Value::Float(x) => x.to_string(),
        Value::Bool(x) => x.to_string(),
        Value::String(x) => format!("{x:?}"),
        Value::None => "none".to_string(),
        Value::Some(x) => format!("some {}", emit(x)?),
        Value::List(items) => {
            let parts = items.iter().map(emit).collect::<Result<Vec<_>>>()?;
            format!("[{}]", parts.join(", "))
        }
        Value::Record(fields) => {
            let parts = fields
                .iter()
                .map(|(name, value)| Ok(format!("{name} = {}", emit(value)?)))
                .collect::<Result<Vec<_>>>()?;
            format!("{{ {} }}", parts.join(", "))
        }
        Value::Closure { .. } | Value::Native(_) => {
            return Err(Error::new("function escaped into output"));
        }
    })
}
