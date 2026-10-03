use std::net::SocketAddr;

use axum::Router;
use tokio::net::TcpListener;
use tokio::signal;

use crate::config::HttpConfig;

/// Starts the HTTP server and listens for incoming requests with graceful shutdown support.
pub async fn serve(config: &HttpConfig, router: Router) -> Result<(), std::io::Error> {
    let addr = config
        .socket_addr()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

    let listener = TcpListener::bind(addr).await?;
    tracing::info!("HTTP server listening on {}", addr);

    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
}

/// Waits for a termination signal (SIGINT / SIGTERM) to initiate graceful shutdown.
pub async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut stream) = signal::unix::signal(signal::unix::SignalKind::terminate()) {
            stream.recv().await;
        } else {
            std::future::pending::<()>().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("received SIGINT (Ctrl+C), starting graceful shutdown");
        },
        _ = terminate => {
            tracing::info!("received SIGTERM, starting graceful shutdown");
        },
    }
}
