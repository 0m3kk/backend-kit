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
