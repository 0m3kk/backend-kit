use thiserror::Error;

/// Errors that can occur during JWT signing, encoding, or verification.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum JwtError {
    #[error("JWT signature has expired")]
    ExpiredSignature,

    #[error("Invalid JWT token: {0}")]
    InvalidToken(String),

    #[error("Token generation failed: {0}")]
    GenerationFailed(String),
}

impl From<jsonwebtoken::errors::Error> for JwtError {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        match err.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => Self::ExpiredSignature,
            _ => Self::InvalidToken(err.to_string()),
        }
    }
}
