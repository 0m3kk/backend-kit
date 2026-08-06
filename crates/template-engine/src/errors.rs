use thiserror::Error;

/// Error types encountered during template loading, compilation, or rendering.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TemplateError {
    /// Specified template name was not found in the engine registry or directory.
    #[error("Template '{0}' not found")]
    TemplateNotFound(String),

    /// Syntax error in template string syntax.
    #[error("Template syntax error in '{template}': {message}")]
    SyntaxError {
        /// Template name or identifier.
        template: String,
        /// Description of the syntax error.
        message: String,
    },

    /// Error during rendering of template with given context.
    #[error("Template render error: {0}")]
    RenderError(String),

    /// Failed to serialize context data into JSON/template values.
    #[error("Context serialization error: {0}")]
    SerializationError(String),

    /// IO error reading template files.
    #[error("Template IO error: {0}")]
    IoError(String),
}
