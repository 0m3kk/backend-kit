use crate::errors::AttemptError;
use crate::tracker::{AttemptTracker, AttemptTrackerTx};
use crate::types::{AttemptPolicy, AttemptRecord, AttemptStatus};
use async_trait::async_trait;
use kv_store::{Key, KvStore, KvStoreTx, SetOptions, Value};
use std::time::{Duration, SystemTime};

/// Persistent attempt tracker backed by any `kv_store::KvStore` (Redis, Postgres, Redb, Memory, etc.).
#[derive(Debug, Clone)]
pub struct KvAttemptTracker<S: KvStore> {
    store: S,
    prefix: String,
}

impl<S: KvStore> KvAttemptTracker<S> {
    pub fn new(store: S) -> Self {
        Self {
            store,
            prefix: "attempt_policy".to_string(),
        }
    }

    pub fn with_prefix(store: S, prefix: impl Into<String>) -> Self {
        Self {
            store,
            prefix: prefix.into(),
        }
    }

    fn make_key(&self, key: &str) -> Key {
        Key::from(format!("{}:{}", self.prefix, key))
    }
}

// ---------------------------------------------------------------------------
// Shared helpers (pure logic, no I/O)
// ---------------------------------------------------------------------------

fn decode_record(raw: Option<&Value>) -> Option<AttemptRecord> {
    raw.and_then(|v| serde_json::from_slice(&v.0).ok())
}

fn new_record(now: SystemTime) -> AttemptRecord {
    AttemptRecord {
        failed_attempts: 0,
        first_failed_at: now,
        last_failed_at: now,
        locked_until: None,
    }
}

fn evaluate_status(
    record: Option<AttemptRecord>,
    policy: &AttemptPolicy,
    now: SystemTime,
) -> AttemptStatus {
    let Some(record) = record else {
        return AttemptStatus::Allowed {
            attempts_made: 0,
            attempts_remaining: policy.max_attempts,
        };
    };

    if let Some(locked_until) = record.locked_until
        && now < locked_until
    {
        let retry_after = locked_until.duration_since(now).unwrap_or(Duration::ZERO);
        return AttemptStatus::LockedOut {
            locked_until,
            retry_after,
        };
    }

    if let Some(win) = policy.window_duration
        && let Ok(elapsed) = now.duration_since(record.last_failed_at)
        && elapsed >= win
    {
        return AttemptStatus::Allowed {
            attempts_made: 0,
            attempts_remaining: policy.max_attempts,
        };
    }

    let attempts_remaining = policy.max_attempts.saturating_sub(record.failed_attempts);
    AttemptStatus::Allowed {
        attempts_made: record.failed_attempts,
        attempts_remaining,
    }
}

fn apply_failed_attempt(
    record: Option<AttemptRecord>,
    policy: &AttemptPolicy,
    now: SystemTime,
) -> (AttemptRecord, AttemptStatus) {
    let mut record = record.unwrap_or_else(|| new_record(now));

    if let Some(locked_until) = record.locked_until {
        if now < locked_until {
            let retry_after = locked_until.duration_since(now).unwrap_or(Duration::ZERO);
            return (
                record,
                AttemptStatus::LockedOut {
                    locked_until,
                    retry_after,
                },
            );
        } else {
            record.failed_attempts = 0;
            record.first_failed_at = now;
            record.locked_until = None;
        }
    }

    if let Some(win) = policy.window_duration
        && let Ok(elapsed) = now.duration_since(record.last_failed_at)
        && elapsed >= win
    {
        record.failed_attempts = 0;
        record.first_failed_at = now;
    }

    record.failed_attempts += 1;
    record.last_failed_at = now;

    let status = if record.failed_attempts >= policy.max_attempts {
        let locked_until = now + policy.lockout_duration;
        record.locked_until = Some(locked_until);
        AttemptStatus::LockedOut {
            locked_until,
            retry_after: policy.lockout_duration,
        }
    } else {
        let attempts_remaining = policy.max_attempts.saturating_sub(record.failed_attempts);
        AttemptStatus::Allowed {
            attempts_made: record.failed_attempts,
            attempts_remaining,
        }
    };

    (record, status)
}

fn encode_record(record: &AttemptRecord) -> Result<Vec<u8>, AttemptError> {
    serde_json::to_vec(record).map_err(|e| AttemptError::StorageError(e.to_string()))
}

// ---------------------------------------------------------------------------
// AttemptTracker impl (non-transactional, uses KvStore trait)
// ---------------------------------------------------------------------------

#[async_trait]
impl<S: KvStore> AttemptTracker for KvAttemptTracker<S> {
    async fn check_status(
        &self,
        key: &str,
        policy: &AttemptPolicy,
    ) -> Result<AttemptStatus, AttemptError> {
        let full_key = self.make_key(key);
        let now = SystemTime::now();

        let raw = self
            .store
            .get(&full_key)
            .await
            .map_err(|e| AttemptError::StorageError(e.to_string()))?;

        let record = decode_record(raw.as_ref());
        Ok(evaluate_status(record, policy, now))
    }

    async fn record_failed_attempt(
        &self,
        key: &str,
        policy: &AttemptPolicy,
    ) -> Result<AttemptStatus, AttemptError> {
        let full_key = self.make_key(key);
        let now = SystemTime::now();

        let raw = self
            .store
            .get(&full_key)
            .await
            .map_err(|e| AttemptError::StorageError(e.to_string()))?;

        let existing = decode_record(raw.as_ref());
        let (record, status) = apply_failed_attempt(existing, policy, now);

        let json_bytes = encode_record(&record)?;
        let options = SetOptions::new().with_ttl(policy.lockout_duration);

        self.store
            .set(full_key, Value::new(json_bytes), options)
            .await
            .map_err(|e| AttemptError::StorageError(e.to_string()))?;

        Ok(status)
    }

    async fn record_success(&self, key: &str) -> Result<(), AttemptError> {
        let full_key = self.make_key(key);
        self.store
            .delete(&full_key)
            .await
            .map_err(|e| AttemptError::StorageError(e.to_string()))?;
        Ok(())
    }

    async fn reset(&self, key: &str) -> Result<(), AttemptError> {
        self.record_success(key).await
    }
}

// ---------------------------------------------------------------------------
// AttemptTrackerTx impl (uses KvStoreTx<Conn> trait)
// ---------------------------------------------------------------------------

#[async_trait]
impl<Conn: Send, S: KvStore + KvStoreTx<Conn>> AttemptTrackerTx<Conn> for KvAttemptTracker<S> {
    async fn check_status_tx(
        &self,
        conn: &mut Conn,
        key: &str,
        policy: &AttemptPolicy,
    ) -> Result<AttemptStatus, AttemptError> {
        let full_key = self.make_key(key);
        let now = SystemTime::now();

        let raw = self
            .store
            .get_tx(conn, &full_key)
            .await
            .map_err(|e| AttemptError::StorageError(e.to_string()))?;

        let record = decode_record(raw.as_ref());
        Ok(evaluate_status(record, policy, now))
    }

    async fn record_failed_attempt_tx(
        &self,
        conn: &mut Conn,
        key: &str,
        policy: &AttemptPolicy,
    ) -> Result<AttemptStatus, AttemptError> {
        let full_key = self.make_key(key);
        let now = SystemTime::now();

        let raw = self
            .store
            .get_tx(conn, &full_key)
            .await
            .map_err(|e| AttemptError::StorageError(e.to_string()))?;

        let existing = decode_record(raw.as_ref());
        let (record, status) = apply_failed_attempt(existing, policy, now);

        let json_bytes = encode_record(&record)?;
        let options = SetOptions::new().with_ttl(policy.lockout_duration);

        self.store
            .set_tx(conn, full_key, Value::new(json_bytes), options)
            .await
            .map_err(|e| AttemptError::StorageError(e.to_string()))?;

        Ok(status)
    }

    async fn record_success_tx(&self, conn: &mut Conn, key: &str) -> Result<(), AttemptError> {
        let full_key = self.make_key(key);
        self.store
            .delete_tx(conn, &full_key)
            .await
            .map_err(|e| AttemptError::StorageError(e.to_string()))?;
        Ok(())
    }

    async fn reset_tx(&self, conn: &mut Conn, key: &str) -> Result<(), AttemptError> {
        self.record_success_tx(conn, key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AttemptPolicy;
    use std::time::{Duration, SystemTime};

    fn test_policy() -> AttemptPolicy {
        AttemptPolicy {
            max_attempts: 3,
            lockout_duration: Duration::from_secs(60),
            window_duration: Some(Duration::from_secs(300)),
            reset_on_success: true,
        }
    }

    fn test_record(failed_attempts: u32, now: SystemTime) -> AttemptRecord {
        AttemptRecord {
            failed_attempts,
            first_failed_at: now - Duration::from_secs(10),
            last_failed_at: now,
            locked_until: None,
        }
    }

    // -----------------------------------------------------------------------
    // decode_record
    // -----------------------------------------------------------------------

    #[test]
    fn test_decode_record_none_input() {
        assert!(decode_record(None).is_none());
    }

    #[test]
    fn test_decode_record_valid_json() {
        let now = SystemTime::now();
        let record = new_record(now);
        let bytes = serde_json::to_vec(&record).unwrap();
        let value = Value::new(bytes);
        let decoded = decode_record(Some(&value));
        assert!(decoded.is_some());
        assert_eq!(decoded.unwrap().failed_attempts, 0);
    }

    #[test]
    fn test_decode_record_invalid_json() {
        let value = Value::new(b"not-json".to_vec());
        assert!(decode_record(Some(&value)).is_none());
    }

    // -----------------------------------------------------------------------
    // new_record
    // -----------------------------------------------------------------------

    #[test]
    fn test_new_record_defaults() {
        let now = SystemTime::now();
        let record = new_record(now);
        assert_eq!(record.failed_attempts, 0);
        assert_eq!(record.first_failed_at, now);
        assert_eq!(record.last_failed_at, now);
        assert!(record.locked_until.is_none());
    }

    // -----------------------------------------------------------------------
    // encode_record / decode_record roundtrip
    // -----------------------------------------------------------------------

    #[test]
    fn test_encode_decode_roundtrip() {
        let now = SystemTime::now();
        let original = AttemptRecord {
            failed_attempts: 5,
            first_failed_at: now - Duration::from_secs(100),
            last_failed_at: now,
            locked_until: Some(now + Duration::from_secs(60)),
        };
        let bytes = encode_record(&original).unwrap();
        let value = Value::new(bytes);
        let decoded = decode_record(Some(&value)).unwrap();
        assert_eq!(decoded.failed_attempts, original.failed_attempts);
        assert_eq!(decoded.locked_until, original.locked_until);
    }

    // -----------------------------------------------------------------------
    // evaluate_status
    // -----------------------------------------------------------------------

    #[test]
    fn test_evaluate_status_no_record() {
        let policy = test_policy();
        let now = SystemTime::now();
        let status = evaluate_status(None, &policy, now);
        assert_eq!(
            status,
            AttemptStatus::Allowed {
                attempts_made: 0,
                attempts_remaining: 3,
            }
        );
    }

    #[test]
    fn test_evaluate_status_under_limit() {
        let policy = test_policy();
        let now = SystemTime::now();
        let record = test_record(2, now);
        let status = evaluate_status(Some(record), &policy, now);
        assert_eq!(
            status,
            AttemptStatus::Allowed {
                attempts_made: 2,
                attempts_remaining: 1,
            }
        );
    }

    #[test]
    fn test_evaluate_status_locked_out() {
        let policy = test_policy();
        let now = SystemTime::now();
        let locked_until = now + Duration::from_secs(30);
        let record = AttemptRecord {
            failed_attempts: 3,
            first_failed_at: now - Duration::from_secs(10),
            last_failed_at: now,
            locked_until: Some(locked_until),
        };
        let status = evaluate_status(Some(record), &policy, now);
        assert!(status.is_locked());
    }

    #[test]
    fn test_evaluate_status_lockout_expired() {
        let policy = test_policy();
        let now = SystemTime::now();
        let locked_until = now - Duration::from_secs(1); // already expired
        let record = AttemptRecord {
            failed_attempts: 3,
            first_failed_at: now - Duration::from_secs(100),
            last_failed_at: now - Duration::from_secs(10),
            locked_until: Some(locked_until),
        };
        // Expired lockout means we fall through to check attempts
        let status = evaluate_status(Some(record), &policy, now);
        assert!(status.is_allowed());
    }

    #[test]
    fn test_evaluate_status_window_expired() {
        let policy = test_policy(); // window_duration = 300s
        let now = SystemTime::now();
        let record = AttemptRecord {
            failed_attempts: 2,
            first_failed_at: now - Duration::from_secs(600),
            last_failed_at: now - Duration::from_secs(400), // > 300s ago
            locked_until: None,
        };
        let status = evaluate_status(Some(record), &policy, now);
        assert_eq!(
            status,
            AttemptStatus::Allowed {
                attempts_made: 0,
                attempts_remaining: 3,
            }
        );
    }

    #[test]
    fn test_evaluate_status_no_window() {
        let policy = AttemptPolicy {
            window_duration: None,
            ..test_policy()
        };
        let now = SystemTime::now();
        let record = AttemptRecord {
            failed_attempts: 2,
            first_failed_at: now - Duration::from_secs(600),
            last_failed_at: now - Duration::from_secs(400),
            locked_until: None,
        };
        // No window means old attempts still count
        let status = evaluate_status(Some(record), &policy, now);
        assert_eq!(
            status,
            AttemptStatus::Allowed {
                attempts_made: 2,
                attempts_remaining: 1,
            }
        );
    }

    // -----------------------------------------------------------------------
    // apply_failed_attempt
    // -----------------------------------------------------------------------

    #[test]
    fn test_apply_failed_attempt_first_attempt() {
        let policy = test_policy();
        let now = SystemTime::now();
        let (record, status) = apply_failed_attempt(None, &policy, now);
        assert_eq!(record.failed_attempts, 1);
        assert_eq!(record.first_failed_at, now);
        assert_eq!(record.last_failed_at, now);
        assert!(record.locked_until.is_none());
        assert_eq!(
            status,
            AttemptStatus::Allowed {
                attempts_made: 1,
                attempts_remaining: 2,
            }
        );
    }

    #[test]
    fn test_apply_failed_attempt_increments() {
        let policy = test_policy();
        let now = SystemTime::now();
        let existing = test_record(1, now - Duration::from_secs(5));
        let (record, status) = apply_failed_attempt(Some(existing), &policy, now);
        assert_eq!(record.failed_attempts, 2);
        assert_eq!(record.last_failed_at, now);
        assert_eq!(
            status,
            AttemptStatus::Allowed {
                attempts_made: 2,
                attempts_remaining: 1,
            }
        );
    }

    #[test]
    fn test_apply_failed_attempt_triggers_lockout() {
        let policy = test_policy(); // max_attempts = 3
        let now = SystemTime::now();
        let existing = test_record(2, now - Duration::from_secs(5));
        let (record, status) = apply_failed_attempt(Some(existing), &policy, now);
        assert_eq!(record.failed_attempts, 3);
        assert!(record.locked_until.is_some());
        assert!(status.is_locked());
        if let AttemptStatus::LockedOut { retry_after, .. } = status {
            assert_eq!(retry_after, policy.lockout_duration);
        }
    }

    #[test]
    fn test_apply_failed_attempt_while_locked() {
        let policy = test_policy();
        let now = SystemTime::now();
        let locked_until = now + Duration::from_secs(30);
        let existing = AttemptRecord {
            failed_attempts: 3,
            first_failed_at: now - Duration::from_secs(10),
            last_failed_at: now - Duration::from_secs(5),
            locked_until: Some(locked_until),
        };
        let (record, status) = apply_failed_attempt(Some(existing), &policy, now);
        // Should NOT increment, just return locked status
        assert_eq!(record.failed_attempts, 3);
        assert!(status.is_locked());
    }

    #[test]
    fn test_apply_failed_attempt_after_lockout_expired() {
        let policy = test_policy();
        let now = SystemTime::now();
        let locked_until = now - Duration::from_secs(1); // expired
        let existing = AttemptRecord {
            failed_attempts: 3,
            first_failed_at: now - Duration::from_secs(100),
            last_failed_at: now - Duration::from_secs(10),
            locked_until: Some(locked_until),
        };
        let (record, status) = apply_failed_attempt(Some(existing), &policy, now);
        // Should reset counter and start fresh
        assert_eq!(record.failed_attempts, 1);
        assert_eq!(record.first_failed_at, now);
        assert!(record.locked_until.is_none());
        assert!(status.is_allowed());
    }

    #[test]
    fn test_apply_failed_attempt_window_expired_resets() {
        let policy = test_policy(); // window = 300s
        let now = SystemTime::now();
        let existing = AttemptRecord {
            failed_attempts: 2,
            first_failed_at: now - Duration::from_secs(600),
            last_failed_at: now - Duration::from_secs(400), // > 300s ago
            locked_until: None,
        };
        let (record, status) = apply_failed_attempt(Some(existing), &policy, now);
        // Window expired — counter resets, this becomes attempt #1
        assert_eq!(record.failed_attempts, 1);
        assert_eq!(record.first_failed_at, now);
        assert!(status.is_allowed());
    }

    // -----------------------------------------------------------------------
    // KvAttemptTracker::make_key
    // -----------------------------------------------------------------------

    #[test]
    fn test_make_key_default_prefix() {
        let tracker = KvAttemptTracker::new(kv_store::memory::MemoryKvStore::new());
        let key = tracker.make_key("user@example.com");
        assert_eq!(key.to_string(), "attempt_policy:user@example.com");
    }

    #[test]
    fn test_make_key_custom_prefix() {
        let tracker =
            KvAttemptTracker::with_prefix(kv_store::memory::MemoryKvStore::new(), "login_attempts");
        let key = tracker.make_key("user@example.com");
        assert_eq!(key.to_string(), "login_attempts:user@example.com");
    }
}
