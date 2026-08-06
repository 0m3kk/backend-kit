use async_trait::async_trait;
use futures_util::StreamExt;
use std::ops::{Deref, DerefMut};

use crate::store::{EventStore, ReadError};
use crate::types::{Event, Query, ReadOptions, SequencePosition, SequencedEvent};

/// Trait implemented by domain decision models.
pub trait DecisionModel: Send + Sync {
    /// Returns the [`Query`] required to hydrate this decision model instance.
    fn query(&self) -> Query;

    /// Applies a historical domain [`Event`] to update internal state.
    fn apply_event(&mut self, event: &Event);
}

/// A hydrated Decision Model wrapper maintaining domain model state `M` and sequence position `last_position`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedModel<M: DecisionModel> {
    pub model: M,
    pub last_position: Option<SequencePosition>,
}

impl<M: DecisionModel> LoadedModel<M> {
    pub fn new(model: M) -> Self {
        Self {
            model,
            last_position: None,
        }
    }

    /// Applies a [`SequencedEvent`], updating domain model state and advancing `last_position`.
    pub fn apply_sequenced(&mut self, seq_event: &SequencedEvent) {
        self.model.apply_event(&seq_event.event);
        self.last_position = match self.last_position {
            Some(curr) => Some(curr.max(seq_event.position)),
            None => Some(seq_event.position),
        };
    }
}

impl<M: DecisionModel> Deref for LoadedModel<M> {
    type Target = M;

    fn deref(&self) -> &Self::Target {
        &self.model
    }
}

impl<M: DecisionModel> DerefMut for LoadedModel<M> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.model
    }
}

/// Extension trait for [`EventStore`] to support decision model hydration.
#[async_trait]
pub trait EventStoreExt: EventStore {
    /// Hydrates a decision model instance from the store and returns a [`LoadedModel<M>`].
    async fn load_decision_model<M: DecisionModel>(&self, model: M) -> Result<LoadedModel<M>, ReadError> {
        let mut loaded = LoadedModel::new(model);
        let query = loaded.model.query();
        let mut stream = self.read(&query, ReadOptions::default()).await;

        while let Some(res) = stream.next().await {
            let seq_event = res?;
            loaded.apply_sequenced(&seq_event);
        }

        Ok(loaded)
    }
}

impl<T: EventStore + ?Sized> EventStoreExt for T {}
