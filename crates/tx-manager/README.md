# tx-manager

Production-grade **Transaction Manager** and **Runner** abstractions for **backend-kit**, providing unified multi-store transactional coordination, automatic PostgreSQL `SERIALIZABLE` retries with exponential backoff, and strict ACID guarantees across events, key-value caches, and secrets.

---

## Key Features

- **Unified Multi-Store Coordination**: Coordinates [`PostgresEventStore`](../event-store-postgres), [`PostgresKvStore`](../kv-store-postgres), and [`PostgresSecretStore`](../secret-store-postgres) within a single PostgreSQL transaction.
- **ACID Guarantees**: All domain events, key-value mutations, and secret updates commit or rollback together in a single `COMMIT` / `ROLLBACK`.
- **Automatic Conflict Retries**: Built-in exponential backoff for transient serialization errors (`40001` serialization failure and `40P01` deadlock).
- **Configurable Isolation**: Supports `ReadCommitted`, `RepeatableRead`, and `Serializable` isolation levels.
- **Closure-Based Lifecycle**: `run(...)` automatically manages transaction begin, execution, commit, rollback on error, and backoff retries.
- **TransactionProvider Trait**: Standard interface implemented for `sqlx::PgPool` and `Arc<T>` used by background workers (e.g. CQRS `CatchupWorkerTx`).

---

## Architecture

```mermaid
flowchart TD
    Runner[PostgresTransactionRunner] -->|BEGIN TX| DB[(PostgreSQL)]
    Runner -->|Creates| Ctx[PostgresTxContext]
    
    subgraph Single Postgres Transaction
        Ctx -->|append_tx| Events[PostgresEventStore]
        Ctx -->|set_tx| KV[PostgresKvStore]
        Ctx -->|set_tx| Secrets[PostgresSecretStore]
    end
    
    Ctx -->|COMMIT / ROLLBACK| DB
```

---

## Usage Example

```rust
use sqlx::PgPool;
use tx_manager::{PostgresTransactionRunner, IsolationLevel, RetryPolicy};
use event_sourcing::Event;
use kv_store::{Key, Value, SetOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = PgPool::connect("postgres://postgres:postgres@localhost/db").await?;
    let runner = PostgresTransactionRunner::new(pool)?
        .with_isolation_level(IsolationLevel::Serializable);

    // All operations inside the closure execute in ONE atomic PostgreSQL transaction
    let result = runner.run(|ctx| Box::pin(async move {
        // 1. Append domain events
        let event = Event::new("order-123", "OrderCreated", serde_json::json!({"total": 100}), vec![]);
        ctx.append_events(&[event], None).await?;

        // 2. Update KV cache
        ctx.set_kv(Key::from("order:order-123"), Value::from("active"), SetOptions::default()).await?;

        Ok::<_, Box<dyn std::error::Error + Send + Sync>>("Order processed successfully")
    })).await?;

    println!("Result: {result}");
    Ok(())
}
```

---

## License

Dual-licensed under MIT or Apache-2.0.
