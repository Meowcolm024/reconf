use std::io::{self, BufRead, IsTerminal};

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
    for line in io::stdin().lock().lines() {
        let line = line.map_err(|error| Error::new(format!("repl error: {error}")))?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if matches!(line, ":quit" | ":q" | "quit" | "exit") {
            break;
        }
        match evaluator.eval(line) {
            Ok(output) => println!("{output}"),
            Err(error) => eprintln!("{:?}", miette::Report::new(error)),
        }
    }
    Ok(())
}
