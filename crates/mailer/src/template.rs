use minijinja::{Environment, Value};
use std::path::{Path, PathBuf};

use crate::error::MailerError;

/// Service for rendering Minijinja email templates from a configurable directory.
#[derive(Clone)]
pub struct TemplateEngine {
    templates_dir: PathBuf,
}

impl TemplateEngine {
    /// Creates a new template engine pointing to the given templates directory.
    pub fn new(templates_dir: impl AsRef<Path>) -> Self {
        Self {
            templates_dir: templates_dir.as_ref().to_path_buf(),
        }
    }

    /// Returns the configured templates directory.
    pub fn templates_dir(&self) -> &Path {
        &self.templates_dir
    }

    fn resolve_dir(&self) -> PathBuf {
        if self.templates_dir.exists() {
            return self.templates_dir.clone();
        }
        if let Ok(cwd) = std::env::current_dir() {
            let mut cur = cwd.as_path();
            while let Some(parent) = cur.parent() {
                let candidate = parent.join(&self.templates_dir);
                if candidate.exists() {
                    return candidate;
                }
                cur = parent;
            }
        }
        self.templates_dir.clone()
    }

    /// Creates and configures a fresh Minijinja Environment loaded from the filesystem.
    fn create_env(&self) -> Result<Environment<'static>, MailerError> {
        let mut env = Environment::new();
        let dir = self.resolve_dir();
        if dir.exists() {
            env.set_loader(minijinja::path_loader(&dir));
        }
        Ok(env)
    }

    /// Renders a template file with the provided context data.
    pub fn render<S: serde::Serialize>(
        &self,
        template_name: &str,
        ctx: &S,
    ) -> Result<String, MailerError> {
        let env = self.create_env()?;
        let template = env.get_template(template_name).map_err(|e| {
            MailerError::Template(format!(
                "Failed to find template '{template_name}' in {:?}: {e}",
                self.templates_dir
            ))
        })?;

        let value = Value::from_serialize(ctx);
        template.render(value).map_err(|e| {
            MailerError::Template(format!("Failed to render template '{template_name}': {e}"))
        })
    }
}
