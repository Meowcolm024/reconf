fn main() {
    if let Err(error) = reconf_cli::run_cli() {
        eprintln!("{:?}", miette::Report::new(error));
        std::process::exit(1);
    }
}
