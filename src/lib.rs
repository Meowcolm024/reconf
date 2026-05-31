mod cli;
mod compiler;
mod diagnostic;
mod emit;
mod eval;
mod syntax;

pub use cli::cli_main;
pub use compiler::Compiler;
pub use diagnostic::{Diagnostic, SourceMap, Span};
pub use emit::emit_json;
pub use eval::Value;
