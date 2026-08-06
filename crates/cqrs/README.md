# cqrs

Command Query Responsibility Segregation (CQRS) crate for **backend-kit**, providing a generalized 5-step Command execution pipeline (`Command<M, C>`), multi-model hydration (`DecisionModels`), snapshot-backed execution (`dispatch_command_with_snapshot`), object-safe read views (`View<C>`), transactional checkpoint tracking (`CheckpointStore<C>`), on-demand queries (`ViewQueryEngine`), and parallel multi-view background workers (`CatchupWorker`).

## Overview

In CQRS architectures, system mutations (Commands) are decoupled from query read views (Views):

- **Command Side**: Validates and normalizes payload, hydrates decision model(s) from `EventStore` (with optional KV snapshots), evaluates business logic, and commits domain events using Dynamic Consistency Boundaries (DCB).
- **Read Side**: Projects domain events into database read tables (`View<C>`), updates progress checkpoints (`CheckpointStore<C>`), and executes parallel catch-ups (`CatchupWorker`).

---

## The 5-Step Command Pipeline

| Step                   | Responsible Party                  | Method / Function                       | Description                                                                 |
| :--------------------- | :--------------------------------- | :-------------------------------------- | :-------------------------------------------------------------------------- |
| **1. Validate**        | Developer (`Command`)              | `command.validate()`                    | Validates payload syntax, non-empty IDs, format constraints                 |
| **2. Normalize**       | Developer (`Command`)              | `command.normalize()`                   | Sanitizes inputs (e.g. `.trim()`, lowercasing emails)                       |
| **3. Load Model**      | **Framework** (`dispatch_command`) | `event_store.load_decision_model(...)`  | Hydrates `LoadedModels<M>` (with optional $O(1)$ KV snapshot loading)       |
| **4. Domain Decision** | Developer (`Command`)              | `command.decide(&models, ctx)`          | Evaluates domain rules against loaded state; returns `Vec<Event>`           |
| **5. Save Events**     | **Framework** (`dispatch_command`) | `event_store.append(events, condition)` | Atomically commits events with `AppendCondition::new(query).after_opt(...)` |

---

## Core Component Index

| CQRS Component            | Rust Type / Trait in `cqrs`                        | Description                                                                                   |
| :------------------------ | :------------------------------------------------- | :-------------------------------------------------------------------------------------------- |
| **Command Trait**         | [`Command<M, C>`](src/command.rs)                  | Generalized trait for CQRS commands targeting decision model(s) `M`                           |
| **Decision Models Trait** | [`DecisionModels`](src/command.rs)                 | Re-exported trait from `event-sourcing` implemented for `Single<M>`, `Pair`, `Triple`, `Quad` |
| **Standard Dispatcher**   | [`dispatch_command`](src/command.rs)               | Runner executing the 5-step Command lifecycle automatically                                   |
| **Snapshot Dispatcher**   | [`dispatch_command_with_snapshot`](src/command.rs) | Runner with $O(1)$ KV snapshot loading and threshold auto-saving                              |
| **Object-Safe View**      | [`View<C>`](src/view.rs)                           | Unified trait representing a read model and its event projection logic                        |
| **Checkpoint Store**      | [`CheckpointStore<C>`](src/view_checkpoint.rs)     | Interface for retrieving and committing sequence positions using context `C`                  |
| **KV Checkpoint Adapter** | [`KvCheckpointStore<K>`](src/view_checkpoint.rs)   | Adapter backing `CheckpointStore<C>` using any `KvStore` (`kv-store-postgres`, Redis)         |
| **Read Consistency**      | [`ReadConsistency`](src/query.rs)                  | Enum specifying consistency requirement (`Eventual`, `Strong`)                                |
| **View Query Engine**     | [`ViewQueryEngine`](src/query.rs)                  | Query executor providing on-demand catch-up to the latest Event Store head position           |
| **Multi-View Worker**     | [`CatchupWorker`](src/catchup_worker.rs)           | Concurrent worker managing a list of `Box<dyn View<C>>` views catching them up in parallel    |

---

## Command Usage Examples

### 1. Single-Model Command (`Single<M>`)

```rust
use cqrs::{Command, CommandError, Single};
use event_sourcing::{DecisionModel, DomainEvent, Event, EventId, EventType, Query};
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
pub struct UserRegistrationModel {
    pub email: String,
    pub is_registered: bool,
}

impl DecisionModel for UserRegistrationModel {
    fn query(&self) -> Query {
        Query::all()
    }
    fn apply_event(&mut self, _event: &Event) {
        self.is_registered = true;
    }
}

#[derive(Serialize, Deserialize)]
pub struct UserRegistered {
    pub user_id: String,
    pub email: String,
}

impl DomainEvent for UserRegistered {
    fn event_type() -> EventType {
        EventType::new("UserRegistered")
    }
}

pub struct RegisterUserCommand {
    pub user_id: String,
    pub email: String,
}

impl Command<Single<UserRegistrationModel>> for RegisterUserCommand {
    type Error = CommandError;

    fn validate(&self) -> Result<(), CommandError> {
        if self.user_id.is_empty() {
            return Err(CommandError::Validation("User ID cannot be empty".to_string()));
        }
        Ok(())
    }

    fn normalize(&mut self) -> Result<(), CommandError> {
        self.user_id = self.user_id.trim().to_string();
        self.email = self.email.trim().to_lowercase();
        Ok(())
    }

    fn models(&self) -> Single<UserRegistrationModel> {
        Single(UserRegistrationModel { email: self.email.clone(), is_registered: false })
    }

    fn decide(&self, model: &UserRegistrationModel, _ctx: &()) -> Result<Vec<Event>, CommandError> {
        if model.is_registered {
            return Err(CommandError::Decision("Email already registered".to_string()));
        }

        let event = UserRegistered { user_id: self.user_id.clone(), email: self.email.clone() }
            .to_event(EventId::new("evt-1"))
            .map_err(|e| CommandError::Decision(e.to_string()))?;

        Ok(vec![event])
    }
}
```

---

### 2. Multi-Model Command (`Pair<M1, M2>`)

Commands loading multiple decision models as type-safe tuples (e.g. Money Transfer loading two bank accounts):

```rust
use cqrs::{Command, CommandError, Pair};
use event_sourcing::{DecisionModel, DomainEvent, Event, EventId, EventType, Query};
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
pub struct BankAccountModel {
    pub account_id: String,
    pub balance: u64,
}

impl BankAccountModel {
    pub fn new(id: &str) -> Self {
        Self { account_id: id.to_string(), balance: 100 }
    }
}

impl DecisionModel for BankAccountModel {
    fn query(&self) -> Query { Query::all() }
    fn apply_event(&mut self, _event: &Event) {}
}

#[derive(Serialize, Deserialize)]
pub struct MoneyWithdrawn { pub account_id: String, pub amount: u64 }
impl DomainEvent for MoneyWithdrawn { fn event_type() -> EventType { EventType::new("MoneyWithdrawn") } }

#[derive(Serialize, Deserialize)]
pub struct MoneyDeposited { pub account_id: String, pub amount: u64 }
impl DomainEvent for MoneyDeposited { fn event_type() -> EventType { EventType::new("MoneyDeposited") } }

pub struct TransferMoneyCommand {
    pub from_account_id: String,
    pub to_account_id: String,
    pub amount: u64,
}

impl Command<Pair<BankAccountModel, BankAccountModel>> for TransferMoneyCommand {
    type Error = CommandError;

    fn models(&self) -> Pair<BankAccountModel, BankAccountModel> {
        Pair(
            BankAccountModel::new(&self.from_account_id),
            BankAccountModel::new(&self.to_account_id),
        )
    }

    fn decide(
        &self,
        (from_acc, _to_acc): &(BankAccountModel, BankAccountModel),
        _ctx: &(),
    ) -> Result<Vec<Event>, CommandError> {
        if from_acc.balance < self.amount {
            return Err(CommandError::Decision("Insufficient funds".to_string()));
        }

        let ev1 = MoneyWithdrawn { account_id: self.from_account_id.clone(), amount: self.amount }
            .to_event(EventId::new("ev-1"))
            .map_err(|e| CommandError::Decision(e.to_string()))?;
        let ev2 = MoneyDeposited { account_id: self.to_account_id.clone(), amount: self.amount }
            .to_event(EventId::new("ev-2"))
            .map_err(|e| CommandError::Decision(e.to_string()))?;

        Ok(vec![ev1, ev2])
    }
}
```

---

### 3. Snapshot-Backed Command Execution (`dispatch_command_with_snapshot`)

Loads decision models from KV store in $O(1)$ time, catches up remaining events, and auto-saves updated snapshots when `threshold` is met:

```rust
use cqrs::dispatch_command_with_snapshot;
use event_sourcing::snapshot::SnapshotOptions;

pub async fn run_snapshot_dispatch<ES, KS, C, M>(
    cmd: C,
    event_store: &ES,
    kv_store: &KS,
) -> Result<Vec<event_sourcing::SequencedEvent>, cqrs::CommandError>
where
    ES: event_sourcing::EventStore,
    KS: kv_store::KvStore,
    M: cqrs::DecisionModels,
    C: cqrs::Command<M, Error = cqrs::CommandError>,
{
    let snapshot_options = SnapshotOptions::new(10); // auto-snapshot every 10 events
    let appended = dispatch_command_with_snapshot(cmd, event_store, kv_store, snapshot_options, &()).await?;
    Ok(appended)
}
```

---

## Read Side View Pipeline (`View<C>`)

```rust
use cqrs::{CatchupWorker, View, ViewError};
use event_sourcing::{Query, SequencedEvent};
use async_trait::async_trait;

pub struct UserProfileView;

#[async_trait]
impl<C: Send + Sync> View<C> for UserProfileView {
    fn view_name(&self) -> &'static str {
        "UserProfileView"
    }
    fn subscription_query(&self) -> Query {
        Query::all()
    }
    async fn apply_event(&self, event: &SequencedEvent, _ctx: &C) -> Result<(), ViewError> {
        println!("Projected event {}", event.position);
        Ok(())
    }
}

pub async fn start_worker<C, ES, CP>(ctx: C, event_store: ES, checkpoint_store: CP)
where
    C: Send + Sync + 'static,
    ES: event_sourcing::EventStore + 'static,
    CP: cqrs::CheckpointStore<C> + 'static,
{
    let worker = CatchupWorker::new(ctx, event_store, checkpoint_store)
        .register_view(UserProfileView);

    // Run parallel catch-up background loop:
    tokio::spawn(worker.run_loop(std::time::Duration::from_secs(1)));
}
```

---

## License

Licensed under MIT.
