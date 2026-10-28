//! Graceful shutdown signal handling for worker processes.

/// Waits for `Ctrl+C` or, on Unix, `SIGTERM`.
///
/// Containers and orchestrators normally stop processes with `SIGTERM`, so
/// handling it is required for a graceful checkpoint write and leader release.
pub async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut terminate = match signal(SignalKind::terminate()) {
            Ok(signal) => signal,
            Err(error) => {
                tracing::warn!(%error, "failed to install SIGTERM handler");
                let _ = tokio::signal::ctrl_c().await;
                return;
            }
        };

        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = terminate.recv() => {}
        }
    }

    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
