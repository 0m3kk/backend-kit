//! DCB compliant event store: types, [`EventStore`] trait, and PostgreSQL implementation.

mod domain_event;
mod error;
mod event;
mod metadata;
mod postgres;
mod query;
mod store;

pub use domain_event::{DomainEvent, tag_value};
pub use error::{AppendConditionError, Error};
pub use event::{Event, SequencePosition, SequencedEvent, Tags};
pub use futures_core::Stream;
pub use metadata::EventMetadata;
pub use postgres::{EVENT_STORE_MIGRATOR, PostgresEventStore, migrator};
pub use query::{AppendCondition, Query, QueryItem};
pub use store::{EventStore, EventStream, MockEventStore};

/// PostgreSQL notification channel emitted after an event is appended.
pub const EVENTS_NOTIFY_CHANNEL: &str = "events_appended";
