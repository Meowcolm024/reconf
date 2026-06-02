use std::path::Path;

use crate::repl::semantic::SemanticState;
use reconf_compiler::Result;
use reconf_compiler::compiler::session::CompilerSession;
use reconf_compiler::emit::{EmitOptions, EmitterRegistry, OutputFormat};

pub struct ReplEvaluator {
    session: CompilerSession,
    semantics: SemanticState,
}

impl ReplEvaluator {
    pub fn new(semantics: SemanticState) -> Self {
        Self {
            session: CompilerSession::new("repl", Path::new(".")),
            semantics,
        }
    }

    pub fn eval(&mut self, src: &str) -> Result<String> {
        if src.ends_with(';') {
            let checked = self.session.check_declarations(src)?;
            self.semantics.learn_file(checked.checked().surface());
            Ok("0".to_string())
        } else {
            let compiled = self.session.eval_expression(src)?;
            self.semantics.learn_file(compiled.surface());
            EmitterRegistry::new().emit(
                OutputFormat::Reconf,
                compiled.data_output(),
                &EmitOptions::default(),
            )
        }
    }
}
