use std::io::{self, BufRead, IsTerminal};
use std::path::Path;

use reedline::{DefaultPrompt, DefaultPromptSegment, Reedline, Signal};

use crate::eval::{Loader, emit, eval_file};
use crate::parser::parse;
use crate::{Error, Result};

pub fn run() -> Result<()> {
    if !io::stdin().is_terminal() {
        return run_piped();
    }

    let mut line_editor = Reedline::create();
    let prompt = DefaultPrompt::new(
        DefaultPromptSegment::Basic("reconf".to_string()),
        DefaultPromptSegment::Empty,
    );

    loop {
        match line_editor.read_line(&prompt) {
            Ok(Signal::Success(line)) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if matches!(line, ":quit" | ":q" | "quit" | "exit") {
                    break;
                }
                match eval_snippet(line) {
                    Ok(output) => println!("{output}"),
                    Err(error) => eprintln!("{error}"),
                }
            }
            Ok(Signal::CtrlC) => continue,
            Ok(Signal::CtrlD) => break,
            Err(error) => return Err(Error::new(format!("repl error: {error}"))),
        }
    }

    Ok(())
}

fn eval_snippet(src: &str) -> Result<String> {
    let source = if src.ends_with(';') {
        format!("{src}\n0")
    } else {
        src.to_string()
    };
    let ast = parse(&source)?;
    let mut loader = Loader::default();
    let module = eval_file(&mut loader, Path::new("."), ast)?;
    emit(module.values.get("$output").unwrap())
}

fn run_piped() -> Result<()> {
    for line in io::stdin().lock().lines() {
        let line = line.map_err(|error| Error::new(format!("repl error: {error}")))?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if matches!(line, ":quit" | ":q" | "quit" | "exit") {
            break;
        }
        match eval_snippet(line) {
            Ok(output) => println!("{output}"),
            Err(error) => eprintln!("{error}"),
        }
    }
    Ok(())
}
