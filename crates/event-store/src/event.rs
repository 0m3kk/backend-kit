use serde_json::Value;

/// Unique, monotonic increasing sequence of an event in a store. Gaps may occur.
pub type SequencePosition = u64;

/// Tags of an event, without duplicates (e.g. `product:p123`).
pub type Tags = Vec<String>;

/// An event to be appended to an Event Store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// Type of the event, used for filtering in a [Query](crate::Query).
    pub event_type: String,
    /// Opaque JSON payload of the event.
    pub data: Value,
    /// Metadata defined by the client, not used for filtering.
    pub metadata: Value,
    /// Tags associated with the event.
    pub tags: Tags,
}

impl Event {
    /// Create a new event with empty metadata.
    pub fn new(event_type: impl Into<String>, data: Value, tags: Tags) -> Self {
        Self {
            event_type: event_type.into(),
            data,
            metadata: Value::Null,
            tags,
        }
    }

    /// Set the metadata of the event.
    pub fn with_metadata(mut self, metadata: Value) -> Self {
        self.metadata = metadata;
        self
    }

    /// Set typed metadata by serializing it to JSON.
    pub fn with_typed_metadata(
        self,
        metadata: &crate::metadata::EventMetadata,
    ) -> Result<Self, serde_json::Error> {
        Ok(self.with_metadata(serde_json::to_value(metadata)?))
    }

    /// Deserializes the event's metadata into [`EventMetadata`](crate::EventMetadata).
    pub fn typed_metadata(&self) -> Result<crate::metadata::EventMetadata, serde_json::Error> {
        serde_json::from_value(self.metadata.clone())
    }

    /// Deserializes the event's metadata into any custom type.
    pub fn metadata_as<T: serde::de::DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.metadata.clone())
    }
}

/// An [Event] that was persisted in an Event Store, together with its position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequencedEvent {
    /// The event that was appended.
    pub event: Event,
    /// Position of the event within the Event Store.
    pub position: SequencePosition,
}
