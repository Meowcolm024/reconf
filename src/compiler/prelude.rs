use std::path::Path;

use crate::compiler::front::{CompilerSource, FrontendCompiler};
use crate::compiler::loader::{EvaluatingModuleCompiler, ModuleLoader};
use crate::compiler::module::{Module, ModuleContext, ModuleEvaluator};
use crate::error::Result;
use crate::resolve::resolved::ResolvedModuleBody;
use crate::source::FilesystemSourceProvider;

const SOURCE: &str = include_str!("../eval/prelude.reconf");

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
    SOURCE
}
