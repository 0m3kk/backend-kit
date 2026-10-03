use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::Serialize;
use serde::de::DeserializeOwned;
use time::OffsetDateTime;

use crate::claims::Claims;
use crate::config::JwtConfig;
use crate::error::JwtError;

/// Service for generating and verifying JSON Web Tokens with generic claims.
#[derive(Clone)]
pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    config: JwtConfig,
}

impl std::fmt::Debug for JwtService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwtService")
            .field("config", &self.config)
            .finish()
    }
}

impl JwtService {
    /// Creates a new JWT service with the given secret and configuration.
    pub fn new(secret: &str, config: JwtConfig) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            config,
        }
    }

    /// Returns a reference to the active configuration.
    pub fn config(&self) -> &JwtConfig {
        &self.config
    }

    /// Generates a signed JWT token for a subject with standard claims using default configured TTL.
    pub fn generate_token(&self, sub: &str) -> Result<String, JwtError> {
        self.generate_token_with_custom(sub, ())
    }

    /// Generates a signed JWT token for a subject with custom data using default configured TTL.
    pub fn generate_token_with_custom<T: Serialize>(
        &self,
        sub: &str,
        custom: T,
    ) -> Result<String, JwtError> {
        let now = OffsetDateTime::now_utc();
        self.generate_token_with_custom_at(sub, custom, now, self.config.expiration_secs)
    }

    /// Generates a signed JWT token for a subject with custom data, specific issued time, and TTL.
    pub fn generate_token_with_custom_at<T: Serialize>(
        &self,
        sub: &str,
        custom: T,
        now: OffsetDateTime,
        ttl_secs: u64,
    ) -> Result<String, JwtError> {
        let iat = now.unix_timestamp() as u64;
        let exp = iat + ttl_secs;
        let claims = Claims::new(sub, custom, iat, exp);
        self.generate_custom_token(&claims)
    }

    /// Generates a signed JWT token from any serializable claims payload.
    pub fn generate_custom_token<T: Serialize>(&self, claims: &T) -> Result<String, JwtError> {
        encode(&Header::default(), claims, &self.encoding_key)
            .map_err(|e| JwtError::GenerationFailed(e.to_string()))
    }

    /// Verifies and decodes a signed JWT token into [`Claims<T>`].
    pub fn verify_token<T: DeserializeOwned>(&self, token: &str) -> Result<Claims<T>, JwtError> {
        self.verify_custom_token(token)
    }

    /// Verifies and decodes a signed JWT token into any custom claims type.
    pub fn verify_custom_token<T: DeserializeOwned>(&self, token: &str) -> Result<T, JwtError> {
        let validation = Validation::default();
        let data = decode::<T>(token, &self.decoding_key, &validation)?;
        Ok(data.claims)
    }

    /// Verifies and decodes a signed JWT token with custom validation rules.
    pub fn verify_custom_token_with_validation<T: DeserializeOwned>(
        &self,
        token: &str,
        validation: &Validation,
    ) -> Result<T, JwtError> {
        let data = decode::<T>(token, &self.decoding_key, validation)?;
        Ok(data.claims)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use time::Duration;

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct CustomPayload {
        tenant_id: String,
        permissions: Vec<String>,
    }

    #[test]
    fn generates_and_verifies_standard_token() {
        let config = JwtConfig {
            expiration_secs: 3600,
        };
        let service = JwtService::new("my_test_jwt_secret_key_1234567890", config);

        let token = service.generate_token("user-123").unwrap();
        let claims = service.verify_token::<()>(&token).unwrap();

        assert_eq!(claims.sub, "user-123");
        assert_eq!(claims.custom, ());
        assert!(claims.exp > claims.iat);
    }

    #[test]
    fn generates_and_verifies_generic_custom_token() {
        let config = JwtConfig::default();
        let service = JwtService::new("my_test_jwt_secret_key_1234567890", config);

        let payload = CustomPayload {
            tenant_id: "tenant-abc".to_string(),
            permissions: vec!["read".to_string(), "write".to_string()],
        };

        let token = service
            .generate_token_with_custom("user-456", &payload)
            .unwrap();

        let claims: Claims<CustomPayload> = service.verify_token(&token).unwrap();

        assert_eq!(claims.sub, "user-456");
        assert_eq!(claims.custom, payload);
    }

    #[test]
    fn generates_and_verifies_arbitrary_serializable_struct() {
        #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
        struct DirectToken {
            sub: String,
            exp: u64,
            nonce: String,
        }

        let config = JwtConfig::default();
        let service = JwtService::new("my_test_jwt_secret_key_1234567890", config);

        let token_payload = DirectToken {
            sub: "device-789".to_string(),
            exp: OffsetDateTime::now_utc().unix_timestamp() as u64 + 600,
            nonce: "random-nonce-value".to_string(),
        };

        let token = service.generate_custom_token(&token_payload).unwrap();
        let decoded: DirectToken = service.verify_custom_token(&token).unwrap();

        assert_eq!(decoded, token_payload);
    }

    #[test]
    fn rejects_expired_token() {
        let config = JwtConfig {
            expiration_secs: 60,
        };
        let service = JwtService::new("my_test_jwt_secret_key_1234567890", config);

        let past_time = OffsetDateTime::now_utc() - Duration::hours(2);
        let token = service
            .generate_token_with_custom_at("user-uuid-1", (), past_time, 60)
            .unwrap();

        let result = service.verify_token::<()>(&token);
        assert_eq!(result, Err(JwtError::ExpiredSignature));
    }

    #[test]
    fn rejects_tampered_token() {
        let config = JwtConfig::default();
        let service = JwtService::new("my_test_jwt_secret_key_1234567890", config);

        let invalid_token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.invalid.signature";
        assert!(service.verify_token::<()>(invalid_token).is_err());
    }
}
