use reconf_compiler::compiler::{CompileInput, Compiler, SourceInput};
use reconf_compiler::emit::{EmitOptions, EmitterRegistry, OutputFormat};

pub fn eval_src(src: &str) -> reconf_compiler::Result<String> {
    let compiled = Compiler::new().eval(CompileInput::from(SourceInput::new("test", ".", src)))?;
    EmitterRegistry::new().emit(
        OutputFormat::Reconf,
        compiled.data_output(),
        &EmitOptions::default(),
    )
}

pub fn expect_refinement_failure(src: &str) {
    let error = eval_src(src).unwrap_err();
    assert_eq!(error.code().as_str(), "E_REFINE_004");
}
