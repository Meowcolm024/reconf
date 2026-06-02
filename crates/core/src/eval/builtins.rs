use crate::core::CoreType;
use crate::error::{Error, ErrorCode, Result};
use crate::eval::Value;
use crate::eval::core::{RuntimeValueApplicator, ValueApplicator};

#[derive(Clone, Copy)]
pub struct NativeSpec {
    name: &'static str,
    arity: usize,
    ty: NativeTypeSpec,
    implementation: NativeImplementation,
}

impl NativeSpec {
    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn arity(&self) -> usize {
        self.arity
    }

    pub fn ty(&self) -> NativeTypeSpec {
        self.ty
    }

    fn apply(&self, args: Vec<Value>) -> Result<Value> {
        self.implementation.apply(self.name, args)
    }
}

#[derive(Clone, Copy)]
enum NativeImplementation {
    Show,
    Length,
    IsSome,
    IsNone,
    Contains,
    StartsWith,
    EndsWith,
    UnwrapOr,
    All,
    Any,
    Map,
    Filter,
}

impl NativeImplementation {
    fn apply(&self, name: &'static str, args: Vec<Value>) -> Result<Value> {
        NativeCall::new(name, args).apply(*self)
    }
}

#[derive(Clone, Copy)]
pub enum NativeTypeSpec {
    Int,
    Bool,
    String,
    Option(&'static NativeTypeSpec),
    List(&'static NativeTypeSpec),
    Function(&'static NativeTypeSpec, &'static NativeTypeSpec),
}

impl NativeTypeSpec {
    pub fn to_core(self) -> CoreType {
        match self {
            Self::Int => CoreType::Int,
            Self::Bool => CoreType::Bool,
            Self::String => CoreType::String,
            Self::Option(inner) => CoreType::Option(Box::new(inner.to_core())),
            Self::List(inner) => CoreType::List(Box::new(inner.to_core())),
            Self::Function(input, output) => {
                CoreType::Function(Box::new(input.to_core()), Box::new(output.to_core()))
            }
        }
    }
}

pub struct NativeRegistry;

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
        let spec = NativeRegistry::get(&self.name)?;
        if self.args.len() == spec.arity() {
            spec.apply(self.args)
        } else {
            Ok(Value::Native(self))
        }
    }
}

impl NativeRegistry {
    pub fn all() -> &'static [NativeSpec] {
        ALL
    }

    pub fn get(name: &str) -> Result<&'static NativeSpec> {
        ALL.iter()
            .find(|spec| spec.name() == name)
            .ok_or_else(|| Error::new(format!("unknown native `{name}`")))
    }

    pub fn declared(name: &str) -> bool {
        Self::get(name).is_ok()
    }
}

pub fn declared(name: &str) -> bool {
    NativeRegistry::declared(name)
}

const ALL: &[NativeSpec] = &[
    NativeSpec {
        name: "show",
        arity: 1,
        ty: SHOW_TY,
        implementation: NativeImplementation::Show,
    },
    NativeSpec {
        name: "length",
        arity: 1,
        ty: LENGTH_TY,
        implementation: NativeImplementation::Length,
    },
    NativeSpec {
        name: "isSome",
        arity: 1,
        ty: IS_SOME_TY,
        implementation: NativeImplementation::IsSome,
    },
    NativeSpec {
        name: "isNone",
        arity: 1,
        ty: IS_NONE_TY,
        implementation: NativeImplementation::IsNone,
    },
    NativeSpec {
        name: "contains",
        arity: 2,
        ty: CONTAINS_TY,
        implementation: NativeImplementation::Contains,
    },
    NativeSpec {
        name: "startsWith",
        arity: 2,
        ty: STARTS_WITH_TY,
        implementation: NativeImplementation::StartsWith,
    },
    NativeSpec {
        name: "endsWith",
        arity: 2,
        ty: ENDS_WITH_TY,
        implementation: NativeImplementation::EndsWith,
    },
    NativeSpec {
        name: "unwrapOr",
        arity: 2,
        ty: UNWRAP_OR_TY,
        implementation: NativeImplementation::UnwrapOr,
    },
    NativeSpec {
        name: "all",
        arity: 1,
        ty: ALL_TY,
        implementation: NativeImplementation::All,
    },
    NativeSpec {
        name: "any",
        arity: 1,
        ty: ANY_TY,
        implementation: NativeImplementation::Any,
    },
    NativeSpec {
        name: "map",
        arity: 2,
        ty: MAP_TY,
        implementation: NativeImplementation::Map,
    },
    NativeSpec {
        name: "filter",
        arity: 2,
        ty: FILTER_TY,
        implementation: NativeImplementation::Filter,
    },
];

const INT: NativeTypeSpec = NativeTypeSpec::Int;
const BOOL: NativeTypeSpec = NativeTypeSpec::Bool;
const STRING: NativeTypeSpec = NativeTypeSpec::String;
const LIST_INT: NativeTypeSpec = NativeTypeSpec::List(&INT);
const LIST_BOOL: NativeTypeSpec = NativeTypeSpec::List(&BOOL);
const OPTION_INT: NativeTypeSpec = NativeTypeSpec::Option(&INT);
const INT_TO_INT: NativeTypeSpec = NativeTypeSpec::Function(&INT, &INT);
const INT_TO_BOOL: NativeTypeSpec = NativeTypeSpec::Function(&INT, &BOOL);
const STRING_TO_BOOL: NativeTypeSpec = NativeTypeSpec::Function(&STRING, &BOOL);
const INT_TO_INT_TO_LIST_INT: NativeTypeSpec = NativeTypeSpec::Function(&INT_TO_INT, &LIST_INT);
const INT_TO_BOOL_TO_LIST_INT: NativeTypeSpec = NativeTypeSpec::Function(&INT_TO_BOOL, &LIST_INT);
const INT_TO_STRING: NativeTypeSpec = NativeTypeSpec::Function(&INT, &STRING);
const LIST_INT_TO_INT: NativeTypeSpec = NativeTypeSpec::Function(&LIST_INT, &INT);
const OPTION_INT_TO_BOOL: NativeTypeSpec = NativeTypeSpec::Function(&OPTION_INT, &BOOL);
const STRING_TO_STRING_TO_BOOL: NativeTypeSpec = NativeTypeSpec::Function(&STRING, &STRING_TO_BOOL);
const OPTION_INT_TO_INT_TO_INT: NativeTypeSpec = NativeTypeSpec::Function(&OPTION_INT, &INT_TO_INT);
const LIST_BOOL_TO_BOOL: NativeTypeSpec = NativeTypeSpec::Function(&LIST_BOOL, &BOOL);
const LIST_INT_TO_INT_FN_TO_LIST_INT: NativeTypeSpec =
    NativeTypeSpec::Function(&LIST_INT, &INT_TO_INT_TO_LIST_INT);
const LIST_INT_TO_INT_BOOL_FN_TO_LIST_INT: NativeTypeSpec =
    NativeTypeSpec::Function(&LIST_INT, &INT_TO_BOOL_TO_LIST_INT);

const SHOW_TY: NativeTypeSpec = INT_TO_STRING;
const LENGTH_TY: NativeTypeSpec = LIST_INT_TO_INT;
const IS_SOME_TY: NativeTypeSpec = OPTION_INT_TO_BOOL;
const IS_NONE_TY: NativeTypeSpec = OPTION_INT_TO_BOOL;
const CONTAINS_TY: NativeTypeSpec = STRING_TO_STRING_TO_BOOL;
const STARTS_WITH_TY: NativeTypeSpec = STRING_TO_STRING_TO_BOOL;
const ENDS_WITH_TY: NativeTypeSpec = STRING_TO_STRING_TO_BOOL;
const UNWRAP_OR_TY: NativeTypeSpec = OPTION_INT_TO_INT_TO_INT;
const ALL_TY: NativeTypeSpec = LIST_BOOL_TO_BOOL;
const ANY_TY: NativeTypeSpec = LIST_BOOL_TO_BOOL;
const MAP_TY: NativeTypeSpec = LIST_INT_TO_INT_FN_TO_LIST_INT;
const FILTER_TY: NativeTypeSpec = LIST_INT_TO_INT_BOOL_FN_TO_LIST_INT;

struct NativeCall {
    name: &'static str,
    args: Vec<Value>,
}

impl NativeCall {
    fn new(name: &'static str, args: Vec<Value>) -> Self {
        Self { name, args }
    }

    fn apply(&self, implementation: NativeImplementation) -> Result<Value> {
        match implementation {
            NativeImplementation::Show => self.show(),
            NativeImplementation::Length => self.length(),
            NativeImplementation::IsSome => self.is_some(),
            NativeImplementation::IsNone => self.is_none(),
            NativeImplementation::Contains => self.contains(),
            NativeImplementation::StartsWith => self.starts_with(),
            NativeImplementation::EndsWith => self.ends_with(),
            NativeImplementation::UnwrapOr => self.unwrap_or(),
            NativeImplementation::All => self.all(),
            NativeImplementation::Any => self.any(),
            NativeImplementation::Map => self.map(),
            NativeImplementation::Filter => self.filter(),
        }
    }

    fn show(&self) -> Result<Value> {
        match self.args.as_slice() {
            [Value::Int(value)] => Ok(Value::String(value.to_string())),
            [Value::Float(value)] => Ok(Value::String(value.to_string())),
            [Value::Bool(value)] => Ok(Value::String(value.to_string())),
            [Value::String(value)] => Ok(Value::String(value.clone())),
            _ => self.unsupported_args(),
        }
    }

    fn length(&self) -> Result<Value> {
        match self.args.as_slice() {
            [Value::String(value)] => Ok(Value::Int(value.chars().count() as i64)),
            [Value::List(items)] => Ok(Value::Int(items.len() as i64)),
            _ => self.unsupported_args(),
        }
    }

    fn is_some(&self) -> Result<Value> {
        match self.args.as_slice() {
            [Value::Some(_)] => Ok(Value::Bool(true)),
            [Value::None] => Ok(Value::Bool(false)),
            _ => self.unsupported_args(),
        }
    }

    fn is_none(&self) -> Result<Value> {
        match self.args.as_slice() {
            [Value::None] => Ok(Value::Bool(true)),
            [Value::Some(_)] => Ok(Value::Bool(false)),
            _ => self.unsupported_args(),
        }
    }

    fn unwrap_or(&self) -> Result<Value> {
        match self.args.as_slice() {
            [Value::None, default] => Ok(default.clone()),
            [Value::Some(value), _] => Ok((**value).clone()),
            _ => self.unsupported_args(),
        }
    }

    fn contains(&self) -> Result<Value> {
        match self.args.as_slice() {
            [Value::String(haystack), Value::String(needle)] => {
                Ok(Value::Bool(haystack.contains(needle)))
            }
            [Value::List(items), needle] => {
                Ok(Value::Bool(items.iter().any(|item| value_eq(item, needle))))
            }
            _ => self.unsupported_args(),
        }
    }

    fn starts_with(&self) -> Result<Value> {
        match self.args.as_slice() {
            [Value::String(value), Value::String(prefix)] => {
                Ok(Value::Bool(value.starts_with(prefix)))
            }
            _ => self.unsupported_args(),
        }
    }

    fn ends_with(&self) -> Result<Value> {
        match self.args.as_slice() {
            [Value::String(value), Value::String(suffix)] => {
                Ok(Value::Bool(value.ends_with(suffix)))
            }
            _ => self.unsupported_args(),
        }
    }

    fn all(&self) -> Result<Value> {
        match self.args.as_slice() {
            [Value::List(items)] => items
                .iter()
                .try_fold(true, |acc, item| match item {
                    Value::Bool(value) => Ok(acc && *value),
                    _ => Err(Error::new("type mismatch: all expects [Bool]")),
                })
                .map(Value::Bool),
            _ => self.unsupported_args(),
        }
    }

    fn any(&self) -> Result<Value> {
        match self.args.as_slice() {
            [Value::List(items)] => items
                .iter()
                .try_fold(false, |acc, item| match item {
                    Value::Bool(value) => Ok(acc || *value),
                    _ => Err(Error::new("type mismatch: any expects [Bool]")),
                })
                .map(Value::Bool),
            _ => self.unsupported_args(),
        }
    }

    fn map(&self) -> Result<Value> {
        match self.args.as_slice() {
            [Value::List(items), function] => {
                let applicator = RuntimeValueApplicator::without_type_context();
                let mapped = items
                    .iter()
                    .map(|item| applicator.apply(function.clone(), item.clone()))
                    .collect::<Result<Vec<_>>>()?;
                Ok(Value::List(mapped))
            }
            _ => self.unsupported_args(),
        }
    }

    fn filter(&self) -> Result<Value> {
        match self.args.as_slice() {
            [Value::List(items), function] => {
                let applicator = RuntimeValueApplicator::without_type_context();
                let mut out = Vec::new();
                for item in items {
                    match applicator.apply(function.clone(), item.clone())? {
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
            _ => self.unsupported_args(),
        }
    }

    fn unsupported_args(&self) -> Result<Value> {
        Err(Error::with_code(
            ErrorCode::TypeUnsupportedBuiltinArg,
            format!("type mismatch: invalid arguments to native `{}`", self.name),
        ))
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
