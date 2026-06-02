use crate::Result;
use crate::compiler::front::{CompilerSource, FrontendCompiler};
use crate::compiler::loader::{ModuleCompiler, ModuleLoader};
use crate::compiler::{CheckOutput, SourceInput};
use crate::core::CoreModule;
use crate::diagnostic::DiagnosticSource;
use crate::resolve::resolved::ResolvedModuleBody;
use crate::source::SourceProvider;
use crate::syntax::surface::FileAst;

pub(super) struct CompilerPipeline<'a, S, C> {
    loader: &'a mut ModuleLoader<S, C>,
}

impl<'a, S: SourceProvider, C: ModuleCompiler> CompilerPipeline<'a, S, C> {
    pub(super) fn new(loader: &'a mut ModuleLoader<S, C>) -> Self {
        Self { loader }
    }

    pub(super) fn check_source(&mut self, input: &SourceInput) -> Result<CheckOutput> {
        let output = FrontendCompiler::new()
            .compile_source(&CompilerSource::new(input.name(), input.text()))?;
        self.check_module(input, output.surface, output.core)
    }

    pub(super) fn check_module(
        &mut self,
        input: &SourceInput,
        surface: FileAst,
        core: CoreModule,
    ) -> Result<CheckOutput> {
        let diagnostics = DiagnosticSource::new(input.name(), input.text());
        let module = self
            .loader
            .compile_entry(
                input.base_dir(),
                ResolvedModuleBody::from_core(core.clone()),
            )
            .map_err(|error| diagnostics.attach(error))?;
        Ok(CheckOutput::new(surface, core, module))
    }
}
