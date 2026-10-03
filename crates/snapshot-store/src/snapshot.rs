use event_store::SequencePosition;
use serde_json::Value;

/// Persisted model state, folded up to a sequence position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// Position of the last event folded into the state; `None` if none.
    pub position: Option<SequencePosition>,
    /// The serialized model.
    pub state: Value,
}

impl Snapshot {
    /// Create a new snapshot.
    pub fn new(position: Option<SequencePosition>, state: Value) -> Self {
        Self { position, state }
    }
}
