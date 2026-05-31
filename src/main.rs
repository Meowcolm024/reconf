use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "reconf",
    version,
    about = "Evaluate ReConf configuration files"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Run { file: PathBuf },
    Repl,
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Some(Command::Run { file }) => reconf::run(&file).map(|output| println!("{output}")),
        Some(Command::Repl) => reconf::repl::run(),
        None => {
            Cli::command().print_help().ok();
            println!();
            Ok(())
        }
    };

    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
