USER ?= $(shell whoami)
DATABASE_URL ?= postgres://$(USER)@localhost:5432/postgres
UMADB_URL ?= http://127.0.0.1:50051
REDIS_URL ?= redis://127.0.0.1:6379
export DATABASE_URL
export UMADB_URL
export REDIS_URL

.PHONY: install-tools check lint fmt fmt-check test sort upgrade upgrade-latest remove-unused sql-fmt prettier crate-add-lib crate-add-bin crate-remove umadb-up umadb-down redis-up redis-down help

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

install-tools: ## Install required cargo tools
	cargo install cargo-sort cargo-machete cargo-upgrades cargo-workspace
	npm install -g sql-formatter prettier

check: ## Check workspace
	cargo check --workspace --all-targets

lint: ## Run clippy
	cargo clippy --workspace --all-targets -- -D warnings

fmt: ## Format code
	cargo fmt --all
	$(MAKE) remove-unused
	$(MAKE) sort
	$(MAKE) sql-fmt
	$(MAKE) prettier

fmt-check: ## Check formatting
	cargo fmt --all -- --check

test: ## Run tests (usage: make test or make test features=xxx)
	cargo test --workspace --all-features

sort: ## Sort Cargo.toml dependencies
	cargo sort --workspace

remove-unused: ## Remove unused dependencies
	cargo machete --fix || true

upgrade: ## Upgrade dependencies (compatible versions)
	cargo upgrade

upgrade-latest: ## Upgrade dependencies to latest versions
	cargo upgrade --incompatible

sql-fmt: ## Format SQL files
	@find . -name "*.sql" -exec sql-formatter -l postgresql -o {} {} \;

prettier: ## Format YAML, JSON, MD files
	npx prettier --write "**/*.{yml,yaml,json,md}"

umadb-up: ## Start UmaDB server in container
	container run -d --name umadb-test --rm -p 50051:50051 umadb/umadb:latest

umadb-down: ## Stop UmaDB server container
	container stop umadb-test || true

redis-up: ## Start Redis server in container
	container run -d --name redis-test --rm -p 6379:6379 redis:latest

redis-down: ## Stop Redis server container
	container stop redis-test || true

crate-add-lib: ## Add library crate (usage: make crate-add-lib xxx)
	cargo new --lib --edition 2024 crates/$(word 2, $(MAKECMDGOALS))
	@grep -q "\[lints\]" crates/$(word 2, $(MAKECMDGOALS))/Cargo.toml || (echo "" >> crates/$(word 2, $(MAKECMDGOALS))/Cargo.toml && echo "[lints]" >> crates/$(word 2, $(MAKECMDGOALS))/Cargo.toml && echo "workspace = true" >> crates/$(word 2, $(MAKECMDGOALS))/Cargo.toml)
	$(MAKE) sort

crate-add-bin: ## Add binary crate (usage: make crate-add-bin xxx)
	cargo new --bin --edition 2024 crates/$(word 2, $(MAKECMDGOALS))
	@grep -q "\[lints\]" crates/$(word 2, $(MAKECMDGOALS))/Cargo.toml || (echo "" >> crates/$(word 2, $(MAKECMDGOALS))/Cargo.toml && echo "[lints]" >> crates/$(word 2, $(MAKECMDGOALS))/Cargo.toml && echo "workspace = true" >> crates/$(word 2, $(MAKECMDGOALS))/Cargo.toml)
	$(MAKE) sort

crate-remove: ## Remove crate (usage: make crate-remove xxx)
	@sed -i '' '/crates\/$(word 2, $(MAKECMDGOALS))/d' Cargo.toml
	@rm -rf crates/$(word 2, $(MAKECMDGOALS))
	$(MAKE) sort

version: ## Bump version (usage: make version major|minor|patch)
	cargo workspaces version $(word 2, $(MAKECMDGOALS)) -y --allow-branch main

version-patch: ## Bump patch version (0.1.0 -> 0.1.1)
	cargo workspaces version patch -y --allow-branch main

version-minor: ## Bump minor version (0.1.0 -> 0.2.0)
	cargo workspaces version minor -y --allow-branch main

version-major: ## Bump major version (0.1.0 -> 1.0.0)
	cargo workspaces version major -y --allow-branch main

# Ignore extra arguments passed to targets
%:
	@true
