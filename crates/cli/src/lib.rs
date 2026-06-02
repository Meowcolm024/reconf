mod cli;
pub mod repl;

pub use reconf_compiler::{Error, Result};

pub fn run_cli() -> Result<()> {
    repl::reporter::init_reporter();
    cli::run()
}
