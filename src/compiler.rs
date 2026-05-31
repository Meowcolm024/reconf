use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::diagnostic::{Diagnostic, SourceMap, Span};
use crate::eval::{RuntimeEnv, Value, eval, reject_function_output, value_debug};
use crate::syntax::{
    BinaryOp, Expr, ExprKind, FieldExpr, FieldTy, FileAst, InterpPart, Parser, TopDecl, Ty, TyKind,
    UnaryOp,
};

#[derive(Clone, Debug)]
struct ValueInfo {
    ty: Ty,
    value: Value,
}

#[derive(Clone, Debug, Default)]
struct Ctx {
    types: HashMap<String, Ty>,
    values: HashMap<String, ValueInfo>,
}

#[derive(Clone, Debug, Default)]
struct Exports {
    types: HashMap<String, Ty>,
    values: HashMap<String, ValueInfo>,
}

#[derive(Clone, Debug)]
struct ModuleResult {
    exports: Exports,
    output_value: Value,
}

pub struct Compiler {
    sources: SourceMap,
    modules: HashMap<PathBuf, ModuleResult>,
    stack: Vec<PathBuf>,
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            sources: SourceMap::default(),
            modules: HashMap::new(),
            stack: Vec::new(),
        }
    }

    pub fn check_file(&mut self, path: &Path) -> Result<(), Diagnostic> {
        self.load_module(path)?;
        Ok(())
    }

    pub fn eval_file(&mut self, path: &Path) -> Result<Value, Diagnostic> {
        let module = self.load_module(path)?;
        reject_function_output(&module.output_value, self.empty_span())?;
        Ok(module.output_value)
    }

    pub fn render(&self, diagnostic: Diagnostic) -> String {
        self.sources.render(diagnostic)
    }

    fn empty_span(&self) -> Span {
        Span::empty(0, 0)
    }

    fn load_module(&mut self, path: &Path) -> Result<ModuleResult, Diagnostic> {
        let canonical = canonicalize_existing(path)
            .map_err(|message| Diagnostic::new("E_MODULE_001", message, self.empty_span()))?;
        if let Some(module) = self.modules.get(&canonical) {
            return Ok(module.clone());
        }
        if let Some(idx) = self.stack.iter().position(|p| p == &canonical) {
            let cycle = self.stack[idx..]
                .iter()
                .chain(std::iter::once(&canonical))
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(" -> ");
            return Err(Diagnostic::new(
                "E_MODULE_002",
                format!("cyclic import detected: {cycle}"),
                self.empty_span(),
            ));
        }

        let text = std::fs::read_to_string(&canonical).map_err(|err| {
            Diagnostic::new(
                "E_MODULE_003",
                format!("failed to read {}: {err}", canonical.display()),
                self.empty_span(),
            )
        })?;
        let file_id = self.sources.add(canonical.clone(), text.clone());
        let ast = Parser::parse_file(file_id, &text)?;

        self.stack.push(canonical.clone());
        let module = self.process_module(&canonical, ast);
        self.stack.pop();

        let module = module?;
        self.modules.insert(canonical, module.clone());
        Ok(module)
    }

    fn process_module(&mut self, path: &Path, ast: FileAst) -> Result<ModuleResult, Diagnostic> {
        let mut ctx = Ctx::default();
        let mut exports = Exports::default();

        for import in ast.imports {
            let import_path = resolve_import_path(path, &import.path)
                .map_err(|message| Diagnostic::new("E_MODULE_004", message, import.span))?;
            let imported = self.load_module(&import_path)?;
            for name in import.names {
                let mut found = false;
                if let Some(ty) = imported.exports.types.get(&name) {
                    if ctx.types.insert(name.clone(), ty.clone()).is_some() {
                        return Err(Diagnostic::new(
                            "E_NAME_001",
                            format!("duplicate imported type `{name}`"),
                            import.span,
                        ));
                    }
                    found = true;
                }
                if let Some(value) = imported.exports.values.get(&name) {
                    if ctx.values.insert(name.clone(), value.clone()).is_some() {
                        return Err(Diagnostic::new(
                            "E_NAME_002",
                            format!("duplicate imported value `{name}`"),
                            import.span,
                        ));
                    }
                    found = true;
                }
                if !found {
                    return Err(Diagnostic::new(
                        "E_MODULE_005",
                        format!("`{name}` is not exported by {}", import_path.display()),
                        import.span,
                    ));
                }
            }
        }

        for decl in ast.decls {
            match decl {
                TopDecl::Type {
                    export,
                    name,
                    ty,
                    span,
                } => {
                    if ctx.types.contains_key(&name) {
                        return Err(Diagnostic::new(
                            "E_NAME_003",
                            format!("duplicate type `{name}`"),
                            span,
                        ));
                    }
                    if ty_mentions_alias(&ty, &name) {
                        return Err(Diagnostic::new(
                            "E_TYPE_002",
                            format!("recursive type alias `{name}`"),
                            ty.span,
                        ));
                    }
                    self.check_well_formed_type(&ty, &ctx, &mut Vec::new())?;
                    ctx.types.insert(name.clone(), ty.clone());
                    if export {
                        exports.types.insert(name, ty);
                    }
                }
                TopDecl::Let {
                    export,
                    name,
                    ann,
                    value,
                    span,
                } => {
                    if ctx.values.contains_key(&name) {
                        return Err(Diagnostic::new(
                            "E_NAME_004",
                            format!("duplicate value `{name}`"),
                            span,
                        ));
                    }
                    let (ty, elaborated) = if let Some(ann) = ann {
                        self.check_well_formed_type(&ann, &ctx, &mut Vec::new())?;
                        let elaborated = self.check_expr(&value, &ann, &ctx)?;
                        (ann, elaborated)
                    } else {
                        self.synth_expr(&value, &ctx)?
                    };
                    let value = eval(&elaborated, &runtime_from_ctx(&ctx), elaborated.span)?;
                    let info = ValueInfo {
                        ty: ty.clone(),
                        value,
                    };
                    ctx.values.insert(name.clone(), info.clone());
                    if export {
                        exports.values.insert(name, info);
                    }
                }
            }
        }

        let (_output_ty, output_expr) = self.synth_expr(&ast.output, &ctx)?;
        let output_value = eval(&output_expr, &runtime_from_ctx(&ctx), ast.output.span)?;
        reject_function_output(&output_value, ast.output.span)?;
        Ok(ModuleResult {
            exports,
            output_value,
        })
    }

    fn check_well_formed_type(
        &self,
        ty: &Ty,
        ctx: &Ctx,
        stack: &mut Vec<String>,
    ) -> Result<(), Diagnostic> {
        match &ty.kind {
            TyKind::Int
            | TyKind::Float
            | TyKind::Bool
            | TyKind::String
            | TyKind::LiteralUnion(_) => Ok(()),
            TyKind::Builtin(_) => Ok(()),
            TyKind::Option(inner) | TyKind::List(inner) => {
                self.check_well_formed_type(inner, ctx, stack)
            }
            TyKind::Fun(param, result) => {
                self.check_well_formed_type(param, ctx, stack)?;
                self.check_well_formed_type(result, ctx, stack)
            }
            TyKind::Record(fields) => {
                let mut seen = HashSet::new();
                for field in fields {
                    if !seen.insert(field.name.clone()) {
                        return Err(Diagnostic::new(
                            "E_TYPE_001",
                            format!("duplicate field `{}` in record type", field.name),
                            field.span,
                        ));
                    }
                    self.check_well_formed_type(&field.ty, ctx, stack)?;
                }
                Ok(())
            }
            TyKind::Refine { binder, base, pred } => {
                self.check_well_formed_type(base, ctx, stack)?;
                let mut refine_ctx = ctx.clone();
                refine_ctx.values.insert(
                    binder.clone(),
                    ValueInfo {
                        ty: (*base.clone()),
                        value: Value::Builtin {
                            name: "__refinement_binder__".to_string(),
                            args: Vec::new(),
                        },
                    },
                );
                let (pred_ty, _) = self.synth_expr(pred, &refine_ctx)?;
                if !self.compatible(&pred_ty, &bool_ty(pred.span), ctx)? {
                    return Err(Diagnostic::new(
                        "E_REFINE_001",
                        "refinement predicate must have type Bool",
                        pred.span,
                    ));
                }
                Ok(())
            }
            TyKind::Alias(name) => {
                if stack.contains(name) {
                    return Err(Diagnostic::new(
                        "E_TYPE_002",
                        format!("recursive type alias `{name}`"),
                        ty.span,
                    ));
                }
                let Some(alias) = ctx.types.get(name) else {
                    return Err(Diagnostic::new(
                        "E_TYPE_003",
                        format!("unknown type `{name}`"),
                        ty.span,
                    ));
                };
                stack.push(name.clone());
                self.check_well_formed_type(alias, ctx, stack)?;
                stack.pop();
                Ok(())
            }
        }
    }

    fn synth_expr(&self, expr: &Expr, ctx: &Ctx) -> Result<(Ty, Expr), Diagnostic> {
        match &expr.kind {
            ExprKind::Int(_) => Ok((int_ty(expr.span), expr.clone())),
            ExprKind::Float(value) => {
                if !value.is_finite() {
                    Err(Diagnostic::new(
                        "E_TYPE_004",
                        "float literals must be finite",
                        expr.span,
                    ))
                } else {
                    Ok((float_ty(expr.span), expr.clone()))
                }
            }
            ExprKind::Bool(_) => Ok((bool_ty(expr.span), expr.clone())),
            ExprKind::String(_) => Ok((string_ty(expr.span), expr.clone())),
            ExprKind::Interp(parts) => {
                let mut checked = Vec::new();
                for part in parts {
                    match part {
                        InterpPart::Text(text) => checked.push(InterpPart::Text(text.clone())),
                        InterpPart::Expr(part_expr) => {
                            let (part_ty, elab) = self.synth_expr(part_expr, ctx)?;
                            if !is_showable(&self.expand_alias(&part_ty, ctx)?) {
                                return Err(Diagnostic::new(
                                    "E_TYPE_005",
                                    format!(
                                        "cannot interpolate value of type {}",
                                        self.ty_name(&part_ty, ctx)
                                    ),
                                    part_expr.span,
                                ));
                            }
                            checked.push(InterpPart::Expr(elab));
                        }
                    }
                }
                Ok((
                    string_ty(expr.span),
                    Expr {
                        kind: ExprKind::Interp(checked),
                        span: expr.span,
                    },
                ))
            }
            ExprKind::Var(name) => {
                if let Some(info) = ctx.values.get(name) {
                    Ok((info.ty.clone(), expr.clone()))
                } else if is_builtin_name(name) {
                    Ok((
                        Ty {
                            kind: TyKind::Builtin(name.clone()),
                            span: expr.span,
                        },
                        expr.clone(),
                    ))
                } else {
                    Err(Diagnostic::new(
                        "E_NAME_005",
                        format!("unknown identifier `{name}`"),
                        expr.span,
                    ))
                }
            }
            ExprKind::None => Err(Diagnostic::new(
                "E_TYPE_006",
                "`none` requires an expected option type",
                expr.span,
            )),
            ExprKind::Some(inner) => {
                let (inner_ty, inner_elab) = self.synth_expr(inner, ctx)?;
                Ok((
                    Ty {
                        kind: TyKind::Option(Box::new(inner_ty)),
                        span: expr.span,
                    },
                    Expr {
                        kind: ExprKind::Some(Box::new(inner_elab)),
                        span: expr.span,
                    },
                ))
            }
            ExprKind::List(items) => {
                let Some(first) = items.first() else {
                    return Err(Diagnostic::new(
                        "E_TYPE_007",
                        "empty lists require an expected list type",
                        expr.span,
                    ));
                };
                let (item_ty, first_elab) = self.synth_expr(first, ctx)?;
                let mut elaborated = vec![first_elab];
                for item in &items[1..] {
                    elaborated.push(self.check_expr(item, &item_ty, ctx)?);
                }
                Ok((
                    Ty {
                        kind: TyKind::List(Box::new(item_ty)),
                        span: expr.span,
                    },
                    Expr {
                        kind: ExprKind::List(elaborated),
                        span: expr.span,
                    },
                ))
            }
            ExprKind::Record(fields) => {
                let mut seen = HashSet::new();
                let mut field_tys = Vec::new();
                let mut field_exprs = Vec::new();
                for field in fields {
                    if !seen.insert(field.name.clone()) {
                        return Err(Diagnostic::new(
                            "E_RECORD_001",
                            format!("duplicate field `{}`", field.name),
                            field.span,
                        ));
                    }
                    let (ty, value) = self.synth_expr(&field.value, ctx)?;
                    field_tys.push(FieldTy {
                        name: field.name.clone(),
                        ty,
                        span: field.span,
                    });
                    field_exprs.push(FieldExpr {
                        name: field.name.clone(),
                        value,
                        span: field.span,
                    });
                }
                Ok((
                    Ty {
                        kind: TyKind::Record(field_tys),
                        span: expr.span,
                    },
                    Expr {
                        kind: ExprKind::Record(field_exprs),
                        span: expr.span,
                    },
                ))
            }
            ExprKind::Field(base, name) => {
                let (base_ty, base_elab) = self.synth_expr(base, ctx)?;
                match self.expand_alias(&base_ty, ctx)?.kind {
                    TyKind::Record(fields) => {
                        let Some(field) = fields.iter().find(|field| field.name == *name) else {
                            return Err(Diagnostic::new(
                                "E_RECORD_002",
                                format!("unknown field `{name}`"),
                                expr.span,
                            ));
                        };
                        Ok((
                            field.ty.clone(),
                            Expr {
                                kind: ExprKind::Field(Box::new(base_elab), name.clone()),
                                span: expr.span,
                            },
                        ))
                    }
                    other => self.synth_method(expr.span, &base_ty, &other, base_elab, name, ctx),
                }
            }
            ExprKind::If {
                cond,
                then_expr,
                else_expr,
            } => {
                let cond_elab = self.check_expr(cond, &bool_ty(cond.span), ctx)?;
                let (then_ty, then_elab) = self.synth_expr(then_expr, ctx)?;
                let else_elab = self.check_expr(else_expr, &then_ty, ctx)?;
                Ok((
                    then_ty,
                    Expr {
                        kind: ExprKind::If {
                            cond: Box::new(cond_elab),
                            then_expr: Box::new(then_elab),
                            else_expr: Box::new(else_elab),
                        },
                        span: expr.span,
                    },
                ))
            }
            ExprKind::Let {
                name,
                ann,
                value,
                body,
            } => {
                let mut local = ctx.clone();
                let (value_ty, value_elab) = if let Some(ann) = ann {
                    self.check_well_formed_type(ann, ctx, &mut Vec::new())?;
                    let value_elab = self.check_expr(value, ann, ctx)?;
                    (ann.clone(), value_elab)
                } else {
                    self.synth_expr(value, ctx)?
                };
                let value = eval(&value_elab, &runtime_from_ctx(ctx), value.span)?;
                local.values.insert(
                    name.clone(),
                    ValueInfo {
                        ty: value_ty.clone(),
                        value,
                    },
                );
                let (body_ty, body_elab) = self.synth_expr(body, &local)?;
                Ok((
                    body_ty,
                    Expr {
                        kind: ExprKind::Let {
                            name: name.clone(),
                            ann: ann.clone(),
                            value: Box::new(value_elab),
                            body: Box::new(body_elab),
                        },
                        span: expr.span,
                    },
                ))
            }
            ExprKind::Lam {
                param,
                param_ty,
                body,
            } => {
                self.check_well_formed_type(param_ty, ctx, &mut Vec::new())?;
                let mut local = ctx.clone();
                local.values.insert(
                    param.clone(),
                    ValueInfo {
                        ty: param_ty.clone(),
                        value: Value::Builtin {
                            name: "__lambda_param__".to_string(),
                            args: Vec::new(),
                        },
                    },
                );
                let (body_ty, body_elab) = self.synth_expr(body, &local)?;
                Ok((
                    Ty {
                        kind: TyKind::Fun(Box::new(param_ty.clone()), Box::new(body_ty)),
                        span: expr.span,
                    },
                    Expr {
                        kind: ExprKind::Lam {
                            param: param.clone(),
                            param_ty: param_ty.clone(),
                            body: Box::new(body_elab),
                        },
                        span: expr.span,
                    },
                ))
            }
            ExprKind::App(func, arg) => {
                let (func_ty, func_elab) = self.synth_expr(func, ctx)?;
                match self.expand_alias(&func_ty, ctx)?.kind {
                    TyKind::Fun(param, result) => {
                        let arg_elab = self.check_expr(arg, &param, ctx)?;
                        Ok((
                            *result,
                            Expr {
                                kind: ExprKind::App(Box::new(func_elab), Box::new(arg_elab)),
                                span: expr.span,
                            },
                        ))
                    }
                    TyKind::Builtin(name) => {
                        self.synth_builtin_app(expr.span, &name, func_elab, arg, ctx)
                    }
                    _ => Err(Diagnostic::new(
                        "E_TYPE_008",
                        format!(
                            "cannot apply non-function of type {}",
                            self.ty_name(&func_ty, ctx)
                        ),
                        func.span,
                    )),
                }
            }
            ExprKind::Ascribe(inner, ty) => {
                self.check_well_formed_type(ty, ctx, &mut Vec::new())?;
                let elab = self.check_expr(inner, ty, ctx)?;
                Ok((
                    ty.clone(),
                    Expr {
                        kind: ExprKind::Ascribe(Box::new(elab), ty.clone()),
                        span: expr.span,
                    },
                ))
            }
            ExprKind::Unary(op, inner) => match op {
                UnaryOp::Not => {
                    let inner_elab = self.check_expr(inner, &bool_ty(inner.span), ctx)?;
                    Ok((
                        bool_ty(expr.span),
                        Expr {
                            kind: ExprKind::Unary(*op, Box::new(inner_elab)),
                            span: expr.span,
                        },
                    ))
                }
                UnaryOp::Neg => {
                    let (inner_ty, inner_elab) = self.synth_expr(inner, ctx)?;
                    if matches!(
                        self.expand_alias(&inner_ty, ctx)?.kind,
                        TyKind::Int | TyKind::Float
                    ) {
                        Ok((
                            inner_ty,
                            Expr {
                                kind: ExprKind::Unary(*op, Box::new(inner_elab)),
                                span: expr.span,
                            },
                        ))
                    } else {
                        Err(Diagnostic::new(
                            "E_TYPE_009",
                            "numeric negation requires Int or Float",
                            inner.span,
                        ))
                    }
                }
            },
            ExprKind::Binary(op, left, right) => {
                self.synth_binary(expr.span, *op, left, right, ctx)
            }
        }
    }

    fn check_expr(&self, expr: &Expr, expected: &Ty, ctx: &Ctx) -> Result<Expr, Diagnostic> {
        let expanded = self.expand_alias(expected, ctx)?;
        match &expanded.kind {
            TyKind::Refine { binder, base, pred } => {
                let elab = self.check_expr(expr, base, ctx)?;
                let value = eval(&elab, &runtime_from_ctx(ctx), expr.span)?;
                self.validate_refinement(binder, pred, &value, expected, expr.span, ctx)?;
                Ok(elab)
            }
            TyKind::LiteralUnion(literals) => {
                let elab = self.check_expr(expr, &string_ty(expected.span), ctx)?;
                let value = eval(&elab, &runtime_from_ctx(ctx), expr.span)?;
                match value {
                    Value::String(value) if literals.iter().any(|lit| lit == &value) => Ok(elab),
                    Value::String(value) => Err(Diagnostic::new(
                        "E_REFINE_002",
                        format!(
                            "string literal `{value}` is not in {}",
                            self.ty_name(expected, ctx)
                        ),
                        expr.span,
                    )),
                    _ => Err(Diagnostic::new(
                        "E_REFINE_003",
                        "literal union predicate did not normalize to a string",
                        expr.span,
                    )),
                }
            }
            TyKind::Option(inner) => match &expr.kind {
                ExprKind::None => Ok(Expr {
                    kind: ExprKind::None,
                    span: expr.span,
                }),
                ExprKind::Some(value) => Ok(Expr {
                    kind: ExprKind::Some(Box::new(self.check_expr(value, inner, ctx)?)),
                    span: expr.span,
                }),
                _ => {
                    if let Ok((actual, elab)) = self.synth_expr(expr, ctx)
                        && self.compatible(&actual, expected, ctx)?
                    {
                        return Ok(elab);
                    }
                    let inner_elab = self.check_expr(expr, inner, ctx)?;
                    Ok(Expr {
                        kind: ExprKind::Some(Box::new(inner_elab)),
                        span: expr.span,
                    })
                }
            },
            TyKind::Record(expected_fields) => {
                let ExprKind::Record(actual_fields) = &expr.kind else {
                    let (actual, elab) = self.synth_expr(expr, ctx)?;
                    if self.compatible(&actual, expected, ctx)? {
                        return Ok(elab);
                    }
                    return Err(self.type_mismatch(expr.span, expected, &actual, ctx));
                };
                let mut provided: HashMap<String, &FieldExpr> = HashMap::new();
                for field in actual_fields {
                    if provided.insert(field.name.clone(), field).is_some() {
                        return Err(Diagnostic::new(
                            "E_RECORD_003",
                            format!("duplicate field `{}`", field.name),
                            field.span,
                        ));
                    }
                }
                for field in actual_fields {
                    if !expected_fields
                        .iter()
                        .any(|expected| expected.name == field.name)
                    {
                        return Err(Diagnostic::new(
                            "E_RECORD_004",
                            format!("unknown field `{}`", field.name),
                            field.span,
                        ));
                    }
                }
                let mut elaborated_fields = Vec::new();
                for expected_field in expected_fields {
                    if let Some(actual) = provided.get(&expected_field.name) {
                        elaborated_fields.push(FieldExpr {
                            name: expected_field.name.clone(),
                            value: self.check_expr(&actual.value, &expected_field.ty, ctx)?,
                            span: actual.span,
                        });
                    } else if is_option_ty(&self.expand_alias(&expected_field.ty, ctx)?) {
                        elaborated_fields.push(FieldExpr {
                            name: expected_field.name.clone(),
                            value: Expr {
                                kind: ExprKind::None,
                                span: expr.span,
                            },
                            span: expected_field.span,
                        });
                    } else {
                        return Err(Diagnostic::new(
                            "E_RECORD_005",
                            format!("missing field `{}`", expected_field.name),
                            expr.span,
                        ));
                    }
                }
                Ok(Expr {
                    kind: ExprKind::Record(elaborated_fields),
                    span: expr.span,
                })
            }
            TyKind::List(item_ty) => {
                if let ExprKind::List(items) = &expr.kind {
                    let mut elaborated = Vec::new();
                    for item in items {
                        elaborated.push(self.check_expr(item, item_ty, ctx)?);
                    }
                    Ok(Expr {
                        kind: ExprKind::List(elaborated),
                        span: expr.span,
                    })
                } else {
                    let (actual, elab) = self.synth_expr(expr, ctx)?;
                    if self.compatible(&actual, expected, ctx)? {
                        Ok(elab)
                    } else {
                        Err(self.type_mismatch(expr.span, expected, &actual, ctx))
                    }
                }
            }
            _ => {
                let (actual, elab) = self.synth_expr(expr, ctx)?;
                if self.compatible(&actual, expected, ctx)? {
                    Ok(elab)
                } else {
                    Err(self.type_mismatch(expr.span, expected, &actual, ctx))
                }
            }
        }
    }

    fn validate_refinement(
        &self,
        binder: &str,
        pred: &Expr,
        value: &Value,
        expected: &Ty,
        span: Span,
        ctx: &Ctx,
    ) -> Result<(), Diagnostic> {
        let mut env = runtime_from_ctx(ctx);
        env.insert(binder.to_string(), value.clone());
        match eval(pred, &env, pred.span)? {
            Value::Bool(true) => Ok(()),
            Value::Bool(false) => Err(Diagnostic::new("E_REFINE_004", "refinement failed", span)
                .note(format!(
                    "value `{}` does not satisfy {}",
                    value_debug(value),
                    self.ty_name(expected, ctx)
                ))),
            other => Err(Diagnostic::new(
                "E_REFINE_005",
                "refinement predicate did not normalize to Bool",
                pred.span,
            )
            .note(format!("predicate normalized to `{}`", value_debug(&other)))),
        }
    }

    fn synth_binary(
        &self,
        span: Span,
        op: BinaryOp,
        left: &Expr,
        right: &Expr,
        ctx: &Ctx,
    ) -> Result<(Ty, Expr), Diagnostic> {
        match op {
            BinaryOp::And | BinaryOp::Or => {
                let left_elab = self.check_expr(left, &bool_ty(left.span), ctx)?;
                let right_elab = self.check_expr(right, &bool_ty(right.span), ctx)?;
                Ok((bool_ty(span), binary_expr(span, op, left_elab, right_elab)))
            }
            BinaryOp::Concat => {
                let left_elab = self.check_expr(left, &string_ty(left.span), ctx)?;
                let right_elab = self.check_expr(right, &string_ty(right.span), ctx)?;
                Ok((
                    string_ty(span),
                    binary_expr(span, op, left_elab, right_elab),
                ))
            }
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => {
                let (left_ty, left_elab) = self.synth_expr(left, ctx)?;
                let left_expanded = self.expand_alias(&left_ty, ctx)?;
                match left_expanded.kind {
                    TyKind::Int => {
                        let right_elab = self.check_expr(right, &int_ty(right.span), ctx)?;
                        Ok((int_ty(span), binary_expr(span, op, left_elab, right_elab)))
                    }
                    TyKind::Float if !matches!(op, BinaryOp::Mod) => {
                        let right_elab = self.check_expr(right, &float_ty(right.span), ctx)?;
                        Ok((float_ty(span), binary_expr(span, op, left_elab, right_elab)))
                    }
                    _ => Err(Diagnostic::new(
                        "E_TYPE_010",
                        "arithmetic operators require matching numeric operands",
                        span,
                    )),
                }
            }
            BinaryOp::Eq | BinaryOp::Ne => {
                let (left_ty, left_elab) = self.synth_expr(left, ctx)?;
                let right_elab = self.check_expr(right, &left_ty, ctx)?;
                if !is_comparable(&self.expand_alias(&left_ty, ctx)?) {
                    return Err(Diagnostic::new(
                        "E_TYPE_011",
                        "functions and built-ins cannot be compared",
                        span,
                    ));
                }
                Ok((bool_ty(span), binary_expr(span, op, left_elab, right_elab)))
            }
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                let (left_ty, left_elab) = self.synth_expr(left, ctx)?;
                let expanded = self.expand_alias(&left_ty, ctx)?;
                match expanded.kind {
                    TyKind::Int | TyKind::Float | TyKind::String => {
                        let right_elab = self.check_expr(right, &left_ty, ctx)?;
                        Ok((bool_ty(span), binary_expr(span, op, left_elab, right_elab)))
                    }
                    _ => Err(Diagnostic::new(
                        "E_TYPE_012",
                        "ordering operators require Int, Float, or String",
                        span,
                    )),
                }
            }
        }
    }

    fn synth_method(
        &self,
        span: Span,
        base_ty: &Ty,
        expanded: &TyKind,
        base_elab: Expr,
        name: &str,
        _ctx: &Ctx,
    ) -> Result<(Ty, Expr), Diagnostic> {
        let base_expr = Box::new(base_elab);
        match (name, expanded) {
            ("isSome" | "isNone", TyKind::Option(_)) => Ok((
                bool_ty(span),
                Expr {
                    kind: ExprKind::Field(base_expr, name.to_string()),
                    span,
                },
            )),
            ("length", TyKind::List(_) | TyKind::String) => Ok((
                int_ty(span),
                Expr {
                    kind: ExprKind::Field(base_expr, name.to_string()),
                    span,
                },
            )),
            ("contains", TyKind::List(item)) => Ok((
                Ty {
                    kind: TyKind::Fun(item.clone(), Box::new(bool_ty(span))),
                    span,
                },
                Expr {
                    kind: ExprKind::Field(base_expr, name.to_string()),
                    span,
                },
            )),
            ("contains" | "startsWith" | "endsWith", TyKind::String) => Ok((
                Ty {
                    kind: TyKind::Fun(Box::new(string_ty(span)), Box::new(bool_ty(span))),
                    span,
                },
                Expr {
                    kind: ExprKind::Field(base_expr, name.to_string()),
                    span,
                },
            )),
            ("unwrapOr", TyKind::Option(item)) => Ok((
                Ty {
                    kind: TyKind::Fun(item.clone(), item.clone()),
                    span,
                },
                Expr {
                    kind: ExprKind::Field(base_expr, name.to_string()),
                    span,
                },
            )),
            _ => Err(Diagnostic::new(
                "E_TYPE_013",
                format!(
                    "type {} has no field or method `{name}`",
                    self.ty_name(base_ty, _ctx)
                ),
                span,
            )),
        }
    }

    fn synth_builtin_app(
        &self,
        span: Span,
        name: &str,
        func_elab: Expr,
        arg: &Expr,
        ctx: &Ctx,
    ) -> Result<(Ty, Expr), Diagnostic> {
        let (arg_ty, arg_elab) = self.synth_expr(arg, ctx)?;
        let arg_expanded = self.expand_alias(&arg_ty, ctx)?;
        let result_ty = match (name, &arg_expanded.kind) {
            ("show", ty) if is_showable_kind(ty) => string_ty(span),
            ("isSome" | "isNone", TyKind::Option(_)) => bool_ty(span),
            ("length", TyKind::List(_) | TyKind::String) => int_ty(span),
            ("contains", TyKind::List(item)) => Ty {
                kind: TyKind::Fun(item.clone(), Box::new(bool_ty(span))),
                span,
            },
            ("contains" | "startsWith" | "endsWith", TyKind::String) => Ty {
                kind: TyKind::Fun(Box::new(string_ty(span)), Box::new(bool_ty(span))),
                span,
            },
            ("unwrapOr", TyKind::Option(item)) => Ty {
                kind: TyKind::Fun(item.clone(), item.clone()),
                span,
            },
            _ => {
                return Err(Diagnostic::new(
                    "E_TYPE_014",
                    format!(
                        "unsupported built-in `{name}` for {}",
                        self.ty_name(&arg_ty, ctx)
                    ),
                    arg.span,
                ));
            }
        };
        Ok((
            result_ty,
            Expr {
                kind: ExprKind::App(Box::new(func_elab), Box::new(arg_elab)),
                span,
            },
        ))
    }

    fn compatible(&self, actual: &Ty, expected: &Ty, ctx: &Ctx) -> Result<bool, Diagnostic> {
        let actual = self.erase_refinements(&self.expand_alias(actual, ctx)?, ctx)?;
        let expected = self.erase_refinements(&self.expand_alias(expected, ctx)?, ctx)?;
        Ok(match (&actual.kind, &expected.kind) {
            (TyKind::Int, TyKind::Int)
            | (TyKind::Float, TyKind::Float)
            | (TyKind::Bool, TyKind::Bool)
            | (TyKind::String, TyKind::String) => true,
            (TyKind::LiteralUnion(_), TyKind::String)
            | (TyKind::String, TyKind::LiteralUnion(_)) => true,
            (TyKind::LiteralUnion(a), TyKind::LiteralUnion(b)) => a == b,
            (TyKind::Option(a), TyKind::Option(b)) | (TyKind::List(a), TyKind::List(b)) => {
                self.compatible(a, b, ctx)?
            }
            (TyKind::Record(a), TyKind::Record(b)) => {
                a.len() == b.len()
                    && a.iter().zip(b.iter()).all(|(af, bf)| {
                        af.name == bf.name && self.compatible(&af.ty, &bf.ty, ctx).unwrap_or(false)
                    })
            }
            (TyKind::Fun(ap, ar), TyKind::Fun(bp, br)) => {
                self.compatible(ap, bp, ctx)? && self.compatible(ar, br, ctx)?
            }
            _ => false,
        })
    }

    fn erase_refinements(&self, ty: &Ty, ctx: &Ctx) -> Result<Ty, Diagnostic> {
        Ok(match &ty.kind {
            TyKind::Refine { base, .. } => self.erase_refinements(base, ctx)?,
            TyKind::LiteralUnion(_) => string_ty(ty.span),
            TyKind::Option(inner) => Ty {
                kind: TyKind::Option(Box::new(self.erase_refinements(inner, ctx)?)),
                span: ty.span,
            },
            TyKind::List(inner) => Ty {
                kind: TyKind::List(Box::new(self.erase_refinements(inner, ctx)?)),
                span: ty.span,
            },
            TyKind::Record(fields) => Ty {
                kind: TyKind::Record(
                    fields
                        .iter()
                        .map(|field| {
                            Ok(FieldTy {
                                name: field.name.clone(),
                                ty: self.erase_refinements(&field.ty, ctx)?,
                                span: field.span,
                            })
                        })
                        .collect::<Result<Vec<_>, Diagnostic>>()?,
                ),
                span: ty.span,
            },
            TyKind::Fun(a, b) => Ty {
                kind: TyKind::Fun(
                    Box::new(self.erase_refinements(a, ctx)?),
                    Box::new(self.erase_refinements(b, ctx)?),
                ),
                span: ty.span,
            },
            TyKind::Alias(_) => self.erase_refinements(&self.expand_alias(ty, ctx)?, ctx)?,
            _ => ty.clone(),
        })
    }

    fn expand_alias(&self, ty: &Ty, ctx: &Ctx) -> Result<Ty, Diagnostic> {
        self.expand_alias_inner(ty, ctx, &mut Vec::new())
    }

    fn expand_alias_inner(
        &self,
        ty: &Ty,
        ctx: &Ctx,
        stack: &mut Vec<String>,
    ) -> Result<Ty, Diagnostic> {
        match &ty.kind {
            TyKind::Alias(name) => {
                if stack.contains(name) {
                    return Err(Diagnostic::new(
                        "E_TYPE_015",
                        format!("recursive type alias `{name}`"),
                        ty.span,
                    ));
                }
                let Some(alias) = ctx.types.get(name) else {
                    return Err(Diagnostic::new(
                        "E_TYPE_016",
                        format!("unknown type `{name}`"),
                        ty.span,
                    ));
                };
                stack.push(name.clone());
                let expanded = self.expand_alias_inner(alias, ctx, stack)?;
                stack.pop();
                Ok(expanded)
            }
            _ => Ok(ty.clone()),
        }
    }

    fn type_mismatch(&self, span: Span, expected: &Ty, actual: &Ty, ctx: &Ctx) -> Diagnostic {
        Diagnostic::new(
            "E_TYPE_017",
            format!(
                "type mismatch: expected {}, found {}",
                self.ty_name(expected, ctx),
                self.ty_name(actual, ctx)
            ),
            span,
        )
    }

    fn ty_name(&self, ty: &Ty, ctx: &Ctx) -> String {
        match self
            .expand_alias(ty, ctx)
            .unwrap_or_else(|_| ty.clone())
            .kind
        {
            TyKind::Int => "Int".to_string(),
            TyKind::Float => "Float".to_string(),
            TyKind::Bool => "Bool".to_string(),
            TyKind::String => "String".to_string(),
            TyKind::Option(inner) => format!("{}?", self.ty_name(&inner, ctx)),
            TyKind::List(inner) => format!("[{}]", self.ty_name(&inner, ctx)),
            TyKind::Record(fields) => {
                let fields = fields
                    .iter()
                    .map(|field| format!("{} : {}", field.name, self.ty_name(&field.ty, ctx)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{ {fields} }}")
            }
            TyKind::Refine { binder, base, .. } => {
                format!("{{ {binder} : {} | ... }}", self.ty_name(&base, ctx))
            }
            TyKind::Fun(param, result) => {
                format!(
                    "{} -> {}",
                    self.ty_name(&param, ctx),
                    self.ty_name(&result, ctx)
                )
            }
            TyKind::Alias(name) => name,
            TyKind::LiteralUnion(values) => values
                .iter()
                .map(|value| format!("{value:?}"))
                .collect::<Vec<_>>()
                .join(" | "),
            TyKind::Builtin(name) => format!("<builtin {name}>"),
        }
    }
}

fn canonicalize_existing(path: &Path) -> Result<PathBuf, String> {
    std::fs::canonicalize(path)
        .map_err(|err| format!("failed to resolve {}: {err}", path.display()))
}

fn resolve_import_path(from: &Path, import: &str) -> Result<PathBuf, String> {
    let base = from.parent().unwrap_or_else(|| Path::new("."));
    let path = base.join(import);
    canonicalize_existing(&path)
}

fn runtime_from_ctx(ctx: &Ctx) -> RuntimeEnv {
    ctx.values
        .iter()
        .map(|(name, info)| (name.clone(), info.value.clone()))
        .collect()
}

fn bool_ty(span: Span) -> Ty {
    Ty {
        kind: TyKind::Bool,
        span,
    }
}

fn int_ty(span: Span) -> Ty {
    Ty {
        kind: TyKind::Int,
        span,
    }
}

fn float_ty(span: Span) -> Ty {
    Ty {
        kind: TyKind::Float,
        span,
    }
}

fn string_ty(span: Span) -> Ty {
    Ty {
        kind: TyKind::String,
        span,
    }
}

fn is_option_ty(ty: &Ty) -> bool {
    matches!(ty.kind, TyKind::Option(_))
}

fn ty_mentions_alias(ty: &Ty, name: &str) -> bool {
    match &ty.kind {
        TyKind::Alias(alias) => alias == name,
        TyKind::Option(inner) | TyKind::List(inner) => ty_mentions_alias(inner, name),
        TyKind::Record(fields) => fields
            .iter()
            .any(|field| ty_mentions_alias(&field.ty, name)),
        TyKind::Refine { base, .. } => ty_mentions_alias(base, name),
        TyKind::Fun(param, result) => {
            ty_mentions_alias(param, name) || ty_mentions_alias(result, name)
        }
        TyKind::Int
        | TyKind::Float
        | TyKind::Bool
        | TyKind::String
        | TyKind::LiteralUnion(_)
        | TyKind::Builtin(_) => false,
    }
}

fn binary_expr(span: Span, op: BinaryOp, left: Expr, right: Expr) -> Expr {
    Expr {
        kind: ExprKind::Binary(op, Box::new(left), Box::new(right)),
        span,
    }
}

fn is_builtin_name(name: &str) -> bool {
    matches!(
        name,
        "show"
            | "isSome"
            | "isNone"
            | "length"
            | "contains"
            | "startsWith"
            | "endsWith"
            | "unwrapOr"
    )
}

fn is_showable(ty: &Ty) -> bool {
    is_showable_kind(&ty.kind)
}

fn is_showable_kind(ty: &TyKind) -> bool {
    matches!(
        ty,
        TyKind::Int | TyKind::Float | TyKind::Bool | TyKind::String | TyKind::LiteralUnion(_)
    )
}

fn is_comparable(ty: &Ty) -> bool {
    match &ty.kind {
        TyKind::Fun(_, _) | TyKind::Builtin(_) => false,
        TyKind::Option(inner) | TyKind::List(inner) => is_comparable(inner),
        TyKind::Record(fields) => fields.iter().all(|field| is_comparable(&field.ty)),
        TyKind::Refine { base, .. } => is_comparable(base),
        _ => true,
    }
}
