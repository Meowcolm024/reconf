use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};

use crate::emit::json::emit_json;
use crate::eval::emit;
use crate::lower::lower_file;
use crate::repl::diagnostics::attach_best_effort_span;
use crate::resolve::modules::{Loader, eval_file};
use crate::syntax::parser::parse;
use crate::{Result, repl};

#[derive(Parser)]
#[command(
    name = "reconf",
    version,
    about = "Check and evaluate ReConf configuration files"
)]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Check {
        file: PathBuf,
    },
    Eval {
        file: PathBuf,
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
        #[arg(long)]
        pretty: bool,
    },
    Repl,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum OutputFormat {
    Json,
    Reconf,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Check { file }) => {
            let _ = eval_path(&file)?;
            Ok(())
        }
        Some(Command::Eval {
            file,
            format,
            pretty,
        }) => {
            let value = eval_path(&file)?;
            let output = match format {
                OutputFormat::Json => emit_json(&value, pretty)?,
                OutputFormat::Reconf => emit(&value)?,
            };
            println!("{output}");
            Ok(())
        }
        Some(Command::Repl) => repl::run(),
        None => {
            Cli::command().print_help().ok();
            println!();
            Ok(())
        }
    }
}

fn eval_path(path: &PathBuf) -> Result<crate::eval::Value> {
    let src = std::fs::read_to_string(path).map_err(|error| {
        crate::Error::new(format!("unknown import `{}`: {error}", path.display()))
    })?;
    let name = path.display().to_string();
    let ast = lower_file(parse(&src).map_err(|error| attach_best_effort_span(error, &name, &src))?);
    let mut loader = Loader::default();
    let base_dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let module = eval_file(&mut loader, base_dir, ast)
        .map_err(|error| attach_best_effort_span(error, &name, &src))?;
    Ok(module.values.get("$output").unwrap().clone())
}
