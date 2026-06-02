use std::path::PathBuf;

use crate::Result;
use crate::compiler::front::{CompilerSource, FrontendCompiler, FrontendOutput};
use crate::compiler::module::Module;
use crate::compiler::output::OutputValidator;
use crate::compiler::pipeline::CompilerPipeline;
use crate::compiler::{CheckOutput, EvalOutput, SourceInput};
use crate::core::{CoreDecl, CoreImport, CoreModule};
use crate::emit::DataValue;
use crate::eval::Value;
use crate::source::{FilesystemSourceProvider, SourceProvider};
use crate::syntax::surface::{Decl, FileAst};

use super::loader::{ContextualModuleCompiler, ModuleLoader};

pub struct CompilerSession<S = FilesystemSourceProvider> {
    name: String,
    base_dir: PathBuf,
    artifacts: SessionArtifacts,
    loader: ModuleLoader<S>,
}

#[derive(Default)]
struct SessionArtifacts {
    surface_decls: Vec<Decl>,
    core_imports: Vec<CoreImport>,
    core_decls: Vec<CoreDecl>,
}

struct SessionInputArtifacts {
    surface: FileAst,
    core: CoreModule,
}

enum SessionInputKind {
    Declarations,
    Expression,
}

pub struct SessionCheckOutput {
    checked: CheckOutput,
}

pub struct SessionEvalOutput {
    checked: CheckOutput,
    output: Value,
    data_output: DataValue,
}

impl CompilerSession<FilesystemSourceProvider> {
    pub fn new(name: impl Into<String>, base_dir: impl Into<PathBuf>) -> Self {
        Self::with_sources(name, base_dir, FilesystemSourceProvider)
    }
}

impl<S: SourceProvider> CompilerSession<S> {
    pub fn with_sources(name: impl Into<String>, base_dir: impl Into<PathBuf>, sources: S) -> Self {
        Self {
            name: name.into(),
            base_dir: base_dir.into(),
            artifacts: SessionArtifacts::default(),
            loader: ModuleLoader::with_sources(sources, ContextualModuleCompiler::with_prelude()),
        }
    }

    pub fn check_declarations(&mut self, source: &str) -> Result<SessionCheckOutput> {
        let input = self.input(source);
        let frontend = self.compile_frontend(&input)?;
        let checked_artifacts = self
            .artifacts
            .with_frontend(&frontend, SessionInputKind::Declarations);
        let mut pipeline = CompilerPipeline::new(&mut self.loader);
        let checked =
            pipeline.check_module(&input, checked_artifacts.surface, checked_artifacts.core)?;
        self.artifacts.commit(frontend);
        Ok(SessionCheckOutput { checked })
    }

    pub fn eval_expression(&mut self, source: &str) -> Result<SessionEvalOutput> {
        let input = self.input(source);
        let frontend = self.compile_frontend(&input)?;
        let checked_artifacts = self
            .artifacts
            .with_frontend(&frontend, SessionInputKind::Expression);
        let mut pipeline = CompilerPipeline::new(&mut self.loader);
        let checked =
            pipeline.check_module(&input, checked_artifacts.surface, checked_artifacts.core)?;
        let output = checked.output()?.clone();
        let data_output = OutputValidator::new().validate(&output)?;
        Ok(SessionEvalOutput {
            checked,
            output,
            data_output,
        })
    }

    fn input(&self, source: &str) -> SourceInput {
        SourceInput::new(self.name.clone(), self.base_dir.clone(), source.to_string())
    }

    fn compile_frontend(&self, input: &SourceInput) -> Result<FrontendOutput> {
        FrontendCompiler::new().compile_source(&CompilerSource::new(input.name(), input.text()))
    }
}

impl SessionArtifacts {
    fn commit(&mut self, frontend: FrontendOutput) {
        self.surface_decls.extend(frontend.surface.decls);
        self.core_imports.extend(frontend.core.imports);
        self.core_decls.extend(frontend.core.decls);
    }

    fn with_frontend(
        &self,
        frontend: &FrontendOutput,
        kind: SessionInputKind,
    ) -> SessionInputArtifacts {
        let (surface_output, core_output) = match kind {
            SessionInputKind::Declarations => (None, None),
            SessionInputKind::Expression => (
                frontend.surface.output.clone(),
                frontend.core.output.clone(),
            ),
        };
        SessionInputArtifacts {
            surface: self.surface_with(&frontend.surface, surface_output),
            core: self.core_with(&frontend.core, core_output),
        }
    }

    fn surface_with(
        &self,
        next: &FileAst,
        output: Option<crate::syntax::surface::Expr>,
    ) -> FileAst {
        let mut decls = self.surface_decls.clone();
        decls.extend(next.decls.clone());
        FileAst { decls, output }
    }

    fn core_with(&self, next: &CoreModule, output: Option<crate::core::CoreExpr>) -> CoreModule {
        let mut imports = self.core_imports.clone();
        imports.extend(next.imports.clone());
        let mut decls = self.core_decls.clone();
        decls.extend(next.decls.clone());
        CoreModule {
            imports,
            decls,
            output,
        }
    }
}

impl SessionCheckOutput {
    pub fn checked(&self) -> &CheckOutput {
        &self.checked
    }

    pub fn into_checked(self) -> CheckOutput {
        self.checked
    }
}

impl SessionEvalOutput {
    pub fn checked(&self) -> &CheckOutput {
        &self.checked
    }

    pub fn surface(&self) -> &FileAst {
        self.checked.surface()
    }

    pub fn core(&self) -> &CoreModule {
        self.checked.core()
    }

    pub fn module(&self) -> &Module {
        self.checked.module()
    }

    pub fn output(&self) -> &Value {
        &self.output
    }

    pub fn data_output(&self) -> &DataValue {
        &self.data_output
    }
}

impl From<SessionEvalOutput> for EvalOutput {
    fn from(output: SessionEvalOutput) -> Self {
        EvalOutput::from_parts(output.checked, output.output, output.data_output)
    }
}
