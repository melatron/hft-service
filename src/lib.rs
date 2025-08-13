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
use tracing::{error, info, instrument};

// Declare modules, making them public
pub mod config;
pub mod segment_tree;
pub mod store;

use store::{Store, SymbolStats};

// The central, shared application state.
pub type SharedState = Arc<Store>;

// The maximum size of a batch we can accept in a single request.
const MAX_BATCH_SIZE: usize = 10000;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),
    #[error("Not enough data to calculate stats")]
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

// Implement conversion from our internal stats struct to the web response.
impl From<SymbolStats> for StatsResponse {
    fn from(stats: SymbolStats) -> Self {
        Self {
            min: stats.min,
            max: stats.max,
            last: stats.last,
            avg: stats.avg,
            var: stats.var,
        }
    }
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

    if payload.values.len() > MAX_BATCH_SIZE {
        return Err(AppError::BadRequest(format!(
            "Batch size cannot exceed {} values.",
            MAX_BATCH_SIZE
        )));
    }

    if payload.values.iter().any(|&v| v < 0.0) {
        return Err(AppError::BadRequest(
            "Negative trading prices are not allowed".to_string(),
        ));
    }

    // The handler now just delegates to the store.
    state.add_batch(&payload.symbol, &payload.values)?;

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

    // The handler delegates and then converts the result to the response type.
    let stats = state.get_stats(&params.symbol, window_size)?;

    info!("Successfully retrieved stats");
    Ok(Json(stats.into()))
}
