use askama::Template;
use serde::{Deserialize, Serialize};
use template_engine::{TemplateEngine, TemplateError};
use template_engine_askama::{AskamaTemplateEngine, render_askama};

#[derive(Template, Serialize, Deserialize)]
#[template(source = "<h1>Welcome {{ username }}</h1>", ext = "html")]
struct WelcomeTemplate {
    username: String,
}

#[test]
fn test_askama_integration() -> Result<(), TemplateError> {
    let tpl = WelcomeTemplate {
        username: "Developer".to_string(),
    };
    let rendered = render_askama(&tpl)?;
    assert_eq!(rendered, "<h1>Welcome Developer</h1>");

    let engine = AskamaTemplateEngine::new();
    engine.register("welcome.html", |t: WelcomeTemplate| render_askama(&t))?;

    let res = engine.render("welcome.html", &tpl)?;
    assert_eq!(res, "<h1>Welcome Developer</h1>");

    Ok(())
}
