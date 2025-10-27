# Current Task

## Implement schema infer preview output

- [x] Add a `--preview` flag to `schema infer` in `src/cli.rs` and wire it into `SchemaInferArgs`.
- [x] Update `execute_infer` in `src/schema_cmd.rs` to honor preview mode, short-circuit file writes, and render inferred schema plus datatype notes to stdout.
- [x] Ensure placeholder replacement, NA handling, and template substitution logic still apply when previewing.
- [x] Add coverage (likely in `tests/schema.rs` or a new integration test) exercising preview mode with and without placeholder options.
- [x] Document the new flag in `docs/cli-help.md`, `docs/schema-examples.md`, and `README.md` with an example invocation.
- [x] Run `cargo test` (and relevant targeted tests if needed) to confirm everything passes after the changes.

