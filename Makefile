DATABASE_URL ?= postgres://postgres:password@localhost:5432/dev
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

# Either an explicit version or a semver level to bump by.
BUMP_ARGS := $(if $(LEVEL),--bump $(LEVEL),$(VERSION))

.PHONY: help check build test fmt clippy bump clean

help:
	@echo "Available commands:"
	@echo "  make check    - Run cargo check for the workspace"
	@echo "  make build    - Build the workspace in debug mode"
	@echo "  make test     - Run unit and integration tests"
	@echo "  make fmt      - Format code with rustfmt"
	@echo "  make clippy   - Run clippy linter"
	@echo "  make bump     - Bump a crate version"
	@echo "                  CRATE=<name> VERSION=<x.y.z>"
	@echo "                  CRATE=<name> LEVEL=<major|minor|patch>"
	@echo "  make clean    - Clean the target directory"

check:
	cargo check --workspace --all-targets

build:
	cargo build --workspace

test:
	cargo test --workspace

fmt:
	cargo fmt --all

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

bump:
	@test -n "$(CRATE)" || { echo "usage: make bump CRATE=<name> VERSION=<x.y.z>"; echo "       make bump CRATE=<name> LEVEL=<major|minor|patch>"; exit 1; }
	@test -n "$(BUMP_ARGS)" || { echo "error: set VERSION=<x.y.z> or LEVEL=<major|minor|patch>"; exit 1; }
	cargo set-version -p $(CRATE) $(BUMP_ARGS)

clean:
	cargo clean
