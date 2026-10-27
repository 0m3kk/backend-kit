//! Decision-state rehydration from snapshots and events.

use std::fmt::Display;

use event_store::{
    AppendCondition, Error as EventStoreError, EventStore, Query, SequencePosition, SequencedEvent,
};
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

/// A group of decision states loaded by a single event-store read.
///
/// Implemented for tuples of up to five [`State`] values that share one error
/// type. Pass the tuple to [`StateLoader::load_set`] to query the union of
/// their queries once and dispatch each event to the states that match it.
pub trait StateSet {
    /// Error type shared by every state in the set.
    type Error;

    /// Returns mutable handles to every state.
    #[doc(hidden)]
    fn erased_mut(&mut self) -> Vec<&mut dyn ErasedState<Self::Error>>;

    /// Returns shared handles to every state.
    #[doc(hidden)]
    fn erased(&self) -> Vec<&dyn ErasedState<Self::Error>>;
}

/// Object-safe view of a [`State`], used to process heterogeneous state sets.
#[doc(hidden)]
pub trait ErasedState<Err>: Send {
    fn key(&self) -> String;

    fn query(&self) -> Query;

    fn restore(&mut self, state: serde_json::Value) -> bool;

    fn snapshot(&self) -> Option<serde_json::Value>;

    fn apply(&mut self, sequenced: &SequencedEvent) -> Result<(), Err>;
}

impl<T, Err> ErasedState<Err> for T
where
    T: State<Error = Err>,
{
    fn key(&self) -> String {
        State::key(self)
    }

    fn query(&self) -> Query {
        State::query(self)
    }

    fn restore(&mut self, state: serde_json::Value) -> bool {
        match serde_json::from_value::<T>(state) {
            Ok(restored) => {
                *self = restored;
                true
            }
            Err(_) => false,
        }
    }

    fn snapshot(&self) -> Option<serde_json::Value> {
        serde_json::to_value(self).ok()
    }

    fn apply(&mut self, sequenced: &SequencedEvent) -> Result<(), Err> {
        State::apply(self, sequenced)
    }
}

macro_rules! impl_state_set {
    ($($index:tt => $name:ident),+) => {
        impl<Err, $($name),+> StateSet for ($($name,)+)
        where
            $($name: State<Error = Err>,)+
        {
            type Error = Err;

            fn erased_mut(&mut self) -> Vec<&mut dyn ErasedState<Self::Error>> {
                vec![$(&mut self.$index),+]
            }

            fn erased(&self) -> Vec<&dyn ErasedState<Self::Error>> {
                vec![$(&self.$index),+]
            }
        }
    };
}

impl_state_set!(0 => A);
impl_state_set!(0 => A, 1 => B);
impl_state_set!(0 => A, 1 => B, 2 => C);
impl_state_set!(0 => A, 1 => B, 2 => C, 3 => D);
impl_state_set!(0 => A, 1 => B, 2 => C, 3 => D, 4 => E);

/// States loaded for one decision, plus the boundary they observed.
///
/// Every state in the set was folded from one event-store read. The combined
/// query of the set is therefore also the correct append condition: no event
/// matching any state may have been stored after [`Decision::boundary`].
#[derive(Debug)]
pub struct Decision<T> {
    states: T,
    query: Query,
    boundary: Option<SequencePosition>,
}

impl<T> Decision<T> {
    /// Returns the loaded states.
    pub fn states(&self) -> &T {
        &self.states
    }

    /// Consumes the decision and returns the loaded states.
    pub fn into_states(self) -> T {
        self.states
    }

    /// Highest event position observed while loading the states.
    pub fn boundary(&self) -> Option<SequencePosition> {
        self.boundary
    }

    /// Builds the append condition that protects this decision.
    ///
    /// The condition rejects any event matching the query used to load the
    /// states at or after the observed boundary. Returns `None` only when the
    /// combined query matches no event at all.
    pub fn append_condition(&self) -> Option<AppendCondition> {
        match &self.query {
            Query::Items(items) if items.is_empty() => None,
            query => Some(AppendCondition::new(query.clone(), self.boundary)),
        }
    }
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

/// Loads decision states by restoring snapshots and replaying newer events.
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
    /// The returned [`Decision`] carries the query and boundary that can be
    /// used as an append condition.
    pub async fn load<T>(&self, state: T) -> Result<Decision<T>, StateLoadError<T::Error>>
    where
        T: State,
    {
        let mut states = (state,);
        let (query, boundary) = self.fill(&mut states.erased_mut()).await?;

        Ok(Decision {
            states: states.0,
            query,
            boundary,
        })
    }

    /// Loads a set of states with one event-store read.
    ///
    /// The read uses the union of every state's query, starting at the oldest
    /// snapshot position. Each event is applied only to the states whose query
    /// matches it and that have not already included it in a snapshot.
    pub async fn load_set<T>(&self, mut states: T) -> Result<Decision<T>, StateLoadError<T::Error>>
    where
        T: StateSet,
    {
        let (query, boundary) = self.fill(&mut states.erased_mut()).await?;

        Ok(Decision {
            states,
            query,
            boundary,
        })
    }

    async fn fill<Err>(
        &self,
        parts: &mut [&mut dyn ErasedState<Err>],
    ) -> Result<(Query, Option<SequencePosition>), StateLoadError<Err>> {
        let mut snapshot_positions = Vec::with_capacity(parts.len());

        for part in parts.iter_mut() {
            let snapshot = self
                .snapshot_store
                .get(&part.key())
                .await
                .map_err(|error| StateLoadError::Snapshot(error.to_string()))?;

            // A snapshot that no longer matches the state shape is ignored and
            // the state is rebuilt from events.
            let position = match snapshot {
                Some(snapshot) => {
                    let position = snapshot.position;
                    part.restore(snapshot.state).then_some(position).flatten()
                }
                None => None,
            };

            snapshot_positions.push(position);
        }

        let queries: Vec<Query> = parts.iter().map(|part| part.query()).collect();
        let query = Query::combine(queries.iter().cloned());

        // A state without a snapshot needs the whole stream, and a single read
        // cannot start before the oldest snapshot.
        let from = snapshot_positions
            .iter()
            .copied()
            .collect::<Option<Vec<_>>>()
            .and_then(|positions| positions.into_iter().min())
            .map(|position| position.saturating_add(1));

        let mut replayed = vec![0u64; parts.len()];
        // A state that was restored from a snapshot has observed every event up to
        // the snapshot position, even when nothing newer is replayed.
        let mut boundary = snapshot_positions.iter().flatten().copied().min();

        let mut stream = self
            .event_store
            .read(query.clone(), from)
            .await
            .map_err(StateLoadError::<Err>::event_store)?;

        while let Some(item) = stream.next().await {
            let sequenced = item.map_err(StateLoadError::<Err>::event_store)?;
            boundary = Some(
                boundary.map_or(sequenced.position, |current: SequencePosition| {
                    current.max(sequenced.position)
                }),
            );

            for (index, part) in parts.iter_mut().enumerate() {
                if snapshot_positions[index].is_some_and(|position| sequenced.position <= position)
                {
                    continue;
                }

                if queries[index].matches(&sequenced.event) {
                    part.apply(&sequenced).map_err(StateLoadError::State)?;
                    replayed[index] += 1;
                }
            }
        }

        for (key, snapshot) in pending_snapshots(parts, &replayed, boundary, self.snapshot_store) {
            // Snapshots are an optimization: a failed write must not fail the load.
            let _ = self.snapshot_store.put(&key, snapshot).await;
        }

        Ok((query, boundary))
    }
}

fn pending_snapshots<Err, S>(
    parts: &[&mut dyn ErasedState<Err>],
    replayed: &[u64],
    boundary: Option<SequencePosition>,
    snapshot_store: &S,
) -> Vec<(String, Snapshot)>
where
    S: SnapshotStore,
{
    let mut pending = Vec::new();

    for (index, part) in parts.iter().enumerate() {
        let part = &**part;
        let replayed = replayed[index];

        if replayed == 0 || !snapshot_store.should_snapshot(replayed) {
            continue;
        }

        if let Some(value) = part.snapshot() {
            pending.push((part.key(), Snapshot::new(boundary, value)));
        }
    }

    pending
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use event_store::{Event, MockEventStore, QueryItem, SequencedEvent};
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
            Query::from_items([QueryItem::new(vec!["counter_set".to_string()], vec![])])
        }

        fn apply(&mut self, sequenced: &SequencedEvent) -> Result<(), Self::Error> {
            let value: i64 = serde_json::from_value(sequenced.event.data.clone())
                .map_err(|error| error.to_string())?;
            self.value = value;
            Ok(())
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    struct Names {
        values: Vec<String>,
    }

    impl State for Names {
        type Error = String;

        fn key(&self) -> String {
            "names:1".to_string()
        }

        fn query(&self) -> Query {
            Query::from_items([QueryItem::new(vec!["named".to_string()], vec![])])
        }

        fn apply(&mut self, sequenced: &SequencedEvent) -> Result<(), Self::Error> {
            let value: String = serde_json::from_value(sequenced.event.data.clone())
                .map_err(|error| error.to_string())?;
            self.values.push(value);
            Ok(())
        }
    }

    fn event(value: i64, position: u64) -> SequencedEvent {
        SequencedEvent {
            event: Event::new("counter_set", serde_json::json!(value), vec![]),
            position,
        }
    }

    fn named(value: &str, position: u64) -> SequencedEvent {
        SequencedEvent {
            event: Event::new("named", serde_json::json!(value), vec![]),
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
        snapshots
            .expect_put()
            .withf(|_, snapshot| snapshot.position == Some(2))
            .times(1)
            .returning(|_, _| Ok(()));

        let loader = StateLoader::new(&event_store, &snapshots);
        let decision = loader.load(Counter::default()).await.unwrap();

        assert_eq!(decision.states(), &Counter { value: 2 });
        assert_eq!(decision.boundary(), Some(2));
        assert_eq!(decision.append_condition().unwrap().after, Some(2));
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
        let decision = loader.load(Counter::default()).await.unwrap();

        assert_eq!(decision.states(), &Counter { value: 11 });
        assert_eq!(decision.boundary(), Some(7));
    }

    #[tokio::test]
    async fn keeps_snapshot_position_when_nothing_is_replayed() {
        let snapshot = Snapshot::new(Some(5), serde_json::json!({ "value": 10 }));

        let mut event_store = MockEventStore::new();
        event_store
            .expect_read()
            .withf(|_, from| *from == Some(6))
            .returning(|_, _| Ok(Box::pin(stream::empty())));

        let mut snapshots = MockSnapshotStore::new();
        snapshots
            .expect_get()
            .returning(move |_| Ok(Some(snapshot.clone())));
        snapshots.expect_should_snapshot().returning(|_| false);

        let loader = StateLoader::new(&event_store, &snapshots);
        let decision = loader.load(Counter::default()).await.unwrap();

        assert_eq!(decision.states(), &Counter { value: 10 });
        assert_eq!(decision.boundary(), Some(5));
        assert_eq!(decision.append_condition().unwrap().after, Some(5));
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
        let decision = loader.load(Counter::default()).await.unwrap();

        assert_eq!(decision.states(), &Counter { value: 3 });
    }

    #[tokio::test]
    async fn loads_a_state_set_with_one_read() {
        let mut event_store = MockEventStore::new();
        event_store
            .expect_read()
            .times(1)
            .withf(|query, from| {
                from.is_none()
                    && query.matches(&Event::new("counter_set", serde_json::json!(1), vec![]))
                    && query.matches(&Event::new("named", serde_json::json!("a"), vec![]))
                    && !query.matches(&Event::new("other", serde_json::Value::Null, vec![]))
            })
            .returning(|_, _| {
                Ok(Box::pin(stream::iter(vec![
                    Ok(event(1, 1)),
                    Ok(named("a", 2)),
                    Ok(event(3, 3)),
                ])))
            });

        let mut snapshots = MockSnapshotStore::new();
        snapshots.expect_get().returning(|_| Ok(None));
        snapshots.expect_should_snapshot().returning(|_| false);

        let loader = StateLoader::new(&event_store, &snapshots);
        let decision = loader
            .load_set((Counter::default(), Names::default()))
            .await
            .unwrap();

        let (counter, names) = decision.states();
        assert_eq!(counter, &Counter { value: 3 });
        assert_eq!(
            names,
            &Names {
                values: vec!["a".to_string()]
            }
        );

        let condition = decision.append_condition().unwrap();
        assert_eq!(condition.after, Some(3));
        assert!(condition.fail_if_events_match.matches(&Event::new(
            "counter_set",
            serde_json::json!(4),
            vec![]
        )));
        assert!(condition.fail_if_events_match.matches(&Event::new(
            "named",
            serde_json::json!("b"),
            vec![]
        )));
        assert!(!condition.fail_if_events_match.matches(&Event::new(
            "other",
            serde_json::Value::Null,
            vec![]
        )));
    }

    #[tokio::test]
    async fn applies_events_newer_than_each_snapshot() {
        let mut event_store = MockEventStore::new();
        event_store
            .expect_read()
            .withf(|_, from| from.is_none())
            .returning(|_, _| {
                Ok(Box::pin(stream::iter(vec![
                    Ok(event(99, 1)),
                    Ok(named("a", 2)),
                    Ok(event(5, 3)),
                ])))
            });

        let mut snapshots = MockSnapshotStore::new();
        snapshots.expect_get().returning(|key| match key {
            "counter:1" => Ok(Some(Snapshot::new(
                Some(2),
                serde_json::json!({ "value": 1 }),
            ))),
            _ => Ok(None),
        });
        snapshots.expect_should_snapshot().returning(|_| false);

        let loader = StateLoader::new(&event_store, &snapshots);
        let decision = loader
            .load_set((Counter::default(), Names::default()))
            .await
            .unwrap();

        let (counter, names) = decision.states();
        assert_eq!(counter, &Counter { value: 5 });
        assert_eq!(
            names,
            &Names {
                values: vec!["a".to_string()]
            }
        );
        assert_eq!(decision.boundary(), Some(3));
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
                Query::from_items([QueryItem::new(vec!["counter_set".to_string()], vec![])])
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
