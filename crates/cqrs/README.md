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
use cqrs::{dispatch_command, Command, CommandError, Single};
use event_sourcing::{DomainEvent, Event, EventId, EventType, SequencedEvent};

#[derive(Debug, thiserror::Error)]
pub enum RegisterUserError {
    #[error("User ID cannot be empty")]
    EmptyUserId,
    #[error("Email '{0}' is already registered")]
    EmailAlreadyRegistered(String),
    #[error(transparent)]
    Infrastructure(#[from] cqrs::CommandError),
}

pub struct RegisterUserCommand {
    pub user_id: String,
    pub email: String,
}

impl Command<Single<UserRegistrationModel>> for RegisterUserCommand {
    type Error = RegisterUserError;

    fn validate(&self) -> Result<(), RegisterUserError> {
        if self.user_id.is_empty() {
            return Err(RegisterUserError::EmptyUserId);
        }
        Ok(())
    }

    fn normalize(&mut self) -> Result<(), CommandError> {
        self.user_id = self.user_id.trim().to_string();
        self.email = self.email.trim().to_lowercase();
        Ok(())
    }

    fn models(&self) -> Single<UserRegistrationModel> {
        Single(UserRegistrationModel::new(&self.email))
    }

    fn decide(&self, model: &UserRegistrationModel, _ctx: &()) -> Result<Vec<Event>, CommandError> {
        if model.is_registered {
            return Err(CommandError::Decision("Email already registered".to_string()));
        }

        let event = UserRegistered { user_id: self.user_id.clone(), email: self.email.clone() }
            .to_event(EventId::new_v4())?;

        Ok(vec![event])
    }
}

// Execution (Framework automatically hydrates model & commits events with DCB concurrency condition):
let appended: Vec<SequencedEvent> = dispatch_command(cmd, &event_store, &()).await?;
```

---

### 2. Multi-Model Command (`Pair<M1, M2>`)

Commands loading multiple decision models as type-safe tuples (e.g. Money Transfer loading two bank accounts):

```rust
use cqrs::{dispatch_command, Command, CommandError, Pair};

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
        (from_acc, to_acc): &(BankAccountModel, BankAccountModel),
        _ctx: &(),
    ) -> Result<Vec<Event>, CommandError> {
        if from_acc.balance < self.amount {
            return Err(CommandError::Decision("Insufficient funds".to_string()));
        }

        // Generate events for both accounts
        let ev1 = MoneyWithdrawn { account_id: self.from_account_id.clone(), amount: self.amount }.to_event(...)?:
        let ev2 = MoneyDeposited { account_id: self.to_account_id.clone(), amount: self.amount }.to_event(...)?:

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

let snapshot_options = SnapshotOptions::new(10); // auto-snapshot every 10 events
let appended = dispatch_command_with_snapshot(cmd, &event_store, &kv_store, snapshot_options, &()).await?;
```

---

## Read Side View Pipeline (`View<C>`)

```rust
use cqrs::{CatchupWorker, KvCheckpointStore};
use std::time::Duration;

let worker = CatchupWorker::new(db_context, event_store, checkpoint_store)
    .register_view(UserProfileView::default())
    .register_view(OrderSummaryView::default());

// Run parallel catch-up background loop:
tokio::spawn(worker.run_loop(Duration::from_secs(1)));
```

---

## License

Licensed under MIT.
