# Development Workflow

- Format Rust code with `cargo fmt --all` before committing.
- Run `cargo clippy --workspace --all-targets -- -D warnings` to ensure the code builds without warnings.
- Execute `cargo test` and fix any failing tests prior to submission.
- Treat any new compiler or linter warnings as errors and resolve them in the same change.
