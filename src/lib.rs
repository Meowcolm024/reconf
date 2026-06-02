mod cli;
pub mod compiler;
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

pub use emit::{DataValue, EmitOptions, Emitter, EmitterRegistry, OutputFormat, OutputStyle};
pub use error::{Error, Result};

pub fn run_cli() -> Result<()> {
    repl::reporter::init_reporter();
    cli::run()
}
