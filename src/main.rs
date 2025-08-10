use hft_service::{app_router, config::Config, store::Store, SharedState};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::signal;
use tokio::sync::RwLock;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod config;

#[tokio::main]
async fn main() {
    // Load configuration
    let config = match Config::new() {
        // This call is still valid
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            std::process::exit(1);
        }
    };

    // Initialize structured logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(&config.log.level))
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    // Create shared state
    let state = SharedState::new(RwLock::new(Store::new()));

    // Create the Axum router
    let app = app_router(state);

    // Start the server
    let addr_str = format!("{}:{}", config.server.host, config.server.port);
    let addr = addr_str
        .parse::<SocketAddr>()
        .expect("Invalid server address");

    info!(address = %addr, "Server starting");

    let listener = TcpListener::bind(addr)
        .await
        .expect("Failed to bind to address");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();

    info!("Server has shut down gracefully");
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => { info!("Received Ctrl+C, initiating graceful shutdown.") },
        _ = terminate => { info!("Received terminate signal, initiating graceful shutdown.") },
    }
}
