use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;

/// Trait implemented by strongly-typed domain events representing payload data in an [`Event`].
pub trait DomainEvent: Serialize + DeserializeOwned + Send + Sync + 'static {
    /// Returns the static [`EventType`] name for this domain event (e.g. `"OrderCreated"`).
    fn event_type() -> EventType
    where
        Self: Sized;

    /// Optional tags associated with instances of this domain event (e.g. `Tag::key_value("order", "100")`).
    fn tags(&self) -> Vec<Tag> {
        Vec::new()
    }

    /// Helper method to create an unsequenced [`Event`] wrapping this domain event instance.
    fn to_event(&self, id: impl Into<EventId>) -> Result<Event, serde_json::Error>
    where
        Self: Sized,
    {
        let data = serde_json::to_value(self)?;
        Ok(Event::new(id, Self::event_type(), data, self.tags()))
    }
}

/// Unique identifier for an unsequenced domain event.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventId(pub String);

impl EventId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<T: Into<String>> From<T> for EventId {
    fn from(s: T) -> Self {
        Self(s.into())
    }
}

/// Represents a unique, monotonically increasing position of an event in the Event Store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SequencePosition(pub u64);

impl SequencePosition {
    pub const ZERO: SequencePosition = SequencePosition(0);

    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn value(&self) -> u64 {
        self.0
    }

    pub fn next(&self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

impl fmt::Display for SequencePosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for SequencePosition {
    fn from(val: u64) -> Self {
        Self(val)
    }
}

/// Identifies the type of an event used for filtering and domain dispatch.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventType(pub String);

impl EventType {
    pub fn new(type_name: impl Into<String>) -> Self {
        Self(type_name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<T: Into<String>> From<T> for EventType {
    fn from(s: T) -> Self {
        Self(s.into())
    }
}

/// Domain tag added to an event for dynamic consistency boundary partitioning.
///
/// Example: `Tag::new("user:123")` or `Tag::key_value("order", "ord-99")`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Tag(pub String);

impl Tag {
    pub fn new(tag: impl Into<String>) -> Self {
        Self(tag.into())
    }

    pub fn key_value(key: &str, value: &str) -> Self {
        Self(format!("{}:{}", key, value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<T: Into<String>> From<T> for Tag {
    fn from(s: T) -> Self {
        Self(s.into())
    }
}

/// Unsequenced event to be appended to the Event Store.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// Unique identifier for this event.
    pub id: EventId,
    /// Event type string.
    #[serde(rename = "type")]
    pub event_type: EventType,
    /// Opaque JSON payload data.
    pub data: serde_json::Value,
    /// Associated domain tags.
    pub tags: Vec<Tag>,
    /// Optional metadata defined by the client.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl Event {
    pub fn new(
        id: impl Into<EventId>,
        event_type: impl Into<EventType>,
        data: impl Into<serde_json::Value>,
        tags: Vec<Tag>,
    ) -> Self {
        Self {
            id: id.into(),
            event_type: event_type.into(),
            data: data.into(),
            tags,
            metadata: None,
        }
    }

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Deserializes the JSON `data` payload of this event into a strongly-typed [`DomainEvent`].
    pub fn to_domain_event<T: DomainEvent>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.data.clone())
    }

    /// Creates an [`Event`] from a strongly-typed [`DomainEvent`] instance and an event ID.
    pub fn from_domain_event<T: DomainEvent>(
        id: impl Into<EventId>,
        domain_event: &T,
    ) -> Result<Self, serde_json::Error> {
        domain_event.to_event(id)
    }
}

/// An event that has been assigned a SequencePosition and timestamp by the Event Store upon append.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SequencedEvent {
    /// Monotonic sequence position assigned by the store.
    pub position: SequencePosition,
    /// Unix timestamp in milliseconds when the event was appended.
    pub timestamp: u64,
    /// The inner domain event.
    pub event: Event,
}

impl SequencedEvent {
    pub fn new(position: SequencePosition, timestamp: u64, event: Event) -> Self {
        Self {
            position,
            timestamp,
            event,
        }
    }

    /// Deserializes the inner event's JSON `data` payload into a strongly-typed [`DomainEvent`].
    pub fn to_domain_event<T: DomainEvent>(&self) -> Result<T, serde_json::Error> {
        self.event.to_domain_event()
    }
}

/// A Query Item filters events by matching specified event types and/or tags.
///
/// Matching rules:
/// - Event type MUST match at least ONE of `types` (if `types` is non-empty).
/// - Event tags MUST contain ALL tags in `tags` (if `tags` is non-empty).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct QueryItem {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub types: Vec<EventType>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<Tag>,
}

impl QueryItem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_type(mut self, event_type: impl Into<EventType>) -> Self {
        self.types.push(event_type.into());
        self
    }

    pub fn with_types(mut self, types: impl IntoIterator<Item = impl Into<EventType>>) -> Self {
        self.types.extend(types.into_iter().map(Into::into));
        self
    }

    pub fn with_tag(mut self, tag: impl Into<Tag>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<Tag>>) -> Self {
        self.tags.extend(tags.into_iter().map(Into::into));
        self
    }

    /// Evaluates if an event matches this query item.
    pub fn matches(&self, event: &Event) -> bool {
        // If types are specified, event.event_type must match at least one type (OR condition)
        if !self.types.is_empty() && !self.types.contains(&event.event_type) {
            return false;
        }

        // If tags are specified, event.tags must contain all tags in self.tags (AND condition)
        if !self.tags.is_empty() {
            let event_tags_set: HashSet<&Tag> = event.tags.iter().collect();
            for req_tag in &self.tags {
                if !event_tags_set.contains(req_tag) {
                    return false;
                }
            }
        }

        true
    }
}

/// Query representing constraints that must be matched by events in the Event Store.
///
/// Multiple `QueryItem`s inside `Items` are evaluated using **OR** logic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Query {
    All,
    Items { items: Vec<QueryItem> },
}

impl Query {
    /// Creates a query that matches all events.
    pub fn all() -> Self {
        Query::All
    }

    /// Creates a query from a set of query items combined with OR logic.
    pub fn from_items(items: Vec<QueryItem>) -> Self {
        if items.is_empty() {
            Query::All
        } else {
            Query::Items { items }
        }
    }

    /// Creates a single-item query.
    pub fn item(item: QueryItem) -> Self {
        Query::Items { items: vec![item] }
    }

    /// Evaluates whether an event matches this query.
    pub fn matches(&self, event: &Event) -> bool {
        match self {
            Query::All => true,
            Query::Items { items } => {
                if items.is_empty() {
                    true
                } else {
                    items.iter().any(|item| item.matches(event))
                }
            }
        }
    }
}

/// AppendCondition specifies consistency constraints for appending events.
///
/// An append operation will fail if any existing event stored at position > `after`
/// matches `fail_if_events_match`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppendCondition {
    #[serde(rename = "failIfEventsMatch")]
    pub fail_if_events_match: Query,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<SequencePosition>,
}

impl AppendCondition {
    pub fn new(fail_if_events_match: Query) -> Self {
        Self {
            fail_if_events_match,
            after: None,
        }
    }

    pub fn after(mut self, position: SequencePosition) -> Self {
        self.after = Some(position);
        self
    }

    pub fn after_opt(mut self, position: Option<SequencePosition>) -> Self {
        self.after = position;
        self
    }

    /// Checks if a sequenced event violates this condition.
    pub fn is_violated_by(&self, sequenced_event: &SequencedEvent) -> bool {
        if matches!(self.after, Some(after_pos) if sequenced_event.position <= after_pos) {
            return false;
        }
        self.fail_if_events_match.matches(&sequenced_event.event)
    }
}

/// Read direction for querying events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Direction {
    #[default]
    Forward,
    Backward,
}

/// Options for reading events from the Event Store.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ReadOptions {
    /// Read events with sequence position > `after`.
    pub after: Option<SequencePosition>,
    /// Limit max number of events returned.
    pub limit: Option<usize>,
    /// Read direction (Forward / Backward).
    pub direction: Direction,
}

impl ReadOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn after(mut self, position: SequencePosition) -> Self {
        self.after = Some(position);
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn direction(mut self, direction: Direction) -> Self {
        self.direction = direction;
        self
    }
}
