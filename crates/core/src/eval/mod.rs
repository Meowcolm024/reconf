use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

pub mod builtins;
pub mod core;

pub const PRELUDE_SOURCE: &str = include_str!("prelude.reconf");

use crate::core::CoreExpr;
use crate::core::GlobalRef;
use crate::error::{Error, ErrorCode, Result};
use crate::eval::builtins::NativeFunction;

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
    CoreClosure {
        param: String,
        body: CoreExpr,
        env: Env,
    },
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
            Value::CoreClosure { .. } => write!(f, "<function>"),
            Value::Native(function) => write!(f, "<native {}>", function.name),
        }
    }
}

#[derive(Clone, Default)]
pub struct Env {
    globals: Rc<BTreeMap<GlobalRef, Value>>,
    values: Rc<BTreeMap<String, Value>>,
    locals: Rc<Vec<Value>>,
}

impl Env {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn from_values(values: BTreeMap<String, Value>) -> Self {
        Self {
            globals: Rc::new(BTreeMap::new()),
            values: Rc::new(values),
            locals: Rc::new(Vec::new()),
        }
    }

    pub fn from_bindings(
        globals: BTreeMap<GlobalRef, Value>,
        values: BTreeMap<String, Value>,
    ) -> Self {
        Self {
            globals: Rc::new(globals),
            values: Rc::new(values),
            locals: Rc::new(Vec::new()),
        }
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.values.get(name)
    }

    pub fn global(&self, binding: GlobalRef) -> Option<&Value> {
        self.globals.get(&binding)
    }

    pub fn local(&self, index: usize) -> Option<&Value> {
        self.locals
            .len()
            .checked_sub(index + 1)
            .and_then(|position| self.locals.get(position))
    }

    pub fn extend(&self, name: impl Into<String>, value: Value) -> Self {
        let mut next = (*self.values).clone();
        next.insert(name.into(), value);
        Self {
            globals: self.globals.clone(),
            values: Rc::new(next),
            locals: self.locals.clone(),
        }
    }

    pub fn push_local(&self, value: Value) -> Self {
        let mut locals = (*self.locals).clone();
        locals.push(value);
        Self {
            globals: self.globals.clone(),
            values: self.values.clone(),
            locals: Rc::new(locals),
        }
    }
}

pub(crate) fn binary(op: &str, a: Value, b: Value) -> Result<Value> {
    match (op, a, b) {
        ("+", Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
        ("-", Value::Int(a), Value::Int(b)) => Ok(Value::Int(a - b)),
        ("*", Value::Int(a), Value::Int(b)) => Ok(Value::Int(a * b)),
        ("/", Value::Int(_), Value::Int(0)) | ("%", Value::Int(_), Value::Int(0)) => Err(
            Error::with_code(ErrorCode::RuntimeDivisionByZero, "division by zero"),
        ),
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

pub(crate) fn value_eq(a: &Value, b: &Value) -> bool {
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
