use hft_service::{app_router, store::Store, SharedState};

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use tower::ServiceExt;

/// A helper for comparing floating-point numbers using a relative epsilon.
/// This is robust for both very large and very small numbers.
fn fuzzy_assert_eq(a: f64, b: f64, message: &str) {
    let abs_diff = (a - b).abs();
    let epsilon = 1e-9; // A small relative tolerance

    // Use a relative comparison, but fall back to an absolute one for very small numbers.
    let max_val = a.abs().max(b.abs());
    let relative_epsilon = epsilon * max_val;

    assert!(
        abs_diff < relative_epsilon,
        "{}: Assertion failed: Expected {}, got {} (diff: {})",
        message,
        b,
        a,
        abs_diff
    );
}

#[tokio::test]
async fn test_health_check() {
    let state = SharedState::new(Store::new());
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
    let state = SharedState::new(Store::new());
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
async fn test_data_availability_errors() {
    let state = SharedState::new(Store::new());
    let app = app_router(state);
    let symbol = "EDGECASE-XYZ";

    // Scenario 1: Symbol does not exist
    let stats_request_nonexistent = Request::builder()
        .uri(format!("/stats/?symbol={}&exponent=1", symbol))
        .body(Body::empty())
        .unwrap();
    let response_nonexistent = app
        .clone()
        .oneshot(stats_request_nonexistent)
        .await
        .unwrap();
    assert_eq!(response_nonexistent.status(), StatusCode::NOT_FOUND);

    // Scenario 2: Symbol exists, but the requested window is larger than the available data.
    let values_to_add = vec![10.0, 20.0, 5.0, 15.0, 25.0];
    let add_request = Request::builder()
        .uri("/add_batch/")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "symbol": symbol,
                "values": values_to_add
            }))
            .unwrap(),
        ))
        .unwrap();
    let add_response = app.clone().oneshot(add_request).await.unwrap();
    assert_eq!(add_response.status(), StatusCode::OK);

    // Request 100 points (exponent=2), but only 5 are available.
    let stats_request_larger_window = Request::builder()
        .uri(format!("/stats/?symbol={}&exponent=2", symbol))
        .body(Body::empty())
        .unwrap();
    let response_larger_window = app.oneshot(stats_request_larger_window).await.unwrap();

    assert_eq!(response_larger_window.status(), StatusCode::OK);

    let body = to_bytes(response_larger_window.into_body(), usize::MAX)
        .await
        .unwrap();
    let stats: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(stats["min"].as_f64().unwrap(), 5.0);
    assert_eq!(stats["max"].as_f64().unwrap(), 25.0);
    assert_eq!(stats["last"].as_f64().unwrap(), 25.0);
    fuzzy_assert_eq(stats["avg"].as_f64().unwrap(), 15.0, "avg mismatch");
    fuzzy_assert_eq(stats["var"].as_f64().unwrap(), 50.0, "var mismatch");
}

#[tokio::test]
async fn test_exponent_out_of_range() {
    let state = SharedState::new(Store::new());
    let app = app_router(state);
    let symbol = "TEST-SYMBOL";

    let add_request = Request::builder()
        .uri("/add_batch/")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({
                "symbol": symbol,
                "values": [100.0]
            }))
            .unwrap(),
        ))
        .unwrap();

    let add_response = app.clone().oneshot(add_request).await.unwrap();
    assert_eq!(add_response.status(), StatusCode::OK);

    let stats_request = Request::builder()
        .uri(format!("/stats/?symbol={}&exponent=9", symbol))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(stats_request).await.unwrap();

    // Assert: We should get a 400 Bad Request for the invalid exponent
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("exponent must be an integer between 1 and 8"));
}

// This test is ignored by default because it is resource-intensive.
// To run it, use: cargo test --release -- --ignored
#[tokio::test]
#[ignore]
async fn test_large_data_and_variable_exponent() {
    let state = SharedState::new(Store::new());
    let app = app_router(state);
    let symbol = "BIG-DATA";
    let total_points = 100_000_000_u64;
    let batch_size = 1_000_000_u64;

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

    let test_exponents = [7, 8];
    for exponent in test_exponents {
        let window_size = 10_u64.pow(exponent);
        let last = total_points as f64;
        let min_val = (total_points - window_size + 1) as f64;
        let expected_avg = (min_val + last) / 2.0;
        let sum_sq = |x: u64| -> f64 {
            let xf = x as f64;
            xf * (xf + 1.0) * (2.0 * xf + 1.0) / 6.0
        };
        let sum_of_squares_in_range = sum_sq(total_points) - sum_sq(total_points - window_size);
        let expected_e_x2 = sum_of_squares_in_range / (window_size as f64);
        let expected_var = expected_e_x2 - expected_avg.powi(2);

        let stats_request = Request::builder()
            .uri(format!("/stats/?symbol={}&exponent={}", symbol, exponent))
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(stats_request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let stats: Value = serde_json::from_slice(&body).unwrap();

        let msg_prefix = format!("exponent={}", exponent);
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
