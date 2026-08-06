use std::path::Path;

use serde::Serialize;

use crate::errors::TemplateError;

/// Universal trait for template rendering engines.
pub trait TemplateEngine: Send + Sync {
    /// Renders an inline template string with any serializable context data.
    fn render_str(
        &self,
        template_str: &str,
        context: &impl Serialize,
    ) -> Result<String, TemplateError>;

    /// Renders a registered or loaded template by name with any serializable context data.
    fn render(
        &self,
        template_name: &str,
        context: &impl Serialize,
    ) -> Result<String, TemplateError>;

    /// Checks if a template with the given name is registered or available.
    fn has_template(&self, template_name: &str) -> bool;

    /// Registers a raw in-memory template string under the specified `name`.
    fn add_template(&self, name: &str, content: &str) -> Result<(), TemplateError>;

    /// Loads and registers template files recursively from a folder path.
    fn load_folder(&self, dir_path: &Path) -> Result<(), TemplateError>;
}
