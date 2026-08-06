use std::fs;
use tempfile::tempdir;
use template_engine::{TemplateContext, TemplateEngine, TemplateError};
use template_engine_tera::TeraTemplateEngine;

#[test]
fn test_tera_loops_and_conditionals() -> Result<(), TemplateError> {
    let engine = TeraTemplateEngine::new();
    engine.add_template(
        "item_list",
        "Items: {% for item in items %}{{ item }}{% if not loop.last %}, {% endif %}{% endfor %}",
    )?;

    let ctx = TemplateContext::new().insert("items", &vec!["Apple", "Banana", "Cherry"])?;
    let rendered = engine.render("item_list", &ctx)?;
    assert_eq!(rendered, "Items: Apple, Banana, Cherry");

    Ok(())
}

#[test]
fn test_folder_loading_trait_method() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let folder = dir.path().join("templates");
    fs::create_dir_all(&folder)?;
    fs::write(folder.join("email.txt"), "Hello {{ name }}!")?;

    let engine = TeraTemplateEngine::new();
    engine.load_folder(&folder)?;

    assert!(engine.has_template("email.txt"));
    let ctx = TemplateContext::new().insert("name", &"Developer")?;
    let output = engine.render("email.txt", &ctx)?;
    assert_eq!(output, "Hello Developer!");

    Ok(())
}
