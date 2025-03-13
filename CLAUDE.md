# CLAUDE.md - Drum Synth Project Guidelines

## Build Commands
- Build and bundle plugin: `cargo xtask bundle drum_synth --release`
- Build without bundling: `cargo build --release`
- Run clippy lints: `cargo clippy -- -D warnings`
- Run tests: `cargo test`
- Run single test: `cargo test test_name`

## Code Style Guidelines
- **Formatting**: Use `rustfmt` for consistent formatting
- **Imports**: Group std imports first, then external crates, then local modules
- **Types**: Use Rust's strong type system; avoid `as` casts when possible
- **Naming**: Use snake_case for variables/functions, CamelCase for types/traits
- **Error Handling**: Use Result<T,E> for functions that can fail; avoid unwrap() in production code
- **Constants**: Use SCREAMING_SNAKE_CASE for constants, prefer const over static
- **Comments**: Document public API with /// comments, explain "why" not "what"
- **Audio Processing**: Avoid allocations in audio processing code (note nih_plug's assert_process_allocs feature)