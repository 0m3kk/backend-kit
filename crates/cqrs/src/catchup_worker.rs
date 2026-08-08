use event_sourcing::{Direction, EventStore, ReadOptions};
use futures_util::StreamExt;
use futures_util::future::join_all;
use std::time::Duration;
use tracing::{debug, error, info};

use crate::checkpoint::{CheckpointError, CheckpointStore, CheckpointStoreTx};
use crate::view::{View, ViewError};

/// Background worker that manages a list of registered [`View<C>`]s and catches them up to the latest events in parallel.
pub struct CatchupWorker<C, ES, CP>
where
    C: Send + Sync + 'static,
    ES: EventStore,
    CP: CheckpointStore,
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
    CP: CheckpointStore,
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
            .get_position(view_name)
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
                        .save_position(view_name, last_pos)
                        .await;
                }
                return Err(err);
            }

            last_successful_pos = Some(event.position);
            count += 1;
        }

        if let (Some(pos), true) = (last_successful_pos, count > 0) {
            self.checkpoint_store
                .save_position(view_name, pos)
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

pub use tx_manager::TransactionProvider;

/// Catches up a single view by self-managing the transaction lifecycle internally:
/// 1. Begins transaction: `tx_provider.begin_tx().await`
/// 2. Reads checkpoint position: `checkpoint_store.get_position_tx(&mut conn, view_name)`
/// 3. Reads event stream from `event_store` within `conn`
/// 4. Projects each event into the view using `view.apply_event(event, ctx)`
/// 5. Saves new checkpoint position: `checkpoint_store.save_position_tx(&mut conn, view_name, pos)`
/// 6. Commits transaction: `tx_provider.commit_tx(conn).await`
///
/// If any step or event projection fails, the transaction is automatically rolled back via `tx_provider.rollback_tx(conn).await`.
pub async fn catchup_view_in_tx<C, ES, CP, TP>(
    ctx: &C,
    view: &dyn View<C>,
    event_store: &ES,
    checkpoint_store: &CP,
    tx_provider: &TP,
    limit: Option<usize>,
) -> Result<usize, ViewError>
where
    C: Send + Sync + 'static,
    TP: TransactionProvider,
    ES: event_sourcing::EventStoreTx<TP::Conn>,
    CP: CheckpointStoreTx<TP::Conn>,
{
    let view_name = view.view_name();
    let mut conn = tx_provider
        .begin_tx()
        .await
        .map_err(|e| ViewError::Execution(e.to_string()))?;

    let catchup_result: Result<usize, ViewError> = async {
        let current_pos = checkpoint_store
            .get_position_tx(&mut conn, view_name)
            .await
            .map_err(|e| {
                error!(view_name = view_name, error = %e, "Failed to read checkpoint position in self-managed transaction");
                ViewError::Execution(e.to_string())
            })?;

        debug!(view_name = view_name, start_position = ?current_pos, "Starting view catchup in self-managed transaction");

        let query = view.subscription_query();

        let mut read_opts = ReadOptions::new().direction(Direction::Forward);
        if let Some(pos) = current_pos {
            read_opts = read_opts.after(pos);
        }
        if let Some(lim) = limit {
            read_opts = read_opts.limit(lim);
        }

        let events = event_store
            .read_tx(&mut conn, &query, read_opts)
            .await
            .map_err(|e| {
                error!(view_name = view_name, error = %e, "Failed to read events in self-managed transaction");
                ViewError::Execution(e.to_string())
            })?;

        let count = events.len();
        if count == 0 {
            debug!(view_name = view_name, "No new events to project for view in self-managed transaction");
            return Ok(0);
        }

        let mut last_successful_pos = current_pos;

        for event in &events {
            debug!(
                view_name = view_name,
                event_id = %event.event.id.as_str(),
                event_type = %event.event.event_type.as_str(),
                position = %event.position,
                "Applying event to view in self-managed transaction"
            );

            view.apply_event(event, ctx).await?;
            last_successful_pos = Some(event.position);
        }

        if let Some(pos) = last_successful_pos {
            checkpoint_store
                .save_position_tx(&mut conn, view_name, pos)
                .await
                .map_err(|e: CheckpointError| {
                    error!(view_name = view_name, position = %pos, error = %e, "Failed to save checkpoint position in self-managed transaction");
                    ViewError::Execution(e.to_string())
                })?;
        }

        Ok(count)
    }
    .await;

    match catchup_result {
        Ok(count) => {
            tx_provider
                .commit_tx(conn)
                .await
                .map_err(|e| ViewError::Execution(e.to_string()))?;
            if count > 0 {
                info!(
                    view_name = view_name,
                    processed_count = count,
                    "Self-managed transactional view catchup committed successfully"
                );
            }
            Ok(count)
        }
        Err(err) => {
            error!(
                view_name = view_name,
                error = %err,
                "Self-managed transactional view catchup failed; rolling back transaction"
            );
            let _ = tx_provider.rollback_tx(conn).await;
            Err(err)
        }
    }
}

/// Background worker managing multiple registered views with self-managed transaction lifecycles per batch.
///
/// Each catch-up batch chunk automatically begins, projects events, saves checkpoint positions,
/// and commits its own transaction internally without relying on external transaction management.
pub struct CatchupWorkerTx<C, ES, CP, TP>
where
    C: Send + Sync + 'static,
    TP: TransactionProvider,
    ES: event_sourcing::EventStoreTx<TP::Conn>,
    CP: CheckpointStoreTx<TP::Conn>,
{
    ctx: C,
    event_store: ES,
    checkpoint_store: CP,
    tx_provider: TP,
    views: Vec<Box<dyn View<C>>>,
    batch_size: Option<usize>,
}

impl<C, ES, CP, TP> CatchupWorkerTx<C, ES, CP, TP>
where
    C: Send + Sync + 'static,
    TP: TransactionProvider,
    ES: event_sourcing::EventStoreTx<TP::Conn>,
    CP: CheckpointStoreTx<TP::Conn>,
{
    pub fn new(ctx: C, event_store: ES, checkpoint_store: CP, tx_provider: TP) -> Self {
        Self {
            ctx,
            event_store,
            checkpoint_store,
            tx_provider,
            views: Vec::new(),
            batch_size: None,
        }
    }

    /// Configure batch size limit per transaction.
    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = Some(batch_size);
        self
    }

    /// Access reference to the storage context.
    pub fn context(&self) -> &C {
        &self.ctx
    }

    /// Register a view instance `view: impl View<C>`.
    pub fn register_view(mut self, view: impl View<C>) -> Self {
        info!(
            view_name = view.view_name(),
            "Registering view in CatchupWorkerTx"
        );
        self.views.push(Box::new(view));
        self
    }

    /// Register a boxed view `Box<dyn View<C>>`.
    pub fn register_boxed_view(mut self, view: Box<dyn View<C>>) -> Self {
        info!(
            view_name = view.view_name(),
            "Registering boxed view in CatchupWorkerTx"
        );
        self.views.push(view);
        self
    }

    /// Catch up a single registered view, managing its transaction internally (begin -> apply -> save_checkpoint -> commit).
    pub async fn catchup_view(&self, view: &dyn View<C>) -> Result<usize, ViewError> {
        catchup_view_in_tx(
            &self.ctx,
            view,
            &self.event_store,
            &self.checkpoint_store,
            &self.tx_provider,
            self.batch_size,
        )
        .await
    }

    /// Catch up all registered views in parallel, each self-managing its own transaction.
    pub async fn catchup_all(&self) -> Result<usize, ViewError> {
        debug!(
            view_count = self.views.len(),
            "Starting parallel catchup across all registered views in CatchupWorkerTx"
        );

        let futures = self.views.iter().map(|view| self.catchup_view(&**view));
        let results = join_all(futures).await;

        let mut total_processed = 0;
        for res in results {
            match res {
                Ok(count) => total_processed += count,
                Err(err) => {
                    error!(error = %err, "CatchupWorkerTx encountered an error");
                    return Err(err);
                }
            }
        }

        if total_processed > 0 {
            info!(
                total_processed = total_processed,
                view_count = self.views.len(),
                "Completed transactional catchup batch across all views"
            );
        }

        Ok(total_processed)
    }

    /// Run a background loop continuously processing views with self-managed transactions.
    pub async fn run_loop(self, poll_interval: Duration) {
        info!(
            view_count = self.views.len(),
            poll_interval_ms = poll_interval.as_millis(),
            "Starting continuous CatchupWorkerTx background loop"
        );

        loop {
            match self.catchup_all().await {
                Ok(0) => {
                    tokio::time::sleep(poll_interval).await;
                }
                Ok(_) => {}
                Err(err) => {
                    error!(error = %err, "CatchupWorkerTx loop error; sleeping before retry");
                    tokio::time::sleep(poll_interval).await;
                }
            }
        }
    }
}




