# Backend Kit

Rust workspace project.

## Commands

```bash
make help            # Show all commands
make install-tools   # Install required cargo tools
make lint            # Run clippy
make fmt             # Format all code (Rust, SQL, YAML, JSON, MD)
make sort            # Sort Cargo.toml dependencies
make remove-unused   # Remove unused dependencies
make upgrade         # Upgrade dependencies (compatible versions)
make upgrade-latest  # Upgrade dependencies to latest
make sql-fmt         # Format SQL files
make prettier        # Format YAML, JSON, MD files
make crate-add-lib xxx  # Add library crate
make crate-add-bin xxx  # Add binary crate
make crate-remove xxx   # Remove crate
make version-patch   # Bump patch version (0.1.0 -> 0.1.1)
make version-minor   # Bump minor version (0.1.0 -> 0.2.0)
make version-major   # Bump major version (0.1.0 -> 1.0.0)
```

## Requirements

- Rust (stable)
- Run `make install-tools` to install all dependencies
