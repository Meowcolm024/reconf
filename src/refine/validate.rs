use std::borrow::Cow;

use crate::core::{CoreExpr, LocalRef};
use crate::error::{Error, ErrorCode, Result};
use crate::eval::core::CoreEvaluator;
use crate::eval::{Env, Value};

pub struct CheckedRefinementPredicate<'a> {
    pred: Cow<'a, CoreExpr>,
}

pub struct CheckedRefinementPredicateBuilder<'a> {
    binder: &'a str,
}

pub struct CoreRefinementValidator {
    env: Env,
}

impl<'a> CheckedRefinementPredicateBuilder<'a> {
    pub fn new(binder: &'a str) -> Self {
        Self { binder }
    }

    pub fn build(&self, pred: &CoreExpr) -> CheckedRefinementPredicate<'static> {
        CheckedRefinementPredicate::owned(self.prepare_at_depth(pred, 0))
    }

    fn prepare_at_depth(&self, pred: &CoreExpr, depth: usize) -> CoreExpr {
        match pred {
            CoreExpr::Spanned(expr, span) => {
                CoreExpr::Spanned(Box::new(self.prepare_at_depth(expr, depth)), span.clone())
            }
            CoreExpr::Var(name) if name == self.binder => CoreExpr::Local(LocalRef::new(depth)),
            CoreExpr::Some(expr) => CoreExpr::Some(Box::new(self.prepare_at_depth(expr, depth))),
            CoreExpr::List(items) => CoreExpr::List(
                items
                    .iter()
                    .map(|item| self.prepare_at_depth(item, depth))
                    .collect(),
            ),
            CoreExpr::Record(fields) => CoreExpr::Record(
                fields
                    .iter()
                    .map(|(name, expr)| (name.clone(), self.prepare_at_depth(expr, depth)))
                    .collect(),
            ),
            CoreExpr::Field(expr, field) => {
                CoreExpr::Field(Box::new(self.prepare_at_depth(expr, depth)), field.clone())
            }
            CoreExpr::If(cond, then_expr, else_expr) => CoreExpr::If(
                Box::new(self.prepare_at_depth(cond, depth)),
                Box::new(self.prepare_at_depth(then_expr, depth)),
                Box::new(self.prepare_at_depth(else_expr, depth)),
            ),
            CoreExpr::Let(name, annotation, value, body) => {
                let body = if name == self.binder {
                    (**body).clone()
                } else {
                    self.prepare_at_depth(body, depth + 1)
                };
                CoreExpr::Let(
                    name.clone(),
                    annotation.clone(),
                    Box::new(self.prepare_at_depth(value, depth)),
                    Box::new(body),
                )
            }
            CoreExpr::Lambda(param, ty, body) => {
                let body = if param == self.binder {
                    (**body).clone()
                } else {
                    self.prepare_at_depth(body, depth + 1)
                };
                CoreExpr::Lambda(param.clone(), ty.clone(), Box::new(body))
            }
            CoreExpr::Apply(function, arg) => CoreExpr::Apply(
                Box::new(self.prepare_at_depth(function, depth)),
                Box::new(self.prepare_at_depth(arg, depth)),
            ),
            CoreExpr::Ascribe(expr, ty) => {
                CoreExpr::Ascribe(Box::new(self.prepare_at_depth(expr, depth)), ty.clone())
            }
            CoreExpr::Unary(op, expr) => {
                CoreExpr::Unary(op.clone(), Box::new(self.prepare_at_depth(expr, depth)))
            }
            CoreExpr::Binary(op, left, right) => CoreExpr::Binary(
                op.clone(),
                Box::new(self.prepare_at_depth(left, depth)),
                Box::new(self.prepare_at_depth(right, depth)),
            ),
            CoreExpr::Int(_)
            | CoreExpr::Float(_)
            | CoreExpr::Bool(_)
            | CoreExpr::String(_)
            | CoreExpr::None
            | CoreExpr::Var(_)
            | CoreExpr::Global(_)
            | CoreExpr::Local(_) => pred.clone(),
        }
    }
}

impl<'a> CheckedRefinementPredicate<'a> {
    pub fn new(pred: &'a CoreExpr) -> Self {
        Self {
            pred: Cow::Borrowed(pred),
        }
    }

    pub fn owned(pred: CoreExpr) -> Self {
        Self {
            pred: Cow::Owned(pred),
        }
    }
}

impl CoreRefinementValidator {
    pub fn new(env: Env) -> Self {
        Self { env }
    }

    pub fn validate(&self, value: Value, binder: &str, pred: &CoreExpr) -> Result<Value> {
        self.validate_with_options(value, binder, pred, RefinementValidationOptions::default())
    }

    pub fn validate_with_code(
        &self,
        value: Value,
        binder: &str,
        pred: &CoreExpr,
        code: ErrorCode,
    ) -> Result<Value> {
        self.validate_with_options(
            value,
            binder,
            pred,
            RefinementValidationOptions {
                code,
                value_span: None,
            },
        )
    }

    pub fn validate_with_options(
        &self,
        value: Value,
        binder: &str,
        pred: &CoreExpr,
        options: RefinementValidationOptions,
    ) -> Result<Value> {
        self.validate_checked_with_options(
            value,
            CheckedRefinementPredicate::new(pred),
            options,
            Some(binder),
        )
    }

    pub fn validate_checked(
        &self,
        value: Value,
        pred: CheckedRefinementPredicate<'_>,
    ) -> Result<Value> {
        self.validate_checked_with_options(
            value,
            pred,
            RefinementValidationOptions::default(),
            None,
        )
    }

    pub fn validate_checked_with_options(
        &self,
        value: Value,
        pred: CheckedRefinementPredicate<'_>,
        options: RefinementValidationOptions,
        binder: Option<&str>,
    ) -> Result<Value> {
        let env = match binder {
            Some(binder) => self
                .env
                .extend(binder, value.clone())
                .push_local(value.clone()),
            None => self.env.clone().push_local(value.clone()),
        };
        match CoreEvaluator::new().eval(pred.pred.into_owned(), &env)? {
            Value::Bool(true) => Ok(value),
            Value::Bool(false) => {
                let error = Error::with_code(options.code, "refinement failed");
                Err(match options.value_span {
                    Some(span) => error.with_label(span, "value does not satisfy refinement"),
                    None => error,
                })
            }
            _ => Err(Error::new("unknown predicate")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RefinementValidationOptions {
    pub code: ErrorCode,
    pub value_span: Option<std::ops::Range<usize>>,
}

impl Default for RefinementValidationOptions {
    fn default() -> Self {
        Self {
            code: ErrorCode::RefineFailed,
            value_span: None,
        }
    }
}
