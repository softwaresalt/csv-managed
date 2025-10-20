fn main() {
    if csv_managed::run().is_err() {
        std::process::exit(1);
    }
}
