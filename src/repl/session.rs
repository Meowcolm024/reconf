use reedline::{
    ColumnarMenu, DefaultCompleter, Emacs, KeyCode, KeyModifiers, MenuBuilder, Reedline,
    ReedlineEvent, ReedlineMenu, Signal, default_emacs_keybindings,
};

use crate::repl::eval::ReplEvaluator;
use crate::repl::highlighter::ReconfHighlighter;
use crate::repl::prompt::ReconfPrompt;
use crate::repl::semantic::SemanticState;
use crate::repl::validator::ReconfValidator;
use crate::{Error, Result};

pub struct ReconfRepl {
    line_editor: Reedline,
    evaluator: ReplEvaluator,
    counter: usize,
}

impl Default for ReconfRepl {
    fn default() -> Self {
        Self::new()
    }
}

impl ReconfRepl {
    pub fn new() -> Self {
        let semantics = SemanticState::default();
        let completer = Box::new(DefaultCompleter::new_with_wordlen(keywords(), 2));
        let completion_menu = Box::new(ColumnarMenu::default().with_name("completion_menu"));

        let mut keybindings = default_emacs_keybindings();
        keybindings.add_binding(
            KeyModifiers::NONE,
            KeyCode::Tab,
            ReedlineEvent::UntilFound(vec![
                ReedlineEvent::Menu("completion_menu".to_string()),
                ReedlineEvent::MenuNext,
            ]),
        );

        let line_editor = Reedline::create()
            .with_highlighter(Box::new(ReconfHighlighter::new(semantics.clone())))
            .with_validator(Box::new(ReconfValidator))
            .with_completer(completer)
            .with_menu(ReedlineMenu::EngineCompleter(completion_menu))
            .with_edit_mode(Box::new(Emacs::new(keybindings)));

        Self {
            line_editor,
            evaluator: ReplEvaluator::new(semantics),
            counter: 0,
        }
    }

    pub fn run(mut self) -> Result<()> {
        loop {
            match self.line_editor.read_line(&ReconfPrompt::new(self.counter)) {
                Ok(Signal::Success(line)) => {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    if matches!(line, ":quit" | ":q" | "quit" | "exit") {
                        break;
                    }
                    self.counter += 1;
                    match self.evaluator.eval(line) {
                        Ok(output) => println!("{output}"),
                        Err(error) => eprintln!("{:?}", miette::Report::new(error)),
                    }
                }
                Ok(Signal::CtrlC) => continue,
                Ok(Signal::CtrlD) => break,
                Err(error) => return Err(Error::new(format!("repl error: {error}"))),
            }
        }

        Ok(())
    }
}

fn keywords() -> Vec<String> {
    [
        "import", "export", "native", "type", "let", "in", "if", "then", "else", "true", "false",
        "none", "some",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}
