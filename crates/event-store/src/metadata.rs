//! Strongly-typed event metadata definition and builders.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;

/// Standard metadata associated with an event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventMetadata {
    /// Time when the event was recorded or originated (UTC).
    #[serde(with = "time::serde::rfc3339")]
    pub timestamp: OffsetDateTime,

    /// Identity of the actor or system that triggered the action.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor_id: Option<String>,

    /// Correlation ID linking related operations across services.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,

    /// ID of the command or antecedent event that caused this event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub causation_id: Option<String>,

    /// Distributed tracing Trace ID (W3C traceparent).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,

    /// Distributed tracing Span ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span_id: Option<String>,

    /// Client IP address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_ip: Option<String>,

    /// Client User-Agent string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,

    /// Arbitrary custom metadata properties.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<Value>,
}

impl EventMetadata {
    /// Creates a new metadata instance with the given timestamp.
    pub fn new(timestamp: OffsetDateTime) -> Self {
        Self {
            timestamp,
            actor_id: None,
            correlation_id: None,
            causation_id: None,
            trace_id: None,
            span_id: None,
            client_ip: None,
            user_agent: None,
            extra: None,
        }
    }

    /// Creates metadata initialized with the current UTC timestamp.
    pub fn now() -> Self {
        Self::new(OffsetDateTime::now_utc())
    }

    /// Sets the actor ID.
    pub fn with_actor_id(mut self, actor_id: impl Into<String>) -> Self {
        self.actor_id = Some(actor_id.into());
        self
    }

    /// Sets the correlation ID.
    pub fn with_correlation_id(mut self, correlation_id: impl Into<String>) -> Self {
        self.correlation_id = Some(correlation_id.into());
        self
    }

    /// Sets the causation ID.
    pub fn with_causation_id(mut self, causation_id: impl Into<String>) -> Self {
        self.causation_id = Some(causation_id.into());
        self
    }

    /// Sets the trace ID.
    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self
    }

    /// Sets the span ID.
    pub fn with_span_id(mut self, span_id: impl Into<String>) -> Self {
        self.span_id = Some(span_id.into());
        self
    }

    /// Sets the client IP address.
    pub fn with_client_ip(mut self, client_ip: impl Into<String>) -> Self {
        self.client_ip = Some(client_ip.into());
        self
    }

    /// Sets the user agent string.
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// Sets additional custom metadata.
    pub fn with_extra(mut self, extra: Value) -> Self {
        self.extra = Some(extra);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_builder_and_serde_roundtrip() {
        let ts = OffsetDateTime::UNIX_EPOCH;
        let metadata = EventMetadata::new(ts)
            .with_actor_id("user-123")
            .with_correlation_id("corr-456")
            .with_causation_id("cmd-789")
            .with_trace_id("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01")
            .with_span_id("00f067aa0ba902b7")
            .with_client_ip("192.168.1.100")
            .with_user_agent("Mozilla/5.0");

        #[allow(clippy::unwrap_used)]
        let json = serde_json::to_value(&metadata).unwrap();
        assert_eq!(json["actor_id"], "user-123");
        assert_eq!(json["correlation_id"], "corr-456");
        assert_eq!(json["causation_id"], "cmd-789");
        assert_eq!(json["client_ip"], "192.168.1.100");
        assert_eq!(json["user_agent"], "Mozilla/5.0");
        assert_eq!(json["timestamp"], "1970-01-01T00:00:00Z");

        #[allow(clippy::unwrap_used)]
        let deserialized: EventMetadata = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized, metadata);
    }

    #[test]
    fn metadata_with_extra_roundtrip() {
        let metadata = EventMetadata::now()
            .with_actor_id("user-1")
            .with_extra(serde_json::json!({ "client_ip": "127.0.0.1" }));

        #[allow(clippy::unwrap_used)]
        let json = serde_json::to_value(&metadata).unwrap();
        assert_eq!(json["extra"]["client_ip"], "127.0.0.1");

        #[allow(clippy::unwrap_used)]
        let deserialized: EventMetadata = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized, metadata);
    }
}
