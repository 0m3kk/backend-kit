use async_trait::async_trait;
use futures_util::StreamExt;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};

use crate::snapshot::{EventStoreSnapshotExt, SnapshotError, SnapshotOptions};
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// Container holding hydrated decision models, their combined query, and their maximum sequence position.
pub struct LoadedModels<M> {
    pub models: M,
    pub max_position: Option<SequencePosition>,
    pub combined_query: Query,
}

/// Wrapper for a single decision model.
pub struct Single<M>(pub M);

/// Wrapper for a pair of decision models.
pub struct Pair<M1, M2>(pub M1, pub M2);

/// Wrapper for three decision models.
pub struct Triple<M1, M2, M3>(pub M1, pub M2, pub M3);

/// Wrapper for four decision models.
pub struct Quad<M1, M2, M3, M4>(pub M1, pub M2, pub M3, pub M4);

/// Trait representing single or multiple decision models that can be hydrated together from an [`EventStore`].
#[async_trait]
pub trait DecisionModels: Send + Sync + 'static {
    type Hydrated: Send + Sync;

    /// Hydrates decision models starting from sequence position 1.
    async fn load_all<ES: EventStore>(
        self,
        event_store: &ES,
    ) -> Result<LoadedModels<Self::Hydrated>, ReadError>;

    /// Hydrates decision models using KV snapshots (if available) and catching up remaining events via [`EventStoreSnapshotExt`].
    async fn load_all_with_snapshot<ES: EventStore, KV: kv_store::KvStore>(
        self,
        event_store: &ES,
        kv: &KV,
        options: SnapshotOptions,
    ) -> Result<LoadedModels<Self::Hydrated>, SnapshotError>;
}

// -----------------------------------------------------------------------------
// DecisionModels Implementation for Single<M>
// -----------------------------------------------------------------------------

#[async_trait]
impl<M> DecisionModels for Single<M>
where
    M: DecisionModel + Serialize + DeserializeOwned + 'static,
{
    type Hydrated = M;

    async fn load_all<ES: EventStore>(
        self,
        event_store: &ES,
    ) -> Result<LoadedModels<Self::Hydrated>, ReadError> {
        let query = self.0.query();
        let loaded = event_store.load_decision_model(self.0).await?;

        Ok(LoadedModels {
            models: loaded.model,
            max_position: loaded.last_position,
            combined_query: query,
        })
    }

    async fn load_all_with_snapshot<ES: EventStore, KV: kv_store::KvStore>(
        self,
        event_store: &ES,
        kv: &KV,
        options: SnapshotOptions,
    ) -> Result<LoadedModels<Self::Hydrated>, SnapshotError> {
        let query = self.0.query();
        let loaded = event_store
            .load_decision_model_with_snapshot(kv, self.0, options)
            .await?;

        Ok(LoadedModels {
            models: loaded.model,
            max_position: loaded.last_position,
            combined_query: query,
        })
    }
}

// -----------------------------------------------------------------------------
// Declarative Macro for Multi-Model Tuple Implementations
// -----------------------------------------------------------------------------

macro_rules! impl_decision_models_tuple {
    ($( $name:ident ( $( $idx:tt : $T:ident ),+ ) ),+ $(,)?) => {
        $(
            #[async_trait]
            impl<$( $T ),+> DecisionModels for $name<$( $T ),+>
            where
                $( $T: DecisionModel + Serialize + DeserializeOwned + 'static, )+
            {
                type Hydrated = ( $( $T, )+ );

                async fn load_all<ES: EventStore>(
                    self,
                    event_store: &ES,
                ) -> Result<LoadedModels<Self::Hydrated>, ReadError> {
                    let combined_query = Query::combine(vec![ $( self.$idx.query() ),+ ]);

                    let loaded = (
                        $(
                            event_store.load_decision_model(self.$idx).await?,
                        )+
                    );

                    let positions = vec![ $( loaded.$idx.last_position ),+ ];
                    let max_pos = positions.into_iter().flatten().max();

                    Ok(LoadedModels {
                        models: ( $( loaded.$idx.model, )+ ),
                        max_position: max_pos,
                        combined_query,
                    })
                }

                async fn load_all_with_snapshot<ES: EventStore, KV: kv_store::KvStore>(
                    self,
                    event_store: &ES,
                    kv: &KV,
                    options: SnapshotOptions,
                ) -> Result<LoadedModels<Self::Hydrated>, SnapshotError> {
                    let combined_query = Query::combine(vec![ $( self.$idx.query() ),+ ]);

                    let loaded = (
                        $(
                            Single(self.$idx)
                                .load_all_with_snapshot(event_store, kv, options.clone())
                                .await?,
                        )+
                    );

                    let positions = vec![ $( loaded.$idx.max_position ),+ ];
                    let max_pos = positions.into_iter().flatten().max();

                    Ok(LoadedModels {
                        models: ( $( loaded.$idx.models, )+ ),
                        max_position: max_pos,
                        combined_query,
                    })
                }
            }
        )+
    };
}

impl_decision_models_tuple! {
    Pair(0: M1, 1: M2),
    Triple(0: M1, 1: M2, 2: M3),
    Quad(0: M1, 1: M2, 2: M3, 3: M4),
}

/// Extension trait for [`EventStore`] to support decision model hydration.
#[async_trait]
pub trait EventStoreExt: EventStore {
    /// Hydrates a decision model instance from the store and returns a [`LoadedModel<M>`].
    async fn load_decision_model<M: DecisionModel>(
        &self,
        model: M,
    ) -> Result<LoadedModel<M>, ReadError> {
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
