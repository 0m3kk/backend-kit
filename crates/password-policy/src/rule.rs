use crate::types::{PolicyViolation, UserContext};

/// Extensible validation rule interface for custom password policy checks.
pub trait Rule: Send + Sync {
    /// Name or identifier for this rule.
    fn name(&self) -> &str;

    /// Evaluate password against this rule. Returns `Some(PolicyViolation)` if rule fails.
    fn check(&self, password: &str, context: Option<&UserContext>) -> Option<PolicyViolation>;
}

/// Helper function to detect sequential patterns (numeric, alphabetic, or keyboard row sequences).
pub fn find_sequential_pattern(password: &str, min_seq_len: usize) -> Option<String> {
    let lower = password.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();

    if chars.len() < min_seq_len {
        return None;
    }

    // Keyboard sequence rows
    let keyboard_rows = [
        "qwertyuiop",
        "asdfghjkl",
        "zxcvbnm",
        "1234567890",
        "0987654321",
        "poiuytrewq",
        "lkjhgfdsa",
        "mnbvcxz",
    ];

    for row in keyboard_rows {
        for window in row.chars().collect::<Vec<char>>().windows(min_seq_len) {
            let seq: String = window.iter().collect();
            if lower.contains(&seq) {
                return Some(seq);
            }
        }
    }

    // Check ASCII numeric or alphabetic ascending / descending sequences
    let mut seq_len = 1;
    let mut start_idx = 0;

    for i in 1..chars.len() {
        let prev = chars[i - 1] as i32;
        let curr = chars[i] as i32;

        if curr == prev + 1 || curr == prev - 1 {
            if seq_len == 1 {
                start_idx = i - 1;
            }
            seq_len += 1;
            if seq_len >= min_seq_len {
                let seq: String = chars[start_idx..=i].iter().collect();
                return Some(seq);
            }
        } else {
            seq_len = 1;
        }
    }

    None
}

/// Helper function to detect consecutive repetitive characters (e.g., "aaaa").
pub fn find_repetitive_pattern(password: &str, max_repeat: usize) -> Option<(char, usize)> {
    let mut current_char = '\0';
    let mut current_count = 0;

    for c in password.chars() {
        if c == current_char {
            current_count += 1;
            if current_count > max_repeat {
                return Some((current_char, current_count));
            }
        } else {
            current_char = c;
            current_count = 1;
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- find_sequential_pattern tests ---

    #[test]
    fn test_sequential_keyboard_row() {
        assert!(find_sequential_pattern("myqwertypass", 3).is_some());
        let result = find_sequential_pattern("myqwertypass", 5).unwrap();
        assert!(result.contains("qwert"));
    }

    #[test]
    fn test_sequential_numeric_ascending() {
        let result = find_sequential_pattern("abc12345xyz", 3);
        assert!(result.is_some());
    }

    #[test]
    fn test_sequential_numeric_descending() {
        let result = find_sequential_pattern("abc98765xyz", 3);
        assert!(result.is_some());
    }

    #[test]
    fn test_sequential_alpha_ascending() {
        let result = find_sequential_pattern("xxabcdexx", 4);
        assert!(result.is_some());
    }

    #[test]
    fn test_no_sequential_pattern() {
        let result = find_sequential_pattern("xk9mQ2vL", 3);
        assert!(result.is_none());
    }

    #[test]
    fn test_sequential_too_short_password() {
        assert!(find_sequential_pattern("ab", 3).is_none());
    }

    #[test]
    fn test_sequential_case_insensitive() {
        assert!(find_sequential_pattern("QWERTY", 3).is_some());
    }

    // --- find_repetitive_pattern tests ---

    #[test]
    fn test_repetitive_exceeds_max() {
        let result = find_repetitive_pattern("aaaa", 3);
        assert!(result.is_some());
        let (ch, count) = result.unwrap();
        assert_eq!(ch, 'a');
        assert_eq!(count, 4);
    }

    #[test]
    fn test_repetitive_at_max_not_exceeding() {
        // "aaa" with max_repeat=3 means 3 is allowed, only >3 triggers
        assert!(find_repetitive_pattern("aaa", 3).is_none());
    }

    #[test]
    fn test_repetitive_in_middle() {
        let result = find_repetitive_pattern("ab!!!!cd", 3);
        assert!(result.is_some());
        let (ch, _) = result.unwrap();
        assert_eq!(ch, '!');
    }

    #[test]
    fn test_no_repetitive_pattern() {
        assert!(find_repetitive_pattern("abcdef", 3).is_none());
    }

    #[test]
    fn test_repetitive_empty_password() {
        assert!(find_repetitive_pattern("", 3).is_none());
    }
}
