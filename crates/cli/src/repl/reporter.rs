use miette::highlighters::SyntectHighlighter;

use crate::repl::highlighter::ReconfHighlighter;
use crate::repl::semantic::SemanticState;

pub fn init_reporter() {
    let _ = miette::set_hook(Box::new(|_| {
        let highlighter: SyntectHighlighter =
            ReconfHighlighter::new(SemanticState::default()).into();
        let handler = miette::MietteHandlerOpts::new()
            .terminal_links(true)
            .context_lines(5)
            .with_syntax_highlighting(highlighter)
            .build();
        Box::new(handler)
    }));
}
