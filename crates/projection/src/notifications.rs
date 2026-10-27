use std::time::Duration;

use sqlx::PgPool;
use sqlx::postgres::PgListener;
use tokio::sync::{broadcast, watch};
use tokio::task::JoinHandle;
use tokio::time::sleep;

/// Spawns a task that listens for PostgreSQL event notifications.
///
/// The task owns one connection from `pool`. It reconnects with exponential
/// backoff and emits one wake signal after every successful (re)connect,
/// because notifications sent while the listener was disconnected are lost.
pub fn spawn_listener(
    pool: PgPool,
    channel: impl Into<String>,
    wake: broadcast::Sender<()>,
    mut shutdown: watch::Receiver<bool>,
) -> JoinHandle<()> {
    let channel = channel.into();

    tokio::spawn(async move {
        let mut backoff = Duration::from_secs(1);

        loop {
            if *shutdown.borrow() {
                return;
            }

            let mut listener = match PgListener::connect_with(&pool).await {
                Ok(listener) => listener,
                Err(error) => {
                    tracing::warn!(%error, "failed to connect event notification listener");
                    if !wait_backoff(&mut backoff, &mut shutdown).await {
                        return;
                    }
                    continue;
                }
            };

            if let Err(error) = listener.listen(&channel).await {
                tracing::warn!(%error, channel, "failed to listen for event notifications");
                if !wait_backoff(&mut backoff, &mut shutdown).await {
                    return;
                }
                continue;
            }

            tracing::info!(channel, "listening for event notifications");
            backoff = Duration::from_secs(1);
            let _ = wake.send(());

            loop {
                tokio::select! {
                    changed = shutdown.changed() => {
                        if changed.is_err() || *shutdown.borrow() {
                            return;
                        }
                    }
                    result = listener.recv() => {
                        match result {
                            Ok(notification) => {
                                tracing::debug!(
                                    channel = notification.channel(),
                                    event_type = notification.payload(),
                                    "event notification received"
                                );
                                let _ = wake.send(());
                            }
                            Err(error) => {
                                tracing::warn!(%error, channel, "event notification listener disconnected");
                                break;
                            }
                        }
                    }
                }
            }
        }
    })
}

/// Waits for the backoff window or shutdown. Returns `false` on shutdown.
async fn wait_backoff(backoff: &mut Duration, shutdown: &mut watch::Receiver<bool>) -> bool {
    tokio::select! {
        _ = sleep(*backoff) => {
            *backoff = (*backoff * 2).min(Duration::from_secs(30));
            true
        }
        changed = shutdown.changed() => {
            changed.is_ok() && !*shutdown.borrow()
        }
    }
}
