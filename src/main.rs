fn main() {
    if let Err(err) = csv_managed::run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
