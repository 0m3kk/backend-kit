# template-engine

Core traits, context builder, and error definitions for generic template rendering in `backend-kit`.

## Features

- **Generic Abstraction**: [`TemplateEngine`] trait for rendering strings or pre-loaded templates.
- **Fluent Context Builder**: [`TemplateContext`] for safely composing template data variables.
- **Standardized Errors**: [`TemplateError`] for syntax, missing template, IO, and serialization failures.

## Usage

```rust
use serde::Serialize;
use template_engine::{TemplateContext, TemplateError};

#[derive(Serialize)]
struct User {
    name: String,
}

fn main() -> Result<(), TemplateError> {
    let ctx = TemplateContext::new()
        .insert("app_name", &"BackendKit")?
        .insert("user", &User { name: "Alice".to_string() })?;

    println!("Context data: {:?}", ctx.as_map());
    Ok(())
}
```
