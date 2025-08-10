use hft_service::{app_router, store::Store, SharedState};

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use tokio::sync::RwLock;
use tower::ServiceExt;

/// A helper for comparing floating-point numbers in tests.
fn fuzzy_assert_eq(a: f64, b: f64, message: &str) {
    const EPSILON: f64 = 1e-6;
    assert!(
        (a - b).abs() < EPSILON,
        "{}: Expected {}, got {}",
        message,
        b,
        a
    );
}

#[tokio::test]
async fn test_health_check() {
    let state = SharedState::new(RwLock::new(Store::new()));
    let app = app_router(state);

    let request = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body, json!({"status": "ok"}));
}

#[tokio::test]
async fn test_reject_batch_with_negative_prices() {
    let state = SharedState::new(RwLock::new(Store::new()));
    let app = app_router(state);
    let request_body = json!({ "symbol": "BTC-USD", "values": [68000.0, -50.0] });

    let request = Request::builder()
        .uri("/add_batch/")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&request_body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Add the usize::MAX limit here
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        body,
        json!({"error": "Negative trading prices are not allowed"})
    );
}

#[tokio::test]
async fn test_large_data_and_variable_k() {
    let state = SharedState::new(RwLock::new(Store::new()));
    let app = app_router(state);
    let symbol = "BIG-DATA";
    let total_points = 100_000_u64;
    let batch_size = 10_000_u64;

    for i in 0..(total_points / batch_size) {
        let start = i * batch_size + 1;
        let end = (i + 1) * batch_size;
        let values: Vec<f64> = (start..=end).map(|v| v as f64).collect();
        let add_request = Request::builder()
            .uri("/add_batch/")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_string(&json!({ "symbol": symbol, "values": values })).unwrap(),
            ))
            .unwrap();
        let response = app.clone().oneshot(add_request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    let test_ks = [2, 4, 5];
    for k in test_ks {
        let n = 10_u64.pow(k);
        let last = total_points as f64;
        let min_val = (total_points - n + 1) as f64;
        let expected_avg = (min_val + last) / 2.0;
        let sum_sq = |x: u64| -> f64 {
            let xf = x as f64;
            xf * (xf + 1.0) * (2.0 * xf + 1.0) / 6.0
        };
        let sum_of_squares_in_range = sum_sq(total_points) - sum_sq(total_points - n);
        let expected_e_x2 = sum_of_squares_in_range / (n as f64);
        let expected_var = expected_e_x2 - expected_avg.powi(2);

        let stats_request = Request::builder()
            .uri(format!("/stats/?symbol={}&k={}", symbol, k))
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(stats_request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Add the usize::MAX limit here as well
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let stats: Value = serde_json::from_slice(&body).unwrap();

        let msg_prefix = format!("k={}", k);
        fuzzy_assert_eq(
            stats["avg"].as_f64().unwrap(),
            expected_avg,
            &format!("Mismatch 'avg' for {}", msg_prefix),
        );
        fuzzy_assert_eq(
            stats["var"].as_f64().unwrap(),
            expected_var,
            &format!("Mismatch 'var' for {}", msg_prefix),
        );
    }
}
