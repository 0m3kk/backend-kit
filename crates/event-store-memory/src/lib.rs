use async_trait::async_trait;
use futures_util::stream::{self, StreamExt};
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

pub use event_sourcing::*;

/// An in-memory, thread-safe implementation of an Event Store compliant with the DCB specification.
#[derive(Debug, Default)]
pub struct InMemoryEventStore {
    events: RwLock<Vec<SequencedEvent>>,
    current_position: RwLock<u64>,
}

impl InMemoryEventStore {
    pub fn new() -> Self {
        Self {
            events: RwLock::new(Vec::new()),
            current_position: RwLock::new(0),
        }
    }

    /// Helper to fetch total event count stored.
    pub fn len(&self) -> usize {
        self.events.read().map(|e| e.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[async_trait]
impl EventStore for InMemoryEventStore {
    async fn read(&self, query: &Query, options: ReadOptions) -> EventStream {
        let events_guard = match self.events.read() {
            Ok(guard) => guard,
            Err(e) => {
                let err_msg = e.to_string();
                return stream::once(async move { Err(ReadError::StoreError(err_msg)) }).boxed();
            }
        };

        let mut filtered: Vec<SequencedEvent> = events_guard
            .iter()
            .filter(|seq_event| {
                if matches!(options.after, Some(after_pos) if seq_event.position <= after_pos) {
                    return false;
                }
                query.matches(&seq_event.event)
            })
            .cloned()
            .collect();

        if options.direction == Direction::Backward {
            filtered.reverse();
        }

        if let Some(limit) = options.limit {
            filtered.truncate(limit);
        }

        stream::iter(filtered.into_iter().map(Ok)).boxed()
    }

    async fn append(
        &self,
        events: Vec<Event>,
        condition: Option<AppendCondition>,
    ) -> Result<Vec<SequencedEvent>, AppendError> {
        if events.is_empty() {
            return Err(AppendError::EmptyBatch);
        }

        let mut events_guard = self
            .events
            .write()
            .map_err(|e| AppendError::StoreError(e.to_string()))?;

        // 1. Enforce AppendCondition if specified
        if let Some(ref cond) = condition {
            for existing in events_guard.iter() {
                if cond.is_violated_by(existing) {
                    return Err(AppendError::Conflict {
                        condition: cond.clone(),
                        conflicting_event: existing.clone(),
                    });
                }
            }
        }

        // 2. Assign unique, monotonically increasing sequence positions and timestamp
        let mut pos_guard = self
            .current_position
            .write()
            .map_err(|e| AppendError::StoreError(e.to_string()))?;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let mut appended = Vec::with_capacity(events.len());

        for event in events {
            *pos_guard += 1;
            let seq_pos = SequencePosition::new(*pos_guard);
            let seq_event = SequencedEvent::new(seq_pos, timestamp, event);
            events_guard.push(seq_event.clone());
            appended.push(seq_event);
        }

        Ok(appended)
    }
}
