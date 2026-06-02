fn main() {
    if let Err(error) = reconf::run_cli() {
        eprintln!("{:?}", miette::Report::new(error));
        std::process::exit(1);
    }
}
