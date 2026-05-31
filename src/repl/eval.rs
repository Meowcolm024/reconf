use std::path::Path;

use crate::Result;
use crate::diagnostic::attach_best_effort_span;
use crate::eval::emit;
use crate::lower::lower_file;
use crate::repl::semantic::SemanticState;
use crate::resolve::modules::{Loader, eval_file};
use crate::syntax::parser::parse;

pub struct ReplEvaluator {
    source: String,
    semantics: SemanticState,
}

impl ReplEvaluator {
    pub fn new(semantics: SemanticState) -> Self {
        Self {
            source: String::new(),
            semantics,
        }
    }

    pub fn eval(&mut self, src: &str) -> Result<String> {
        let source = self.source_with(src);
        let parsed =
            parse(&source).map_err(|error| attach_best_effort_span(error, "repl", &source))?;
        let ast = lower_file(parsed.clone());
        let mut loader = Loader::default();
        let module = eval_file(&mut loader, Path::new("."), ast)
            .map_err(|error| attach_best_effort_span(error, "repl", &source))?;
        self.semantics.learn_file(&parsed);
        if src.ends_with(';') {
            self.source.push_str(src);
            self.source.push('\n');
        }
        emit(module.values.get("$output").unwrap())
    }

    fn source_with(&self, src: &str) -> String {
        if src.ends_with(';') {
            format!("{}{src}\n0", self.source)
        } else {
            format!("{}{src}", self.source)
        }
    }
}
