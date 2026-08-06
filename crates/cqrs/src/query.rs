use event_sourcing::{Direction, EventStore, ReadOptions, SequencePosition};
use futures_util::StreamExt;
use std::fmt::Debug;

use crate::view::{View, ViewError};
use crate::view_checkpoint::{CheckpointError, CheckpointStore};

/// Specified consistency requirement when executing a view query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadConsistency {
    /// Return the current view state immediately without waiting (eventually consistent).
    Eventual,

    /// Catch up projection synchronously to the latest event in the Event Store before returning (strongly consistent).
    Strong,
}

/// Query Engine for views, providing on-demand projection catch-up before query execution.
pub struct ViewQueryEngine<V, C, ES, CP>
where
    V: View<C>,
    C: Send + Sync + 'static,
    ES: EventStore,
    CP: CheckpointStore,
{
    view: V,
    ctx: C,
    event_store: ES,
    checkpoint_store: CP,
}

impl<V, C, ES, CP> ViewQueryEngine<V, C, ES, CP>
where
    V: View<C>,
    C: Send + Sync + 'static,
    ES: EventStore,
    CP: CheckpointStore,
{
    pub fn new(view: V, ctx: C, event_store: ES, checkpoint_store: CP) -> Self {
        Self {
            view,
            ctx,
            event_store,
            checkpoint_store,
        }
    }

    /// Access reference to the underlying storage context.
    pub fn context(&self) -> &C {
        &self.ctx
    }

    /// Access reference to the view instance.
    pub fn view(&self) -> &V {
        &self.view
    }

    /// Catch up the view projection synchronously to the latest event currently stored in the Event Store.
    pub async fn catchup(&self) -> Result<Option<SequencePosition>, ViewError> {
        let mut current_pos = self
            .checkpoint_store
            .get_position(self.view.view_name())
            .await
            .map_err(|e| ViewError::Execution(e.to_string()))?;

        let query = self.view.subscription_query();

        let mut read_opts = ReadOptions::new().direction(Direction::Forward);
        if let Some(pos) = current_pos {
            read_opts = read_opts.after(pos);
        }

        let mut stream = self.event_store.read(&query, read_opts).await;
        let mut count = 0;

        while let Some(event_res) = stream.next().await {
            let event = event_res.map_err(|e| ViewError::Execution(e.to_string()))?;
            current_pos = Some(event.position);
            self.view.apply_event(&event, &self.ctx).await?;
            count += 1;
        }

        if let (Some(pos), true) = (current_pos, count > 0) {
            self.checkpoint_store
                .save_position(self.view.view_name(), pos)
                .await
                .map_err(|e: CheckpointError| ViewError::Execution(e.to_string()))?;
        }

        Ok(current_pos)
    }

    /// Ensures view projection is caught up to `consistency` requirement, then executes closure `f` with `&C`.
    pub async fn query<F, R>(&self, consistency: ReadConsistency, f: F) -> Result<R, ViewError>
    where
        F: FnOnce(&C) -> Result<R, ViewError>,
    {
        match consistency {
            ReadConsistency::Eventual => {}
            ReadConsistency::Strong => {
                self.catchup().await?;
            }
        }

        f(&self.ctx)
    }
}
