use std::path::Path;

use crate::{Compiler, emit_json};

pub fn cli_main<I>(args: I) -> i32
where
    I: IntoIterator<Item = String>,
{
    match run_cli(args) {
        Ok(output) => {
            if let Some(output) = output {
                println!("{output}");
            }
            0
        }
        Err(report) => {
            eprintln!("{report}");
            1
        }
    }
}

fn run_cli<I>(args: I) -> Result<Option<String>, String>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let program = args.next().unwrap_or_else(|| "reconf".to_string());
    let Some(command) = args.next() else {
        return Err(format!(
            "usage: {program} <check|eval> <file> [--format json] [--pretty|--compact]"
        ));
    };

    match command.as_str() {
        "check" => {
            let Some(file) = args.next() else {
                return Err(format!("usage: {program} check <file>"));
            };
            if let Some(extra) = args.next() {
                return Err(format!("unexpected argument for check: {extra}"));
            }
            let mut compiler = Compiler::new();
            compiler
                .check_file(Path::new(&file))
                .map_err(|err| compiler.render(err))?;
            Ok(None)
        }
        "eval" => {
            let Some(file) = args.next() else {
                return Err(format!(
                    "usage: {program} eval <file> [--format json] [--pretty|--compact]"
                ));
            };
            let mut format = "json".to_string();
            let mut pretty = true;
            let rest: Vec<String> = args.collect();
            let mut idx = 0;
            while idx < rest.len() {
                match rest[idx].as_str() {
                    "--format" => {
                        idx += 1;
                        let Some(value) = rest.get(idx) else {
                            return Err("--format requires a value".to_string());
                        };
                        format = value.clone();
                    }
                    "--pretty" => pretty = true,
                    "--compact" => pretty = false,
                    other => return Err(format!("unexpected argument for eval: {other}")),
                }
                idx += 1;
            }
            if format != "json" {
                return Err(format!(
                    "unsupported output format `{format}`; only `json` is available"
                ));
            }
            let mut compiler = Compiler::new();
            let value = compiler
                .eval_file(Path::new(&file))
                .map_err(|err| compiler.render(err))?;
            Ok(Some(
                emit_json(&value, pretty).map_err(|err| compiler.render(err))?,
            ))
        }
        "--help" | "-h" | "help" => Ok(Some(format!(
            "usage: {program} <check|eval> <file> [--format json] [--pretty|--compact]"
        ))),
        other => Err(format!(
            "unknown command `{other}`\nusage: {program} <check|eval> <file>"
        )),
    }
}
