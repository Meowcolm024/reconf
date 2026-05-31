fn main() {
    if let Err(error) = reconf::cli::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
