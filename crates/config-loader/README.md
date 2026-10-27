# config-loader

File and environment based configuration loading shared by backend services.

It resolves a configuration path from an explicit value, an environment
variable, or [`Default`], and parses either YAML or JSON based on the file
extension.

---

## ✨ Features

- 📄 **YAML and JSON**: `.json` files are parsed as JSON; everything else as YAML.
- 🌍 **Environment fallback**: `CONFIG_FILE` and `SECRET_FILE` are used when no
  explicit path is given.
- 🧩 **Generic**: works with any `serde`-deserializable, `Default` type.
- 🧱 **Typed errors**: `ConfigError` distinguishes read and parse failures.

---

## 🚀 Usage

```rust
use std::path::Path;

use config_loader::{load_config, load_secret};
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
struct AppConfig {
    http: HttpConfig,
}

#[derive(Debug, Default, Deserialize)]
struct AppSecret {
    postgres: PostgresSecret,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config: AppConfig = load_config(None)?;
    let secret: AppSecret = load_secret(None)?;
    Ok(())
}
```

With `CONFIG_FILE=config.yaml` and `SECRET_FILE=secret.yaml`, both files are
loaded relative to the process working directory. Without either, the types
fall back to their `Default` implementation.

---

## 📜 License

Licensed under the [MIT License](LICENSE).
