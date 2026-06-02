use std::collections::BTreeMap;

use crate::core::{CoreType, CoreTypeEnv, GlobalRef, TypeAliasRef};
use crate::error::{Error, Result};
use crate::eval::builtins::NativeFunction;
use crate::eval::{Env, Value};
use crate::resolve::names::{BindingIds, NameScope, Namespace};
use crate::resolve::resolved::{
    ResolvedExports, ResolvedImportSelection, ResolvedImportTarget, ResolvedTypeBindings,
    ResolvedTypeExport, ResolvedValueBindings, ResolvedValueExport,
};
use crate::typeck::CoreValueTypeContext;

#[derive(Clone, Default)]
pub struct Module {
    binding_ids: BindingIds,
    value_bindings: BTreeMap<String, GlobalRef>,
    globals: BTreeMap<GlobalRef, Value>,
    values: BTreeMap<String, Value>,
    value_types: BTreeMap<String, CoreType>,
    types: CoreTypeEnv,
    exports: BTreeMap<String, Export>,
}

#[derive(Clone)]
enum Export {
    Value(ValueExport),
    Type(TypeExport),
}

#[derive(Clone)]
struct ValueExport {
    value: Value,
    ty: Option<CoreType>,
}

#[derive(Clone)]
struct TypeExport {
    alias: TypeAliasRef,
    ty: CoreType,
}

pub(super) struct ModuleValueTypes<'a> {
    module: &'a Module,
}

#[derive(Clone, Default)]
pub struct ModuleContext {
    module: Module,
}

impl ModuleContext {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn from_module(module: Module) -> Self {
        Self { module }
    }

    pub(super) fn into_module(self) -> Module {
        self.module
    }
}

impl Module {
    pub fn value(&self, name: &str) -> Option<&Value> {
        self.values.get(name)
    }

    pub fn output(&self) -> Result<&Value> {
        self.value("$output")
            .ok_or_else(|| Error::new("internal error: missing output"))
    }

    pub fn into_output(self) -> Result<Value> {
        self.values
            .get("$output")
            .cloned()
            .ok_or_else(|| Error::new("internal error: missing output"))
    }

    pub(crate) fn resolved_exports(&self) -> ResolvedExports {
        ResolvedExports::from(self)
    }

    pub(super) fn types(&self) -> &CoreTypeEnv {
        &self.types
    }

    pub(super) fn value_types(&self) -> ModuleValueTypes<'_> {
        ModuleValueTypes { module: self }
    }

    fn value_type(&self, name: &str) -> Option<&CoreType> {
        self.value_types.get(name)
    }

    fn value_binding(&self, name: &str) -> Option<GlobalRef> {
        self.value_bindings.get(name).copied()
    }

    pub(super) fn next_binding_id(&self) -> usize {
        self.binding_ids.next()
    }

    fn value_type_by_binding(&self, binding: GlobalRef) -> Option<&CoreType> {
        self.value_bindings
            .iter()
            .find(|(_, candidate)| **candidate == binding)
            .and_then(|(name, _)| self.value_type(name))
    }

    pub(super) fn runtime_env(&self) -> Env {
        Env::from_bindings(self.globals.clone(), self.values.clone())
    }

    pub(super) fn name_scope(&self) -> NameScope {
        let mut scope = NameScope::new();
        for name in self.values.keys() {
            scope.define(Namespace::Value, name.clone());
        }
        self.types
            .define_names_with(|name| scope.define(Namespace::Type, name));
        scope
    }

    pub(super) fn define_output(&mut self, value: Value) {
        self.values.insert("$output".to_string(), value);
    }

    pub(super) fn define_native(
        &mut self,
        export: bool,
        name: String,
        binding: GlobalRef,
        ty: CoreType,
    ) {
        let value = Value::Native(NativeFunction::new(name.clone()));
        self.define_bound_runtime_value(name.clone(), binding, value.clone());
        self.values.insert(name.clone(), value.clone());
        self.value_types.insert(name.clone(), ty.clone());
        if export {
            self.exports
                .insert(name, Export::Value(ValueExport::typed(value, ty)));
        }
    }

    pub(super) fn define_type(
        &mut self,
        export: bool,
        name: String,
        alias: TypeAliasRef,
        ty: CoreType,
    ) {
        self.types.define_with_ref(name.clone(), alias, ty.clone());
        if export {
            self.exports
                .insert(name, Export::Type(TypeExport { alias, ty }));
        }
    }

    pub(super) fn define_value(
        &mut self,
        export: bool,
        name: String,
        binding: GlobalRef,
        value: Value,
        ty: Option<CoreType>,
    ) {
        self.define_bound_runtime_value(name.clone(), binding, value.clone());
        self.values.insert(name.clone(), value.clone());
        if let Some(ty) = &ty {
            self.value_types.insert(name.clone(), ty.clone());
        }
        if export {
            let export = ty
                .map(|ty| ValueExport::typed(value.clone(), ty))
                .unwrap_or_else(|| ValueExport::untyped(value));
            self.exports.insert(name, Export::Value(export));
        }
    }

    pub(super) fn import_selection(&mut self, selection: &ResolvedImportSelection) {
        selection.apply_to(self);
    }

    fn define_runtime_value(&mut self, name: String, value: Value) -> GlobalRef {
        let binding = self.value_bindings.get(&name).copied().unwrap_or_else(|| {
            let binding = self.binding_ids.fresh();
            self.value_bindings.insert(name.clone(), binding);
            binding
        });
        self.globals.insert(binding, value);
        binding
    }

    fn define_bound_runtime_value(&mut self, name: String, binding: GlobalRef, value: Value) {
        self.binding_ids.reserve(binding);
        self.value_bindings.insert(name, binding);
        self.globals.insert(binding, value);
    }
}

impl CoreValueTypeContext for ModuleValueTypes<'_> {
    fn value_type(&self, name: &str) -> Option<&CoreType> {
        self.module.value_type(name)
    }

    fn global_value(&self, name: &str) -> Option<(GlobalRef, &CoreType)> {
        let binding = self.module.value_binding(name)?;
        let ty = self.module.value_type(name)?;
        Some((binding, ty))
    }

    fn global_type(&self, binding: GlobalRef) -> Option<&CoreType> {
        self.module.value_type_by_binding(binding)
    }
}

impl ResolvedValueBindings for ModuleValueTypes<'_> {
    fn value_binding(&self, name: &str) -> Option<GlobalRef> {
        self.module.value_binding(name)
    }
}

impl ResolvedTypeBindings for ModuleValueTypes<'_> {
    fn type_binding(&self, name: &str) -> Option<TypeAliasRef> {
        self.module.types.alias_ref(name)
    }
}

impl ResolvedImportTarget for Module {
    fn import_value(&mut self, name: &str, value: &ResolvedValueExport) {
        self.define_runtime_value(name.to_string(), value.value().clone());
        self.values.insert(name.to_string(), value.value().clone());
        if let Some(ty) = value.ty() {
            self.value_types.insert(name.to_string(), ty.clone());
        }
    }

    fn import_type(&mut self, name: &str, ty: &ResolvedTypeExport) {
        self.types
            .define_with_ref(name.to_string(), ty.alias(), ty.ty().clone());
    }
}

impl ValueExport {
    fn typed(value: Value, ty: CoreType) -> Self {
        Self {
            value,
            ty: Some(ty),
        }
    }

    fn untyped(value: Value) -> Self {
        Self { value, ty: None }
    }

    fn to_resolved(&self) -> ResolvedValueExport {
        match &self.ty {
            Some(ty) => ResolvedValueExport::new(self.value.clone(), ty.clone()),
            None => ResolvedValueExport::untyped(self.value.clone()),
        }
    }
}

impl TypeExport {
    fn to_resolved(&self) -> ResolvedTypeExport {
        ResolvedTypeExport::new(self.alias, self.ty.clone())
    }
}

impl From<&Module> for ResolvedExports {
    fn from(module: &Module) -> Self {
        let mut exports = ResolvedExports::builder();
        for (name, export) in &module.exports {
            match export {
                Export::Value(value) => exports.define_value(name.clone(), value.to_resolved()),
                Export::Type(ty) => exports.define_type(name.clone(), ty.to_resolved()),
            }
        }
        exports.finish()
    }
}
