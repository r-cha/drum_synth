# Makefile for drum_synth plugin development and testing

.PHONY: build build-release test test-ci test-specific lint format clean setup-pluginval help

# Default target
all: build test

# Build targets
build:
	cargo build

build-release:
	cargo xtask bundle drum_synth --release

# Test targets
test: build-release
	@echo "Running pluginval tests..."
	@bash scripts/test_plugin.sh --strictness-level 8

test-ci:
	@echo "Running CI tests..."
	@bash scripts/ci_test.sh

test-specific: build-release
	@echo "Running specific test categories..."
	@bash scripts/test_plugin.sh --specific-tests

test-quick: build-release
	@echo "Running quick validation (level 3)..."
	@bash scripts/test_plugin.sh --strictness-level 3 --timeout 15

test-comprehensive: build-release
	@echo "Running comprehensive validation (level 10)..."
	@bash scripts/test_plugin.sh --strictness-level 10 --timeout 120

# Development targets
lint:
	cargo clippy -- -D warnings

format:
	cargo fmt

check: lint
	cargo test

# Setup targets
setup-pluginval:
	@echo "Setting up pluginval..."
	@bash scripts/download_pluginval.sh

# Clean targets
clean:
	cargo clean
	rm -rf target/bundled/
	rm -f test_*.txt test_results.txt

clean-all: clean
	rm -rf tools/

# Help target
help:
	@echo "Available targets:"
	@echo "  build            - Build plugin in debug mode"
	@echo "  build-release    - Build and bundle plugin in release mode"
	@echo "  test             - Run standard pluginval tests (level 8)"
	@echo "  test-ci          - Run CI-friendly tests with multiple strictness levels"
	@echo "  test-specific    - Run tests by category"
	@echo "  test-quick       - Run quick validation (level 3, 15s timeout)"
	@echo "  test-comprehensive - Run comprehensive tests (level 10, 120s timeout)"
	@echo "  lint             - Run clippy lints"
	@echo "  format           - Format code with rustfmt"
	@echo "  check            - Run lint and unit tests"
	@echo "  setup-pluginval  - Download pluginval testing tool"
	@echo "  clean            - Clean build artifacts"
	@echo "  clean-all        - Clean everything including tools"
	@echo "  help             - Show this help message"
