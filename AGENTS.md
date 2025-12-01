# Drum Synth Project Guidelines

## Build/Lint/Test Commands
- Build and bundle plugin: `cargo xtask bundle drum_synth --release`
- Build without bundling: `cargo build --release`
- Run clippy lints: `cargo clippy -- -D warnings`
- Run tests: `cargo test`
- Run single test: `cargo test test_name`
- Run pluginval tests: `make test` (standard validation)
- Run quick pluginval tests: `make test-quick` (level 3, faster)
- Run comprehensive tests: `make test-comprehensive` (level 10, thorough)
- Run CI tests: `make test-ci` (multi-level validation)
- Setup pluginval tool: `make setup-pluginval`

## Architecture & Structure
- **Main plugin**: `src/lib.rs` - NIH-Plug audio plugin implementing synthetic drum sounds
- **UI**: `src/ui.rs` - Vizia-based GUI with custom knobs and styling
- **Build system**: `xtask/` - Custom build tasks for plugin bundling using nih_plug_xtask
- **Output**: Compiled as cdylib for VST3/standalone plugin formats
- **Dependencies**: nih_plug (audio framework), vizia (GUI), rand (synthesis randomization)

## Code Style Guidelines
- **Formatting**: Use `rustfmt` for consistent formatting
- **Imports**: Group std imports first, then external crates, then local modules
- **Types**: Use Rust's strong type system; avoid `as` casts when possible
- **Naming**: Use snake_case for variables/functions, CamelCase for types/traits
- **Error Handling**: Use Result<T,E> for functions that can fail; avoid unwrap() in production code
- **Constants**: Use SCREAMING_SNAKE_CASE for constants, prefer const over static
- **Comments**: Document public API with /// comments, explain "why" not "what"
- **Audio Processing**: Avoid allocations in audio processing code (note nih_plug's assert_process_allocs feature)
