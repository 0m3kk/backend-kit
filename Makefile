.PHONY: install-tools check lint fmt fmt-check test sort upgrade upgrade-latest remove-unused sql-fmt prettier crate-add-lib crate-add-bin crate-remove help

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

install-tools: ## Install required cargo tools
	cargo install cargo-sort cargo-machete cargo-upgrades cargo-workspace
	npm install -g sql-formatter prettier

check: ## Check workspace
	cargo check --workspace --all-targets

lint: ## Run clippy
	cargo clippy --workspace --all-targets -- -D warnings -W clippy::unwrap_used -W clippy::expect_used

fmt: ## Format code
	cargo fmt --all
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
	cargo machete --fix

upgrade: ## Upgrade dependencies (compatible versions)
	cargo upgrade --workspace

upgrade-latest: ## Upgrade dependencies to latest versions
	cargo upgrade --workspace --incompatible

sql-fmt: ## Format SQL files
	@find . -name "*.sql" -exec sql-formatter -l postgresql -o {} {} \;

prettier: ## Format YAML, JSON, MD files
	npx prettier --write "**/*.{yml,yaml,json,md}"

crate-add-lib: ## Add library crate (usage: make crate-add-lib xxx)
	cargo workspaces create --lib crates/$(word 2, $(MAKECMDGOALS))

crate-add-bin: ## Add binary crate (usage: make crate-add-bin xxx)
	cargo workspaces create --bin crates/$(word 2, $(MAKECMDGOALS))

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
