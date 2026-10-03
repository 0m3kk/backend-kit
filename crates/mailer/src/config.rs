use serde::{Deserialize, Serialize};

/// Mailer provider type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MailerProvider {
    /// SMTP provider (lettre).
    Smtp,
    /// Mock provider (prints to tracing log or in-memory recording).
    #[default]
    Mock,
}

/// Configuration for SMTP server transport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmtpConfig {
    /// Hostname or IP of the SMTP server.
    pub host: String,

    /// Port of the SMTP server (default 587).
    #[serde(default = "default_smtp_port")]
    pub port: u16,

    /// Whether to enforce TLS/STARTTLS.
    #[serde(default = "default_true")]
    pub tls: bool,
}

fn default_smtp_port() -> u16 {
    587
}

fn default_true() -> bool {
    true
}

impl Default for SmtpConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 587,
            tls: false,
        }
    }
}

/// Secret credentials for SMTP authentication.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MailerSecret {
    /// SMTP authentication username.
    #[serde(default)]
    pub smtp_username: Option<String>,

    /// SMTP authentication password or API key.
    #[serde(default)]
    pub smtp_password: Option<String>,
}

impl std::fmt::Debug for MailerSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MailerSecret")
            .field("smtp_username", &self.smtp_username)
            .field(
                "smtp_password",
                &self.smtp_password.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

/// General mailer configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailerConfig {
    /// Provider type: smtp or mock.
    #[serde(default)]
    pub provider: MailerProvider,

    /// Sender display name.
    #[serde(default = "default_from_name")]
    pub from_name: String,

    /// Sender email address.
    #[serde(default = "default_from_email")]
    pub from_email: String,

    /// Frontend application base URL (e.g. `http://localhost:3000`).
    #[serde(default = "default_frontend_base_url")]
    pub frontend_base_url: String,

    /// Path to template files directory.
    #[serde(default = "default_templates_dir")]
    pub templates_dir: String,

    /// SMTP server configuration.
    #[serde(default)]
    pub smtp: SmtpConfig,
}

/// Environment variable name for overriding the email templates directory.
pub const ENV_TEMPLATES_DIR: &str = "TEMPLATES_DIR";

fn default_from_name() -> String {
    "Auth Platform".to_string()
}

fn default_from_email() -> String {
    "no-reply@example.com".to_string()
}

fn default_frontend_base_url() -> String {
    "http://localhost:3000".to_string()
}

fn default_templates_dir() -> String {
    std::env::var(ENV_TEMPLATES_DIR).unwrap_or_else(|_| "rsc/templates/emails".to_string())
}

impl MailerConfig {
    /// Overrides configuration fields with environment variables if present.
    pub fn apply_env(&mut self) {
        if let Ok(dir) = std::env::var(ENV_TEMPLATES_DIR) {
            self.templates_dir = dir;
        }
    }
}

impl Default for MailerConfig {
    fn default() -> Self {
        Self {
            provider: MailerProvider::default(),
            from_name: default_from_name(),
            from_email: default_from_email(),
            frontend_base_url: default_frontend_base_url(),
            templates_dir: default_templates_dir(),
            smtp: SmtpConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_redacts_smtp_password() {
        let secret = MailerSecret {
            smtp_username: Some("user".to_string()),
            smtp_password: Some("super_secret_smtp_password".to_string()),
        };
        let debug_str = format!("{secret:?}");
        assert!(!debug_str.contains("super_secret_smtp_password"));
        assert!(debug_str.contains("[REDACTED]"));
    }
}
