use std::path::{Path, PathBuf};

mod front;
pub mod loader;
pub mod module;
pub mod output;
mod pipeline;
pub mod prelude;
pub mod session;

use crate::Result;
use crate::compiler::loader::{ContextualModuleCompiler, ModuleLoader};
use crate::compiler::module::Module;
use crate::compiler::output::OutputValidator;
use crate::compiler::pipeline::CompilerPipeline;
use crate::core::CoreModule;
use crate::emit::DataValue;
use crate::eval::Value;
use crate::source::{FilesystemSourceProvider, SourceProvider};
use crate::syntax::surface::FileAst;

pub struct Compiler<S = FilesystemSourceProvider> {
    loader: ModuleLoader<S>,
}

#[derive(Clone, Debug, Default)]
pub struct CompilerOptions;

pub enum CompileInput {
    Path(PathBuf),
    Source(SourceInput),
}

pub struct SourceInput {
    name: String,
    base_dir: PathBuf,
    text: String,
}

pub struct CheckOutput {
    surface: FileAst,
    core: CoreModule,
    module: Module,
}

pub struct EvalOutput {
    checked: CheckOutput,
    output: Value,
    data_output: DataValue,
}

impl Compiler<FilesystemSourceProvider> {
    pub fn new() -> Self {
        Self::with_sources(FilesystemSourceProvider, CompilerOptions)
    }
}

impl Default for Compiler<FilesystemSourceProvider> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: SourceProvider> Compiler<S> {
    pub fn with_sources(sources: S, _options: CompilerOptions) -> Self {
        Self {
            loader: ModuleLoader::with_sources(sources, ContextualModuleCompiler::with_prelude()),
        }
    }

    pub fn check(&mut self, input: CompileInput) -> Result<CheckOutput> {
        let input = self.prepare_input(input)?;
        CompilerPipeline::new(&mut self.loader).check_source(&input)
    }

    pub fn eval(&mut self, input: CompileInput) -> Result<EvalOutput> {
        let checked = self.check(input)?;
        let output = checked.output()?.clone();
        let data_output = OutputValidator::new().validate(&output)?;
        Ok(EvalOutput::from_parts(checked, output, data_output))
    }

    fn prepare_input(&mut self, input: CompileInput) -> Result<SourceInput> {
        match input {
            CompileInput::Path(path) => {
                let source = self.loader.load_input_source(&path)?;
                Ok(SourceInput::from_path(&source.path, source.text))
            }
            CompileInput::Source(input) => Ok(input),
        }
    }
}

impl From<PathBuf> for CompileInput {
    fn from(path: PathBuf) -> Self {
        Self::Path(path)
    }
}

impl From<&Path> for CompileInput {
    fn from(path: &Path) -> Self {
        Self::Path(path.to_path_buf())
    }
}

impl From<SourceInput> for CompileInput {
    fn from(input: SourceInput) -> Self {
        Self::Source(input)
    }
}

impl SourceInput {
    pub fn new(
        name: impl Into<String>,
        base_dir: impl Into<PathBuf>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            base_dir: base_dir.into(),
            text: text.into(),
        }
    }

    pub fn from_path(path: &Path, text: String) -> Self {
        let base_dir = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        Self::new(path.display().to_string(), base_dir, text)
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }
}

impl CheckOutput {
    pub(crate) fn new(surface: FileAst, core: CoreModule, module: Module) -> Self {
        Self {
            surface,
            core,
            module,
        }
    }

    pub fn surface(&self) -> &FileAst {
        &self.surface
    }

    pub fn core(&self) -> &CoreModule {
        &self.core
    }

    pub fn module(&self) -> &Module {
        &self.module
    }

    pub fn output(&self) -> Result<&Value> {
        self.module.output()
    }

    pub fn data_output(&self) -> Result<DataValue> {
        OutputValidator::new().validate(self.output()?)
    }

    pub fn into_output(self) -> Result<Value> {
        self.module.into_output()
    }

    pub fn into_data_output(self) -> Result<DataValue> {
        OutputValidator::new().validate(&self.into_output()?)
    }
}

impl EvalOutput {
    pub(crate) fn from_parts(checked: CheckOutput, output: Value, data_output: DataValue) -> Self {
        Self {
            checked,
            output,
            data_output,
        }
    }

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

    pub fn into_checked(self) -> CheckOutput {
        self.checked
    }

    pub fn into_output(self) -> Value {
        self.output
    }

    pub fn into_data_output(self) -> DataValue {
        self.data_output
    }
}
