# TODO

- [x] Review current index storage format and limitations for multi-column/direction support.
- [x] Design structure for storing multiple index variants keyed by column names and sort directions.
- [x] Update index build logic to optionally create several variants in one file or multiple files.
- [x] Extend index serialization format to capture direction metadata per column.
- [x] Update CLI to allow naming/selecting specific index variants with direction info.
- [x] Teach process command to pick best-matching index variant based on requested sorts, including mixed directions.
- [x] Implement install command that shells out to `cargo install csv-managed` with options for version/force/local path.
- [x] Document install command usage and multi-index support in README.
- [x] Add tests covering install command argument plumbing and multi-index selection logic.
