use event_store::{DomainEvent, Event, EventMetadata};

/// Contextual metadata captured at the application boundary.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecutionContext {
    /// Identity of the actor or caller.
    pub actor_id: Option<String>,
    /// Correlation ID linking related operations.
    pub correlation_id: Option<String>,
    /// ID of the command or event that caused this operation.
    pub causation_id: Option<String>,
    /// W3C trace ID.
    pub trace_id: Option<String>,
    /// W3C span ID.
    pub span_id: Option<String>,
    /// Client IP address.
    pub client_ip: Option<String>,
    /// Client user-agent string.
    pub user_agent: Option<String>,
}

impl ExecutionContext {
    /// Creates an empty execution context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the actor ID.
    #[must_use]
    pub fn with_actor_id(mut self, actor_id: impl Into<String>) -> Self {
        self.actor_id = Some(actor_id.into());
        self
    }

    /// Sets the correlation ID.
    #[must_use]
    pub fn with_correlation_id(mut self, correlation_id: impl Into<String>) -> Self {
        self.correlation_id = Some(correlation_id.into());
        self
    }

    /// Sets the causation ID.
    #[must_use]
    pub fn with_causation_id(mut self, causation_id: impl Into<String>) -> Self {
        self.causation_id = Some(causation_id.into());
        self
    }

    /// Sets the trace ID.
    #[must_use]
    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self
    }

    /// Sets the span ID.
    #[must_use]
    pub fn with_span_id(mut self, span_id: impl Into<String>) -> Self {
        self.span_id = Some(span_id.into());
        self
    }

    /// Sets the client IP address.
    #[must_use]
    pub fn with_client_ip(mut self, client_ip: impl Into<String>) -> Self {
        self.client_ip = Some(client_ip.into());
        self
    }

    /// Sets the client user-agent.
    #[must_use]
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// Builds event metadata for the current operation.
    pub fn to_metadata(&self) -> EventMetadata {
        let mut metadata = EventMetadata::now();
        metadata.actor_id = self.actor_id.clone();
        metadata.correlation_id = self.correlation_id.clone();
        metadata.causation_id = self.causation_id.clone();
        metadata.trace_id = self.trace_id.clone();
        metadata.span_id = self.span_id.clone();
        metadata.client_ip = self.client_ip.clone();
        metadata.user_agent = self.user_agent.clone();
        metadata
    }

    /// Converts a domain event into a store event enriched with this context.
    pub fn create_store_event<E: DomainEvent>(
        &self,
        event: &E,
    ) -> Result<Event, serde_json::Error> {
        event.to_store_event_with_metadata(&self.to_metadata())
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct TestEvent {
        entity_id: String,
    }

    impl DomainEvent for TestEvent {
        fn event_type() -> &'static str {
            "test_event"
        }

        fn tags(&self) -> Vec<String> {
            vec![format!("entity:{}", self.entity_id)]
        }
    }

    #[test]
    fn creates_store_event_with_context_metadata() -> Result<(), Box<dyn std::error::Error>> {
        let context = ExecutionContext::new()
            .with_actor_id("user-1")
            .with_correlation_id("request-1");

        let event = TestEvent {
            entity_id: "entity-1".to_string(),
        };

        let store_event = context.create_store_event(&event)?;
        let metadata: EventMetadata = store_event.metadata_as()?;

        assert_eq!(store_event.event_type, "test_event");
        assert_eq!(store_event.tags, vec!["entity:entity-1"]);
        assert_eq!(metadata.actor_id.as_deref(), Some("user-1"));
        assert_eq!(metadata.correlation_id.as_deref(), Some("request-1"));
        Ok(())
    }
}
