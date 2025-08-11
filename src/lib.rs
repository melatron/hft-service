use axum::{
    extract::{DefaultBodyLimit, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tracing::{error, info, instrument, warn};

// Declare modules, making them public
pub mod config;
pub mod segment_tree;
pub mod store;

use store::{Store, SymbolData};

// The central, shared application state.
pub type SharedState = Arc<Store>;

/// The maximum number of data points a symbol can hold, corresponding to 10^8.
const MAX_CAPACITY: usize = 100_000_000;

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
    exponent: u32,
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
    let add_batch_route = post(add_batch_handler).layer(DefaultBodyLimit::max(15_000_000)); // 15MB limit

    Router::new()
        .route("/health", get(health_check_handler))
        .route("/add_batch/", add_batch_route)
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

    // Get or create the data store for the symbol.
    // The `or_insert_with` closure is only run once for a new symbol.
    let mut symbol_data_guard = state
        .symbols
        .entry(payload.symbol.clone())
        .or_insert_with(|| SymbolData {
            values: Vec::new(),
            tree: segment_tree::SegmentTree::new(MAX_CAPACITY),
        });

    let SymbolData { values, tree } = &mut *symbol_data_guard;

    for value in &payload.values {
        // Check if adding this value would exceed the total capacity.
        if values.len() >= MAX_CAPACITY {
            warn!(
                symbol = %payload.symbol,
                "Maximum capacity reached. Ignoring new data points."
            );
            break; // Stop processing the rest of the batch.
        }

        values.push(*value);
        let new_index = values.len() - 1;

        tree.update(new_index, *value);
    }

    info!("Successfully added batch");
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "status": "success" })),
    ))
}

#[instrument(name = "get_stats_request", skip(state), fields(symbol = %params.symbol, exponent = %params.exponent))]
async fn get_stats_handler(
    State(state): State<SharedState>,
    Query(params): Query<StatsRequest>,
) -> Result<Json<StatsResponse>, AppError> {
    if !(1..=8).contains(&params.exponent) {
        return Err(AppError::BadRequest(
            "exponent must be an integer between 1 and 8".to_string(),
        ));
    }

    let window_size = 10_u64.pow(params.exponent) as usize;
    let (stats_node, last_value) = state.get_stats(&params.symbol, window_size)?;

    let avg = stats_node.sum / stats_node.count as f64;
    // Ensure variance is not negative due to floating point inaccuracies.
    let variance = ((stats_node.sum_of_squares / stats_node.count as f64) - avg.powi(2)).max(0.0);

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
