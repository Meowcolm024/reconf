use std::collections::BTreeMap;

use crate::core::{
    CoreDecl, CoreExpr, CoreModule, CoreType, CoreTypeContext, CoreTypeEnv, CoreTypeEquivalence,
    CoreTypeValidator, ElaboratedDecl, ElaboratedExpr, ElaboratedModule, GlobalRef, LocalRef,
    TypedCoreExpr,
};
use crate::error::{Error, ErrorCode, Result};
use crate::eval::builtins::NativeRegistry;
use crate::resolve::resolved::ResolvedDecl;

#[derive(Default)]
pub struct CoreElaborator;

impl CoreElaborator {
    pub fn new() -> Self {
        Self
    }

    pub fn check_expr(&mut self, expr: CoreExpr, expected: &CoreType) -> Result<TypedCoreExpr> {
        let expr = self.elaborate_expr(expr, expected)?;
        Ok(TypedCoreExpr {
            expr,
            ty: expected.clone(),
        })
    }

    pub fn prepare_expr(&mut self, expr: CoreExpr) -> Result<CoreExpr> {
        self.elaborate_unchecked_expr(expr)
    }

    fn elaborate_expr(&mut self, expr: CoreExpr, expected: &CoreType) -> Result<CoreExpr> {
        if let CoreExpr::Spanned(expr, span) = expr {
            return Ok(CoreExpr::Spanned(
                Box::new(self.elaborate_expr(*expr, expected)?),
                span,
            ));
        }

        if let CoreExpr::Ascribe(expr, ty) = expr {
            let typed = self.check_expr(*expr, &ty)?;
            return self.elaborate_expr(typed.expr, expected);
        }

        let expr = match expected {
            CoreType::Option(inner) => self.elaborate_option(expr, inner),
            CoreType::Record(fields) => self.elaborate_record(expr, fields),
            CoreType::Refinement { base, .. } => self.elaborate_expr(expr, base),
            CoreType::LiteralUnion(_) => self.elaborate_expr(expr, &CoreType::String),
            _ => Ok(expr),
        }?;
        self.elaborate_unchecked_expr(expr)
    }

    fn elaborate_option(&mut self, expr: CoreExpr, inner: &CoreType) -> Result<CoreExpr> {
        match expr {
            CoreExpr::None => Ok(CoreExpr::None),
            CoreExpr::Some(expr) => {
                Ok(CoreExpr::Some(Box::new(self.elaborate_expr(*expr, inner)?)))
            }
            expr => Ok(CoreExpr::Some(Box::new(self.elaborate_expr(expr, inner)?))),
        }
    }

    fn elaborate_record(
        &mut self,
        expr: CoreExpr,
        fields: &BTreeMap<String, CoreType>,
    ) -> Result<CoreExpr> {
        let CoreExpr::Record(expr_fields) = expr else {
            return Ok(expr);
        };

        for name in expr_fields.keys() {
            if !fields.contains_key(name) {
                return Err(Error::with_code(
                    ErrorCode::RecordUnknownField,
                    format!("unknown field `{name}`"),
                ));
            }
        }

        let mut out = BTreeMap::new();
        for (name, ty) in fields {
            match expr_fields.get(name) {
                Some(expr) => {
                    out.insert(name.clone(), self.elaborate_expr(expr.clone(), ty)?);
                }
                None if is_option_type(ty) => {
                    out.insert(name.clone(), CoreExpr::None);
                }
                None => {
                    return Err(Error::with_code(
                        ErrorCode::RecordMissingField,
                        format!("missing field `{name}`"),
                    ));
                }
            }
        }

        Ok(CoreExpr::Record(out))
    }

    fn elaborate_unchecked_expr(&mut self, expr: CoreExpr) -> Result<CoreExpr> {
        Ok(match expr {
            CoreExpr::Spanned(expr, span) => {
                CoreExpr::Spanned(Box::new(self.elaborate_unchecked_expr(*expr)?), span)
            }
            CoreExpr::Some(expr) => CoreExpr::Some(Box::new(self.elaborate_unchecked_expr(*expr)?)),
            CoreExpr::List(items) => CoreExpr::List(
                items
                    .into_iter()
                    .map(|item| self.elaborate_unchecked_expr(item))
                    .collect::<Result<Vec<_>>>()?,
            ),
            CoreExpr::Record(fields) => CoreExpr::Record(
                fields
                    .into_iter()
                    .map(|(name, expr)| Ok((name, self.elaborate_unchecked_expr(expr)?)))
                    .collect::<Result<BTreeMap<_, _>>>()?,
            ),
            CoreExpr::Field(expr, name) => {
                CoreExpr::Field(Box::new(self.elaborate_unchecked_expr(*expr)?), name)
            }
            CoreExpr::If(cond, then_expr, else_expr) => CoreExpr::If(
                Box::new(self.elaborate_unchecked_expr(*cond)?),
                Box::new(self.elaborate_unchecked_expr(*then_expr)?),
                Box::new(self.elaborate_unchecked_expr(*else_expr)?),
            ),
            CoreExpr::Let(name, annotation, value, body) => {
                let value = match annotation {
                    Some(ty) => self.check_expr(*value, &ty)?.expr,
                    None => self.elaborate_unchecked_expr(*value)?,
                };
                CoreExpr::Let(
                    name,
                    None,
                    Box::new(value),
                    Box::new(self.elaborate_unchecked_expr(*body)?),
                )
            }
            CoreExpr::Lambda(param, ty, body) => {
                CoreExpr::Lambda(param, ty, Box::new(self.elaborate_unchecked_expr(*body)?))
            }
            CoreExpr::Apply(function, arg) => CoreExpr::Apply(
                Box::new(self.elaborate_unchecked_expr(*function)?),
                Box::new(self.elaborate_unchecked_expr(*arg)?),
            ),
            CoreExpr::Ascribe(expr, ty) => {
                let typed = self.check_expr(*expr, &ty)?;
                typed.expr
            }
            CoreExpr::Unary(op, expr) => {
                CoreExpr::Unary(op, Box::new(self.elaborate_unchecked_expr(*expr)?))
            }
            CoreExpr::Binary(op, left, right) => CoreExpr::Binary(
                op,
                Box::new(self.elaborate_unchecked_expr(*left)?),
                Box::new(self.elaborate_unchecked_expr(*right)?),
            ),
            CoreExpr::Int(_)
            | CoreExpr::Float(_)
            | CoreExpr::Bool(_)
            | CoreExpr::String(_)
            | CoreExpr::None
            | CoreExpr::Var(_)
            | CoreExpr::Global(_)
            | CoreExpr::Local(_) => expr,
        })
    }
}

fn is_option_type(ty: &CoreType) -> bool {
    match ty {
        CoreType::Spanned(ty, _) => is_option_type(ty),
        CoreType::Option(_) => true,
        CoreType::Refinement { base, .. } => is_option_type(base),
        _ => false,
    }
}

pub trait CoreExprTyper {
    fn prepare_expr(&mut self, expr: CoreExpr) -> Result<CoreExpr>;
    fn check_expr(&mut self, expr: CoreExpr, expected: &CoreType) -> Result<TypedCoreExpr>;
    fn synthesize_expr(
        &mut self,
        expr: CoreExpr,
        values: &dyn CoreValueTypeContext,
    ) -> Result<ElaboratedExpr>;
}

pub trait CoreValueTypeContext {
    fn value_type(&self, name: &str) -> Option<&CoreType>;

    fn global_value(&self, _: &str) -> Option<(GlobalRef, &CoreType)> {
        None
    }

    fn local_value(&self, _: &str) -> Option<(LocalRef, &CoreType)> {
        None
    }

    fn local_type(&self, _: LocalRef) -> Option<&CoreType> {
        None
    }

    fn global_type(&self, _: GlobalRef) -> Option<&CoreType> {
        None
    }
}

impl CoreExprTyper for CoreElaborator {
    fn prepare_expr(&mut self, expr: CoreExpr) -> Result<CoreExpr> {
        CoreElaborator::prepare_expr(self, expr)
    }

    fn check_expr(&mut self, expr: CoreExpr, expected: &CoreType) -> Result<TypedCoreExpr> {
        CoreElaborator::check_expr(self, expr, expected)
    }

    fn synthesize_expr(
        &mut self,
        expr: CoreExpr,
        values: &dyn CoreValueTypeContext,
    ) -> Result<ElaboratedExpr> {
        Ok(ElaboratedExpr::Checked(
            self.synthesize_expr_type(expr, values)?,
        ))
    }
}

impl CoreElaborator {
    pub fn synthesize_expr_type(
        &mut self,
        expr: CoreExpr,
        values: &dyn CoreValueTypeContext,
    ) -> Result<TypedCoreExpr> {
        self.synthesize_known_expr(&expr, values)?
            .ok_or_else(|| self.unsynthesizable_expr_error(&expr))
    }

    fn synthesize_known_expr(
        &mut self,
        expr: &CoreExpr,
        values: &dyn CoreValueTypeContext,
    ) -> Result<Option<TypedCoreExpr>> {
        match expr {
            CoreExpr::Spanned(expr, span) => {
                let Some(typed) = self.synthesize_known_expr(expr, values)? else {
                    return Ok(None);
                };
                if typed.expr.origin_span().is_some() {
                    return Ok(Some(typed));
                }
                Ok(Some(TypedCoreExpr {
                    expr: CoreExpr::Spanned(Box::new(typed.expr), span.clone()),
                    ty: typed.ty,
                }))
            }
            CoreExpr::Int(value) => Ok(Some(TypedCoreExpr {
                expr: CoreExpr::Int(*value),
                ty: CoreType::Int,
            })),
            CoreExpr::Float(value) => Ok(Some(TypedCoreExpr {
                expr: CoreExpr::Float(*value),
                ty: CoreType::Float,
            })),
            CoreExpr::Bool(value) => Ok(Some(TypedCoreExpr {
                expr: CoreExpr::Bool(*value),
                ty: CoreType::Bool,
            })),
            CoreExpr::String(value) => Ok(Some(TypedCoreExpr {
                expr: CoreExpr::String(value.clone()),
                ty: CoreType::String,
            })),
            CoreExpr::Var(name) => Ok(match values.local_value(name) {
                Some((local, ty)) => Some(TypedCoreExpr {
                    expr: CoreExpr::Local(local),
                    ty: ty.clone(),
                }),
                None => None,
            }),
            CoreExpr::Global(binding) => Ok(Some(TypedCoreExpr {
                expr: CoreExpr::Global(*binding),
                ty: values
                    .global_type(*binding)
                    .cloned()
                    .ok_or_else(|| Error::new("internal error: invalid global reference"))?,
            })),
            CoreExpr::Local(local) => Ok(Some(TypedCoreExpr {
                expr: CoreExpr::Local(*local),
                ty: values
                    .local_type(*local)
                    .cloned()
                    .ok_or_else(|| Error::new("internal error: invalid local reference"))?,
            })),
            CoreExpr::Some(expr) => {
                let Some(typed) = self.synthesize_known_expr(expr, values)? else {
                    return Ok(None);
                };
                Ok(Some({
                    let ty = CoreType::Option(Box::new(typed.ty));
                    let expr = CoreExpr::Some(Box::new(typed.expr));
                    TypedCoreExpr { expr, ty }
                }))
            }
            CoreExpr::Ascribe(expr, ty) => Ok(Some(self.check_expr((**expr).clone(), ty)?)),
            CoreExpr::List(items) => self.synthesize_list(items, values),
            CoreExpr::Record(fields) => self.synthesize_record(fields, values),
            CoreExpr::Field(expr, field) => self.synthesize_field(expr, field, values),
            CoreExpr::If(cond, then_expr, else_expr) => {
                self.synthesize_if(cond, then_expr, else_expr, values)
            }
            CoreExpr::Let(name, annotation, value, body) => {
                self.synthesize_let(name, annotation, value, body, values)
            }
            CoreExpr::Lambda(param, ty, body) => self.synthesize_lambda(param, ty, body, values),
            CoreExpr::Apply(function, arg) => self.synthesize_apply(function, arg, values),
            CoreExpr::Unary(op, expr) => self.synthesize_unary(op, expr, values),
            CoreExpr::Binary(op, left, right) => self.synthesize_binary(op, left, right, values),
            CoreExpr::None => Ok(None),
        }
    }

    fn synthesize_list(
        &mut self,
        items: &[CoreExpr],
        values: &dyn CoreValueTypeContext,
    ) -> Result<Option<TypedCoreExpr>> {
        let Some((first, rest)) = items.split_first() else {
            return Ok(None);
        };
        let Some(first) = self.synthesize_known_expr(first, values)? else {
            return Ok(None);
        };
        let mut exprs = vec![first.expr];
        let item_ty = first.ty;
        for item in rest {
            let Some(typed) = self.synthesize_known_expr(item, values)? else {
                return Ok(None);
            };
            if !CoreTypeEquivalence::equivalent(&typed.ty, &item_ty) {
                return Ok(None);
            }
            exprs.push(typed.expr);
        }
        Ok(Some(TypedCoreExpr {
            expr: CoreExpr::List(exprs),
            ty: CoreType::List(Box::new(item_ty)),
        }))
    }

    fn synthesize_record(
        &mut self,
        fields: &BTreeMap<String, CoreExpr>,
        values: &dyn CoreValueTypeContext,
    ) -> Result<Option<TypedCoreExpr>> {
        let mut exprs = BTreeMap::new();
        let mut types = BTreeMap::new();
        for (name, expr) in fields {
            let Some(typed) = self.synthesize_known_expr(expr, values)? else {
                return Ok(None);
            };
            exprs.insert(name.clone(), typed.expr);
            types.insert(name.clone(), typed.ty);
        }
        Ok(Some(TypedCoreExpr {
            expr: CoreExpr::Record(exprs),
            ty: CoreType::Record(types),
        }))
    }

    fn synthesize_field(
        &mut self,
        expr: &CoreExpr,
        field: &str,
        values: &dyn CoreValueTypeContext,
    ) -> Result<Option<TypedCoreExpr>> {
        let Some(receiver) = self.synthesize_known_expr(expr, values)? else {
            return Ok(None);
        };
        let CoreType::Record(fields) = receiver.ty.as_unspanned() else {
            return Ok(None);
        };
        let Some(ty) = fields.get(field).cloned() else {
            return Ok(None);
        };
        Ok(Some(TypedCoreExpr {
            expr: CoreExpr::Field(Box::new(receiver.expr), field.to_string()),
            ty,
        }))
    }

    fn synthesize_if(
        &mut self,
        cond: &CoreExpr,
        then_expr: &CoreExpr,
        else_expr: &CoreExpr,
        values: &dyn CoreValueTypeContext,
    ) -> Result<Option<TypedCoreExpr>> {
        let Some(cond) = self.synthesize_known_expr(cond, values)? else {
            return Ok(None);
        };
        if !CoreTypeEquivalence::equivalent(&cond.ty, &CoreType::Bool) {
            return Ok(None);
        }
        let Some(then_expr) = self.synthesize_known_expr(then_expr, values)? else {
            return Ok(None);
        };
        let Some(else_expr) = self.synthesize_known_expr(else_expr, values)? else {
            return Ok(None);
        };
        if !CoreTypeEquivalence::equivalent(&then_expr.ty, &else_expr.ty) {
            return Ok(None);
        }
        let ty = then_expr.ty;
        Ok(Some(TypedCoreExpr {
            expr: CoreExpr::If(
                Box::new(cond.expr),
                Box::new(then_expr.expr),
                Box::new(else_expr.expr),
            ),
            ty,
        }))
    }

    fn synthesize_let(
        &mut self,
        name: &str,
        annotation: &Option<CoreType>,
        value: &CoreExpr,
        body: &CoreExpr,
        values: &dyn CoreValueTypeContext,
    ) -> Result<Option<TypedCoreExpr>> {
        let typed_value = match annotation {
            Some(ty) => self.check_expr(value.clone(), ty)?,
            None => {
                let Some(typed) = self.synthesize_known_expr(value, values)? else {
                    return Ok(None);
                };
                typed
            }
        };
        let scoped_values = ScopedCoreValueTypes {
            parent: values,
            name,
            ty: &typed_value.ty,
        };
        let Some(typed_body) = self.synthesize_known_expr(body, &scoped_values)? else {
            return Ok(None);
        };
        let ty = typed_body.ty;
        Ok(Some(TypedCoreExpr {
            expr: CoreExpr::Let(
                name.to_string(),
                None,
                Box::new(typed_value.expr),
                Box::new(typed_body.expr),
            ),
            ty,
        }))
    }

    fn synthesize_lambda(
        &mut self,
        param: &str,
        ty: &CoreType,
        body: &CoreExpr,
        values: &dyn CoreValueTypeContext,
    ) -> Result<Option<TypedCoreExpr>> {
        let scoped_values = ScopedCoreValueTypes {
            parent: values,
            name: param,
            ty,
        };
        let Some(body) = self.synthesize_known_expr(body, &scoped_values)? else {
            return Ok(None);
        };
        let output = body.ty;
        Ok(Some(TypedCoreExpr {
            expr: CoreExpr::Lambda(param.to_string(), ty.clone(), Box::new(body.expr)),
            ty: CoreType::Function(Box::new(ty.clone()), Box::new(output)),
        }))
    }

    fn synthesize_apply(
        &mut self,
        function: &CoreExpr,
        arg: &CoreExpr,
        values: &dyn CoreValueTypeContext,
    ) -> Result<Option<TypedCoreExpr>> {
        if self.is_show(function, values) {
            let Some(arg) = self.synthesize_known_expr(arg, values)? else {
                return Ok(None);
            };
            if matches!(
                arg.ty.as_unspanned(),
                CoreType::Int | CoreType::Float | CoreType::Bool | CoreType::String
            ) {
                return Ok(Some(TypedCoreExpr {
                    expr: CoreExpr::Apply(
                        Box::new(self.show_function_expr(function)),
                        Box::new(arg.expr),
                    ),
                    ty: CoreType::String,
                }));
            }
            return Err(Error::with_code(
                ErrorCode::TypeBadInterpolation,
                "cannot interpolate value",
            ));
        }
        let Some(function) = self.synthesize_known_expr(function, values)? else {
            return Ok(None);
        };
        let CoreType::Function(input, output) = function.ty.as_unspanned() else {
            return Err(Error::with_code(
                ErrorCode::TypeApplyNonFunction,
                "type mismatch: applying non-function",
            ));
        };
        let input = (**input).clone();
        let output = (**output).clone();
        let arg = self.check_expr(arg.clone(), &input)?;
        Ok(Some(TypedCoreExpr {
            expr: CoreExpr::Apply(Box::new(function.expr), Box::new(arg.expr)),
            ty: output,
        }))
    }

    fn synthesize_unary(
        &mut self,
        op: &str,
        expr: &CoreExpr,
        values: &dyn CoreValueTypeContext,
    ) -> Result<Option<TypedCoreExpr>> {
        let Some(expr) = self.synthesize_known_expr(expr, values)? else {
            return Ok(None);
        };
        let ty = match (op, expr.ty.as_unspanned()) {
            ("!", CoreType::Bool) => CoreType::Bool,
            ("-", CoreType::Int) => CoreType::Int,
            ("-", CoreType::Float) => CoreType::Float,
            _ => return Ok(None),
        };
        Ok(Some(TypedCoreExpr {
            expr: CoreExpr::Unary(op.to_string(), Box::new(expr.expr)),
            ty,
        }))
    }

    fn synthesize_binary(
        &mut self,
        op: &str,
        left: &CoreExpr,
        right: &CoreExpr,
        values: &dyn CoreValueTypeContext,
    ) -> Result<Option<TypedCoreExpr>> {
        let Some(left) = self.synthesize_known_expr(left, values)? else {
            return Ok(None);
        };
        let Some(right) = self.synthesize_known_expr(right, values)? else {
            return Ok(None);
        };
        let ty = match (op, left.ty.as_unspanned(), right.ty.as_unspanned()) {
            ("+" | "-" | "*" | "/" | "%", CoreType::Int, CoreType::Int) => CoreType::Int,
            ("+" | "-" | "*" | "/", CoreType::Float, CoreType::Float) => CoreType::Float,
            ("++", CoreType::String, CoreType::String) => CoreType::String,
            ("==" | "!=", _, _) if CoreTypeEquivalence::equivalent(&left.ty, &right.ty) => {
                CoreType::Bool
            }
            ("<" | "<=" | ">" | ">=", CoreType::Int, CoreType::Int)
            | ("<" | "<=" | ">" | ">=", CoreType::Float, CoreType::Float)
            | ("&&" | "||", CoreType::Bool, CoreType::Bool) => CoreType::Bool,
            _ => return Ok(None),
        };
        Ok(Some(TypedCoreExpr {
            expr: CoreExpr::Binary(op.to_string(), Box::new(left.expr), Box::new(right.expr)),
            ty,
        }))
    }

    fn unsynthesizable_expr_error(&self, expr: &CoreExpr) -> Error {
        match expr {
            CoreExpr::Spanned(expr, span) => {
                let error = self.unsynthesizable_expr_error(expr);
                let message = error.message().to_string();
                error.with_label(span.clone(), message)
            }
            CoreExpr::None => Error::with_code(
                ErrorCode::TypeNoneNeedsExpected,
                "`none` requires an expected option type",
            ),
            CoreExpr::List(items) if items.is_empty() => Error::with_code(
                ErrorCode::TypeNoneNeedsExpected,
                "empty lists require an expected list type",
            ),
            CoreExpr::Var(name) => Error::new(format!("unknown identifier `{name}`")),
            CoreExpr::Local(_) => Error::new("internal error: invalid local reference"),
            CoreExpr::Global(_) => Error::new("internal error: invalid global reference"),
            _ => Error::with_code(
                ErrorCode::TypeMismatch,
                "type mismatch: cannot synthesize expression type",
            ),
        }
    }

    fn show_function_expr(&self, expr: &CoreExpr) -> CoreExpr {
        match expr {
            CoreExpr::Spanned(inner, span) => {
                CoreExpr::Spanned(Box::new((**inner).clone()), span.clone())
            }
            expr => expr.clone(),
        }
    }

    fn is_show(&self, expr: &CoreExpr, values: &dyn CoreValueTypeContext) -> bool {
        match expr {
            CoreExpr::Spanned(expr, _) => self.is_show(expr, values),
            CoreExpr::Var(name) => name == "show",
            CoreExpr::Global(binding) => {
                let Some(ty) = values.global_type(*binding) else {
                    return false;
                };
                NativeRegistry::get("show")
                    .map(|spec| CoreTypeEquivalence::equivalent(ty, &spec.ty().to_core()))
                    .unwrap_or(false)
            }
            _ => false,
        }
    }
}

pub struct CoreModuleElaborator<'a, T = CoreElaborator> {
    base_types: &'a dyn CoreTypeContext,
    base_values: &'a dyn CoreValueTypeContext,
    local_types: CoreTypeEnv,
    local_values: BTreeMap<String, GlobalBindingType>,
    typer: T,
}

impl<'a> CoreModuleElaborator<'a> {
    pub fn new(types: &'a dyn CoreTypeContext) -> Self {
        Self::with_context(types, &EmptyCoreValueTypeContext, CoreElaborator::new())
    }
}

impl<'a, T: CoreExprTyper> CoreModuleElaborator<'a, T> {
    pub fn with_typer(types: &'a dyn CoreTypeContext, typer: T) -> Self {
        Self::with_context(types, &EmptyCoreValueTypeContext, typer)
    }

    pub fn with_context(
        types: &'a dyn CoreTypeContext,
        values: &'a dyn CoreValueTypeContext,
        typer: T,
    ) -> Self {
        Self {
            base_types: types,
            base_values: values,
            local_types: CoreTypeEnv::default(),
            local_values: BTreeMap::new(),
            typer,
        }
    }

    pub fn elaborate_module(&mut self, module: CoreModule) -> Result<ElaboratedModule> {
        let mut decls = Vec::new();
        for decl in module.decls {
            decls.push(self.elaborate_decl(decl)?);
        }
        let output = match module.output {
            Some(output) => {
                let values = self.values();
                Some(self.typer.synthesize_expr(output, &values)?)
            }
            None => None,
        };
        Ok(ElaboratedModule { decls, output })
    }

    fn elaborate_decl(&mut self, decl: CoreDecl) -> Result<ElaboratedDecl> {
        self.elaborate_resolved_decl(ResolvedDecl::from(decl))
    }

    pub fn elaborate_resolved_module(
        &mut self,
        decls: impl IntoIterator<Item = ResolvedDecl>,
        output: Option<CoreExpr>,
    ) -> Result<ElaboratedModule> {
        let mut elaborated_decls = Vec::new();
        for decl in decls {
            elaborated_decls.push(self.elaborate_resolved_decl(decl)?);
        }
        let output = match output {
            Some(output) => {
                let values = self.values();
                Some(self.typer.synthesize_expr(output, &values)?)
            }
            None => None,
        };
        Ok(ElaboratedModule {
            decls: elaborated_decls,
            output,
        })
    }

    fn elaborate_resolved_decl(&mut self, decl: ResolvedDecl) -> Result<ElaboratedDecl> {
        match decl {
            ResolvedDecl::Native {
                export,
                name,
                binding,
                ty,
            } => {
                self.validate_native(&name, &ty)?;
                self.define_local_value(name.clone(), binding, ty.clone());
                Ok(ElaboratedDecl::Native {
                    export,
                    name,
                    binding,
                    ty,
                })
            }
            ResolvedDecl::Type {
                export,
                name,
                alias,
                ty,
            } => {
                self.local_types
                    .define_with_ref(name.clone(), alias, ty.clone());
                self.validate_type_alias(&name, Some(alias), &ty)?;
                Ok(ElaboratedDecl::Type {
                    export,
                    name,
                    alias,
                    ty,
                })
            }
            ResolvedDecl::Let {
                export,
                name,
                binding,
                annotation,
                expr,
            } => {
                let expr = match annotation {
                    Some(ty) => ElaboratedExpr::Checked(self.typer.check_expr(expr, &ty)?),
                    None => {
                        let values = self.values();
                        self.typer.synthesize_expr(expr, &values)?
                    }
                };
                if let Some(ty) = expr.ty() {
                    self.define_local_value(name.clone(), binding, ty.clone());
                    return Ok(ElaboratedDecl::Let {
                        export,
                        name,
                        binding,
                        expr,
                    });
                }
                Err(Error::new("internal error: elaborated value has no type"))
            }
        }
    }

    fn define_local_value(&mut self, name: String, binding: GlobalRef, ty: CoreType) {
        self.local_values
            .insert(name, GlobalBindingType { binding, ty });
    }

    fn validate_native(&self, name: &str, ty: &CoreType) -> Result<()> {
        CoreTypeValidator::new(&self.types()).well_formed(ty)?;
        let spec = NativeRegistry::get(name)?;
        if !CoreTypeEquivalence::equivalent(ty, &spec.ty().to_core()) {
            return Err(Error::new(format!(
                "native `{name}` declaration does not match registry type"
            )));
        }
        Ok(())
    }

    fn validate_type_alias(
        &self,
        name: &str,
        alias: Option<crate::core::TypeAliasRef>,
        ty: &CoreType,
    ) -> Result<()> {
        let types = self.types();
        let validator = CoreTypeValidator::new(&types);
        if validator.mentions_alias(ty, name, alias) {
            let mut error = Error::with_code(
                ErrorCode::TypeRecursiveAlias,
                format!("recursive type alias `{name}`"),
            );
            if let Some(span) = ty
                .alias_origin(name)
                .or_else(|| alias.and_then(|alias| ty.resolved_alias_origin(alias)))
            {
                let message = error.message().to_string();
                error = error.with_label(span, message);
            }
            return Err(error);
        }
        validator.well_formed(ty)
    }

    fn types(&self) -> ModuleElaborationTypes<'_> {
        ModuleElaborationTypes {
            base: self.base_types,
            local: &self.local_types,
        }
    }

    fn values(&self) -> ModuleElaborationValues<'a> {
        ModuleElaborationValues {
            base: self.base_values,
            local: self.local_values.clone(),
        }
    }
}

#[derive(Clone)]
struct GlobalBindingType {
    binding: GlobalRef,
    ty: CoreType,
}

struct ModuleElaborationTypes<'a> {
    base: &'a dyn CoreTypeContext,
    local: &'a CoreTypeEnv,
}

impl CoreTypeContext for ModuleElaborationTypes<'_> {
    fn alias(&self, name: &str) -> Option<&CoreType> {
        self.local.alias(name).or_else(|| self.base.alias(name))
    }

    fn alias_by_ref(&self, alias: crate::core::TypeAliasRef) -> Option<&CoreType> {
        self.local
            .alias_by_ref(alias)
            .or_else(|| self.base.alias_by_ref(alias))
    }
}

struct ModuleElaborationValues<'a> {
    base: &'a dyn CoreValueTypeContext,
    local: BTreeMap<String, GlobalBindingType>,
}

impl CoreValueTypeContext for ModuleElaborationValues<'_> {
    fn value_type(&self, name: &str) -> Option<&CoreType> {
        self.local
            .get(name)
            .map(|binding| &binding.ty)
            .or_else(|| self.base.value_type(name))
    }

    fn global_value(&self, name: &str) -> Option<(GlobalRef, &CoreType)> {
        self.local
            .get(name)
            .map(|binding| (binding.binding, &binding.ty))
            .or_else(|| self.base.global_value(name))
    }

    fn global_type(&self, binding: GlobalRef) -> Option<&CoreType> {
        self.local
            .values()
            .find(|value| value.binding == binding)
            .map(|binding| &binding.ty)
            .or_else(|| self.base.global_type(binding))
    }
}

struct ScopedCoreValueTypes<'a> {
    parent: &'a dyn CoreValueTypeContext,
    name: &'a str,
    ty: &'a CoreType,
}

impl CoreValueTypeContext for ScopedCoreValueTypes<'_> {
    fn value_type(&self, name: &str) -> Option<&CoreType> {
        self.parent.value_type(name)
    }

    fn local_value(&self, name: &str) -> Option<(LocalRef, &CoreType)> {
        if name == self.name {
            Some((LocalRef::new(0), self.ty))
        } else {
            self.parent
                .local_value(name)
                .map(|(local, ty)| (LocalRef::new(local.index() + 1), ty))
        }
    }

    fn local_type(&self, local: LocalRef) -> Option<&CoreType> {
        if local.index() == 0 {
            Some(self.ty)
        } else {
            self.parent.local_type(LocalRef::new(local.index() - 1))
        }
    }

    fn global_value(&self, name: &str) -> Option<(GlobalRef, &CoreType)> {
        self.parent.global_value(name)
    }

    fn global_type(&self, binding: GlobalRef) -> Option<&CoreType> {
        self.parent.global_type(binding)
    }
}

struct EmptyCoreValueTypeContext;

impl CoreValueTypeContext for EmptyCoreValueTypeContext {
    fn value_type(&self, _: &str) -> Option<&CoreType> {
        None
    }
}
