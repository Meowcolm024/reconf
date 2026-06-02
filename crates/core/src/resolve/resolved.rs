use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::core::{CoreDecl, CoreExpr, CoreImport, CoreModule, CoreType, GlobalRef, TypeAliasRef};
use crate::error::{Error, ErrorCode, Result};
use crate::eval::Value;
use crate::resolve::names::{BindingIds, TypeAliasIds};

pub trait ResolvedValueBindings {
    fn value_binding(&self, name: &str) -> Option<GlobalRef>;
}

pub trait ResolvedTypeBindings {
    fn type_binding(&self, name: &str) -> Option<TypeAliasRef>;
}

#[derive(Clone, Default)]
pub struct ResolvedProgram {
    modules: BTreeMap<PathBuf, ResolvedModule>,
}

#[derive(Clone, Default)]
pub struct ResolvedModule {
    path: PathBuf,
    body: ResolvedModuleBody,
    exports: ResolvedExports,
}

#[derive(Clone)]
pub struct ResolvedModuleBuilder {
    path: PathBuf,
    body: ResolvedModuleBody,
}

#[derive(Clone, Default)]
pub struct ResolvedModuleBody {
    imports: Vec<ResolvedImport>,
    decls: Vec<ResolvedDecl>,
    output: Option<CoreExpr>,
}

#[derive(Clone)]
pub enum ResolvedDecl {
    Native {
        export: bool,
        name: String,
        binding: GlobalRef,
        ty: CoreType,
    },
    Type {
        export: bool,
        name: String,
        alias: TypeAliasRef,
        ty: CoreType,
    },
    Let {
        export: bool,
        name: String,
        binding: GlobalRef,
        annotation: Option<CoreType>,
        expr: CoreExpr,
    },
}

#[derive(Clone)]
pub struct ResolvedImport {
    path: String,
    names: Vec<String>,
}

#[derive(Clone, Default)]
pub struct ResolvedExports {
    exports: BTreeMap<String, ResolvedExport>,
}

#[derive(Default)]
pub struct ResolvedExportsBuilder {
    exports: BTreeMap<String, ResolvedExport>,
}

#[derive(Clone)]
pub enum ResolvedExport {
    Value(ResolvedValueExport),
    Type(ResolvedTypeExport),
}

#[derive(Clone)]
pub struct ResolvedValueExport {
    value: Value,
    ty: Option<CoreType>,
}

#[derive(Clone)]
pub struct ResolvedTypeExport {
    alias: TypeAliasRef,
    ty: CoreType,
}

pub struct ResolvedImportSelection {
    names: BTreeMap<String, ResolvedExport>,
}

pub struct ResolvedImportSelector<'a> {
    exports: &'a ResolvedExports,
}

impl ResolvedProgram {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_module(&mut self, module: ResolvedModule) {
        self.modules.insert(module.path.clone(), module);
    }

    pub fn module(&self, path: &Path) -> Option<&ResolvedModule> {
        self.modules.get(path)
    }
}

impl ResolvedModule {
    pub fn new(path: PathBuf, body: ResolvedModuleBody, exports: ResolvedExports) -> Self {
        Self {
            path,
            body,
            exports,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn body(&self) -> &ResolvedModuleBody {
        &self.body
    }

    pub fn exports(&self) -> &ResolvedExports {
        &self.exports
    }
}

impl ResolvedModuleBuilder {
    pub fn new(path: PathBuf, body: ResolvedModuleBody) -> Self {
        Self { path, body }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn body(&self) -> &ResolvedModuleBody {
        &self.body
    }

    pub fn finish(self, exports: ResolvedExports) -> ResolvedModule {
        ResolvedModule::new(self.path, self.body, exports)
    }
}

impl ResolvedModuleBody {
    pub fn from_core(module: CoreModule) -> Self {
        ResolvedBodyResolver::from_core(module).resolve_module_bindings()
    }

    pub fn from_parts(
        imports: Vec<ResolvedImport>,
        decls: Vec<ResolvedDecl>,
        output: Option<CoreExpr>,
    ) -> Self {
        Self {
            imports,
            decls,
            output,
        }
    }

    pub fn imports(&self) -> &[ResolvedImport] {
        &self.imports
    }

    pub fn decls(&self) -> &[ResolvedDecl] {
        &self.decls
    }

    pub fn output(&self) -> Option<&CoreExpr> {
        self.output.as_ref()
    }

    pub fn into_core_parts(self) -> (Vec<ResolvedImport>, Vec<ResolvedDecl>, Option<CoreExpr>) {
        (self.imports, self.decls, self.output)
    }

    pub fn resolve_external_values(self, values: &dyn ResolvedValueBindings) -> Self {
        ResolvedBodyResolver::from_body(self).resolve_external_values(values)
    }

    pub fn resolve_external_types(self, types: &dyn ResolvedTypeBindings) -> Self {
        ResolvedBodyResolver::from_body(self).resolve_external_types(types)
    }

    pub fn rebase_value_bindings_from(self, next_binding_id: usize) -> Self {
        ResolvedBodyResolver::from_body(self).rebase_value_bindings_from(next_binding_id)
    }
}

impl ResolvedDecl {
    fn from_core(
        decl: CoreDecl,
        bindings: &mut BindingIds,
        type_aliases: &mut TypeAliasIds,
    ) -> Self {
        match decl {
            CoreDecl::Native { export, name, ty } => Self::Native {
                export,
                name,
                binding: bindings.fresh(),
                ty,
            },
            CoreDecl::Type { export, name, ty } => Self::Type {
                export,
                name,
                alias: type_aliases.fresh(),
                ty,
            },
            CoreDecl::Let {
                export,
                name,
                annotation,
                expr,
            } => Self::Let {
                export,
                name,
                binding: bindings.fresh(),
                annotation,
                expr,
            },
        }
    }

    pub fn binding(&self) -> Option<GlobalRef> {
        match self {
            Self::Native { binding, .. } | Self::Let { binding, .. } => Some(*binding),
            Self::Type { .. } => None,
        }
    }

    pub fn type_alias(&self) -> Option<TypeAliasRef> {
        match self {
            Self::Type { alias, .. } => Some(*alias),
            _ => None,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Native { name, .. } | Self::Type { name, .. } | Self::Let { name, .. } => name,
        }
    }
}

impl From<CoreDecl> for ResolvedDecl {
    fn from(decl: CoreDecl) -> Self {
        let mut bindings = BindingIds::new();
        let mut type_aliases = TypeAliasIds::new();
        Self::from_core(decl, &mut bindings, &mut type_aliases)
    }
}

struct ResolvedBodyResolver {
    body: ResolvedModuleBody,
}

impl ResolvedBodyResolver {
    fn from_core(module: CoreModule) -> Self {
        let mut bindings = BindingIds::new();
        let mut type_aliases = TypeAliasIds::new();
        let decls = module
            .decls
            .into_iter()
            .map(|decl| ResolvedDecl::from_core(decl, &mut bindings, &mut type_aliases))
            .collect::<Vec<_>>();

        Self::from_body(ResolvedModuleBody {
            imports: module
                .imports
                .into_iter()
                .map(ResolvedImport::new)
                .collect(),
            decls,
            output: module.output,
        })
    }

    fn from_body(body: ResolvedModuleBody) -> Self {
        Self { body }
    }

    fn resolve_module_bindings(self) -> ResolvedModuleBody {
        let value_bindings = ModuleValueBindings::from_decls(&self.body.decls);
        let type_bindings = ModuleTypeBindings::from_decls(&self.body.decls);
        self.resolve_types(&type_bindings)
            .resolve_values(&value_bindings)
    }

    fn resolve_external_values(self, values: &dyn ResolvedValueBindings) -> ResolvedModuleBody {
        self.resolve_values(&ExternalValueBindings { values })
    }

    fn resolve_external_types(self, types: &dyn ResolvedTypeBindings) -> ResolvedModuleBody {
        self.resolve_types(&ExternalTypeBindings { types }).body
    }

    fn rebase_value_bindings_from(self, next_binding_id: usize) -> ResolvedModuleBody {
        BindingRebaser::new(next_binding_id).rebase(self.body)
    }

    fn resolve_values(mut self, bindings: &dyn ValueBindings) -> ResolvedModuleBody {
        let mut resolver = ValueReferenceResolver::new(bindings);
        self.body.decls = self
            .body
            .decls
            .into_iter()
            .map(|decl| resolver.resolve_decl(decl))
            .collect();
        self.body.output = self.body.output.map(|expr| resolver.resolve_expr(expr));
        self.body
    }

    fn resolve_types(mut self, bindings: &dyn TypeBindings) -> Self {
        let mut resolver = TypeReferenceResolver::new(bindings);
        self.body.decls = self
            .body
            .decls
            .into_iter()
            .map(|decl| resolver.resolve_decl(decl))
            .collect();
        self
    }
}

struct BindingRebaser {
    bindings: BindingIds,
    remap: BTreeMap<GlobalRef, GlobalRef>,
}

impl BindingRebaser {
    fn new(next_binding_id: usize) -> Self {
        Self {
            bindings: BindingIds::from_next(next_binding_id),
            remap: BTreeMap::new(),
        }
    }

    fn rebase(mut self, mut body: ResolvedModuleBody) -> ResolvedModuleBody {
        body.decls = body
            .decls
            .into_iter()
            .map(|decl| self.rebase_decl_binding(decl))
            .collect();
        let mut rebinder = GlobalReferenceRebinder::new(&self.remap);
        body.decls = body
            .decls
            .into_iter()
            .map(|decl| rebinder.rebind_decl(decl))
            .collect();
        body.output = body.output.map(|expr| rebinder.rebind_expr(expr));
        body
    }

    fn rebase_decl_binding(&mut self, decl: ResolvedDecl) -> ResolvedDecl {
        match decl {
            ResolvedDecl::Native {
                export,
                name,
                binding,
                ty,
            } => {
                let next = self.bindings.fresh();
                self.remap.insert(binding, next);
                ResolvedDecl::Native {
                    export,
                    name,
                    binding: next,
                    ty,
                }
            }
            ResolvedDecl::Let {
                export,
                name,
                binding,
                annotation,
                expr,
            } => {
                let next = self.bindings.fresh();
                self.remap.insert(binding, next);
                ResolvedDecl::Let {
                    export,
                    name,
                    binding: next,
                    annotation,
                    expr,
                }
            }
            ResolvedDecl::Type {
                export,
                name,
                alias,
                ty,
            } => ResolvedDecl::Type {
                export,
                name,
                alias,
                ty,
            },
        }
    }
}

struct ModuleTypeBindings {
    aliases: BTreeMap<String, TypeAliasRef>,
}

impl ModuleTypeBindings {
    fn from_decls(decls: &[ResolvedDecl]) -> Self {
        let mut aliases = BTreeMap::new();
        for decl in decls {
            if let Some(alias) = decl.type_alias() {
                aliases.insert(decl.name().to_string(), alias);
            }
        }
        Self { aliases }
    }
}

trait TypeBindings {
    fn get(&self, name: &str) -> Option<TypeAliasRef>;
}

impl TypeBindings for ModuleTypeBindings {
    fn get(&self, name: &str) -> Option<TypeAliasRef> {
        self.aliases.get(name).copied()
    }
}

struct ExternalTypeBindings<'a> {
    types: &'a dyn ResolvedTypeBindings,
}

impl TypeBindings for ExternalTypeBindings<'_> {
    fn get(&self, name: &str) -> Option<TypeAliasRef> {
        self.types.type_binding(name)
    }
}

struct ModuleValueBindings {
    values: BTreeMap<String, GlobalRef>,
}

impl ModuleValueBindings {
    fn from_decls(decls: &[ResolvedDecl]) -> Self {
        let mut values = BTreeMap::new();
        for decl in decls {
            if let Some(binding) = decl.binding() {
                values.insert(decl.name().to_string(), binding);
            }
        }
        Self { values }
    }
}

impl ValueBindings for ModuleValueBindings {
    fn get(&self, name: &str) -> Option<GlobalRef> {
        self.values.get(name).copied()
    }
}

struct TypeReferenceResolver<'a> {
    aliases: &'a dyn TypeBindings,
}

impl<'a> TypeReferenceResolver<'a> {
    fn new(aliases: &'a dyn TypeBindings) -> Self {
        Self { aliases }
    }

    fn resolve_decl(&mut self, decl: ResolvedDecl) -> ResolvedDecl {
        match decl {
            ResolvedDecl::Native {
                export,
                name,
                binding,
                ty,
            } => ResolvedDecl::Native {
                export,
                name,
                binding,
                ty: self.resolve_type(ty),
            },
            ResolvedDecl::Type {
                export,
                name,
                alias,
                ty,
            } => ResolvedDecl::Type {
                export,
                name,
                alias,
                ty: self.resolve_type(ty),
            },
            ResolvedDecl::Let {
                export,
                name,
                binding,
                annotation,
                expr,
            } => ResolvedDecl::Let {
                export,
                name,
                binding,
                annotation: annotation.map(|ty| self.resolve_type(ty)),
                expr,
            },
        }
    }

    fn resolve_type(&mut self, ty: CoreType) -> CoreType {
        match ty {
            CoreType::Spanned(ty, span) => {
                CoreType::Spanned(Box::new(self.resolve_type(*ty)), span)
            }
            CoreType::Alias(name) => self
                .aliases
                .get(&name)
                .map(CoreType::ResolvedAlias)
                .unwrap_or(CoreType::Alias(name)),
            CoreType::Option(inner) => CoreType::Option(Box::new(self.resolve_type(*inner))),
            CoreType::List(inner) => CoreType::List(Box::new(self.resolve_type(*inner))),
            CoreType::Record(fields) => CoreType::Record(
                fields
                    .into_iter()
                    .map(|(name, ty)| (name, self.resolve_type(ty)))
                    .collect(),
            ),
            CoreType::Refinement { binder, base, pred } => CoreType::Refinement {
                binder,
                base: Box::new(self.resolve_type(*base)),
                pred,
            },
            CoreType::Function(input, output) => CoreType::Function(
                Box::new(self.resolve_type(*input)),
                Box::new(self.resolve_type(*output)),
            ),
            CoreType::Int
            | CoreType::Float
            | CoreType::Bool
            | CoreType::String
            | CoreType::LiteralUnion(_)
            | CoreType::ResolvedAlias(_) => ty,
        }
    }
}

trait ValueBindings {
    fn get(&self, name: &str) -> Option<GlobalRef>;
}

struct ExternalValueBindings<'a> {
    values: &'a dyn ResolvedValueBindings,
}

impl ValueBindings for ExternalValueBindings<'_> {
    fn get(&self, name: &str) -> Option<GlobalRef> {
        self.values.value_binding(name)
    }
}

struct ValueReferenceResolver<'a> {
    bindings: &'a dyn ValueBindings,
}

impl<'a> ValueReferenceResolver<'a> {
    fn new(bindings: &'a dyn ValueBindings) -> Self {
        Self { bindings }
    }

    fn resolve_decl(&mut self, decl: ResolvedDecl) -> ResolvedDecl {
        match decl {
            ResolvedDecl::Let {
                export,
                name,
                binding,
                annotation,
                expr,
            } => ResolvedDecl::Let {
                export,
                name,
                binding,
                annotation,
                expr: self.resolve_expr(expr),
            },
            decl => decl,
        }
    }

    fn resolve_expr(&mut self, expr: CoreExpr) -> CoreExpr {
        self.resolve_expr_in_scope(expr, &LexicalShadowScope::new())
    }

    fn resolve_expr_in_scope(&mut self, expr: CoreExpr, scope: &LexicalShadowScope) -> CoreExpr {
        match expr {
            CoreExpr::Spanned(expr, span) => {
                CoreExpr::Spanned(Box::new(self.resolve_expr_in_scope(*expr, scope)), span)
            }
            CoreExpr::Some(expr) => {
                CoreExpr::Some(Box::new(self.resolve_expr_in_scope(*expr, scope)))
            }
            CoreExpr::Var(name) if scope.shadows(&name) => CoreExpr::Var(name),
            CoreExpr::Var(name) => self
                .bindings
                .get(&name)
                .map(CoreExpr::Global)
                .unwrap_or(CoreExpr::Var(name)),
            CoreExpr::List(items) => CoreExpr::List(
                items
                    .into_iter()
                    .map(|item| self.resolve_expr_in_scope(item, scope))
                    .collect(),
            ),
            CoreExpr::Record(fields) => CoreExpr::Record(
                fields
                    .into_iter()
                    .map(|(name, expr)| (name, self.resolve_expr_in_scope(expr, scope)))
                    .collect(),
            ),
            CoreExpr::Field(expr, field) => {
                CoreExpr::Field(Box::new(self.resolve_expr_in_scope(*expr, scope)), field)
            }
            CoreExpr::If(cond, then_expr, else_expr) => CoreExpr::If(
                Box::new(self.resolve_expr_in_scope(*cond, scope)),
                Box::new(self.resolve_expr_in_scope(*then_expr, scope)),
                Box::new(self.resolve_expr_in_scope(*else_expr, scope)),
            ),
            CoreExpr::Let(name, annotation, value, body) => {
                let value = self.resolve_expr_in_scope(*value, scope);
                let body_scope = scope.with_shadow(name.clone());
                let body = self.resolve_expr_in_scope(*body, &body_scope);
                CoreExpr::Let(name, annotation, Box::new(value), Box::new(body))
            }
            CoreExpr::Lambda(param, ty, body) => {
                let body_scope = scope.with_shadow(param.clone());
                CoreExpr::Lambda(
                    param,
                    ty,
                    Box::new(self.resolve_expr_in_scope(*body, &body_scope)),
                )
            }
            CoreExpr::Apply(function, arg) => CoreExpr::Apply(
                Box::new(self.resolve_expr_in_scope(*function, scope)),
                Box::new(self.resolve_expr_in_scope(*arg, scope)),
            ),
            CoreExpr::Ascribe(expr, ty) => {
                CoreExpr::Ascribe(Box::new(self.resolve_expr_in_scope(*expr, scope)), ty)
            }
            CoreExpr::Unary(op, expr) => {
                CoreExpr::Unary(op, Box::new(self.resolve_expr_in_scope(*expr, scope)))
            }
            CoreExpr::Binary(op, left, right) => CoreExpr::Binary(
                op,
                Box::new(self.resolve_expr_in_scope(*left, scope)),
                Box::new(self.resolve_expr_in_scope(*right, scope)),
            ),
            CoreExpr::Int(_)
            | CoreExpr::Float(_)
            | CoreExpr::Bool(_)
            | CoreExpr::String(_)
            | CoreExpr::None
            | CoreExpr::Local(_)
            | CoreExpr::Global(_) => expr,
        }
    }
}

#[derive(Clone, Default)]
struct LexicalShadowScope {
    names: Vec<String>,
}

impl LexicalShadowScope {
    fn new() -> Self {
        Self::default()
    }

    fn with_shadow(&self, name: String) -> Self {
        let mut names = self.names.clone();
        names.push(name);
        Self { names }
    }

    fn shadows(&self, name: &str) -> bool {
        self.names.iter().any(|local| local == name)
    }
}

struct GlobalReferenceRebinder<'a> {
    remap: &'a BTreeMap<GlobalRef, GlobalRef>,
}

impl<'a> GlobalReferenceRebinder<'a> {
    fn new(remap: &'a BTreeMap<GlobalRef, GlobalRef>) -> Self {
        Self { remap }
    }

    fn rebind_decl(&mut self, decl: ResolvedDecl) -> ResolvedDecl {
        match decl {
            ResolvedDecl::Let {
                export,
                name,
                binding,
                annotation,
                expr,
            } => ResolvedDecl::Let {
                export,
                name,
                binding,
                annotation,
                expr: self.rebind_expr(expr),
            },
            decl => decl,
        }
    }

    fn rebind_expr(&mut self, expr: CoreExpr) -> CoreExpr {
        match expr {
            CoreExpr::Spanned(expr, span) => {
                CoreExpr::Spanned(Box::new(self.rebind_expr(*expr)), span)
            }
            CoreExpr::Some(expr) => CoreExpr::Some(Box::new(self.rebind_expr(*expr))),
            CoreExpr::Global(binding) => self
                .remap
                .get(&binding)
                .copied()
                .map(CoreExpr::Global)
                .unwrap_or(CoreExpr::Global(binding)),
            CoreExpr::List(items) => CoreExpr::List(
                items
                    .into_iter()
                    .map(|item| self.rebind_expr(item))
                    .collect(),
            ),
            CoreExpr::Record(fields) => CoreExpr::Record(
                fields
                    .into_iter()
                    .map(|(name, expr)| (name, self.rebind_expr(expr)))
                    .collect(),
            ),
            CoreExpr::Field(expr, field) => {
                CoreExpr::Field(Box::new(self.rebind_expr(*expr)), field)
            }
            CoreExpr::If(cond, then_expr, else_expr) => CoreExpr::If(
                Box::new(self.rebind_expr(*cond)),
                Box::new(self.rebind_expr(*then_expr)),
                Box::new(self.rebind_expr(*else_expr)),
            ),
            CoreExpr::Let(name, annotation, value, body) => CoreExpr::Let(
                name,
                annotation,
                Box::new(self.rebind_expr(*value)),
                Box::new(self.rebind_expr(*body)),
            ),
            CoreExpr::Lambda(param, ty, body) => {
                CoreExpr::Lambda(param, ty, Box::new(self.rebind_expr(*body)))
            }
            CoreExpr::Apply(function, arg) => CoreExpr::Apply(
                Box::new(self.rebind_expr(*function)),
                Box::new(self.rebind_expr(*arg)),
            ),
            CoreExpr::Ascribe(expr, ty) => CoreExpr::Ascribe(Box::new(self.rebind_expr(*expr)), ty),
            CoreExpr::Unary(op, expr) => CoreExpr::Unary(op, Box::new(self.rebind_expr(*expr))),
            CoreExpr::Binary(op, left, right) => CoreExpr::Binary(
                op,
                Box::new(self.rebind_expr(*left)),
                Box::new(self.rebind_expr(*right)),
            ),
            CoreExpr::Int(_)
            | CoreExpr::Float(_)
            | CoreExpr::Bool(_)
            | CoreExpr::String(_)
            | CoreExpr::None
            | CoreExpr::Var(_)
            | CoreExpr::Local(_) => expr,
        }
    }
}

impl ResolvedImport {
    pub fn new(core: CoreImport) -> Self {
        Self {
            path: core.path,
            names: core.names,
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn names(&self) -> &[String] {
        &self.names
    }

    pub fn select_from(&self, exports: &ResolvedExports) -> Result<ResolvedImportSelection> {
        ResolvedImportSelector::new(exports).select(self.names.iter().map(String::as_str))
    }
}

impl ResolvedValueExport {
    pub fn new(value: Value, ty: CoreType) -> Self {
        Self {
            value,
            ty: Some(ty),
        }
    }

    pub fn untyped(value: Value) -> Self {
        Self { value, ty: None }
    }

    pub fn value(&self) -> &Value {
        &self.value
    }

    pub fn ty(&self) -> Option<&CoreType> {
        self.ty.as_ref()
    }

    pub fn into_value(self) -> Value {
        self.value
    }
}

impl ResolvedTypeExport {
    pub fn new(alias: TypeAliasRef, ty: CoreType) -> Self {
        Self { alias, ty }
    }

    pub fn alias(&self) -> TypeAliasRef {
        self.alias
    }

    pub fn ty(&self) -> &CoreType {
        &self.ty
    }
}

impl ResolvedExports {
    fn new(exports: BTreeMap<String, ResolvedExport>) -> Self {
        Self { exports }
    }

    pub fn builder() -> ResolvedExportsBuilder {
        ResolvedExportsBuilder::default()
    }

    pub fn get(&self, name: &str) -> Option<&ResolvedExport> {
        self.exports.get(name)
    }
}

impl ResolvedExportsBuilder {
    pub fn define_value(&mut self, name: String, value: ResolvedValueExport) {
        self.exports.insert(name, ResolvedExport::Value(value));
    }

    pub fn define_type(&mut self, name: String, ty: ResolvedTypeExport) {
        self.exports.insert(name, ResolvedExport::Type(ty));
    }

    pub fn finish(self) -> ResolvedExports {
        ResolvedExports::new(self.exports)
    }
}

impl<'a> ResolvedImportSelector<'a> {
    pub fn new(exports: &'a ResolvedExports) -> Self {
        Self { exports }
    }

    pub fn select<'b>(
        &self,
        names: impl IntoIterator<Item = &'b str>,
    ) -> Result<ResolvedImportSelection> {
        let mut selected = BTreeMap::new();
        for name in names {
            if selected.contains_key(name) {
                return Err(Error::with_code(
                    ErrorCode::NameDuplicateImport,
                    format!("duplicate import `{name}`"),
                ));
            }
            let export = self.exports.get(name).cloned().ok_or_else(|| {
                Error::with_code(
                    ErrorCode::ModuleUnexportedImport,
                    format!("unexported import `{name}`"),
                )
            })?;
            selected.insert(name.to_string(), export);
        }
        Ok(ResolvedImportSelection { names: selected })
    }
}

impl ResolvedImportSelection {
    pub fn apply_to(&self, mut target: impl ResolvedImportTarget) {
        for (name, export) in &self.names {
            match export {
                ResolvedExport::Value(value) => target.import_value(name, value),
                ResolvedExport::Type(ty) => target.import_type(name, ty),
            }
        }
    }
}

pub trait ResolvedImportTarget {
    fn import_value(&mut self, name: &str, value: &ResolvedValueExport);

    fn import_type(&mut self, name: &str, ty: &ResolvedTypeExport);
}

impl<T: ResolvedImportTarget + ?Sized> ResolvedImportTarget for &mut T {
    fn import_value(&mut self, name: &str, value: &ResolvedValueExport) {
        (**self).import_value(name, value);
    }

    fn import_type(&mut self, name: &str, ty: &ResolvedTypeExport) {
        (**self).import_type(name, ty);
    }
}
