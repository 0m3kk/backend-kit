use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

use askama::Template;
use serde::Serialize;
use template_engine::{TemplateEngine, TemplateError};

/// Helper function to render any [`askama::Template`] struct directly into a `Result<String, TemplateError>`.
pub fn render_askama<T: Template>(template: &T) -> Result<String, TemplateError> {
    template
        .render()
        .map_err(|e| TemplateError::RenderError(format!("Askama render error: {e}")))
}

type AskamaRenderFn =
    Arc<dyn Fn(&serde_json::Value) -> Result<String, TemplateError> + Send + Sync>;

/// Askama-backed template engine implementation supporting type-safe compile-time templates.
#[derive(Clone, Default)]
pub struct AskamaTemplateEngine {
    registry: Arc<RwLock<HashMap<String, AskamaRenderFn>>>,
}

impl AskamaTemplateEngine {
    /// Creates a new, empty [`AskamaTemplateEngine`].
    pub fn new() -> Self {
        Self {
            registry: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Renders an [`askama::Template`] struct directly.
    pub fn render_template<T: Template>(&self, template: &T) -> Result<String, TemplateError> {
        render_askama(template)
    }

    /// Registers a type-safe renderer closure for a named Askama template that deserializes context data into `T`.
    pub fn register<T, F>(&self, name: impl Into<String>, render_fn: F) -> Result<(), TemplateError>
    where
        T: for<'de> serde::Deserialize<'de> + Template + 'static,
        F: Fn(T) -> Result<String, TemplateError> + Send + Sync + 'static,
    {
        let name_str = name.into();
        let closure: AskamaRenderFn = Arc::new(move |val: &serde_json::Value| {
            let item: T = serde_json::from_value(val.clone()).map_err(|e| {
                TemplateError::SerializationError(format!(
                    "Failed to deserialize context for Askama template: {e}"
                ))
            })?;
            render_fn(item)
        });

        let mut guard = self
            .registry
            .write()
            .map_err(|e| TemplateError::RenderError(format!("Lock failure: {e}")))?;
        guard.insert(name_str, closure);
        Ok(())
    }
}

impl TemplateEngine for AskamaTemplateEngine {
    fn render_str(
        &self,
        _template_str: &str,
        _context: &impl Serialize,
    ) -> Result<String, TemplateError> {
        Err(TemplateError::RenderError(
            "Askama templates are pre-compiled into Rust code; runtime string template evaluation is unsupported. Use compile-time #[derive(Template)] instead.".to_string(),
        ))
    }

    fn render(
        &self,
        template_name: &str,
        context: &impl Serialize,
    ) -> Result<String, TemplateError> {
        let guard = self
            .registry
            .read()
            .map_err(|e| TemplateError::RenderError(format!("Lock failure: {e}")))?;

        let render_fn = guard
            .get(template_name)
            .ok_or_else(|| TemplateError::TemplateNotFound(template_name.to_string()))?;

        let json_val = serde_json::to_value(context).map_err(|e| {
            TemplateError::SerializationError(format!("Failed to serialize context: {e}"))
        })?;

        render_fn(&json_val)
    }

    fn has_template(&self, template_name: &str) -> bool {
        match self.registry.read() {
            Ok(guard) => guard.contains_key(template_name),
            Err(_) => false,
        }
    }

    fn add_template(&self, _name: &str, _content: &str) -> Result<(), TemplateError> {
        Err(TemplateError::SyntaxError {
            template: _name.to_string(),
            message: "Askama templates are pre-compiled into Rust code and cannot be added as raw strings at runtime.".to_string(),
        })
    }

    fn load_folder(&self, _dir_path: &Path) -> Result<(), TemplateError> {
        Err(TemplateError::IoError(
            "Askama templates are embedded into the binary at compile time via #[template(path = \"...\")]. Use template struct definitions instead.".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use askama::Template;
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Template, Serialize, Deserialize)]
    #[template(source = "Hello {{ name }}!", ext = "txt")]
    struct HelloTemplate {
        name: String,
    }

    #[test]
    fn test_direct_askama_render() -> Result<(), TemplateError> {
        let tpl = HelloTemplate {
            name: "Alice".to_string(),
        };
        let res = render_askama(&tpl)?;
        assert_eq!(res, "Hello Alice!");
        Ok(())
    }

    #[test]
    fn test_askama_engine_registration() -> Result<(), TemplateError> {
        let engine = AskamaTemplateEngine::new();
        engine.register("hello", |tpl: HelloTemplate| render_askama(&tpl))?;

        assert!(engine.has_template("hello"));

        let ctx = HelloTemplate {
            name: "Bob".to_string(),
        };
        let res = engine.render("hello", &ctx)?;
        assert_eq!(res, "Hello Bob!");
        Ok(())
    }
}
