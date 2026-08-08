/// Built-in list of notoriously weak and common passwords.
pub static COMMON_PASSWORDS: &[&str] = &[
    "123456",
    "12345678",
    "123456789",
    "1234567890",
    "12345",
    "password",
    "1234567",
    "qwerty",
    "1234567890",
    "1234",
    "111111",
    "123123",
    "admin",
    "welcome",
    "login",
    "password123",
    "abc123",
    "secret",
    "p@ssword",
    "pass1234",
    "master",
    "administrator",
    "guest",
    "changeme",
    "iloveyou",
    "sunshine",
    "princess",
    "monkey",
    "dragon",
    "football",
    "shadow",
    "superman",
    "baseball",
    "trustno1",
    "mustang",
    "super123",
    "root",
    "sysadmin",
    "testing",
    "letmein",
    "qwertyuiop",
    "zxcvbnm",
    "abcdef",
    "000000",
    "password1",
    "computer",
    "internet",
];

/// Returns true if `candidate` (case-insensitive) is present in the built-in blocklist.
pub fn is_common_password(candidate: &str) -> bool {
    let lower = candidate.to_lowercase();
    COMMON_PASSWORDS.iter().any(|&common| common == lower)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_common_password_exact_match() {
        assert!(is_common_password("password"));
        assert!(is_common_password("123456"));
        assert!(is_common_password("admin"));
    }

    #[test]
    fn test_is_common_password_case_insensitive() {
        assert!(is_common_password("PASSWORD"));
        assert!(is_common_password("Admin"));
        assert!(is_common_password("QWERTY"));
    }

    #[test]
    fn test_is_common_password_not_common() {
        assert!(!is_common_password("xK9mQ!2vL$9xP@7w"));
        assert!(!is_common_password("unique-passphrase-2026"));
    }

    #[test]
    fn test_is_common_password_empty() {
        assert!(!is_common_password(""));
    }
}
