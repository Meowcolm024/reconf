use std::path::Path;

use crate::compiler::front::{CompilerSource, FrontendCompiler};
use crate::compiler::loader::{EvaluatingModuleCompiler, ModuleLoader};
use crate::compiler::module::{Module, ModuleContext, ModuleEvaluator};
use reconf_core::error::Result;
use reconf_core::eval::PRELUDE_SOURCE;
use reconf_core::resolve::resolved::ResolvedModuleBody;
use reconf_core::source::FilesystemSourceProvider;

pub struct PreludeCompiler;

impl PreludeCompiler {
    pub fn new() -> Self {
        Self
    }

    pub fn compile_module(&self) -> Result<Module> {
        let core = FrontendCompiler::new()
            .compile_source(&CompilerSource::new("prelude.reconf", source()))?
            .core;
        let mut loader =
            ModuleLoader::with_sources(FilesystemSourceProvider, EvaluatingModuleCompiler);
        ModuleEvaluator::new(&mut loader, Path::new("."), ModuleContext::empty())
            .evaluate(ResolvedModuleBody::from_core(core))
    }

    pub fn compile_context(&self) -> Result<ModuleContext> {
        self.compile_module().map(ModuleContext::from_module)
    }
}

impl Default for PreludeCompiler {
    fn default() -> Self {
        Self::new()
    }
}

pub fn source() -> &'static str {
    PRELUDE_SOURCE
}
