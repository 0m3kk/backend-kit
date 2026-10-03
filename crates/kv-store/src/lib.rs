//! Key-value store: types, [`KvStore`] trait, and PostgreSQL implementation.

mod condition;
mod error;
mod postgres;
mod store;
mod value;

pub use condition::PutCondition;
pub use error::{Error, PutConditionError};
pub use postgres::{KV_STORE_MIGRATOR, PostgresKvStore};
pub use store::{KvStore, MockKvStore};
pub use value::{ReplacedValue, StoredValue, Timestamp};
