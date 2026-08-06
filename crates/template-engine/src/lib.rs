pub mod context;
pub mod errors;
pub mod traits;

pub use context::TemplateContext;
pub use errors::TemplateError;
pub use traits::TemplateEngine;

#[cfg(test)]
mod tests {
    use serde::Serialize;
    use serde_json::json;

    use super::*;

    #[derive(Serialize)]
    struct User {
        name: String,
        age: u32,
    }

    #[test]
    fn test_template_context_builder() -> Result<(), TemplateError> {
        let user = User {
            name: "Alice".to_string(),
            age: 30,
        };

        let ctx = TemplateContext::new()
            .insert("title", &"Welcome")?
            .insert("user", &user)?;

        let val = ctx.into_value();
        assert_eq!(val["title"], "Welcome");
        assert_eq!(val["user"]["name"], "Alice");
        assert_eq!(val["user"]["age"], 30);

        Ok(())
    }

    #[test]
    fn test_template_context_extend() -> Result<(), TemplateError> {
        let user = User {
            name: "Bob".to_string(),
            age: 25,
        };

        let ctx = TemplateContext::new()
            .insert("extra", &true)?
            .extend_from(&user)?;

        let map = ctx.as_map();
        assert_eq!(map.get("name"), Some(&json!("Bob")));
        assert_eq!(map.get("age"), Some(&json!(25)));
        assert_eq!(map.get("extra"), Some(&json!(true)));

        Ok(())
    }
}
