mod graph;

use std::path::Path;

use crate::compiler::front::FrontendCompiler;
use crate::compiler::loader::graph::ModuleGraph;
use crate::compiler::module::{ImportLoader, ModuleContext, ModuleEvaluator};
use reconf_core::diagnostic::DiagnosticSource;
use reconf_core::error::{Error, ErrorCode, Result};
use reconf_core::resolve::resolved::{
    ResolvedModule, ResolvedModuleBody, ResolvedModuleBuilder, ResolvedProgram,
};
use reconf_core::source::{FilesystemSourceProvider, LoadedSource, SourceProvider};

pub use crate::compiler::module::Module;

pub trait ModuleCompiler: Clone {
    fn compile_module<L: ImportLoader>(
        &self,
        imports: &mut L,
        base_dir: &Path,
        body: ResolvedModuleBody,
    ) -> Result<Module>;
}

#[derive(Clone, Debug, Default)]
pub struct EvaluatingModuleCompiler;

#[derive(Clone)]
pub struct ContextualModuleCompiler {
    context: ModuleContext,
}

pub type DefaultModuleCompiler = ContextualModuleCompiler;

impl ModuleCompiler for EvaluatingModuleCompiler {
    fn compile_module<L: ImportLoader>(
        &self,
        imports: &mut L,
        base_dir: &Path,
        body: ResolvedModuleBody,
    ) -> Result<Module> {
        ModuleEvaluator::new(imports, base_dir, ModuleContext::empty()).evaluate(body)
    }
}

impl ContextualModuleCompiler {
    pub fn new(context: ModuleContext) -> Self {
        Self { context }
    }

    pub fn with_prelude() -> Self {
        Self::new(
            crate::compiler::prelude::PreludeCompiler::new()
                .compile_context()
                .expect("bundled prelude must compile"),
        )
    }
}

impl Default for ContextualModuleCompiler {
    fn default() -> Self {
        Self::with_prelude()
    }
}

impl ModuleCompiler for ContextualModuleCompiler {
    fn compile_module<L: ImportLoader>(
        &self,
        imports: &mut L,
        base_dir: &Path,
        body: ResolvedModuleBody,
    ) -> Result<Module> {
        ModuleEvaluator::new(imports, base_dir, self.context.clone()).evaluate(body)
    }
}

pub struct ModuleLoader<S = FilesystemSourceProvider, C = DefaultModuleCompiler> {
    graph: ModuleGraph,
    sources: S,
    compiler: C,
}

impl ModuleLoader<FilesystemSourceProvider> {
    pub fn new() -> Self {
        Self::with_sources(
            FilesystemSourceProvider,
            ContextualModuleCompiler::with_prelude(),
        )
    }
}

impl Default for ModuleLoader<FilesystemSourceProvider> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S, C> ModuleLoader<S, C> {
    pub fn with_sources(sources: S, compiler: C) -> Self {
        Self {
            graph: ModuleGraph::new(),
            sources,
            compiler,
        }
    }

    pub fn resolved_program(&self) -> &ResolvedProgram {
        self.graph.resolved_program()
    }
}

impl<S: SourceProvider, C: ModuleCompiler> ModuleLoader<S, C> {
    pub fn load_input_source(&mut self, path: &Path) -> Result<LoadedSource> {
        self.sources.load_source(path)
    }

    pub fn compile_entry(&mut self, base_dir: &Path, body: ResolvedModuleBody) -> Result<Module> {
        let compiler = self.compiler.clone();
        compiler.compile_module(self, base_dir, body)
    }

    pub fn load_resolved(&mut self, path: &Path) -> Result<ResolvedModule> {
        if let Some(module) = self.graph.cached_resolved(path) {
            return Ok(module);
        }
        let source = self
            .sources
            .load_source(path)
            .map_err(|error| Error::with_code(ErrorCode::ModuleMissingImport, error.to_string()))?;
        let path = source.path.clone();
        if let Some(module) = self.graph.cached_resolved(&path) {
            return Ok(module.clone());
        }
        self.load_source(source)
    }

    fn load_source(&mut self, source: LoadedSource) -> Result<ResolvedModule> {
        if let Some(module) = self.graph.cached_resolved(&source.path) {
            return Ok(module);
        }
        let diagnostics_name = source.path.display().to_string();
        let diagnostics = DiagnosticSource::new(&diagnostics_name, &source.text);
        let frontend = FrontendCompiler::new().compile_loaded(&source)?;
        let core = frontend.core;
        let body = ResolvedModuleBody::from_core(core);
        let loading = self.graph.begin_loading(ResolvedModuleBuilder::new(
            source.path.clone(),
            body.clone(),
        ))?;
        let parent = source
            .path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let compiler = self.compiler.clone();
        let module = compiler
            .compile_module(self, &parent, body)
            .map_err(|error| diagnostics.attach(error));
        match module {
            Ok(module) => Ok(self.graph.finish_loading(loading, module)),
            Err(error) => {
                self.graph.abort_loading(loading);
                Err(error)
            }
        }
    }
}

impl<S: SourceProvider, C: ModuleCompiler> ImportLoader for ModuleLoader<S, C> {
    fn load_import(&mut self, path: &Path) -> Result<ResolvedModule> {
        self.load_resolved(path)
    }
}
