# template-engine-askama

Askama (type-safe compile-time HTML templates) implementation of [`TemplateEngine`] for `backend-kit`.

## Features

- **Type-Safe Compile-Time Templates**: Zero runtime parsing overhead using compile-time checked Askama templates (`#[derive(Template)]`).
- **Direct & Dynamic Rendering**: Render Askama structs directly via `render_askama(&tpl)` or register them dynamically in `AskamaTemplateEngine`.
- **Zero Allocations & High Performance**: Ultra-fast HTML rendering compiled into machine code.

## Code Example

```rust
use askama::Template;
use serde::{Deserialize, Serialize};
use template_engine::TemplateEngine;
use template_engine_askama::{render_askama, AskamaTemplateEngine};

#[derive(Template, Serialize, Deserialize)]
#[template(source = "Hello {{ name }}!", ext = "html")]
struct HelloTemplate {
    name: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tpl = HelloTemplate { name: "Alice".to_string() };

    // 1. Direct render
    let output = render_askama(&tpl)?;
    println!("{}", output);

    // 2. Engine registration
    let engine = AskamaTemplateEngine::new();
    engine.register("hello", |t: HelloTemplate| render_askama(&t))?;

    let output2 = engine.render("hello", &tpl)?;
    println!("{}", output2);

    Ok(())
}
```
