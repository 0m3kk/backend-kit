use crate::event::SequencedEvent;

/// Error returned when the append condition is not met.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendConditionError {
    /// The events that matched the condition and prevented the append.
    pub conflicting_events: Vec<SequencedEvent>,
}

/// Error returned by [`EventStore`](crate::EventStore).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error<E> {
    /// The append condition was not met.
    Condition(AppendConditionError),
    /// An error from the underlying store.
    Store(E),
}
