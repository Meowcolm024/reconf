use std::collections::BTreeMap;

use crate::core::{CoreExpr, CoreType, CoreTypeContext, EmptyCoreTypeContext, TypedCoreExpr};
use crate::error::{Error, ErrorCode, Result};
use crate::eval::{Env, Value, binary};
use crate::refine::validate::{
    CheckedRefinementPredicateBuilder, CoreRefinementValidator, RefinementValidationOptions,
};

pub struct PreparedCoreNormalizer<'a> {
    env: Env,
    types: &'a dyn CoreTypeContext,
}

impl<'a> PreparedCoreNormalizer<'a> {
    pub fn new(env: Env, types: &'a dyn CoreTypeContext) -> Self {
        Self { env, types }
    }

    pub fn synthesize(&self, expr: CoreExpr) -> Result<Value> {
        self.synthesize_prepared(expr)
    }

    pub fn evaluate_typed(&self, typed: TypedCoreExpr) -> Result<Value> {
        let value_span = typed.expr.origin_span();
        let value = self.eval_prepared(typed.expr)?;
        CoreValueChecker::new(self.types, self.env.clone()).check_with_context(
            value,
            &typed.ty,
            CheckContext { value_span },
        )
    }

    fn eval_prepared(&self, expr: CoreExpr) -> Result<Value> {
        match expr {
            CoreExpr::Spanned(expr, span) => self
                .eval_prepared(*expr)
                .map_err(|error| label_origin_error(error, span)),
            CoreExpr::Int(value) => Ok(Value::Int(value)),
            CoreExpr::Float(value) => Ok(Value::Float(value)),
            CoreExpr::Bool(value) => Ok(Value::Bool(value)),
            CoreExpr::String(value) => Ok(Value::String(value)),
            CoreExpr::None => Ok(Value::None),
            CoreExpr::Ascribe(_, _) => Err(Error::new(
                "internal error: normalizer received unelaborated ascription",
            )),
            CoreExpr::Some(expr) => Ok(Value::Some(Box::new(self.eval_prepared(*expr)?))),
            CoreExpr::Var(name) => self
                .env
                .get(&name)
                .cloned()
                .ok_or_else(|| Error::new(format!("unknown identifier `{name}`"))),
            CoreExpr::Global(binding) => self
                .env
                .global(binding)
                .cloned()
                .ok_or_else(|| Error::new("internal error: invalid global reference")),
            CoreExpr::Local(local) => self
                .env
                .local(local.index())
                .cloned()
                .ok_or_else(|| Error::new("internal error: invalid local reference")),
            CoreExpr::List(items) => items
                .into_iter()
                .map(|item| self.eval_prepared(item))
                .collect::<Result<Vec<_>>>()
                .map(Value::List),
            CoreExpr::Record(fields) => fields
                .into_iter()
                .map(|(name, expr)| Ok((name, self.eval_prepared(expr)?)))
                .collect::<Result<BTreeMap<_, _>>>()
                .map(Value::Record),
            CoreExpr::Field(expr, name) => match self.eval_prepared(*expr)? {
                receiver @ Value::Record(_) => {
                    if let Value::Record(fields) = &receiver
                        && let Some(value) = fields.get(&name)
                    {
                        return Ok(value.clone());
                    }
                    let method = self
                        .env
                        .get(&name)
                        .cloned()
                        .ok_or_else(|| Error::new(format!("unknown field `{name}`")))?;
                    self.apply(method, receiver)
                }
                receiver => {
                    let method = self
                        .env
                        .get(&name)
                        .cloned()
                        .ok_or_else(|| Error::new(format!("unknown field `{name}`")))?;
                    self.apply(method, receiver)
                }
            },
            CoreExpr::If(cond, then_expr, else_expr) => match self.eval_prepared(*cond)? {
                Value::Bool(true) => self.eval_prepared(*then_expr),
                Value::Bool(false) => self.eval_prepared(*else_expr),
                _ => Err(Error::new("type mismatch: if condition must be Bool")),
            },
            CoreExpr::Let(name, None, value, body) => {
                let value = self.eval_prepared(*value)?;
                PreparedCoreNormalizer::new(
                    self.env.extend(name, value.clone()).push_local(value),
                    self.types,
                )
                .eval_prepared(*body)
            }
            CoreExpr::Let(_, Some(_), _, _) => Err(Error::new(
                "internal error: normalizer received unelaborated annotated let",
            )),
            CoreExpr::Lambda(param, _ty, body) => Ok(Value::CoreClosure {
                param,
                body: *body,
                env: self.env.clone(),
            }),
            CoreExpr::Apply(function, arg) => {
                let function = self.eval_prepared(*function)?;
                let arg = self.eval_prepared(*arg)?;
                self.apply(function, arg)
            }
            CoreExpr::Unary(op, expr) => {
                let value = self.eval_prepared(*expr)?;
                match (op.as_str(), value) {
                    ("!", Value::Bool(value)) => Ok(Value::Bool(!value)),
                    ("-", Value::Int(value)) => Ok(Value::Int(-value)),
                    ("-", Value::Float(value)) => Ok(Value::Float(-value)),
                    _ => Err(Error::new(format!("type mismatch: invalid unary `{op}`"))),
                }
            }
            CoreExpr::Binary(op, left, right) => {
                if op == "&&" {
                    return match self.eval_prepared(*left)? {
                        Value::Bool(false) => Ok(Value::Bool(false)),
                        Value::Bool(true) => match self.eval_prepared(*right)? {
                            Value::Bool(value) => Ok(Value::Bool(value)),
                            _ => Err(Error::new("type mismatch: && expects Bool")),
                        },
                        _ => Err(Error::new("type mismatch: && expects Bool")),
                    };
                }
                if op == "||" {
                    return match self.eval_prepared(*left)? {
                        Value::Bool(true) => Ok(Value::Bool(true)),
                        Value::Bool(false) => match self.eval_prepared(*right)? {
                            Value::Bool(value) => Ok(Value::Bool(value)),
                            _ => Err(Error::new("type mismatch: || expects Bool")),
                        },
                        _ => Err(Error::new("type mismatch: || expects Bool")),
                    };
                }
                self.eval_prepared_binary(&op, *left, *right)
            }
        }
    }

    fn synthesize_prepared(&self, expr: CoreExpr) -> Result<Value> {
        match expr {
            CoreExpr::Spanned(expr, span) => self
                .synthesize_prepared(*expr)
                .map_err(|error| label_origin_error(error, span)),
            CoreExpr::Int(value) => Ok(Value::Int(value)),
            CoreExpr::Float(value) => Ok(Value::Float(value)),
            CoreExpr::Bool(value) => Ok(Value::Bool(value)),
            CoreExpr::String(value) => Ok(Value::String(value)),
            CoreExpr::None => Err(Error::with_code(
                ErrorCode::TypeNoneNeedsExpected,
                "`none` requires an expected option type",
            )),
            CoreExpr::Some(expr) => Ok(Value::Some(Box::new(self.synthesize_prepared(*expr)?))),
            CoreExpr::Var(name) => self
                .env
                .get(&name)
                .cloned()
                .ok_or_else(|| Error::new(format!("unknown identifier `{name}`"))),
            CoreExpr::Global(binding) => self
                .env
                .global(binding)
                .cloned()
                .ok_or_else(|| Error::new("internal error: invalid global reference")),
            CoreExpr::Local(local) => self
                .env
                .local(local.index())
                .cloned()
                .ok_or_else(|| Error::new("internal error: invalid local reference")),
            CoreExpr::List(items) if items.is_empty() => Err(Error::with_code(
                ErrorCode::TypeNoneNeedsExpected,
                "empty lists require an expected list type",
            )),
            CoreExpr::List(items) => items
                .into_iter()
                .map(|item| self.synthesize_prepared(item))
                .collect::<Result<Vec<_>>>()
                .map(Value::List),
            CoreExpr::Record(fields) => fields
                .into_iter()
                .map(|(name, expr)| Ok((name, self.synthesize_prepared(expr)?)))
                .collect::<Result<BTreeMap<_, _>>>()
                .map(Value::Record),
            CoreExpr::Field(expr, name) => match self.synthesize_prepared(*expr)? {
                receiver @ Value::Record(_) => {
                    if let Value::Record(fields) = &receiver
                        && let Some(value) = fields.get(&name)
                    {
                        return Ok(value.clone());
                    }
                    let method = self
                        .env
                        .get(&name)
                        .cloned()
                        .ok_or_else(|| Error::new(format!("unknown field `{name}`")))?;
                    self.apply(method, receiver)
                }
                receiver => {
                    let method = self
                        .env
                        .get(&name)
                        .cloned()
                        .ok_or_else(|| Error::new(format!("unknown field `{name}`")))?;
                    self.apply(method, receiver)
                }
            },
            CoreExpr::If(cond, then_expr, else_expr) => match self.synthesize_prepared(*cond)? {
                Value::Bool(true) => self.synthesize_prepared(*then_expr),
                Value::Bool(false) => self.synthesize_prepared(*else_expr),
                _ => Err(Error::new("type mismatch: if condition must be Bool")),
            },
            CoreExpr::Let(name, None, value, body) => {
                let value = self.synthesize_prepared(*value)?;
                PreparedCoreNormalizer::new(
                    self.env.extend(name, value.clone()).push_local(value),
                    self.types,
                )
                .synthesize_prepared(*body)
            }
            CoreExpr::Let(_, Some(_), _, _) => Err(Error::new(
                "internal error: normalizer received unelaborated annotated let",
            )),
            CoreExpr::Lambda(param, _ty, body) => Ok(Value::CoreClosure {
                param,
                body: *body,
                env: self.env.clone(),
            }),
            CoreExpr::Apply(function, arg) => {
                if self.is_show(&function) {
                    return self.synthesize_show_application(*arg);
                }
                let function = self.synthesize_prepared(*function)?;
                let arg = self.synthesize_prepared(*arg)?;
                self.apply(function, arg)
            }
            CoreExpr::Ascribe(_, _) => Err(Error::new(
                "internal error: normalizer received unelaborated ascription",
            )),
            CoreExpr::Unary(op, expr) => {
                let value = self.synthesize_prepared(*expr)?;
                match (op.as_str(), value) {
                    ("!", Value::Bool(value)) => Ok(Value::Bool(!value)),
                    ("-", Value::Int(value)) => Ok(Value::Int(-value)),
                    ("-", Value::Float(value)) => Ok(Value::Float(-value)),
                    _ => Err(Error::new(format!("type mismatch: invalid unary `{op}`"))),
                }
            }
            CoreExpr::Binary(op, left, right) => {
                if op == "&&" {
                    return self.synthesize_and(*left, *right);
                }
                if op == "||" {
                    return self.synthesize_or(*left, *right);
                }
                if op == "++" {
                    return self.synthesize_string_concat(*left, *right);
                }
                self.synthesize_prepared_binary(&op, *left, *right)
            }
        }
    }

    fn eval_prepared_binary(&self, op: &str, left: CoreExpr, right: CoreExpr) -> Result<Value> {
        let right_span = right.origin_span();
        binary(op, self.eval_prepared(left)?, self.eval_prepared(right)?)
            .map_err(|error| label_binary_error(error, right_span))
    }

    fn synthesize_prepared_binary(
        &self,
        op: &str,
        left: CoreExpr,
        right: CoreExpr,
    ) -> Result<Value> {
        let right_span = right.origin_span();
        binary(
            op,
            self.synthesize_prepared(left)?,
            self.synthesize_prepared(right)?,
        )
        .map_err(|error| label_binary_error(error, right_span))
    }

    fn apply(&self, function: Value, arg: Value) -> Result<Value> {
        RuntimeValueApplicator::new(self.types).apply(function, arg)
    }

    fn synthesize_and(&self, left: CoreExpr, right: CoreExpr) -> Result<Value> {
        match self.synthesize_prepared(left)? {
            Value::Bool(false) => Ok(Value::Bool(false)),
            Value::Bool(true) => match self.synthesize_prepared(right)? {
                Value::Bool(value) => Ok(Value::Bool(value)),
                _ => Err(Error::new("type mismatch: && expects Bool")),
            },
            _ => Err(Error::new("type mismatch: && expects Bool")),
        }
    }

    fn synthesize_or(&self, left: CoreExpr, right: CoreExpr) -> Result<Value> {
        match self.synthesize_prepared(left)? {
            Value::Bool(true) => Ok(Value::Bool(true)),
            Value::Bool(false) => match self.synthesize_prepared(right)? {
                Value::Bool(value) => Ok(Value::Bool(value)),
                _ => Err(Error::new("type mismatch: || expects Bool")),
            },
            _ => Err(Error::new("type mismatch: || expects Bool")),
        }
    }

    fn synthesize_string_concat(&self, left: CoreExpr, right: CoreExpr) -> Result<Value> {
        match (
            self.synthesize_prepared(left)?,
            self.synthesize_prepared(right)?,
        ) {
            (Value::String(left), Value::String(right)) => Ok(Value::String(left + &right)),
            _ => Err(Error::with_code(
                ErrorCode::TypeBadInterpolation,
                "cannot interpolate value",
            )),
        }
    }

    fn synthesize_show_application(&self, arg: CoreExpr) -> Result<Value> {
        match self.synthesize_prepared(arg)? {
            value @ (Value::Int(_) | Value::Float(_) | Value::Bool(_) | Value::String(_)) => self
                .apply(
                    self.env
                        .get("show")
                        .cloned()
                        .ok_or_else(|| Error::new("unknown identifier `show`"))?,
                    value,
                ),
            _ => Err(Error::with_code(
                ErrorCode::TypeBadInterpolation,
                "cannot interpolate value",
            )),
        }
    }

    fn is_show(&self, expr: &CoreExpr) -> bool {
        match expr {
            CoreExpr::Spanned(expr, _) => self.is_show(expr),
            CoreExpr::Var(name) => name == "show",
            CoreExpr::Global(binding) => self.env.global(*binding).is_some_and(
                |value| matches!(value, Value::Native(function) if function.name == "show"),
            ),
            _ => false,
        }
    }
}

pub trait ValueApplicator {
    fn apply(&self, function: Value, arg: Value) -> Result<Value>;
}

pub struct RuntimeValueApplicator<'a> {
    types: &'a dyn CoreTypeContext,
}

impl RuntimeValueApplicator<'static> {
    pub fn without_type_context() -> Self {
        static EMPTY_TYPES: EmptyCoreTypeContext = EmptyCoreTypeContext;
        Self::new(&EMPTY_TYPES)
    }
}

impl<'a> RuntimeValueApplicator<'a> {
    pub fn new(types: &'a dyn CoreTypeContext) -> Self {
        Self { types }
    }
}

impl ValueApplicator for RuntimeValueApplicator<'_> {
    fn apply(&self, function: Value, arg: Value) -> Result<Value> {
        match function {
            Value::CoreClosure { param, body, env } => PreparedCoreNormalizer::new(
                env.extend(param, arg.clone()).push_local(arg),
                self.types,
            )
            .synthesize_prepared(body),
            Value::Native(function) => function.apply(arg),
            _ => Err(Error::with_code(
                ErrorCode::TypeApplyNonFunction,
                "type mismatch: applying non-function",
            )),
        }
    }
}

pub struct CoreValueChecker<'a> {
    types: &'a dyn CoreTypeContext,
    env: Env,
}

impl<'a> CoreValueChecker<'a> {
    pub fn new(types: &'a dyn CoreTypeContext, env: Env) -> Self {
        Self { types, env }
    }

    pub fn check(&self, value: Value, expected: &CoreType) -> Result<Value> {
        self.check_with_context(value, expected, CheckContext::default())
    }

    fn check_with_context(
        &self,
        value: Value,
        expected: &CoreType,
        context: CheckContext,
    ) -> Result<Value> {
        let expected = self.expand(expected)?;
        match expected.as_unspanned() {
            CoreType::LiteralUnion(choices) => {
                let value = self.check_with_context(value, &CoreType::String, context.clone())?;
                self.validate_literal_union(value, choices)
            }
            CoreType::Refinement { binder, base, pred } => {
                let value = self.check_with_context(value, base, context.clone())?;
                let pred = CheckedRefinementPredicateBuilder::new(binder).build(pred);
                CoreRefinementValidator::new(self.env.clone()).validate_checked_with_options(
                    value,
                    pred,
                    RefinementValidationOptions {
                        value_span: context.value_span,
                        ..Default::default()
                    },
                    None,
                )
            }
            CoreType::Record(fields) => {
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
                        if self.is_option_type(ty)? {
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
                        self.check_with_context(value.clone(), ty, CheckContext::default())?,
                    );
                }
                Ok(Value::Record(out))
            }
            CoreType::List(inner) => {
                let Value::List(items) = value else {
                    return Err(Error::new("type mismatch: expected list"));
                };
                items
                    .into_iter()
                    .map(|item| self.check_with_context(item, inner, CheckContext::default()))
                    .collect::<Result<Vec<_>>>()
                    .map(Value::List)
            }
            CoreType::Option(inner) => match value {
                Value::None => Ok(Value::None),
                Value::Some(value) => Ok(Value::Some(Box::new(self.check(*value, inner)?))),
                value => Ok(Value::Some(Box::new(self.check_with_context(
                    value,
                    inner,
                    context.clone(),
                )?))),
            },
            _ if self.matches_type(&value, &expected)? => Ok(value),
            _ => Err(Error::with_code(
                ErrorCode::TypeMismatch,
                format!(
                    "type mismatch: expected {}, got {}",
                    self.type_name(expected.as_unspanned()),
                    value_name(&value)
                ),
            )),
        }
    }

    fn validate_literal_union(&self, value: Value, choices: &[String]) -> Result<Value> {
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

    fn matches_type(&self, value: &Value, ty: &CoreType) -> Result<bool> {
        Ok(match self.expand(ty)? {
            CoreType::Spanned(ty, _) => self.matches_type(value, &ty)?,
            CoreType::Int => matches!(value, Value::Int(_)),
            CoreType::Float => matches!(value, Value::Float(_) | Value::Int(_)),
            CoreType::Bool => matches!(value, Value::Bool(_)),
            CoreType::String => matches!(value, Value::String(_)),
            CoreType::LiteralUnion(choices) => match value {
                Value::String(value) => choices.iter().any(|choice| choice == value),
                _ => false,
            },
            CoreType::Option(inner) => match value {
                Value::None => true,
                Value::Some(value) => self.matches_type(value, &inner)?,
                _ => false,
            },
            CoreType::List(inner) => match value {
                Value::List(items) => {
                    for item in items {
                        if !self.matches_type(item, &inner)? {
                            return Ok(false);
                        }
                    }
                    true
                }
                _ => false,
            },
            CoreType::Record(fields) => match value {
                Value::Record(values) => {
                    values.len() == fields.len()
                        && fields.iter().all(|(name, ty)| {
                            values
                                .get(name)
                                .map(|value| self.matches_type(value, ty).unwrap_or(false))
                                .unwrap_or(false)
                        })
                }
                _ => false,
            },
            CoreType::Refinement { .. } => false,
            CoreType::Function(_, _) => {
                matches!(value, Value::CoreClosure { .. } | Value::Native(_))
            }
            CoreType::Alias(_) | CoreType::ResolvedAlias(_) => unreachable!(),
        })
    }

    fn expand(&self, ty: &CoreType) -> Result<CoreType> {
        match ty {
            CoreType::Spanned(ty, span) => self
                .expand(ty)
                .map(|ty| CoreType::Spanned(Box::new(ty), span.clone()))
                .map_err(|error| label_type_error(error, span.clone())),
            CoreType::Alias(name) => {
                let ty = self.types.alias(name).ok_or_else(|| {
                    Error::with_code(ErrorCode::TypeUnknown, format!("unknown type `{name}`"))
                })?;
                if matches!(ty, CoreType::Alias(alias) if alias == name) {
                    return Err(Error::with_code(
                        ErrorCode::TypeRecursiveAlias,
                        format!("recursive type alias `{name}`"),
                    ));
                }
                self.expand(ty)
            }
            CoreType::ResolvedAlias(alias) => {
                let ty = self.types.alias_by_ref(*alias).ok_or_else(|| {
                    Error::with_code(ErrorCode::TypeUnknown, "unknown type alias")
                })?;
                self.expand(ty)
            }
            CoreType::Option(inner) => Ok(CoreType::Option(Box::new(self.expand(inner)?))),
            CoreType::List(inner) => Ok(CoreType::List(Box::new(self.expand(inner)?))),
            CoreType::LiteralUnion(choices) => Ok(CoreType::LiteralUnion(choices.clone())),
            CoreType::Record(fields) => fields
                .iter()
                .map(|(name, ty)| Ok((name.clone(), self.expand(ty)?)))
                .collect::<Result<BTreeMap<_, _>>>()
                .map(CoreType::Record),
            CoreType::Refinement { binder, base, pred } => Ok(CoreType::Refinement {
                binder: binder.clone(),
                base: Box::new(self.expand(base)?),
                pred: pred.clone(),
            }),
            CoreType::Function(input, output) => Ok(CoreType::Function(
                Box::new(self.expand(input)?),
                Box::new(self.expand(output)?),
            )),
            ty => Ok(ty.clone()),
        }
    }

    fn is_option_type(&self, ty: &CoreType) -> Result<bool> {
        Ok(matches!(
            self.expand(ty)?.as_unspanned(),
            CoreType::Option(_)
        ))
    }

    fn type_name(&self, ty: &CoreType) -> &'static str {
        Self::core_type_name(ty)
    }

    fn core_type_name(ty: &CoreType) -> &'static str {
        match ty {
            CoreType::Spanned(ty, _) => Self::core_type_name(ty),
            CoreType::Int => "Int",
            CoreType::Float => "Float",
            CoreType::Bool => "Bool",
            CoreType::String => "String",
            CoreType::LiteralUnion(_) => "literal union",
            CoreType::Option(_) => "option",
            CoreType::List(_) => "list",
            CoreType::Record(_) => "record",
            CoreType::Refinement { .. } => "refinement",
            CoreType::Function(_, _) => "function",
            CoreType::Alias(_) | CoreType::ResolvedAlias(_) => "alias",
        }
    }
}

#[derive(Clone, Default)]
struct CheckContext {
    value_span: Option<std::ops::Range<usize>>,
}

fn label_type_error(error: Error, span: std::ops::Range<usize>) -> Error {
    if !error.diagnostic_labels().is_empty() {
        return error;
    }

    match error.code() {
        ErrorCode::TypeRecursiveAlias | ErrorCode::TypeUnknown => {
            let message = error.message().to_string();
            error.with_label(span, message)
        }
        _ => error,
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
        Value::CoreClosure { .. } | Value::Native(_) => "function",
    }
}

#[derive(Default)]
pub struct CoreEvaluator;

impl CoreEvaluator {
    pub fn new() -> Self {
        Self
    }

    pub fn eval(&self, expr: CoreExpr, env: &Env) -> Result<Value> {
        match expr {
            CoreExpr::Spanned(expr, span) => self
                .eval(*expr, env)
                .map_err(|error| label_origin_error(error, span)),
            CoreExpr::Int(value) => Ok(Value::Int(value)),
            CoreExpr::Float(value) => Ok(Value::Float(value)),
            CoreExpr::Bool(value) => Ok(Value::Bool(value)),
            CoreExpr::String(value) => Ok(Value::String(value)),
            CoreExpr::None => Ok(Value::None),
            CoreExpr::Some(expr) => Ok(Value::Some(Box::new(self.eval(*expr, env)?))),
            CoreExpr::Var(name) => env
                .get(&name)
                .cloned()
                .ok_or_else(|| Error::new(format!("unknown identifier `{name}`"))),
            CoreExpr::Global(binding) => env
                .global(binding)
                .cloned()
                .ok_or_else(|| Error::new("internal error: invalid global reference")),
            CoreExpr::Local(local) => env
                .local(local.index())
                .cloned()
                .ok_or_else(|| Error::new("internal error: invalid local reference")),
            CoreExpr::List(items) => items
                .into_iter()
                .map(|item| self.eval(item, env))
                .collect::<Result<Vec<_>>>()
                .map(Value::List),
            CoreExpr::Record(fields) => fields
                .into_iter()
                .map(|(name, expr)| Ok((name, self.eval(expr, env)?)))
                .collect::<Result<BTreeMap<_, _>>>()
                .map(Value::Record),
            CoreExpr::Field(expr, name) => match self.eval(*expr, env)? {
                receiver @ Value::Record(_) => {
                    if let Value::Record(fields) = &receiver
                        && let Some(value) = fields.get(&name)
                    {
                        return Ok(value.clone());
                    }
                    let method = env
                        .get(&name)
                        .cloned()
                        .ok_or_else(|| Error::new(format!("unknown field `{name}`")))?;
                    self.apply(method, receiver)
                }
                receiver => {
                    let method = env
                        .get(&name)
                        .cloned()
                        .ok_or_else(|| Error::new(format!("unknown field `{name}`")))?;
                    self.apply(method, receiver)
                }
            },
            CoreExpr::If(cond, then_expr, else_expr) => match self.eval(*cond, env)? {
                Value::Bool(true) => self.eval(*then_expr, env),
                Value::Bool(false) => self.eval(*else_expr, env),
                _ => Err(Error::new("type mismatch: if condition must be Bool")),
            },
            CoreExpr::Let(name, annotation, value, body) => {
                if annotation.is_some() {
                    return Err(Error::new(
                        "internal error: evaluator received unelaborated annotated let",
                    ));
                };
                let value = self.eval(*value, env)?;
                self.eval(*body, &env.extend(name, value.clone()).push_local(value))
            }
            CoreExpr::Lambda(param, _ty, body) => Ok(Value::CoreClosure {
                param,
                body: *body,
                env: env.clone(),
            }),
            CoreExpr::Apply(function, arg) => {
                let function = self.eval(*function, env)?;
                let arg = self.eval(*arg, env)?;
                self.apply(function, arg)
            }
            CoreExpr::Ascribe(_, _) => Err(Error::new(
                "internal error: evaluator received unelaborated ascription",
            )),
            CoreExpr::Unary(op, expr) => {
                let value = self.eval(*expr, env)?;
                match (op.as_str(), value) {
                    ("!", Value::Bool(value)) => Ok(Value::Bool(!value)),
                    ("-", Value::Int(value)) => Ok(Value::Int(-value)),
                    ("-", Value::Float(value)) => Ok(Value::Float(-value)),
                    _ => Err(Error::new(format!("type mismatch: invalid unary `{op}`"))),
                }
            }
            CoreExpr::Binary(op, left, right) => {
                if op == "&&" {
                    return self.eval_and(*left, *right, env);
                }
                if op == "||" {
                    return self.eval_or(*left, *right, env);
                }
                self.eval_binary(&op, *left, *right, env)
            }
        }
    }

    fn eval_binary(&self, op: &str, left: CoreExpr, right: CoreExpr, env: &Env) -> Result<Value> {
        let right_span = right.origin_span();
        binary(op, self.eval(left, env)?, self.eval(right, env)?)
            .map_err(|error| label_binary_error(error, right_span))
    }

    fn apply(&self, function: Value, arg: Value) -> Result<Value> {
        match function {
            Value::CoreClosure { param, body, env } => {
                self.eval(body, &env.extend(param, arg.clone()).push_local(arg))
            }
            Value::Native(function) => function.apply(arg),
            _ => Err(Error::with_code(
                ErrorCode::TypeApplyNonFunction,
                "type mismatch: applying non-function",
            )),
        }
    }

    fn eval_and(&self, left: CoreExpr, right: CoreExpr, env: &Env) -> Result<Value> {
        match self.eval(left, env)? {
            Value::Bool(false) => Ok(Value::Bool(false)),
            Value::Bool(true) => match self.eval(right, env)? {
                Value::Bool(value) => Ok(Value::Bool(value)),
                _ => Err(Error::new("type mismatch: && expects Bool")),
            },
            _ => Err(Error::new("type mismatch: && expects Bool")),
        }
    }

    fn eval_or(&self, left: CoreExpr, right: CoreExpr, env: &Env) -> Result<Value> {
        match self.eval(left, env)? {
            Value::Bool(true) => Ok(Value::Bool(true)),
            Value::Bool(false) => match self.eval(right, env)? {
                Value::Bool(value) => Ok(Value::Bool(value)),
                _ => Err(Error::new("type mismatch: || expects Bool")),
            },
            _ => Err(Error::new("type mismatch: || expects Bool")),
        }
    }
}

fn label_origin_error(error: Error, span: std::ops::Range<usize>) -> Error {
    if !error.diagnostic_labels().is_empty() {
        return error;
    }

    match error.code() {
        ErrorCode::RuntimeDivisionByZero => error.with_label(span, "division by zero"),
        _ => error,
    }
}

fn label_binary_error(error: Error, right_span: Option<std::ops::Range<usize>>) -> Error {
    if !error.diagnostic_labels().is_empty() {
        return error;
    }

    match (error.code(), right_span) {
        (ErrorCode::RuntimeDivisionByZero, Some(span)) => {
            error.with_label(span, "division by zero")
        }
        _ => error,
    }
}
