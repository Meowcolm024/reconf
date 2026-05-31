fn main() {
    reconf::repl::reporter::init_reporter();
    if let Err(error) = reconf::cli::run() {
        eprintln!("{:?}", miette::Report::new(error));
        std::process::exit(1);
    }
}
