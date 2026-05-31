use crate::error::{Error, Result};
use crate::eval::{Value, apply_value};

#[derive(Clone)]
pub struct NativeFunction {
    pub name: String,
    pub args: Vec<Value>,
}

impl NativeFunction {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            args: Vec::new(),
        }
    }

    pub fn apply(mut self, arg: Value) -> Result<Value> {
        self.args.push(arg);
        if self.args.len() == arity(&self.name)? {
            call(&self.name, self.args)
        } else {
            Ok(Value::Native(self))
        }
    }
}

pub fn declared(name: &str) -> bool {
    matches!(
        name,
        "show"
            | "length"
            | "isSome"
            | "isNone"
            | "contains"
            | "startsWith"
            | "endsWith"
            | "unwrapOr"
            | "all"
            | "any"
            | "map"
            | "filter"
    )
}

fn arity(name: &str) -> Result<usize> {
    match name {
        "show" | "length" | "isSome" | "isNone" | "all" | "any" => Ok(1),
        "contains" | "startsWith" | "endsWith" | "unwrapOr" | "map" | "filter" => Ok(2),
        _ => Err(Error::new(format!("unknown native `{name}`"))),
    }
}

fn call(name: &str, args: Vec<Value>) -> Result<Value> {
    match (name, args.as_slice()) {
        ("show", [Value::Int(value)]) => Ok(Value::String(value.to_string())),
        ("show", [Value::Float(value)]) => Ok(Value::String(value.to_string())),
        ("show", [Value::Bool(value)]) => Ok(Value::String(value.to_string())),
        ("show", [Value::String(value)]) => Ok(Value::String(value.clone())),
        ("length", [Value::String(value)]) => Ok(Value::Int(value.chars().count() as i64)),
        ("length", [Value::List(items)]) => Ok(Value::Int(items.len() as i64)),
        ("isSome", [Value::Some(_)]) => Ok(Value::Bool(true)),
        ("isSome", [Value::None]) => Ok(Value::Bool(false)),
        ("isNone", [Value::None]) => Ok(Value::Bool(true)),
        ("isNone", [Value::Some(_)]) => Ok(Value::Bool(false)),
        ("unwrapOr", [Value::None, default]) => Ok(default.clone()),
        ("unwrapOr", [Value::Some(value), _]) => Ok((**value).clone()),
        ("contains", [Value::String(haystack), Value::String(needle)]) => {
            Ok(Value::Bool(haystack.contains(needle)))
        }
        ("contains", [Value::List(items), needle]) => {
            Ok(Value::Bool(items.iter().any(|item| value_eq(item, needle))))
        }
        ("startsWith", [Value::String(value), Value::String(prefix)]) => {
            Ok(Value::Bool(value.starts_with(prefix)))
        }
        ("endsWith", [Value::String(value), Value::String(suffix)]) => {
            Ok(Value::Bool(value.ends_with(suffix)))
        }
        ("all", [Value::List(items)]) => items
            .iter()
            .try_fold(true, |acc, item| match item {
                Value::Bool(value) => Ok(acc && *value),
                _ => Err(Error::new("type mismatch: all expects [Bool]")),
            })
            .map(Value::Bool),
        ("any", [Value::List(items)]) => items
            .iter()
            .try_fold(false, |acc, item| match item {
                Value::Bool(value) => Ok(acc || *value),
                _ => Err(Error::new("type mismatch: any expects [Bool]")),
            })
            .map(Value::Bool),
        ("map", [Value::List(items), function]) => {
            let mapped = items
                .iter()
                .map(|item| apply_value(function.clone(), item.clone()))
                .collect::<Result<Vec<_>>>()?;
            Ok(Value::List(mapped))
        }
        ("filter", [Value::List(items), function]) => {
            let mut out = Vec::new();
            for item in items {
                match apply_value(function.clone(), item.clone())? {
                    Value::Bool(true) => out.push(item.clone()),
                    Value::Bool(false) => {}
                    _ => {
                        return Err(Error::new(
                            "type mismatch: filter predicate must return Bool",
                        ));
                    }
                }
            }
            Ok(Value::List(out))
        }
        ("all", [Value::List(items), function]) => {
            for item in items {
                match apply_value(function.clone(), item.clone())? {
                    Value::Bool(true) => {}
                    Value::Bool(false) => return Ok(Value::Bool(false)),
                    _ => return Err(Error::new("type mismatch: all predicate must return Bool")),
                }
            }
            Ok(Value::Bool(true))
        }
        ("any", [Value::List(items), function]) => {
            for item in items {
                match apply_value(function.clone(), item.clone())? {
                    Value::Bool(true) => return Ok(Value::Bool(true)),
                    Value::Bool(false) => {}
                    _ => return Err(Error::new("type mismatch: any predicate must return Bool")),
                }
            }
            Ok(Value::Bool(false))
        }
        _ => Err(Error::new(format!(
            "type mismatch: invalid arguments to native `{name}`"
        ))),
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
