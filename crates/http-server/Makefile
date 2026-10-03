.PHONY: help check build test fmt clippy clean

help:
	@echo "Available commands:"
	@echo "  make check    - Run cargo check"
	@echo "  make build    - Build in debug mode"
	@echo "  make test     - Run unit and integration tests"
	@echo "  make fmt      - Format code with rustfmt"
	@echo "  make clippy   - Run clippy linter"
	@echo "  make clean    - Clean target directory"

check:
	cargo check --all-targets

build:
	cargo build

test:
	cargo test

fmt:
	cargo fmt --all

clippy:
	cargo clippy --all-targets -- -D warnings

clean:
	cargo clean
