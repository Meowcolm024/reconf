use reconf_core::Result;
use reconf_core::core::CoreModule;
use reconf_core::diagnostic::DiagnosticSource;
use reconf_core::lower::SurfaceToCoreLowerer;
use reconf_core::source::LoadedSource;
use reconf_core::syntax::parser::parse;
use reconf_core::syntax::surface::FileAst;

pub(super) struct FrontendCompiler;

pub(super) struct FrontendOutput {
    pub(super) surface: FileAst,
    pub(super) core: CoreModule,
}

impl FrontendCompiler {
    pub(super) fn new() -> Self {
        Self
    }

    pub(super) fn compile_source(&self, input: &CompilerSource<'_>) -> Result<FrontendOutput> {
        let diagnostics = DiagnosticSource::new(input.name, input.text);
        let surface = parse(input.text).map_err(|error| diagnostics.attach(error))?;
        let core = SurfaceToCoreLowerer::new().lower_file(surface.clone());
        Ok(FrontendOutput { surface, core })
    }

    pub(super) fn compile_loaded(&self, source: &LoadedSource) -> Result<FrontendOutput> {
        let name = source.path.display().to_string();
        self.compile_source(&CompilerSource::new(&name, &source.text))
    }
}

pub(super) struct CompilerSource<'a> {
    name: &'a str,
    text: &'a str,
}

impl<'a> CompilerSource<'a> {
    pub(super) fn new(name: &'a str, text: &'a str) -> Self {
        Self { name, text }
    }
}
