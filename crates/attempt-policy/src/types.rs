use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

/// Configuration policy defining maximum allowed failed attempts and lockout durations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttemptPolicy {
    /// Maximum allowed consecutive failed attempts before triggering lockout.
    pub max_attempts: u32,
    /// Lockout duration during which further attempts are blocked.
    pub lockout_duration: Duration,
    /// Optional sliding observation window duration.
    /// Failed attempts older than this window are ignored or reset.
    pub window_duration: Option<Duration>,
    /// Automatically reset the failed attempts counter on successful verification/authentication.
    pub reset_on_success: bool,
}

impl Default for AttemptPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            lockout_duration: Duration::from_secs(900), // 15 minutes
            window_duration: Some(Duration::from_secs(900)),
            reset_on_success: true,
        }
    }
}

impl AttemptPolicy {
    /// Fluent builder for constructing `AttemptPolicy`.
    pub fn builder() -> AttemptPolicyBuilder {
        AttemptPolicyBuilder::new()
    }

    /// Strict policy for high-security credentials (3 attempts, 30 min lockout).
    pub fn strict() -> Self {
        Self {
            max_attempts: 3,
            lockout_duration: Duration::from_secs(1800),
            window_duration: Some(Duration::from_secs(1800)),
            reset_on_success: true,
        }
    }

    /// Relaxed policy (10 attempts, 5 min lockout).
    pub fn relaxed() -> Self {
        Self {
            max_attempts: 10,
            lockout_duration: Duration::from_secs(300),
            window_duration: Some(Duration::from_secs(300)),
            reset_on_success: true,
        }
    }
}

/// Fluent builder for constructing `AttemptPolicy`.
#[derive(Debug, Clone, Default)]
pub struct AttemptPolicyBuilder {
    policy: AttemptPolicy,
}

impl AttemptPolicyBuilder {
    pub fn new() -> Self {
        Self {
            policy: AttemptPolicy::default(),
        }
    }

    pub fn max_attempts(mut self, max: u32) -> Self {
        self.policy.max_attempts = max;
        self
    }

    pub fn lockout_duration(mut self, duration: Duration) -> Self {
        self.policy.lockout_duration = duration;
        self
    }

    pub fn window_duration(mut self, duration: Option<Duration>) -> Self {
        self.policy.window_duration = duration;
        self
    }

    pub fn reset_on_success(mut self, reset: bool) -> Self {
        self.policy.reset_on_success = reset;
        self
    }

    pub fn build(self) -> AttemptPolicy {
        self.policy
    }
}

/// Outcome of checking or recording an attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttemptStatus {
    /// Attempt allowed.
    Allowed {
        attempts_made: u32,
        attempts_remaining: u32,
    },
    /// Key/identifier is locked out due to exceeding maximum allowed attempts.
    LockedOut {
        locked_until: SystemTime,
        retry_after: Duration,
    },
}

impl AttemptStatus {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed { .. })
    }

    pub fn is_locked(&self) -> bool {
        matches!(self, Self::LockedOut { .. })
    }
}

/// State tracking record for an identifier's attempt history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttemptRecord {
    pub failed_attempts: u32,
    pub first_failed_at: SystemTime,
    pub last_failed_at: SystemTime,
    pub locked_until: Option<SystemTime>,
}
