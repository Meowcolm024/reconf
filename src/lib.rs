mod cli;
mod compiler;
mod diagnostic;
mod emit;
mod eval;
mod syntax;
mod typeck;

pub use cli::cli_main;
pub use compiler::Compiler;
pub use diagnostic::{Diagnostic, SourceMap, Span};
pub use emit::emit_json;
pub use eval::Value;
