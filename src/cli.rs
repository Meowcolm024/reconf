use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};

use crate::compiler::{CompileInput, Compiler};
use crate::emit::{EmitOptions, EmitterRegistry, OutputStyle};
use crate::error::ErrorCode;
use crate::{Result, repl};

#[derive(Parser)]
#[command(
    name = "reconf",
    version,
    about = "Check and evaluate ReConf configuration files"
)]
pub struct Cli {
    #[arg(long, value_name = "CODE")]
    explain: Option<String>,
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
        #[arg(long, conflicts_with = "pretty")]
        compact: bool,
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
    if let Some(code) = cli.explain {
        println!("{}", explain_code(&code));
        return Ok(());
    }

    match cli.command {
        Some(Command::Check { file }) => {
            let _ = Compiler::new().check(CompileInput::from(file))?;
            Ok(())
        }
        Some(Command::Eval {
            file,
            format,
            pretty,
            compact,
        }) => {
            let compiled = Compiler::new().eval(CompileInput::from(file))?;
            let options = EmitOptions {
                style: CliOutputStyle::from_flags(pretty, compact).into_output_style(),
            };
            let output =
                EmitterRegistry::new().emit(format.into(), compiled.data_output(), &options)?;
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

struct CliOutputStyle {
    pretty: bool,
    compact: bool,
}

impl CliOutputStyle {
    fn from_flags(pretty: bool, compact: bool) -> Self {
        Self { pretty, compact }
    }

    fn into_output_style(self) -> OutputStyle {
        match (self.pretty, self.compact) {
            (true, _) => OutputStyle::Pretty,
            (_, true) | (false, false) => OutputStyle::Compact,
        }
    }
}

impl From<OutputFormat> for crate::emit::OutputFormat {
    fn from(format: OutputFormat) -> Self {
        match format {
            OutputFormat::Json => Self::Json,
            OutputFormat::Reconf => Self::Reconf,
        }
    }
}

fn explain_code(code: &str) -> String {
    ErrorCode::from_code(code)
        .map(|code| code.info())
        .map(|info| format!("{}: {}", info.code, info.explanation))
        .unwrap_or_else(|| "unknown diagnostic code".to_string())
}
