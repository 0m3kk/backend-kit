//! Decision-state rehydration from snapshots and events.

use std::fmt::Display;

use event_store::{Error as EventStoreError, EventStore, Query, SequencePosition, SequencedEvent};
use futures_util::StreamExt;
use serde::Serialize;
use serde::de::DeserializeOwned;
use snapshot_store::{Snapshot, SnapshotStore};

/// A decision state that knows how to load itself from the event store.
///
/// Implementations are usually application-level wrappers around a generated
/// state contract. The wrapper owns its snapshot key, event query, guards, and
/// the reducer that folds events into the generated state.
pub trait State: Serialize + DeserializeOwned + Send {
    /// Error returned when an event cannot be applied.
    type Error;

    /// Stable snapshot key for this state instance.
    fn key(&self) -> String;

    /// Event-store query used to rehydrate this state.
    fn query(&self) -> Query;

    /// Folds one sequenced event into the state.
    fn apply(&mut self, sequenced: &SequencedEvent) -> Result<(), Self::Error>;
}

/// Errors produced while loading a decision state.
#[derive(Debug)]
pub enum StateLoadError<Err> {
    /// The snapshot store failed.
    Snapshot(String),

    /// The event store failed.
    EventStore(String),

    /// The state reducer rejected an event.
    State(Err),
}

impl<Err> StateLoadError<Err> {
    fn event_store<E: Display>(error: EventStoreError<E>) -> Self {
        let message = match error {
            EventStoreError::Condition(condition) => format!(
                "append condition failed with {} conflicting event(s)",
                condition.conflicting_events.len()
            ),
            EventStoreError::Store(error) => error.to_string(),
        };

        Self::EventStore(message)
    }
}

/// Loads decision states by restoring a snapshot and replaying newer events.
pub struct StateLoader<'a, E, S> {
    event_store: &'a E,
    snapshot_store: &'a S,
}

impl<'a, E, S> StateLoader<'a, E, S>
where
    E: EventStore + Send + Sync,
    E::Error: Display,
    S: SnapshotStore + Send + Sync,
    S::Error: Display,
{
    /// Creates a loader backed by an event store and a snapshot store.
    pub fn new(event_store: &'a E, snapshot_store: &'a S) -> Self {
        Self {
            event_store,
            snapshot_store,
        }
    }

    /// Loads one state instance.
    ///
    /// The snapshot is restored when it deserializes into `T`; otherwise the
    /// supplied initial state is used and the full event stream is replayed.
    /// Snapshot writes are best-effort and never fail the load.
    pub async fn load<T>(&self, mut state: T) -> Result<T, StateLoadError<T::Error>>
    where
        T: State,
    {
        let key = state.key();
        let from = self.restore::<T, T::Error>(&key, &mut state).await?;
        let mut last_position = from.map(|position| position.saturating_sub(1));
        let mut replayed = 0u64;

        let mut stream = self
            .event_store
            .read(state.query(), from)
            .await
            .map_err(StateLoadError::<T::Error>::event_store)?;

        while let Some(item) = stream.next().await {
            let sequenced = item.map_err(StateLoadError::<T::Error>::event_store)?;
            state.apply(&sequenced).map_err(StateLoadError::State)?;
            last_position = Some(sequenced.position);
            replayed += 1;
        }

        self.maybe_snapshot(&key, last_position, &state, replayed)
            .await;

        Ok(state)
    }

    async fn restore<T, Err>(
        &self,
        key: &str,
        state: &mut T,
    ) -> Result<Option<SequencePosition>, StateLoadError<Err>>
    where
        T: DeserializeOwned,
    {
        let snapshot = self
            .snapshot_store
            .get(key)
            .await
            .map_err(|error| StateLoadError::Snapshot(error.to_string()))?;

        let Some(snapshot) = snapshot else {
            return Ok(None);
        };

        // A snapshot that no longer matches the state shape is ignored and the
        // state is rebuilt from events.
        let Ok(restored) = serde_json::from_value::<T>(snapshot.state) else {
            return Ok(None);
        };

        *state = restored;
        Ok(snapshot.position.map(|position| position.saturating_add(1)))
    }

    async fn maybe_snapshot<T>(
        &self,
        key: &str,
        position: Option<SequencePosition>,
        state: &T,
        replayed: u64,
    ) where
        T: Serialize,
    {
        if replayed == 0 || !self.snapshot_store.should_snapshot(replayed) {
            return;
        }

        let Ok(value) = serde_json::to_value(state) else {
            return;
        };

        // Snapshots are an optimization: a failed write must not fail the request.
        let _ = self
            .snapshot_store
            .put(key, Snapshot::new(position, value))
            .await;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use event_store::{Event, MockEventStore, SequencedEvent};
    use futures_util::stream;
    use serde::Deserialize;
    use snapshot_store::{MockSnapshotStore, Snapshot};

    use super::*;

    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    struct Counter {
        value: i64,
    }

    impl State for Counter {
        type Error = String;

        fn key(&self) -> String {
            "counter:1".to_string()
        }

        fn query(&self) -> Query {
            Query::all()
        }

        fn apply(&mut self, sequenced: &SequencedEvent) -> Result<(), Self::Error> {
            let value: i64 = serde_json::from_value(sequenced.event.data.clone())
                .map_err(|error| error.to_string())?;
            self.value = value;
            Ok(())
        }
    }

    fn event(value: i64, position: u64) -> SequencedEvent {
        SequencedEvent {
            event: Event::new("counter_set", serde_json::json!(value), vec![]),
            position,
        }
    }

    #[tokio::test]
    async fn loads_from_events_and_writes_snapshot() {
        let mut event_store = MockEventStore::new();
        event_store.expect_read().returning(|_, _| {
            Ok(Box::pin(stream::iter(vec![
                Ok(event(1, 1)),
                Ok(event(2, 2)),
            ])))
        });

        let mut snapshots = MockSnapshotStore::new();
        snapshots.expect_get().returning(|_| Ok(None));
        snapshots.expect_should_snapshot().returning(|_| true);
        snapshots.expect_put().times(1).returning(|_, _| Ok(()));

        let loader = StateLoader::new(&event_store, &snapshots);
        let state = loader.load(Counter::default()).await.unwrap();

        assert_eq!(state, Counter { value: 2 });
    }

    #[tokio::test]
    async fn restores_snapshot_and_replays_from_next_position() {
        let snapshot = Snapshot::new(Some(5), serde_json::json!({ "value": 10 }));

        let mut event_store = MockEventStore::new();
        event_store
            .expect_read()
            .withf(|_, from| *from == Some(6))
            .returning(|_, _| Ok(Box::pin(stream::iter(vec![Ok(event(11, 7))]))));

        let mut snapshots = MockSnapshotStore::new();
        snapshots
            .expect_get()
            .returning(move |_| Ok(Some(snapshot.clone())));
        snapshots.expect_should_snapshot().returning(|_| false);

        let loader = StateLoader::new(&event_store, &snapshots);
        let state = loader.load(Counter::default()).await.unwrap();

        assert_eq!(state, Counter { value: 11 });
    }

    #[tokio::test]
    async fn ignores_snapshot_that_does_not_match_the_state() {
        let snapshot = Snapshot::new(Some(5), serde_json::json!("not a counter"));

        let mut event_store = MockEventStore::new();
        event_store
            .expect_read()
            .withf(|_, from| from.is_none())
            .returning(|_, _| Ok(Box::pin(stream::iter(vec![Ok(event(3, 1))]))));

        let mut snapshots = MockSnapshotStore::new();
        snapshots
            .expect_get()
            .returning(move |_| Ok(Some(snapshot.clone())));
        snapshots.expect_should_snapshot().returning(|_| false);

        let loader = StateLoader::new(&event_store, &snapshots);
        let state = loader.load(Counter::default()).await.unwrap();

        assert_eq!(state, Counter { value: 3 });
    }

    #[tokio::test]
    async fn preserves_apply_error() {
        #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
        struct FailingState;

        impl State for FailingState {
            type Error = String;

            fn key(&self) -> String {
                "failing:1".to_string()
            }

            fn query(&self) -> Query {
                Query::all()
            }

            fn apply(&mut self, _sequenced: &SequencedEvent) -> Result<(), Self::Error> {
                Err("boom".to_string())
            }
        }

        let mut event_store = MockEventStore::new();
        event_store
            .expect_read()
            .returning(|_, _| Ok(Box::pin(stream::iter(vec![Ok(event(1, 1))]))));

        let mut snapshots = MockSnapshotStore::new();
        snapshots.expect_get().returning(|_| Ok(None));

        let loader = StateLoader::new(&event_store, &snapshots);
        let result = loader.load(FailingState).await;

        match result {
            Err(StateLoadError::State(message)) => assert_eq!(message, "boom"),
            other => panic!("unexpected result: {other:?}"),
        }
    }
}
