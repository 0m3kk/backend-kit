//! File and environment based configuration loading.
//!
//! Services usually ship two YAML (or JSON) files: a non-secret configuration
//! file and a secret file. Both follow the same loading rules:
//!
//! 1. An explicit path, when provided.
//! 2. The path from an environment variable (`CONFIG_FILE` or `SECRET_FILE`).
//! 3. The type's [`Default`] implementation.
//!
//! The file format is selected by extension: `.json` files are parsed as JSON,
//! every other extension is parsed as YAML.

use std::fs;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use thiserror::Error;

/// Environment variable holding the configuration file path.
pub const ENV_CONFIG_FILE: &str = "CONFIG_FILE";

/// Environment variable holding the secret file path.
pub const ENV_SECRET_FILE: &str = "SECRET_FILE";

/// Errors produced while loading a configuration file.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// The file could not be read.
    #[error("failed to read `{path}`: {source}")]
    Read {
        /// Path of the file that could not be read.
        path: String,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// The file could not be parsed.
    #[error("failed to parse `{path}`: {message}")]
    Parse {
        /// Path of the file that could not be parsed.
        path: String,
        /// Parser error message.
        message: String,
    },
}

/// Loads the application configuration.
///
/// The path is resolved from `path`, then `CONFIG_FILE`, then [`Default`].
pub fn load_config<T>(path: Option<&Path>) -> Result<T, ConfigError>
where
    T: DeserializeOwned + Default,
{
    load_with_env(path, ENV_CONFIG_FILE)
}

/// Loads the application secrets.
///
/// The path is resolved from `path`, then `SECRET_FILE`, then [`Default`].
pub fn load_secret<T>(path: Option<&Path>) -> Result<T, ConfigError>
where
    T: DeserializeOwned + Default,
{
    load_with_env(path, ENV_SECRET_FILE)
}

/// Loads a configuration file, resolving its path from `env_var` when omitted.
pub fn load_with_env<T>(path: Option<&Path>, env_var: &str) -> Result<T, ConfigError>
where
    T: DeserializeOwned + Default,
{
    let path = path
        .map(Path::to_path_buf)
        .or_else(|| std::env::var(env_var).ok().map(PathBuf::from));

    match path {
        Some(path) => load_file(&path),
        None => Ok(T::default()),
    }
}

/// Loads a configuration file from an explicit path.
///
/// Files ending in `.json` are parsed as JSON; all other files are parsed as
/// YAML.
pub fn load_file<T>(path: &Path) -> Result<T, ConfigError>
where
    T: DeserializeOwned,
{
    let content = fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.display().to_string(),
        source,
    })?;

    parse(&content, path)
}

fn parse<T>(content: &str, path: &Path) -> Result<T, ConfigError>
where
    T: DeserializeOwned,
{
    let is_json = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"));

    let parsed = if is_json {
        serde_json::from_str(content).map_err(|error| error.to_string())
    } else {
        serde_yaml::from_str(content).map_err(|error| error.to_string())
    };

    parsed.map_err(|message| ConfigError::Parse {
        path: path.display().to_string(),
        message,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Default, PartialEq, Eq, Deserialize)]
    struct TestConfig {
        name: String,
        port: u16,
    }

    fn fixture(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("config-loader-{}-{name}", std::process::id()))
    }

    #[test]
    fn loads_yaml_file() -> Result<(), Box<dyn std::error::Error>> {
        let path = fixture("config.yaml");
        fs::write(&path, "name: api\nport: 8080\n")?;

        let config: TestConfig = load_file(&path)?;

        assert_eq!(
            config,
            TestConfig {
                name: "api".to_string(),
                port: 8080,
            }
        );
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn loads_json_file() -> Result<(), Box<dyn std::error::Error>> {
        let path = fixture("config.json");
        fs::write(&path, r#"{"name":"api","port":8080}"#)?;

        let config: TestConfig = load_file(&path)?;

        assert_eq!(
            config,
            TestConfig {
                name: "api".to_string(),
                port: 8080,
            }
        );
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn missing_file_is_reported_with_path() {
        let path = fixture("missing.yaml");
        let error = load_file::<TestConfig>(&path);

        assert!(matches!(error, Err(ConfigError::Read { .. })));
    }

    #[test]
    fn invalid_content_is_reported_as_parse_error() -> Result<(), Box<dyn std::error::Error>> {
        let path = fixture("invalid.yaml");
        fs::write(&path, "name: [unclosed\n")?;

        let error = load_file::<TestConfig>(&path);

        assert!(matches!(error, Err(ConfigError::Parse { .. })));
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn unset_env_falls_back_to_default() -> Result<(), Box<dyn std::error::Error>> {
        let config: TestConfig = load_with_env(None, "CONFIG_LOADER_TEST_UNSET")?;
        assert_eq!(config, TestConfig::default());
        Ok(())
    }
}
