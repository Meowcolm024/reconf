use std::io::{self, BufRead, IsTerminal};

use reedline::{ValidationResult, Validator};

pub mod diagnostics;
mod eval;
pub mod highlighter;
mod prompt;
pub mod reporter;
pub mod semantic;
mod session;
mod theme;
mod validator;

pub use validator::is_complete_reconf_input;

use crate::{Error, Result};

#[doc(hidden)]
pub mod eval_for_test {
    pub use super::eval::ReplEvaluator;
}

pub fn run() -> Result<()> {
    if !io::stdin().is_terminal() {
        return run_piped();
    }

    session::ReconfRepl::new().run()
}

fn run_piped() -> Result<()> {
    let mut evaluator = eval::ReplEvaluator::new(semantic::SemanticState::default());
    let validator = validator::ReconfValidator;
    let mut buffer = String::new();
    for line in io::stdin().lock().lines() {
        let line = line.map_err(|error| Error::new(format!("repl error: {error}")))?;
        let line = line.trim();
        if buffer.is_empty() && line.is_empty() {
            continue;
        }
        if buffer.is_empty() && matches!(line, ":quit" | ":q" | "quit" | "exit") {
            break;
        }

        if !buffer.is_empty() {
            buffer.push('\n');
        }
        buffer.push_str(line);

        if !matches!(validator.validate(&buffer), ValidationResult::Complete) {
            continue;
        }

        let input = std::mem::take(&mut buffer);
        let trimmed = input.trim();
        match evaluator.eval(trimmed) {
            Ok(output) if trimmed.ends_with(';') && output == "0" => println!(),
            Ok(output) => println!("{output}"),
            Err(error) => eprintln!("{:?}", miette::Report::new(error)),
        }
    }
    Ok(())
}
