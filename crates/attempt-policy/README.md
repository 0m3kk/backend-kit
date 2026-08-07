# attempt-policy

Production-grade, generic, type-safe max attempt policy enforcement, exponential lockout tracking, sliding window rate-limiting, and KV store backends for `backend-kit`.

## Overview

`attempt-policy` provides a unified attempt tracking engine designed to prevent brute-force attacks on sensitive endpoints such as password authentication, 2FA/OTP verification, API key access, and rate-limited actions.

## Features

- **Flexible Policy Configuration (`AttemptPolicy`)**:
  - `max_attempts`: Maximum allowed consecutive failed attempts before triggering lockout.
  - `lockout_duration`: Duration for which authentication/verification is blocked when locked out.
  - `window_duration`: Optional sliding window duration for accumulating failed attempts.
  - `reset_on_success`: Automatically reset failed attempts on successful verification.
- **Built-in Presets**:
  - `AttemptPolicy::default()` (5 max attempts, 15 min lockout, 15 min sliding window).
  - `AttemptPolicy::strict()` (3 max attempts, 30 min lockout, 30 min sliding window).
  - `AttemptPolicy::relaxed()` (10 max attempts, 5 min lockout, 5 min sliding window).
  - `AttemptPolicy::builder()` fluent API for custom parameters.
- **Pluggable Storage Tracker (`KvAttemptTracker`)**:
  - Key-value store tracker backed by `kv_store::KvStore` supporting Redis, PostgreSQL (`PostgresKvStore`), Redb, Memory (`MemoryKvStore`), etc. with automatic TTL expiration.
- **High-Level Manager (`AttemptManager`)**:
  - Ergonomic `check_attempt()`, `record_failed_attempt()`, `record_success()`, and `unlock()` workflow.
- **Transactional API (`AttemptTrackerTx<Conn>`)**:
  - All tracker operations available as `_tx` methods accepting an external connection handle. The caller owns the transaction lifecycle (begin/commit/rollback).
  - `AttemptManager` exposes `_tx` variants (`check_attempt_tx`, `record_failed_attempt_tx`, `record_success_tx`, `unlock_tx`) when the tracker implements `AttemptTrackerTx<Conn>`.

## Quick Example

```rust
use attempt_policy::{AttemptError, AttemptManager, AttemptPolicy, KvAttemptTracker};
use kv_store::memory::MemoryKvStore;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Define attempt policy (e.g. max 3 failed attempts, 15 min lockout)
    let policy = AttemptPolicy::builder()
        .max_attempts(3)
        .lockout_duration(Duration::from_secs(900))
        .build();

    // 2. Instantiate AttemptManager with KvAttemptTracker
    let store = MemoryKvStore::new();
    let tracker = KvAttemptTracker::new(store);
    let manager = AttemptManager::new(policy, tracker);
    let user_key = "user@example.com";

    // 3. Pre-check lockout status before running expensive verification
    let status = manager.check_attempt(user_key).await?;
    println!("Status before attempt: {:?}", status);

    // 4. Record failed attempt
    match manager.record_failed_attempt(user_key).await {
        Ok(status) => println!("Attempts made: {:?}", status),
        Err(AttemptError::MaxAttemptsExceeded { retry_after_secs }) => {
            println!("Account locked out! Try again in {} seconds", retry_after_secs);
        }
        Err(e) => eprintln!("Error: {}", e),
    }

    // 5. On successful login/verification, reset attempts counter
    manager.record_success(user_key).await?;

    Ok(())
}
```

## License

Licensed under MIT.
