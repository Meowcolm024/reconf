use std::path::Path;

use crate::compiler::module::imports::ModuleImportBinder;
use crate::compiler::module::state::{Module, ModuleContext};
use reconf_core::core::{ElaboratedDecl, ElaboratedExpr, ElaboratedModule};
use reconf_core::error::Result;
use reconf_core::eval::Value;
use reconf_core::eval::core::PreparedCoreNormalizer;
use reconf_core::resolve::resolved::{ResolvedImport, ResolvedModule, ResolvedModuleBody};
use reconf_core::typeck::{CoreElaborator, CoreModuleElaborator};

pub trait ImportLoader {
    fn load_import(&mut self, path: &Path) -> Result<ResolvedModule>;
}

pub struct ModuleEvaluator<'a, L> {
    imports: &'a mut L,
    base_dir: &'a Path,
    module: Module,
}

impl<'a, L: ImportLoader> ModuleEvaluator<'a, L> {
    pub fn new(imports: &'a mut L, base_dir: &'a Path, context: ModuleContext) -> Self {
        Self {
            imports,
            base_dir,
            module: context.into_module(),
        }
    }

    pub fn evaluate(mut self, body: ResolvedModuleBody) -> Result<Module> {
        let (imports, decls, output) = body.into_core_parts();
        for import in imports {
            self.import_names(import)?;
        }
        let body = ResolvedModuleBody::from_parts(Vec::new(), decls, output)
            .rebase_value_bindings_from(self.module.next_binding_id())
            .resolve_external_types(&self.module.value_types())
            .resolve_external_values(&self.module.value_types());
        let (_, decls, output) = body.into_core_parts();

        let module_values = self.module.value_types();
        let mut elaborator = CoreModuleElaborator::with_context(
            self.module.types(),
            &module_values,
            CoreElaborator::new(),
        );
        let elaborated = elaborator.elaborate_resolved_module(decls, output)?;

        self.eval_elaborated(elaborated)
    }

    fn eval_elaborated(mut self, elaborated: ElaboratedModule) -> Result<Module> {
        ModuleDeclEvaluator::new(&mut self.module).eval_module(elaborated)?;
        Ok(self.module)
    }

    fn import_names(&mut self, import: ResolvedImport) -> Result<()> {
        let imported = self
            .imports
            .load_import(&self.base_dir.join(import.path()))?;
        ModuleImportBinder::new(&mut self.module).bind(&imported, &import)
    }
}

struct ModuleDeclEvaluator<'a> {
    module: &'a mut Module,
}

impl<'a> ModuleDeclEvaluator<'a> {
    fn new(module: &'a mut Module) -> Self {
        Self { module }
    }

    fn eval_module(&mut self, elaborated: ElaboratedModule) -> Result<()> {
        for decl in elaborated.decls {
            self.eval_decl(decl)?;
        }

        if let Some(output) = elaborated.output {
            let output = self.evaluate_elaborated(output)?;
            self.module.define_output(output);
        }
        Ok(())
    }

    fn eval_decl(&mut self, decl: ElaboratedDecl) -> Result<()> {
        match decl {
            ElaboratedDecl::Native {
                export,
                name,
                binding,
                ty,
            } => {
                self.module.define_native(export, name, binding, ty);
            }
            ElaboratedDecl::Type {
                export,
                name,
                alias,
                ty,
            } => {
                self.module.define_type(export, name, alias, ty);
            }
            ElaboratedDecl::Let {
                export,
                name,
                binding,
                expr,
            } => {
                let ty = expr.ty().cloned();
                let value = self.evaluate_elaborated(expr)?;
                self.module.define_value(export, name, binding, value, ty);
            }
        }
        Ok(())
    }

    fn evaluate_elaborated(&self, expr: ElaboratedExpr) -> Result<Value> {
        match expr {
            ElaboratedExpr::Checked(expr) => self.evaluate_typed(expr),
        }
    }

    fn evaluate_typed(&self, expr: reconf_core::core::TypedCoreExpr) -> Result<Value> {
        PreparedCoreNormalizer::new(self.module.runtime_env(), self.module.types())
            .evaluate_typed(expr)
    }
}
