use serde::{Deserialize, Serialize};

/// Standard claims payload embedded in JSON Web Tokens with generic custom claims.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claims<T = ()> {
    /// Subject identifier (e.g. user ID, entity ID).
    pub sub: String,
    /// Issued at timestamp (seconds since Unix epoch).
    pub iat: u64,
    /// Expiration timestamp (seconds since Unix epoch).
    pub exp: u64,
    /// Custom payload data flattened directly into the claims JSON object.
    #[serde(flatten)]
    pub custom: T,
}

impl<T> Claims<T> {
    /// Creates a new claims payload with custom data.
    pub fn new(sub: impl Into<String>, custom: T, iat: u64, exp: u64) -> Self {
        Self {
            sub: sub.into(),
            iat,
            exp,
            custom,
        }
    }
}

impl Claims<()> {
    /// Creates standard claims without custom data.
    pub fn with_sub(sub: impl Into<String>, iat: u64, exp: u64) -> Self {
        Self {
            sub: sub.into(),
            iat,
            exp,
            custom: (),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct UserRole {
        role: String,
        tenant_id: u32,
    }

    #[test]
    fn creates_standard_claims() {
        let claims = Claims::with_sub("user-123", 1000, 2000);
        assert_eq!(claims.sub, "user-123");
        assert_eq!(claims.iat, 1000);
        assert_eq!(claims.exp, 2000);
        assert_eq!(claims.custom, ());

        let json = serde_json::to_string(&claims).unwrap();
        assert_eq!(json, r#"{"sub":"user-123","iat":1000,"exp":2000}"#);
    }

    #[test]
    fn creates_generic_custom_claims() {
        let custom = UserRole {
            role: "admin".to_string(),
            tenant_id: 42,
        };
        let claims = Claims::new("user-456", custom, 1000, 2000);
        assert_eq!(claims.sub, "user-456");
        assert_eq!(claims.custom.role, "admin");
        assert_eq!(claims.custom.tenant_id, 42);

        let json = serde_json::to_string(&claims).unwrap();
        assert!(json.contains(r#""role":"admin""#));
        assert!(json.contains(r#""tenant_id":42"#));

        let deserialized: Claims<UserRole> = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, claims);
    }
}
