# template-engine-tera

Tera (Jinja2-compatible) template engine implementation of [`TemplateEngine`] for `backend-kit`.

## Features

- **Jinja2 Syntax**: Fully supports variables, conditionals, loops, macros, and filters powered by [`tera`].
- **Folder & Glob Loading**:
  - `TeraTemplateEngine::from_directory("templates")`: Load all templates recursively from a folder path.
  - `TeraTemplateEngine::from_glob("templates/**/*.html")`: Load templates using an explicit glob pattern.
  - `engine.load_folder(Path::new("extra_templates"))`: Dynamically load folder contents at runtime.
- **Thread Safe**: `TeraTemplateEngine` is thread-safe (`Send + Sync`) wrapped in `Arc<RwLock<Tera>>`.

## Code Example

```rust
use template_engine::{TemplateContext, TemplateEngine};
use template_engine_tera::TeraTemplateEngine;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Load recursively from directory path
    let engine = TeraTemplateEngine::from_directory("templates")?;

    // 2. Or register in-memory template
    engine.add_template(
        "welcome_email",
        "<h1>Hello {{ name }}!</h1><p>Your score is {{ score }}.</p>",
    )?;

    let ctx = TemplateContext::new()
        .insert("name", &"Alice")?
        .insert("score", &100)?;

    let html = engine.render("welcome_email", &ctx)?;
    println!("{}", html);

    Ok(())
}
```
