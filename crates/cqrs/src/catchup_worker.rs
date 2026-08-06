use event_sourcing::{Direction, EventStore, ReadOptions};
use futures_util::StreamExt;
use futures_util::future::join_all;
use std::time::Duration;
use tracing::{debug, error, info};

use crate::view::{View, ViewError};
use crate::view_checkpoint::{CheckpointError, CheckpointStore};

/// Background worker that manages a list of registered [`View<C>`]s and catches them up to the latest events in parallel.
pub struct CatchupWorker<C, ES, CP>
where
    C: Send + Sync + 'static,
    ES: EventStore,
    CP: CheckpointStore<C>,
{
    ctx: C,
    event_store: ES,
    checkpoint_store: CP,
    views: Vec<Box<dyn View<C>>>,
}

impl<C, ES, CP> CatchupWorker<C, ES, CP>
where
    C: Send + Sync + 'static,
    ES: EventStore,
    CP: CheckpointStore<C>,
{
    pub fn new(ctx: C, event_store: ES, checkpoint_store: CP) -> Self {
        Self {
            ctx,
            event_store,
            checkpoint_store,
            views: Vec::new(),
        }
    }

    /// Access reference to the underlying storage context.
    pub fn context(&self) -> &C {
        &self.ctx
    }

    /// Register a view instance `view: impl View<C>` to be caught up by this worker.
    pub fn register_view(mut self, view: impl View<C>) -> Self {
        info!(
            view_name = view.view_name(),
            "Registering view in CatchupWorker"
        );
        self.views.push(Box::new(view));
        self
    }

    /// Register a boxed view `Box<dyn View<C>>` directly.
    pub fn register_boxed_view(mut self, view: Box<dyn View<C>>) -> Self {
        info!(
            view_name = view.view_name(),
            "Registering boxed view in CatchupWorker"
        );
        self.views.push(view);
        self
    }

    /// Catch up a single registered view instance to the latest events in the event store.
    pub async fn catchup_view(&self, view: &dyn View<C>) -> Result<usize, ViewError> {
        let view_name = view.view_name();
        let current_pos = self
            .checkpoint_store
            .get_position(&self.ctx, view_name)
            .await
            .map_err(|e| {
                error!(view_name = view_name, error = %e, "Failed to read checkpoint position");
                ViewError::Execution(e.to_string())
            })?;

        debug!(view_name = view_name, start_position = ?current_pos, "Starting view catchup");

        let query = view.subscription_query();

        let mut read_opts = ReadOptions::new().direction(Direction::Forward);
        if let Some(pos) = current_pos {
            read_opts = read_opts.after(pos);
        }

        let mut stream = self.event_store.read(&query, read_opts).await;
        let mut count = 0;
        let mut last_successful_pos = current_pos;

        while let Some(event_res) = stream.next().await {
            let event = event_res.map_err(|e| {
                error!(view_name = view_name, error = %e, "Failed to read event from EventStore");
                ViewError::Execution(e.to_string())
            })?;

            debug!(
                view_name = view_name,
                event_id = %event.event.id.as_str(),
                event_type = %event.event.event_type.as_str(),
                position = %event.position,
                "Applying event to view"
            );

            if let Err(err) = view.apply_event(&event, &self.ctx).await {
                error!(
                    view_name = view_name,
                    event_id = %event.event.id.as_str(),
                    position = %event.position,
                    error = %err,
                    "View event projection failed"
                );

                // Commit checkpoint up to the last successful event before failing
                if let (Some(last_pos), true) = (last_successful_pos, count > 0) {
                    let _ = self
                        .checkpoint_store
                        .save_position(&self.ctx, view_name, last_pos)
                        .await;
                }
                return Err(err);
            }

            last_successful_pos = Some(event.position);
            count += 1;
        }

        if let (Some(pos), true) = (last_successful_pos, count > 0) {
            self.checkpoint_store
                .save_position(&self.ctx, view_name, pos)
                .await
                .map_err(|e: CheckpointError| {
                    error!(view_name = view_name, position = %pos, error = %e, "Failed to save checkpoint position");
                    ViewError::Execution(e.to_string())
                })?;

            info!(
                view_name = view_name,
                processed_count = count,
                new_position = %pos,
                "View catchup completed successfully"
            );
        } else {
            debug!(view_name = view_name, "No new events to project for view");
        }

        Ok(count)
    }

    /// Catch up all registered views concurrently in parallel.
    /// Returns total number of events processed across all views.
    pub async fn catchup_all(&self) -> Result<usize, ViewError> {
        debug!(
            view_count = self.views.len(),
            "Starting parallel catchup for all registered views"
        );

        let futures = self.views.iter().map(|view| self.catchup_view(&**view));
        let results = join_all(futures).await;

        let mut total_processed = 0;
        for res in results {
            match res {
                Ok(count) => total_processed += count,
                Err(err) => {
                    error!(error = %err, "Parallel view catchup encountered an error");
                    return Err(err);
                }
            }
        }

        if total_processed > 0 {
            info!(
                total_processed = total_processed,
                view_count = self.views.len(),
                "Completed parallel catchup batch across all views"
            );
        }

        Ok(total_processed)
    }

    /// Run continuous polling loop to keep all registered views caught up in parallel.
    pub async fn run_loop(self, poll_interval: Duration) {
        info!(
            view_count = self.views.len(),
            poll_interval_ms = poll_interval.as_millis(),
            "Starting continuous CatchupWorker background loop"
        );

        loop {
            match self.catchup_all().await {
                Ok(0) => {
                    tokio::time::sleep(poll_interval).await;
                }
                Ok(_) => {
                    // Continue immediately if work was done
                }
                Err(err) => {
                    error!(error = %err, "CatchupWorker loop encountered error; sleeping before retry");
                    tokio::time::sleep(poll_interval).await;
                }
            }
        }
    }
}
