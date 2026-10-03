use validator::ValidationError;

/// Validates that a string is a valid URL-friendly slug (lowercase alphanumeric and hyphens).
pub fn validate_slug(slug: &str) -> Result<(), ValidationError> {
    if slug.len() < 2 || slug.len() > 64 {
        return Err(ValidationError::new("invalid_slug_length"));
    }

    if slug.starts_with('-') || slug.ends_with('-') {
        return Err(ValidationError::new("invalid_slug_boundary"));
    }

    if !slug
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(ValidationError::new("invalid_slug_characters"));
    }

    if slug.contains("--") {
        return Err(ValidationError::new("invalid_slug_consecutive_hyphens"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_valid_slugs() {
        assert!(validate_slug("acme").is_ok());
        assert!(validate_slug("acme-corp").is_ok());
        assert!(validate_slug("team-123-dev").is_ok());
    }

    #[test]
    fn rejects_invalid_slugs() {
        assert!(validate_slug("a").is_err());
        assert!(validate_slug("-acme").is_err());
        assert!(validate_slug("acme-").is_err());
        assert!(validate_slug("Acme").is_err());
        assert!(validate_slug("acme_corp").is_err());
        assert!(validate_slug("acme--corp").is_err());
    }
}
