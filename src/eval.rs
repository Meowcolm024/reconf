use std::collections::HashMap;
use std::rc::Rc;

use crate::diagnostic::{Diagnostic, Span};
use crate::syntax::{BinaryOp, Expr, ExprKind, InterpPart, UnaryOp};

#[derive(Clone, Debug)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    None,
    Some(Box<Value>),
    List(Vec<Value>),
    Record(Vec<(String, Value)>),
    Closure(Rc<Closure>),
    Builtin { name: String, args: Vec<Value> },
}

#[derive(Clone, Debug)]
pub struct Closure {
    param: String,
    body: Expr,
    env: RuntimeEnv,
}

pub(crate) type RuntimeEnv = HashMap<String, Value>;

fn is_builtin_name(name: &str) -> bool {
    matches!(
        name,
        "show"
            | "isSome"
            | "isNone"
            | "length"
            | "all"
            | "any"
            | "contains"
            | "startsWith"
            | "endsWith"
            | "unwrapOr"
    )
}

pub(crate) fn eval(expr: &Expr, env: &RuntimeEnv, span: Span) -> Result<Value, Diagnostic> {
    match &expr.kind {
        ExprKind::Int(value) => Ok(Value::Int(*value)),
        ExprKind::Float(value) => Ok(Value::Float(*value)),
        ExprKind::Bool(value) => Ok(Value::Bool(*value)),
        ExprKind::String(value) => Ok(Value::String(value.clone())),
        ExprKind::Interp(parts) => {
            let mut out = String::new();
            for part in parts {
                match part {
                    InterpPart::Text(text) => out.push_str(text),
                    InterpPart::Expr(expr) => {
                        let value = eval(expr, env, expr.span)?;
                        out.push_str(&show_value(&value, expr.span)?);
                    }
                }
            }
            Ok(Value::String(out))
        }
        ExprKind::Var(name) => env
            .get(name)
            .cloned()
            .or_else(|| {
                is_builtin_name(name).then(|| Value::Builtin {
                    name: name.clone(),
                    args: Vec::new(),
                })
            })
            .ok_or_else(|| {
                Diagnostic::new(
                    "E_RUNTIME_001",
                    format!("unknown runtime identifier `{name}`"),
                    expr.span,
                )
            }),
        ExprKind::None => Ok(Value::None),
        ExprKind::Some(value) => Ok(Value::Some(Box::new(eval(value, env, value.span)?))),
        ExprKind::List(items) => items
            .iter()
            .map(|item| eval(item, env, item.span))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::List),
        ExprKind::Record(fields) => fields
            .iter()
            .map(|field| {
                Ok((
                    field.name.clone(),
                    eval(&field.value, env, field.value.span)?,
                ))
            })
            .collect::<Result<Vec<_>, Diagnostic>>()
            .map(Value::Record),
        ExprKind::Field(base, name) => {
            let base_value = eval(base, env, base.span)?;
            match base_value {
                Value::Record(fields) => fields
                    .into_iter()
                    .find(|(field, _)| field == name)
                    .map(|(_, value)| value)
                    .ok_or_else(|| {
                        Diagnostic::new(
                            "E_RUNTIME_002",
                            format!("unknown field `{name}`"),
                            expr.span,
                        )
                    }),
                value => eval_method(name, value, expr.span),
            }
        }
        ExprKind::If {
            cond,
            then_expr,
            else_expr,
        } => match eval(cond, env, cond.span)? {
            Value::Bool(true) => eval(then_expr, env, then_expr.span),
            Value::Bool(false) => eval(else_expr, env, else_expr.span),
            other => Err(Diagnostic::new(
                "E_RUNTIME_003",
                format!("if condition evaluated to `{}`", value_debug(&other)),
                cond.span,
            )),
        },
        ExprKind::Let {
            name, value, body, ..
        } => {
            let value = eval(value, env, value.span)?;
            let mut local = env.clone();
            local.insert(name.clone(), value);
            eval(body, &local, body.span)
        }
        ExprKind::Lam { param, body, .. } => Ok(Value::Closure(Rc::new(Closure {
            param: param.clone(),
            body: (*body.clone()),
            env: env.clone(),
        }))),
        ExprKind::App(func, arg) => {
            let func_value = eval(func, env, func.span)?;
            let arg_value = eval(arg, env, arg.span)?;
            apply_value(func_value, arg_value, expr.span)
        }
        ExprKind::Ascribe(inner, _) => eval(inner, env, inner.span),
        ExprKind::Unary(op, inner) => {
            let value = eval(inner, env, inner.span)?;
            match (op, value) {
                (UnaryOp::Not, Value::Bool(value)) => Ok(Value::Bool(!value)),
                (UnaryOp::Neg, Value::Int(value)) => Ok(Value::Int(-value)),
                (UnaryOp::Neg, Value::Float(value)) => Ok(Value::Float(-value)),
                (_, other) => Err(Diagnostic::new(
                    "E_RUNTIME_004",
                    format!("invalid unary operand `{}`", value_debug(&other)),
                    expr.span,
                )),
            }
        }
        ExprKind::Binary(op, left, right) => {
            let left = eval(left, env, left.span)?;
            if matches!(op, BinaryOp::And) {
                return match left {
                    Value::Bool(false) => Ok(Value::Bool(false)),
                    Value::Bool(true) => eval(right, env, right.span),
                    other => Err(Diagnostic::new(
                        "E_RUNTIME_005",
                        format!("invalid boolean operand `{}`", value_debug(&other)),
                        expr.span,
                    )),
                };
            }
            if matches!(op, BinaryOp::Or) {
                return match left {
                    Value::Bool(true) => Ok(Value::Bool(true)),
                    Value::Bool(false) => eval(right, env, right.span),
                    other => Err(Diagnostic::new(
                        "E_RUNTIME_006",
                        format!("invalid boolean operand `{}`", value_debug(&other)),
                        expr.span,
                    )),
                };
            }
            let right = eval(right, env, right.span)?;
            eval_binary(*op, left, right, expr.span)
        }
    }
    .map_err(|err| {
        if err.span.end == 0 {
            Diagnostic { span, ..err }
        } else {
            err
        }
    })
}

fn eval_method(name: &str, receiver: Value, span: Span) -> Result<Value, Diagnostic> {
    match name {
        "isSome" => Ok(Value::Bool(matches!(receiver, Value::Some(_)))),
        "isNone" => Ok(Value::Bool(matches!(receiver, Value::None))),
        "length" => match receiver {
            Value::String(value) => Ok(Value::Int(value.chars().count() as i64)),
            Value::List(items) => Ok(Value::Int(items.len() as i64)),
            other => Err(Diagnostic::new(
                "E_RUNTIME_007",
                format!("length is not supported on `{}`", value_debug(&other)),
                span,
            )),
        },
        "all" => match receiver {
            Value::List(items) => Ok(Value::Bool(
                items.iter().all(|item| matches!(item, Value::Bool(true))),
            )),
            other => Err(Diagnostic::new(
                "E_RUNTIME_022",
                format!("all is not supported on `{}`", value_debug(&other)),
                span,
            )),
        },
        "any" => match receiver {
            Value::List(items) => Ok(Value::Bool(
                items.iter().any(|item| matches!(item, Value::Bool(true))),
            )),
            other => Err(Diagnostic::new(
                "E_RUNTIME_023",
                format!("any is not supported on `{}`", value_debug(&other)),
                span,
            )),
        },
        "contains" | "startsWith" | "endsWith" | "unwrapOr" => Ok(Value::Builtin {
            name: name.to_string(),
            args: vec![receiver],
        }),
        _ => Err(Diagnostic::new(
            "E_RUNTIME_008",
            format!("unsupported method `{name}`"),
            span,
        )),
    }
}

fn apply_value(func: Value, arg: Value, span: Span) -> Result<Value, Diagnostic> {
    match func {
        Value::Closure(closure) => {
            let mut env = closure.env.clone();
            env.insert(closure.param.clone(), arg);
            eval(&closure.body, &env, closure.body.span)
        }
        Value::Builtin { name, mut args } => {
            args.push(arg);
            apply_builtin(name, args, span)
        }
        other => Err(Diagnostic::new(
            "E_RUNTIME_009",
            format!("cannot apply `{}`", value_debug(&other)),
            span,
        )),
    }
}

fn apply_builtin(name: String, args: Vec<Value>, span: Span) -> Result<Value, Diagnostic> {
    let arity = match name.as_str() {
        "show" | "isSome" | "isNone" | "length" | "all" | "any" => 1,
        "contains" | "startsWith" | "endsWith" | "unwrapOr" => 2,
        _ => {
            return Err(Diagnostic::new(
                "E_RUNTIME_010",
                format!("unknown built-in `{name}`"),
                span,
            ));
        }
    };
    if args.len() < arity {
        return Ok(Value::Builtin { name, args });
    }
    match name.as_str() {
        "show" => Ok(Value::String(show_value(&args[0], span)?)),
        "isSome" => Ok(Value::Bool(matches!(args[0], Value::Some(_)))),
        "isNone" => Ok(Value::Bool(matches!(args[0], Value::None))),
        "length" => match &args[0] {
            Value::String(value) => Ok(Value::Int(value.chars().count() as i64)),
            Value::List(items) => Ok(Value::Int(items.len() as i64)),
            other => Err(Diagnostic::new(
                "E_RUNTIME_011",
                format!("length is not supported on `{}`", value_debug(other)),
                span,
            )),
        },
        "all" => match &args[0] {
            Value::List(items) => Ok(Value::Bool(
                items.iter().all(|item| matches!(item, Value::Bool(true))),
            )),
            other => Err(Diagnostic::new(
                "E_RUNTIME_024",
                format!("all is not supported on `{}`", value_debug(other)),
                span,
            )),
        },
        "any" => match &args[0] {
            Value::List(items) => Ok(Value::Bool(
                items.iter().any(|item| matches!(item, Value::Bool(true))),
            )),
            other => Err(Diagnostic::new(
                "E_RUNTIME_025",
                format!("any is not supported on `{}`", value_debug(other)),
                span,
            )),
        },
        "contains" => match (&args[0], &args[1]) {
            (Value::String(haystack), Value::String(needle)) => {
                Ok(Value::Bool(haystack.contains(needle)))
            }
            (Value::List(items), needle) => Ok(Value::Bool(
                items.iter().any(|item| values_equal(item, needle)),
            )),
            (other, _) => Err(Diagnostic::new(
                "E_RUNTIME_012",
                format!("contains is not supported on `{}`", value_debug(other)),
                span,
            )),
        },
        "startsWith" => match (&args[0], &args[1]) {
            (Value::String(value), Value::String(prefix)) => {
                Ok(Value::Bool(value.starts_with(prefix)))
            }
            (other, _) => Err(Diagnostic::new(
                "E_RUNTIME_013",
                format!("startsWith is not supported on `{}`", value_debug(other)),
                span,
            )),
        },
        "endsWith" => match (&args[0], &args[1]) {
            (Value::String(value), Value::String(suffix)) => {
                Ok(Value::Bool(value.ends_with(suffix)))
            }
            (other, _) => Err(Diagnostic::new(
                "E_RUNTIME_014",
                format!("endsWith is not supported on `{}`", value_debug(other)),
                span,
            )),
        },
        "unwrapOr" => match &args[0] {
            Value::Some(value) => Ok((**value).clone()),
            Value::None => Ok(args[1].clone()),
            other => Err(Diagnostic::new(
                "E_RUNTIME_015",
                format!("unwrapOr is not supported on `{}`", value_debug(other)),
                span,
            )),
        },
        _ => unreachable!(),
    }
}

fn eval_binary(op: BinaryOp, left: Value, right: Value, span: Span) -> Result<Value, Diagnostic> {
    match op {
        BinaryOp::Add => match (left, right) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
            (a, b) => runtime_type_error("addition", &a, &b, span),
        },
        BinaryOp::Sub => match (left, right) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a - b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
            (a, b) => runtime_type_error("subtraction", &a, &b, span),
        },
        BinaryOp::Mul => match (left, right) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a * b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
            (a, b) => runtime_type_error("multiplication", &a, &b, span),
        },
        BinaryOp::Div => match (left, right) {
            (Value::Int(_), Value::Int(0)) => {
                Err(Diagnostic::new("E_RUNTIME_016", "division by zero", span))
            }
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a / b)),
            (Value::Float(_), Value::Float(0.0)) => {
                Err(Diagnostic::new("E_RUNTIME_017", "division by zero", span))
            }
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),
            (a, b) => runtime_type_error("division", &a, &b, span),
        },
        BinaryOp::Mod => match (left, right) {
            (Value::Int(_), Value::Int(0)) => {
                Err(Diagnostic::new("E_RUNTIME_018", "modulo by zero", span))
            }
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a % b)),
            (a, b) => runtime_type_error("modulo", &a, &b, span),
        },
        BinaryOp::Concat => match (left, right) {
            (Value::String(a), Value::String(b)) => Ok(Value::String(format!("{a}{b}"))),
            (a, b) => runtime_type_error("string concatenation", &a, &b, span),
        },
        BinaryOp::Eq => Ok(Value::Bool(values_equal(&left, &right))),
        BinaryOp::Ne => Ok(Value::Bool(!values_equal(&left, &right))),
        BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
            order_values(op, left, right, span)
        }
        BinaryOp::And | BinaryOp::Or => unreachable!("short-circuited in eval"),
    }
}

fn runtime_type_error(
    op: &str,
    left: &Value,
    right: &Value,
    span: Span,
) -> Result<Value, Diagnostic> {
    Err(Diagnostic::new(
        "E_RUNTIME_019",
        format!(
            "{op} is not supported for `{}` and `{}`",
            value_debug(left),
            value_debug(right)
        ),
        span,
    ))
}

fn order_values(op: BinaryOp, left: Value, right: Value, span: Span) -> Result<Value, Diagnostic> {
    let ordering = match (left, right) {
        (Value::Int(a), Value::Int(b)) => a.partial_cmp(&b),
        (Value::Float(a), Value::Float(b)) => a.partial_cmp(&b),
        (Value::String(a), Value::String(b)) => a.partial_cmp(&b),
        (a, b) => {
            return runtime_type_error("ordering", &a, &b, span);
        }
    }
    .ok_or_else(|| Diagnostic::new("E_RUNTIME_020", "values are not orderable", span))?;
    Ok(Value::Bool(match op {
        BinaryOp::Lt => ordering.is_lt(),
        BinaryOp::Le => ordering.is_le(),
        BinaryOp::Gt => ordering.is_gt(),
        BinaryOp::Ge => ordering.is_ge(),
        _ => unreachable!(),
    }))
}

fn values_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Int(a), Value::Int(b)) => a == b,
        (Value::Float(a), Value::Float(b)) => a == b,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::String(a), Value::String(b)) => a == b,
        (Value::None, Value::None) => true,
        (Value::Some(a), Value::Some(b)) => values_equal(a, b),
        (Value::List(a), Value::List(b)) => {
            a.len() == b.len() && a.iter().zip(b.iter()).all(|(a, b)| values_equal(a, b))
        }
        (Value::Record(a), Value::Record(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|((ak, av), (bk, bv))| ak == bk && values_equal(av, bv))
        }
        _ => false,
    }
}

fn show_value(value: &Value, span: Span) -> Result<String, Diagnostic> {
    match value {
        Value::Int(value) => Ok(value.to_string()),
        Value::Float(value) if value.is_finite() => Ok(value.to_string()),
        Value::Bool(value) => Ok(value.to_string()),
        Value::String(value) => Ok(value.clone()),
        _ => Err(Diagnostic::new(
            "E_RUNTIME_021",
            format!("cannot show `{}`", value_debug(value)),
            span,
        )),
    }
}

pub(crate) fn reject_function_output(value: &Value, span: Span) -> Result<(), Diagnostic> {
    match value {
        Value::Closure(_) | Value::Builtin { .. } => Err(Diagnostic::new(
            "E_OUTPUT_001",
            "function escaped into output",
            span,
        )),
        Value::Some(value) => reject_function_output(value, span),
        Value::List(values) => {
            for value in values {
                reject_function_output(value, span)?;
            }
            Ok(())
        }
        Value::Record(fields) => {
            for (_, value) in fields {
                reject_function_output(value, span)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

pub(crate) fn value_debug(value: &Value) -> String {
    match value {
        Value::Int(value) => value.to_string(),
        Value::Float(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::String(value) => format!("{value:?}"),
        Value::None => "none".to_string(),
        Value::Some(value) => format!("some {}", value_debug(value)),
        Value::List(items) => format!(
            "[{}]",
            items.iter().map(value_debug).collect::<Vec<_>>().join(", ")
        ),
        Value::Record(fields) => format!(
            "{{ {} }}",
            fields
                .iter()
                .map(|(name, value)| format!("{name} = {}", value_debug(value)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Closure(_) => "<function>".to_string(),
        Value::Builtin { name, .. } => format!("<builtin {name}>"),
    }
}
