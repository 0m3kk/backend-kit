use std::path::Path;
use std::sync::{Arc, RwLock};

use serde::Serialize;
use template_engine::{TemplateEngine, TemplateError};
use tera::{Context, Tera};

/// Tera-backed template engine implementation supporting Jinja2-style template syntax.
#[derive(Clone)]
pub struct TeraTemplateEngine {
    tera: Arc<RwLock<Tera>>,
}

impl TeraTemplateEngine {
    /// Creates a new, empty [`TeraTemplateEngine`].
    pub fn new() -> Self {
        Self {
            tera: Arc::new(RwLock::new(Tera::new())),
        }
    }

    /// Creates a [`TeraTemplateEngine`] loading template files recursively from a directory path (e.g. `"templates"` or `"src/templates"`).
    pub fn from_directory(dir_path: impl AsRef<Path>) -> Result<Self, TemplateError> {
        let path = dir_path.as_ref();
        if !path.exists() || !path.is_dir() {
            return Err(TemplateError::IoError(format!(
                "Directory does not exist or is not a directory: '{}'",
                path.display()
            )));
        }

        let engine = Self::new();
        engine.load_folder(path)?;
        Ok(engine)
    }

    /// Alias for [`from_directory`](Self::from_directory). Creates a [`TeraTemplateEngine`] loading templates recursively from a folder.
    pub fn from_folder(dir_path: impl AsRef<Path>) -> Result<Self, TemplateError> {
        Self::from_directory(dir_path)
    }

    /// Creates a [`TeraTemplateEngine`] loading template files matching an explicit glob pattern (e.g. `templates/**/*.html`).
    pub fn from_glob(glob_pattern: &str) -> Result<Self, TemplateError> {
        let mut tera = Tera::new();
        for entry in glob::glob(glob_pattern)
            .map_err(|e| TemplateError::IoError(format!("Glob error: {e}")))?
        {
            let file_path =
                entry.map_err(|e| TemplateError::IoError(format!("Glob entry error: {e}")))?;
            if file_path.is_file() {
                let name = file_path.to_string_lossy().to_string();
                let content = std::fs::read_to_string(&file_path)
                    .map_err(|e| TemplateError::IoError(format!("Failed to read '{name}': {e}")))?;
                tera.add_raw_template(&name, &content)
                    .map_err(|e| TemplateError::SyntaxError {
                        template: name,
                        message: format!("{e}"),
                    })?;
            }
        }
        Ok(Self {
            tera: Arc::new(RwLock::new(tera)),
        })
    }

    /// Creates a [`TeraTemplateEngine`] wrapping an existing [`Tera`] instance.
    pub fn from_tera(tera: Tera) -> Self {
        Self {
            tera: Arc::new(RwLock::new(tera)),
        }
    }

    /// Registers multiple in-memory templates from an iterator of `(name, content)` pairs.
    pub fn add_raw_templates<'a>(
        &self,
        templates: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Result<(), TemplateError> {
        let mut guard = self
            .tera
            .write()
            .map_err(|e| TemplateError::RenderError(format!("Lock failure: {e}")))?;
        guard
            .add_raw_templates(templates)
            .map_err(|e| TemplateError::SyntaxError {
                template: "batch_templates".to_string(),
                message: format!("{e}"),
            })?;
        Ok(())
    }

    fn to_tera_context(context: &impl Serialize) -> Result<Context, TemplateError> {
        Context::from_serialize(context).map_err(|e| {
            TemplateError::SerializationError(format!("Failed to convert context to Tera: {e}"))
        })
    }
}

impl Default for TeraTemplateEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl TemplateEngine for TeraTemplateEngine {
    fn render_str(
        &self,
        template_str: &str,
        context: &impl Serialize,
    ) -> Result<String, TemplateError> {
        let tera_ctx = Self::to_tera_context(context)?;
        Tera::one_off(template_str, &tera_ctx, true)
            .map_err(|e| TemplateError::RenderError(format!("{e}")))
    }

    fn render(
        &self,
        template_name: &str,
        context: &impl Serialize,
    ) -> Result<String, TemplateError> {
        if !self.has_template(template_name) {
            return Err(TemplateError::TemplateNotFound(template_name.to_string()));
        }

        let guard = self
            .tera
            .read()
            .map_err(|e| TemplateError::RenderError(format!("Lock failure: {e}")))?;

        let tera_ctx = Self::to_tera_context(context)?;

        guard
            .render(template_name, &tera_ctx)
            .map_err(|e| TemplateError::RenderError(format!("{e}")))
    }

    fn has_template(&self, template_name: &str) -> bool {
        match self.tera.read() {
            Ok(guard) => guard.get_template_names().any(|n| n == template_name),
            Err(_) => false,
        }
    }

    fn add_template(&self, name: &str, content: &str) -> Result<(), TemplateError> {
        let mut guard = self
            .tera
            .write()
            .map_err(|e| TemplateError::RenderError(format!("Lock failure: {e}")))?;
        guard
            .add_raw_template(name, content)
            .map_err(|e| TemplateError::SyntaxError {
                template: name.to_string(),
                message: format!("{e}"),
            })?;
        Ok(())
    }

    fn load_folder(&self, dir_path: &Path) -> Result<(), TemplateError> {
        if !dir_path.exists() || !dir_path.is_dir() {
            return Err(TemplateError::IoError(format!(
                "Directory does not exist or is not a directory: '{}'",
                dir_path.display()
            )));
        }

        let walk_dir = walkdir::WalkDir::new(dir_path);

        let mut guard = self
            .tera
            .write()
            .map_err(|e| TemplateError::RenderError(format!("Lock failure: {e}")))?;

        for entry in walk_dir.into_iter().flatten() {
            let path = entry.path();
            if path.is_file() {
                let relative = path
                    .strip_prefix(dir_path)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .trim_start_matches('/')
                    .trim_start_matches('\\')
                    .to_string();

                let content = std::fs::read_to_string(path).map_err(|e| {
                    TemplateError::IoError(format!("Failed to read '{relative}': {e}"))
                })?;
                guard.add_raw_template(&relative, &content).map_err(|e| {
                    TemplateError::SyntaxError {
                        template: relative,
                        message: format!("{e}"),
                    }
                })?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use tempfile::tempdir;
    use template_engine::TemplateContext;

    use super::*;

    #[test]
    fn test_render_str() -> Result<(), TemplateError> {
        let engine = TeraTemplateEngine::new();
        let ctx = TemplateContext::new().insert("name", &"World")?;

        let output = engine.render_str("Hello {{ name }}!", &ctx)?;
        assert_eq!(output, "Hello World!");
        Ok(())
    }

    #[test]
    fn test_registered_template() -> Result<(), TemplateError> {
        let engine = TeraTemplateEngine::new();
        engine.add_template("greeting", "Hello {{ username }}, welcome to {{ app }}!")?;

        assert!(engine.has_template("greeting"));
        assert!(!engine.has_template("unknown"));

        let ctx = TemplateContext::new()
            .insert("username", &"Alice")?
            .insert("app", &"BackendKit")?;

        let output = engine.render("greeting", &ctx)?;
        assert_eq!(output, "Hello Alice, welcome to BackendKit!");
        Ok(())
    }

    #[test]
    fn test_from_directory_and_from_glob() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempdir()?;
        let sub_dir = dir.path().join("templates");
        fs::create_dir_all(&sub_dir)?;

        let t1_path = sub_dir.join("welcome.html");
        fs::write(&t1_path, "<h1>Welcome {{ name }}</h1>")?;

        // 1. Load via from_directory(path)
        let engine = TeraTemplateEngine::from_directory(&sub_dir)?;
        assert!(engine.has_template("welcome.html"));

        let ctx = TemplateContext::new().insert("name", &"Charlie")?;
        let res = engine.render("welcome.html", &ctx)?;
        assert_eq!(res, "<h1>Welcome Charlie</h1>");

        // 2. Load via from_glob(pattern)
        let glob_pat = format!("{}/*.html", sub_dir.to_string_lossy());
        let glob_engine = TeraTemplateEngine::from_glob(&glob_pat)?;
        assert!(glob_engine.has_template(&t1_path.to_string_lossy()));

        Ok(())
    }

    #[test]
    fn test_missing_template_error() {
        let engine = TeraTemplateEngine::new();
        let ctx = TemplateContext::new();
        let err = engine.render("missing", &ctx);
        assert!(err.is_err());
        assert_eq!(
            err.unwrap_err(),
            TemplateError::TemplateNotFound("missing".to_string())
        );
    }
}
