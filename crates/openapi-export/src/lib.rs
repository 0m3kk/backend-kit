//! Serialize and write OpenAPI documents built with `utoipa`.
//!
//! A service keeps a tiny binary that names its own document:
//!
//! ```ignore
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     Ok(openapi_export::run::<api::ApiDoc>()?)
//! }
//! ```
//!
//! The runner parses `--format` (`json` or `yaml`) and `--output`, creates
//! parent directories, and writes the document. Without `--output` it prints
//! the document to stdout.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use clap::{Parser, ValueEnum};
use utoipa::OpenApi;
use utoipa::openapi::OpenApi as Document;

/// Output format for an exported OpenAPI document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// Pretty-printed JSON.
    Json,

    /// YAML.
    Yaml,
}

/// Errors produced while exporting an OpenAPI document.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    /// The document could not be serialized.
    #[error("failed to serialize the OpenAPI document: {0}")]
    Serialize(String),

    /// The output could not be written.
    #[error("failed to write `{}`: {source}", path.display())]
    Write {
        /// Path that failed, or `<stdout>`.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: io::Error,
    },
}

/// Serializes an OpenAPI document in the requested format.
pub fn render_document(document: &Document, format: OutputFormat) -> Result<String, ExportError> {
    match format {
        OutputFormat::Json => document
            .to_pretty_json()
            .map_err(|error| ExportError::Serialize(error.to_string())),
        OutputFormat::Yaml => document
            .to_yaml()
            .map_err(|error| ExportError::Serialize(error.to_string())),
    }
}

/// Serializes the document of `T` in the requested format.
pub fn render<T: OpenApi>(format: OutputFormat) -> Result<String, ExportError> {
    render_document(&T::openapi(), format)
}

/// Parses export arguments and writes the document of `T`.
pub fn run<T: OpenApi>() -> Result<(), ExportError> {
    let cli = Cli::parse();
    let content = render::<T>(cli.format)?;

    match cli.output {
        Some(path) => write_file(&path, &content),
        None => {
            let mut stdout = io::stdout().lock();
            stdout
                .write_all(content.as_bytes())
                .and_then(|()| stdout.write_all(b"\n"))
                .map_err(|source| ExportError::Write {
                    path: PathBuf::from("<stdout>"),
                    source,
                })
        }
    }
}

/// Writes a serialized document, creating its parent directories.
pub fn write_file(path: &Path, content: &str) -> Result<(), ExportError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|source| ExportError::Write {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    fs::write(path, content).map_err(|source| ExportError::Write {
        path: path.to_path_buf(),
        source,
    })?;

    eprintln!("OpenAPI spec successfully written to {}", path.display());
    Ok(())
}

#[derive(Debug, Parser)]
#[command(
    name = "openapi-codegen",
    about = "Generate and export OpenAPI specification files",
    version
)]
struct Cli {
    /// Output format: `json` or `yaml`.
    #[arg(short, long, value_enum, default_value = "json")]
    format: OutputFormat,

    /// Output file path. If not provided, prints to stdout.
    #[arg(short, long)]
    output: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use std::fs;

    use utoipa::openapi::{Info, OpenApi as Document, Paths};

    use super::*;

    fn document() -> Document {
        Document::new(Info::new("Test API", "1.0.0"), Paths::new())
    }

    #[test]
    fn renders_json() -> Result<(), ExportError> {
        let json = render_document(&document(), OutputFormat::Json)?;

        assert!(json.contains("\"title\": \"Test API\""));
        Ok(())
    }

    #[test]
    fn renders_yaml() -> Result<(), ExportError> {
        let yaml = render_document(&document(), OutputFormat::Yaml)?;

        assert!(yaml.contains("title: Test API"));
        Ok(())
    }

    #[test]
    fn writes_nested_output() -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!("openapi-export-{}", std::process::id()));
        let path = dir.join("nested/openapi.json");

        write_file(&path, "{}")?;

        assert_eq!(fs::read_to_string(&path)?, "{}");
        fs::remove_dir_all(&dir)?;
        Ok(())
    }
}
