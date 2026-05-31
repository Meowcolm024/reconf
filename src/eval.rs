use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::ast::{Decl, Expr, FileAst, StrPart, Type};
use crate::error::{Error, Result};
use crate::parser::parse;
use crate::prelude;

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
    Closure {
        param: String,
        body: Expr,
        env: Env,
    },
    Builtin {
        name: &'static str,
        args: Vec<Value>,
    },
    Native {
        name: String,
    },
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
            Value::Builtin { name, .. } => write!(f, "<builtin {name}>"),
            Value::Native { name } => write!(f, "<native {name}>"),
        }
    }
}

pub type Env = Rc<BTreeMap<String, Value>>;

#[derive(Clone, Default)]
pub struct Module {
    pub values: BTreeMap<String, Value>,
    pub types: BTreeMap<String, Type>,
    pub exports: BTreeMap<String, Export>,
}

#[derive(Clone)]
pub enum Export {
    Value(Value),
    Type(Type),
}

#[derive(Default)]
pub struct Loader {
    cache: HashMap<PathBuf, Module>,
    loading: HashSet<PathBuf>,
}

impl Loader {
    pub fn load(&mut self, path: &Path) -> Result<Module> {
        let path = path
            .canonicalize()
            .map_err(|e| Error::new(format!("unknown import `{}`: {e}", path.display())))?;
        if let Some(module) = self.cache.get(&path) {
            return Ok(module.clone());
        }
        if !self.loading.insert(path.clone()) {
            return Err(Error::new(format!("cyclic import `{}`", path.display())));
        }
        let src = fs::read_to_string(&path)
            .map_err(|e| Error::new(format!("unknown import `{}`: {e}", path.display())))?;
        let ast = parse(&src)?;
        let parent = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let module = eval_file(self, &parent, ast)?;
        self.loading.remove(&path);
        self.cache.insert(path, module.clone());
        Ok(module)
    }
}

pub fn eval_file(loader: &mut Loader, base_dir: &Path, ast: FileAst) -> Result<Module> {
    eval_file_inner(loader, base_dir, ast, true)
}

pub(crate) fn eval_file_without_prelude(
    loader: &mut Loader,
    base_dir: &Path,
    ast: FileAst,
) -> Result<Module> {
    eval_file_inner(loader, base_dir, ast, false)
}

fn eval_file_inner(
    loader: &mut Loader,
    base_dir: &Path,
    ast: FileAst,
    include_prelude: bool,
) -> Result<Module> {
    let mut module = if include_prelude {
        prelude::module()
    } else {
        Module::default()
    };

    for decl in ast.decls {
        match decl {
            Decl::Import { path, names } => {
                let imported = loader.load(&base_dir.join(path))?;
                for name in names {
                    if module.values.contains_key(&name) || module.types.contains_key(&name) {
                        return Err(Error::new(format!("duplicate import `{name}`")));
                    }
                    match imported.exports.get(&name) {
                        Some(Export::Value(value)) => {
                            module.values.insert(name, value.clone());
                        }
                        Some(Export::Type(ty)) => {
                            module.types.insert(name, ty.clone());
                        }
                        None => return Err(Error::new(format!("unexported import `{name}`"))),
                    }
                }
            }
            Decl::Native { export, name, ty } => {
                well_formed_type(&ty, &module.types)?;
                let value = prelude::native_value(&name)
                    .ok_or_else(|| Error::new(format!("unknown native `{name}`")))?;
                module.values.insert(name.clone(), value.clone());
                if export {
                    module.exports.insert(name, Export::Value(value));
                }
            }
            Decl::Type { export, name, ty } => {
                well_formed_type(&ty, &module.types)?;
                module.types.insert(name.clone(), ty.clone());
                if export {
                    module.exports.insert(name, Export::Type(ty));
                }
            }
            Decl::Let {
                export,
                name,
                annotation,
                expr,
            } => {
                let env = Rc::new(module.values.clone());
                let value = if let Some(ty) = annotation {
                    well_formed_type(&ty, &module.types)?;
                    check_expr(&expr, &ty, &env, &module.types)?
                } else {
                    eval(&expr, &env, &module.types)?
                };
                module.values.insert(name.clone(), value.clone());
                if export {
                    module.exports.insert(name, Export::Value(value));
                }
            }
        }
    }

    let env = Rc::new(module.values.clone());
    let output = eval(&ast.output, &env, &module.types)?;
    if contains_function(&output) {
        return Err(Error::new("function escaped into output"));
    }
    module.values.insert("$output".to_string(), output);
    Ok(module)
}

fn well_formed_type(ty: &Type, aliases: &BTreeMap<String, Type>) -> Result<()> {
    match ty {
        Type::Alias(name) if !aliases.contains_key(name) => {
            Err(Error::new(format!("unknown type `{name}`")))
        }
        Type::Option(inner) | Type::List(inner) => well_formed_type(inner, aliases),
        Type::Record(fields) => {
            for field in fields.values() {
                well_formed_type(field, aliases)?;
            }
            Ok(())
        }
        Type::Refinement { base, .. } => well_formed_type(base, aliases),
        Type::Function(a, b) => {
            well_formed_type(a, aliases)?;
            well_formed_type(b, aliases)
        }
        _ => Ok(()),
    }
}

fn env_with(env: &Env, name: String, value: Value) -> Env {
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
        Expr::Interp(parts) => {
            let mut out = String::new();
            for part in parts {
                match part {
                    StrPart::Text(text) => out.push_str(text),
                    StrPart::Expr(expr) => out.push_str(&show_value(&eval(expr, env, aliases)?)?),
                }
            }
            Ok(Value::String(out))
        }
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

fn check_expr(
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

fn check_value_against(
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

fn validate_refinement(
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

fn expand_type(ty: &Type, aliases: &BTreeMap<String, Type>) -> Result<Type> {
    match ty {
        Type::Alias(name) => {
            let ty = aliases
                .get(name)
                .ok_or_else(|| Error::new(format!("unknown type `{name}`")))?;
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

fn is_option_type(ty: &Type, aliases: &BTreeMap<String, Type>) -> Result<bool> {
    Ok(matches!(expand_type(ty, aliases)?, Type::Option(_)))
}

fn matches_type(value: &Value, ty: &Type, aliases: &BTreeMap<String, Type>) -> Result<bool> {
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
        Type::Function(_, _) => matches!(
            value,
            Value::Closure { .. } | Value::Builtin { .. } | Value::Native { .. }
        ),
        Type::Alias(_) => unreachable!(),
    })
}

fn apply(function: Value, arg: Value, aliases: &BTreeMap<String, Type>) -> Result<Value> {
    match function {
        Value::Closure { param, body, env } => eval(&body, &env_with(&env, param, arg), aliases),
        Value::Native { name } => apply_native(name, vec![arg], aliases),
        Value::Builtin { name, mut args } => {
            args.push(arg);
            let arity = builtin_arity(name);
            if args.len() == arity {
                apply_builtin(name, args, aliases)
            } else {
                Ok(Value::Builtin { name, args })
            }
        }
        _ => Err(Error::new("type mismatch: applying non-function")),
    }
}

fn apply_native(
    name: String,
    args: Vec<Value>,
    _aliases: &BTreeMap<String, Type>,
) -> Result<Value> {
    if args.len() != 1 {
        return Err(Error::new(format!(
            "type mismatch: invalid arguments to native `{name}`"
        )));
    }
    match (name.as_str(), args.as_slice()) {
        ("showInt", [Value::Int(value)]) => Ok(Value::String(value.to_string())),
        ("showFloat", [Value::Float(value)]) => Ok(Value::String(value.to_string())),
        ("showBool", [Value::Bool(value)]) => Ok(Value::String(value.to_string())),
        ("lengthString", [Value::String(value)]) => Ok(Value::Int(value.chars().count() as i64)),
        ("lengthList", [Value::List(items)]) => Ok(Value::Int(items.len() as i64)),
        ("isSome", [Value::Some(_)]) => Ok(Value::Bool(true)),
        ("isSome", [Value::None]) => Ok(Value::Bool(false)),
        ("isNone", [Value::None]) => Ok(Value::Bool(true)),
        ("isNone", [Value::Some(_)]) => Ok(Value::Bool(false)),
        _ => Err(Error::new(format!(
            "type mismatch: invalid arguments to native `{name}`"
        ))),
    }
}

fn builtin_arity(name: &str) -> usize {
    match name {
        "show" | "length" | "isSome" | "isNone" => 1,
        _ => 2,
    }
}

fn apply_builtin(name: &str, args: Vec<Value>, aliases: &BTreeMap<String, Type>) -> Result<Value> {
    match (name, args.as_slice()) {
        ("show", [value]) => Ok(Value::String(show_value(value)?)),
        ("length", [Value::List(items)]) => Ok(Value::Int(items.len() as i64)),
        ("length", [Value::String(text)]) => Ok(Value::Int(text.chars().count() as i64)),
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
            Ok(Value::Bool(items.iter().any(|x| value_eq(x, needle))))
        }
        ("startsWith", [Value::String(value), Value::String(prefix)]) => {
            Ok(Value::Bool(value.starts_with(prefix)))
        }
        ("endsWith", [Value::String(value), Value::String(suffix)]) => {
            Ok(Value::Bool(value.ends_with(suffix)))
        }
        ("map", [Value::List(items), function]) => {
            let mapped = items
                .iter()
                .map(|item| apply(function.clone(), item.clone(), aliases))
                .collect::<Result<Vec<_>>>()?;
            Ok(Value::List(mapped))
        }
        ("filter", [Value::List(items), function]) => {
            let mut out = Vec::new();
            for item in items {
                match apply(function.clone(), item.clone(), aliases)? {
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
                match apply(function.clone(), item.clone(), aliases)? {
                    Value::Bool(true) => {}
                    Value::Bool(false) => return Ok(Value::Bool(false)),
                    _ => return Err(Error::new("type mismatch: all predicate must return Bool")),
                }
            }
            Ok(Value::Bool(true))
        }
        ("any", [Value::List(items), function]) => {
            for item in items {
                match apply(function.clone(), item.clone(), aliases)? {
                    Value::Bool(true) => return Ok(Value::Bool(true)),
                    Value::Bool(false) => {}
                    _ => return Err(Error::new("type mismatch: any predicate must return Bool")),
                }
            }
            Ok(Value::Bool(false))
        }
        _ => Err(Error::new(format!(
            "type mismatch: invalid arguments to `{name}`"
        ))),
    }
}

fn binary(op: &str, a: Value, b: Value) -> Result<Value> {
    match (op, a, b) {
        ("+", Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
        ("-", Value::Int(a), Value::Int(b)) => Ok(Value::Int(a - b)),
        ("*", Value::Int(a), Value::Int(b)) => Ok(Value::Int(a * b)),
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

fn show_value(value: &Value) -> Result<String> {
    Ok(match value {
        Value::Int(x) => x.to_string(),
        Value::Float(x) => x.to_string(),
        Value::Bool(x) => x.to_string(),
        Value::String(x) => x.clone(),
        _ => return Err(Error::new("type mismatch: cannot show value")),
    })
}

fn contains_function(value: &Value) -> bool {
    match value {
        Value::Closure { .. } | Value::Builtin { .. } => true,
        Value::Native { .. } => true,
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
        Value::Closure { .. } | Value::Builtin { .. } | Value::Native { .. } => {
            return Err(Error::new("function escaped into output"));
        }
    })
}

fn type_name(ty: &Type) -> &'static str {
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

fn value_name(value: &Value) -> &'static str {
    match value {
        Value::Int(_) => "Int",
        Value::Float(_) => "Float",
        Value::Bool(_) => "Bool",
        Value::String(_) => "String",
        Value::None | Value::Some(_) => "option",
        Value::List(_) => "list",
        Value::Record(_) => "record",
        Value::Closure { .. } | Value::Builtin { .. } | Value::Native { .. } => "function",
    }
}

pub fn run(path: &Path) -> Result<String> {
    let mut loader = Loader::default();
    let module = loader.load(path)?;
    let output = module
        .values
        .get("$output")
        .ok_or_else(|| Error::new("internal error: missing output"))?;
    emit(output)
}
