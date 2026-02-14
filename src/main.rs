//! Entry point for the csv-managed binary.
//!
//! Delegates to [`csv_managed::run()`] and translates its `Result` into
//! process exit codes: `0` on success, `1` on any error (FR-059).

fn main() {
    if csv_managed::run().is_err() {
        std::process::exit(1);
    }
}
