//! Shared behaviour implemented by every domain event.

use serde::{Serialize, de::DeserializeOwned};

use crate::Event;

/// Behaviour shared by every domain event.
///
/// Each event struct implements this trait instead of being part of a central
/// event enum. Events stay owned by their domain while generic code (event
/// stores, dispatchers, projections) can still handle them uniformly.
pub trait DomainEvent: Serialize + DeserializeOwned + Send + Sync + 'static {
    /// Identifier for the event type.
    fn event_type() -> &'static str;

    /// Tags associated with this event for stream filtering.
    fn tags(&self) -> Vec<String>;

    /// Converts the domain event into a generic [`Event`].
    fn to_store_event(&self) -> Result<Event, serde_json::Error> {
        Ok(Event::new(
            Self::event_type(),
            serde_json::to_value(self)?,
            self.tags(),
        ))
    }

    /// Converts the domain event into an [`Event`] with attached metadata.
    fn to_store_event_with_metadata<M: Serialize>(
        &self,
        metadata: &M,
    ) -> Result<Event, serde_json::Error> {
        let event = self.to_store_event()?;
        Ok(event.with_metadata(serde_json::to_value(metadata)?))
    }
}

/// Normalises a tagged field into a stable tag value.
///
/// Strings and string-like newtypes are used as-is; other serialisable values
/// fall back to their JSON representation.
pub fn tag_value<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(value)) => value,
        Ok(other) => other.to_string(),
        Err(_) => String::new(),
    }
}
