use cqrs::{Command, CommandError, Pair, Single, dispatch_command, dispatch_command_with_snapshot};
use event_sourcing::memory::InMemoryEventStore;
use event_sourcing::snapshot::SnapshotOptions;
use event_sourcing::{
    DecisionModel, DomainEvent, Event, EventId, EventType, Query, QueryItem, Tag,
};
use kv_store::KvStore;
use kv_store::memory::MemoryKvStore;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// -----------------------------------------------------------------------------
// Domain Events & Decision Models for Testing
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserRegistered {
    pub user_id: String,
    pub email: String,
}

impl DomainEvent for UserRegistered {
    fn event_type() -> EventType {
        EventType::new("UserRegistered")
    }

    fn tags(&self) -> Vec<Tag> {
        vec![Tag::key_value("email", &self.email)]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct UserRegistrationModel {
    pub email: String,
    pub is_registered: bool,
}

impl UserRegistrationModel {
    pub fn new(email: impl Into<String>) -> Self {
        Self {
            email: email.into(),
            is_registered: false,
        }
    }
}

impl DecisionModel for UserRegistrationModel {
    fn query(&self) -> Query {
        Query::item(
            QueryItem::new()
                .with_type(UserRegistered::event_type())
                .with_tag(Tag::key_value("email", &self.email)),
        )
    }

    fn apply_event(&mut self, event: &Event) {
        if event.event_type == UserRegistered::event_type() {
            self.is_registered = true;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct BankAccountModel {
    pub account_id: String,
    pub balance: u64,
}

impl BankAccountModel {
    pub fn new(account_id: impl Into<String>) -> Self {
        Self {
            account_id: account_id.into(),
            balance: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MoneyDeposited {
    pub account_id: String,
    pub amount: u64,
}

impl DomainEvent for MoneyDeposited {
    fn event_type() -> EventType {
        EventType::new("MoneyDeposited")
    }

    fn tags(&self) -> Vec<Tag> {
        vec![Tag::key_value("account", &self.account_id)]
    }
}

impl DecisionModel for BankAccountModel {
    fn query(&self) -> Query {
        Query::item(
            QueryItem::new()
                .with_type(MoneyDeposited::event_type())
                .with_tag(Tag::key_value("account", &self.account_id)),
        )
    }

    fn apply_event(&mut self, event: &Event) {
        if event.event_type == MoneyDeposited::event_type() {
            if let Ok(payload) = serde_json::from_value::<MoneyDeposited>(event.data.clone()) {
                self.balance += payload.amount;
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Custom Owned Domain Error Types
// -----------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum RegisterUserError {
    #[error("User ID cannot be empty")]
    EmptyUserId,

    #[error("Invalid email address format")]
    InvalidEmail,

    #[error("Email '{0}' is already registered")]
    EmailAlreadyRegistered(String),

    #[error(transparent)]
    Cqrs(#[from] CommandError),
}

#[derive(Debug, Error)]
pub enum TransferMoneyError {
    #[error("Transfer amount must be greater than zero")]
    ZeroAmount,

    #[error("Account {account_id} has insufficient balance: {balance} < {amount}")]
    InsufficientBalance {
        account_id: String,
        balance: u64,
        amount: u64,
    },

    #[error(transparent)]
    Cqrs(#[from] CommandError),
}

// -----------------------------------------------------------------------------
// Command Implementations
// -----------------------------------------------------------------------------

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
        if !self.email.contains('@') {
            return Err(RegisterUserError::InvalidEmail);
        }
        Ok(())
    }

    fn normalize(&mut self) -> Result<(), RegisterUserError> {
        self.user_id = self.user_id.trim().to_string();
        self.email = self.email.trim().to_lowercase();
        Ok(())
    }

    fn models(&self) -> Single<UserRegistrationModel> {
        Single(UserRegistrationModel::new(&self.email))
    }

    fn decide(
        &self,
        model: &UserRegistrationModel,
    ) -> Result<Vec<Event>, RegisterUserError> {
        if model.is_registered {
            return Err(RegisterUserError::EmailAlreadyRegistered(
                self.email.clone(),
            ));
        }

        let event = UserRegistered {
            user_id: self.user_id.clone(),
            email: self.email.clone(),
        }
        .to_event(EventId::new("evt_reg_1"))
        .unwrap();

        Ok(vec![event])
    }
}

pub struct DepositMoneyCommand {
    pub account_id: String,
    pub amount: u64,
}

impl Command<Single<BankAccountModel>> for DepositMoneyCommand {
    type Error = CommandError;

    fn validate(&self) -> Result<(), CommandError> {
        if self.amount == 0 {
            return Err(CommandError::Validation("Amount must be > 0".to_string()));
        }
        Ok(())
    }

    fn normalize(&mut self) -> Result<(), CommandError> {
        self.account_id = self.account_id.trim().to_string();
        Ok(())
    }

    fn models(&self) -> Single<BankAccountModel> {
        Single(BankAccountModel::new(&self.account_id))
    }

    fn decide(&self, _model: &BankAccountModel) -> Result<Vec<Event>, CommandError> {
        let event = MoneyDeposited {
            account_id: self.account_id.clone(),
            amount: self.amount,
        }
        .to_event(EventId::new("evt_dep_1"))
        .unwrap();

        Ok(vec![event])
    }
}

pub struct TransferMoneyCommand {
    pub from_account_id: String,
    pub to_account_id: String,
    pub amount: u64,
}

impl Command<Pair<BankAccountModel, BankAccountModel>> for TransferMoneyCommand {
    type Error = TransferMoneyError;

    fn validate(&self) -> Result<(), TransferMoneyError> {
        if self.amount == 0 {
            return Err(TransferMoneyError::ZeroAmount);
        }
        Ok(())
    }

    fn normalize(&mut self) -> Result<(), TransferMoneyError> {
        self.from_account_id = self.from_account_id.trim().to_string();
        self.to_account_id = self.to_account_id.trim().to_string();
        Ok(())
    }

    fn models(&self) -> Pair<BankAccountModel, BankAccountModel> {
        Pair(
            BankAccountModel::new(&self.from_account_id),
            BankAccountModel::new(&self.to_account_id),
        )
    }

    fn decide(
        &self,
        (from_acc, _to_acc): &(BankAccountModel, BankAccountModel),
    ) -> Result<Vec<Event>, TransferMoneyError> {
        if from_acc.balance < self.amount {
            return Err(TransferMoneyError::InsufficientBalance {
                account_id: self.from_account_id.clone(),
                balance: from_acc.balance,
                amount: self.amount,
            });
        }

        let ev1 = MoneyDeposited {
            account_id: self.from_account_id.clone(),
            amount: 0,
        }
        .to_event(EventId::new("evt_tr_1"))
        .unwrap();

        let ev2 = MoneyDeposited {
            account_id: self.to_account_id.clone(),
            amount: self.amount,
        }
        .to_event(EventId::new("evt_tr_2"))
        .unwrap();

        Ok(vec![ev1, ev2])
    }
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[tokio::test]
async fn test_single_model_command_lifecycle() {
    let store = InMemoryEventStore::new();

    let cmd = RegisterUserCommand {
        user_id: " user_1 ".to_string(),
        email: " ALICE@EXAMPLE.COM ".to_string(),
    };

    let appended = dispatch_command(cmd, &store).await.unwrap();
    assert_eq!(appended.len(), 1);
    assert_eq!(appended[0].position.value(), 1);

    let restored: UserRegistered = appended[0].to_domain_event().unwrap();
    assert_eq!(restored.user_id, "user_1");
    assert_eq!(restored.email, "alice@example.com");
}

#[tokio::test]
async fn test_command_owned_custom_error_type() {
    let store = InMemoryEventStore::new();

    let cmd = RegisterUserCommand {
        user_id: "".to_string(),
        email: "alice@example.com".to_string(),
    };

    let result = dispatch_command(cmd, &store).await;
    assert!(result.is_err());
    match result.err().unwrap() {
        RegisterUserError::EmptyUserId => {}
        other => panic!("Unexpected error variant: {:?}", other),
    }
}

#[tokio::test]
async fn test_transfer_command_owned_insufficient_funds_error() {
    let store = InMemoryEventStore::new();

    let cmd = TransferMoneyCommand {
        from_account_id: "acc_a".to_string(),
        to_account_id: "acc_b".to_string(),
        amount: 200,
    };

    let result = dispatch_command(cmd, &store).await;
    assert!(result.is_err());
    match result.err().unwrap() {
        TransferMoneyError::InsufficientBalance {
            account_id,
            balance,
            amount,
        } => {
            assert_eq!(account_id, "acc_a");
            assert_eq!(balance, 0);
            assert_eq!(amount, 200);
        }
        other => panic!("Unexpected error variant: {:?}", other),
    }
}

#[tokio::test]
async fn test_multi_model_tuple_command_execution() {
    let store = InMemoryEventStore::new();

    // 1. Deposit 500 into Account A
    let dep_cmd = DepositMoneyCommand {
        account_id: "acc_a".to_string(),
        amount: 500,
    };
    dispatch_command(dep_cmd, &store).await.unwrap();

    // 2. Transfer 200 from Account A to Account B
    let transfer_cmd = TransferMoneyCommand {
        from_account_id: "acc_a".to_string(),
        to_account_id: "acc_b".to_string(),
        amount: 200,
    };

    let appended = dispatch_command(transfer_cmd, &store).await.unwrap();
    assert_eq!(appended.len(), 2);
}

#[tokio::test]
async fn test_snapshot_backed_command_execution() {
    let store = InMemoryEventStore::new();
    let kv = MemoryKvStore::new();

    // 1. First command appends event 1 to event store
    let cmd1 = DepositMoneyCommand {
        account_id: "acc_snap".to_string(),
        amount: 100,
    };
    dispatch_command(cmd1, &store).await.unwrap();

    // 2. Second command runs with snapshot loading enabled (threshold 0)
    let cmd2 = DepositMoneyCommand {
        account_id: "acc_snap".to_string(),
        amount: 200,
    };

    let snapshot_opts = SnapshotOptions::new(0); // auto-snapshot when historical events read >= 0
    let appended = dispatch_command_with_snapshot(cmd2, &store, &kv, snapshot_opts)
        .await
        .unwrap();

    assert_eq!(appended.len(), 1);

    // Verify snapshot was created in KV store when cmd2 hydrated from store
    use futures_util::StreamExt;
    let mut items = kv.scan(kv_store::ScanOptions::default()).await;
    let mut count = 0;
    while let Some(_) = items.next().await {
        count += 1;
    }
    assert_eq!(count, 1);
}

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Default)]
struct MockTxProvider {
    pub active_tx_count: Arc<AtomicUsize>,
    pub committed_count: Arc<AtomicUsize>,
    pub rolled_back_count: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl tx_manager::TransactionProvider for MockTxProvider {
    type Conn = ();
    type Error = String;

    async fn begin_tx(&self) -> Result<(), Self::Error> {
        self.active_tx_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn commit_tx(&self, _conn: ()) -> Result<(), Self::Error> {
        self.committed_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn rollback_tx(&self, _conn: ()) -> Result<(), Self::Error> {
        self.rolled_back_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test]
async fn test_dispatch_command_in_tx_success() {
    let store = InMemoryEventStore::new();
    let tx_provider = MockTxProvider::default();

    let cmd = RegisterUserCommand {
        user_id: "u_tx_1".to_string(),
        email: "tx1@example.com".to_string(),
    };

    let appended = cqrs::dispatch_command_in_tx(cmd, &store, &tx_provider)
        .await
        .unwrap();

    assert_eq!(appended.len(), 1);
    assert_eq!(tx_provider.committed_count.load(Ordering::SeqCst), 1);
    assert_eq!(tx_provider.rolled_back_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_dispatch_command_in_tx_rollback_on_domain_error() {
    let store = InMemoryEventStore::new();
    let tx_provider = MockTxProvider::default();

    let cmd = RegisterUserCommand {
        user_id: "".to_string(), // Invalid validation
        email: "invalid".to_string(),
    };

    let result = cqrs::dispatch_command_in_tx(cmd, &store, &tx_provider).await;
    assert!(result.is_err());
    assert_eq!(tx_provider.rolled_back_count.load(Ordering::SeqCst), 1);
    assert_eq!(tx_provider.committed_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_command_dispatcher_tx_struct() {
    let store = InMemoryEventStore::new();
    let tx_provider = MockTxProvider::default();
    let dispatcher = cqrs::CommandDispatcherTx::new(store, tx_provider.clone());

    let cmd = RegisterUserCommand {
        user_id: "u_disp_1".to_string(),
        email: "disp@example.com".to_string(),
    };

    let appended = dispatcher.dispatch(cmd).await.unwrap();
    assert_eq!(appended.len(), 1);
    assert_eq!(tx_provider.committed_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_dispatch_command_with_snapshot_in_tx_success() {
    let store = InMemoryEventStore::new();
    let kv = MemoryKvStore::new();
    let tx_provider = MockTxProvider::default();

    let cmd1 = DepositMoneyCommand {
        account_id: "acc_tx_snap".to_string(),
        amount: 300,
    };
    cqrs::dispatch_command_in_tx(cmd1, &store, &tx_provider)
        .await
        .unwrap();

    let cmd2 = DepositMoneyCommand {
        account_id: "acc_tx_snap".to_string(),
        amount: 200,
    };
    let appended = cqrs::dispatch_command_with_snapshot_in_tx(
        cmd2,
        &store,
        &kv,
        &tx_provider,
        SnapshotOptions::new(0),
    )
    .await
    .unwrap();

    assert_eq!(appended.len(), 1);
    assert_eq!(tx_provider.committed_count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_dispatch_command_tx_success() {
    let store = InMemoryEventStore::new();
    let mut conn = ();

    let cmd = RegisterUserCommand {
        user_id: "u_tx_raw_1".to_string(),
        email: "raw1@example.com".to_string(),
    };

    let appended = cqrs::dispatch_command_tx(cmd, &store, &mut conn)
        .await
        .unwrap();

    assert_eq!(appended.len(), 1);
}

#[tokio::test]
async fn test_dispatch_command_with_snapshot_tx_success() {
    let store = InMemoryEventStore::new();
    let kv = MemoryKvStore::new();
    let mut conn = ();

    let cmd1 = DepositMoneyCommand {
        account_id: "acc_raw_snap".to_string(),
        amount: 500,
    };
    cqrs::dispatch_command_tx(cmd1, &store, &mut conn)
        .await
        .unwrap();

    let cmd2 = DepositMoneyCommand {
        account_id: "acc_raw_snap".to_string(),
        amount: 250,
    };
    let appended = cqrs::dispatch_command_with_snapshot_tx(
        cmd2,
        &store,
        &kv,
        &mut conn,
        SnapshotOptions::new(0),
    )
    .await
    .unwrap();

    assert_eq!(appended.len(), 1);
}

