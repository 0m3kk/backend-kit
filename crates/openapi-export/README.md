# openapi-export

Serialize and write [`utoipa`](https://docs.rs/utoipa) OpenAPI documents, with a
small CLI runner shared by services.

A service keeps a tiny binary that names its own document:

```rust
use api::ApiDoc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Ok(openapi_export::run::<ApiDoc>()?)
}
```

```sh
cargo run -p api --bin openapi-codegen -- --format json --output crates/api/openapi/openapi.json
cargo run -p api --bin openapi-codegen -- --format yaml --output crates/api/openapi/openapi.yaml
```

Without `--output`, the document is printed to stdout.

The library also exposes `render::<T>(format)`,
`render_document(&document, format)`, and `write_file(path, content)` for build
scripts and tests.
