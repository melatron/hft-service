use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{error, info, instrument, warn};

// Declare modules, making them public
pub mod config;
pub mod segment_tree;
pub mod store;

use store::{Store, SymbolData};

// The central, shared application state.
pub type SharedState = Arc<RwLock<Store>>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),
    #[error("Not enough data points for the given window size")]
    NotEnoughData,
    #[error("Invalid request: {0}")]
    BadRequest(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            AppError::SymbolNotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::NotEnoughData => (StatusCode::BAD_REQUEST, self.to_string()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
        };

        match &self {
            AppError::SymbolNotFound(symbol) => {
                error!(%symbol, error.message = %self, "Symbol not found")
            }
            AppError::BadRequest(reason) => {
                warn!(%reason, error.message = %self, "Bad request received")
            }
            _ => error!(error.message = %self, "Request failed"),
        }

        let body = Json(serde_json::json!({ "error": error_message }));
        (status, body).into_response()
    }
}

#[derive(Debug, Deserialize)]
struct AddBatchRequest {
    symbol: String,
    values: Vec<f64>,
}
#[derive(Debug, Deserialize)]
struct StatsRequest {
    symbol: String,
    k: u32,
}
#[derive(Serialize)]
struct StatsResponse {
    min: f64,
    max: f64,
    last: f64,
    avg: f64,
    var: f64,
}

pub fn app_router(state: SharedState) -> Router {
    Router::new()
        .route("/health", get(health_check_handler))
        .route("/add_batch/", post(add_batch_handler))
        .route("/stats/", get(get_stats_handler))
        .with_state(state)
}

#[instrument(name = "health_check")]
async fn health_check_handler() -> impl IntoResponse {
    info!("Health check successful");
    (StatusCode::OK, Json(serde_json::json!({"status": "ok"})))
}

#[instrument(name = "add_batch_request", skip(state, payload), fields(symbol = %payload.symbol, count = payload.values.len()))]
async fn add_batch_handler(
    State(state): State<SharedState>,
    Json(payload): Json<AddBatchRequest>,
) -> Result<impl IntoResponse, AppError> {
    if payload.values.is_empty() {
        return Err(AppError::BadRequest(
            "Cannot add an empty batch of values".to_string(),
        ));
    }
    if payload.values.iter().any(|&v| v < 0.0) {
        return Err(AppError::BadRequest(
            "Negative trading prices are not allowed".to_string(),
        ));
    }

    let mut store = state.write().await;
    let data = store
        .symbols
        .entry(payload.symbol.clone())
        .or_insert_with(|| SymbolData {
            values: Vec::new(),
            tree: segment_tree::SegmentTree::new(1_000_000),
        });

    for value in &payload.values {
        data.values.push(*value);
        data.tree.update(data.values.len() - 1, *value);
    }

    info!("Successfully added batch");
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "status": "success" })),
    ))
}

#[instrument(name = "get_stats_request", skip(state), fields(symbol = %params.symbol, k = %params.k))]
async fn get_stats_handler(
    State(state): State<SharedState>,
    Query(params): Query<StatsRequest>,
) -> Result<Json<StatsResponse>, AppError> {
    if !(1..=8).contains(&params.k) {
        return Err(AppError::BadRequest(
            "k must be an integer between 1 and 8".to_string(),
        ));
    }

    let store = state.read().await;
    let n = 10_u64.pow(params.k) as usize;
    let (stats_node, last_value) = store.get_stats(&params.symbol, n)?;

    let avg = stats_node.sum / stats_node.count as f64;
    let variance = (stats_node.sum_of_squares / stats_node.count as f64) - avg.powi(2);

    info!(
        retrieved_count = stats_node.count,
        "Successfully retrieved stats"
    );
    Ok(Json(StatsResponse {
        min: stats_node.min,
        max: stats_node.max,
        last: last_value,
        avg,
        var: variance,
    }))
}
