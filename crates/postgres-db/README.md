# 🐘 postgres-db

Production-ready PostgreSQL configuration, secrets, connection pooling, and OpenTelemetry monitoring for Rust applications.

---

## ✨ Features

- ⚙️ **Configurable Pool**: Connection pool options ([`PostgresConfig`]) with timeouts, min/max connections, and TCP keepalives.
- 🔒 **Credentials & SSL**: Flexible authentication ([`PostgresSecret`]) supporting SSL modes (Disable, Prefer, Require).
- 📊 **OpenTelemetry Pool Monitoring**: Built-in background monitor ([`spawn_pool_metrics_monitor`]) reporting pool size, idle, and active connections.
- 🛡️ **Zeroize / Redaction**: Sensitive database password strings are safely redacted in debug logs.

---

## 🚀 Quick Start

### 1. Add Dependency

Add `postgres-db` to your `Cargo.toml`:

```toml
[dependencies]
postgres-db = "0.1"
```

### 2. Usage Example

```rust
use postgres_db::{PostgresConfig, PostgresSecret, init_pool, spawn_pool_metrics_monitor};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize configuration and secrets
    let config = PostgresConfig::default();
    let secret = PostgresSecret::default();

    // 2. Connect pool
    let pool = init_pool(&config, &secret).await?;

    // 3. Start pool metrics background monitor (every 15s)
    let _monitor = spawn_pool_metrics_monitor("my-service", pool.clone(), std::time::Duration::from_secs(15));

    // 4. Query the database
    let row: (i32,) = sqlx::query_as("SELECT 1").fetch_one(&pool).await?;
    println!("Database ping result: {}", row.0);

    Ok(())
}
```

---

## 📜 License

Licensed under the [MIT License](LICENSE).
