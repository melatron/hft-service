use hft_service::{app_router, config::Config, store::Store, SharedState};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::signal;
use tracing::{error, info};
use tracing_appender::non_blocking;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // Load configuration
    let config = match Config::new() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("FATAL: Failed to load configuration: {}", e);
            std::process::exit(1);
        }
    };

    // Set up logging to a rotating file
    let file_appender = tracing_appender::rolling::daily("logs", "app.log");
    let (non_blocking_writer, _guard) = non_blocking(file_appender);

    // Initialize structured logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(&config.log.level))
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_writer(non_blocking_writer),
        )
        .init();

    // Create shared state
    let state = SharedState::new(Store::new());

    // Create the Axum router from the library
    let app = app_router(state);

    // Start the server
    let addr_str = format!("{}:{}", config.server.host, config.server.port);
    let addr: SocketAddr = match addr_str.parse() {
        Ok(addr) => addr,
        Err(_) => {
            error!(address = %addr_str, "Invalid server address format");
            return;
        }
    };

    info!(address = %addr, "Server starting");

    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            error!(address = %addr, error = %e, "Failed to bind to address");
            return;
        }
    };

    if let Err(e) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        error!(error = %e, "Server error");
    }

    info!("Server has shut down gracefully");
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = signal::ctrl_c().await {
            error!(error = %e, "Failed to install Ctrl+C handler");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut stream) => stream.recv().await,
            Err(e) => {
                error!(error = %e, "Failed to install terminate signal handler");
                None
            }
        };
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => { info!("Received Ctrl+C, initiating graceful shutdown.") },
        _ = terminate => { info!("Received terminate signal, initiating graceful shutdown.") },
    }
}
