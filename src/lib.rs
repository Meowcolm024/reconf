pub mod cli;
pub mod core;
pub mod diagnostic;
pub mod emit;
pub mod error;
pub mod eval;
pub mod lower;
pub mod refine;
pub mod repl;
pub mod resolve;
pub mod source;
pub mod syntax;
pub mod typeck;

pub use emit::json::emit_json;
pub use error::{Error, Result};
pub use resolve::modules::run;
