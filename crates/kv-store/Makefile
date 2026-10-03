DATABASE_URL ?= postgres://postgres:password@localhost:5432/kv_store_test
export DATABASE_URL

# Automatically configure Homebrew OpenSSL pkgconfig path on macOS
ifeq ($(shell uname -s),Darwin)
  ifneq ($(wildcard /opt/homebrew/opt/openssl@3/lib/pkgconfig),)
    PKG_CONFIG_PATH := /opt/homebrew/opt/openssl@3/lib/pkgconfig:$(PKG_CONFIG_PATH)
    export PKG_CONFIG_PATH
  else ifneq ($(wildcard /usr/local/opt/openssl@3/lib/pkgconfig),)
    PKG_CONFIG_PATH := /usr/local/opt/openssl@3/lib/pkgconfig:$(PKG_CONFIG_PATH)
    export PKG_CONFIG_PATH
  endif
endif

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
