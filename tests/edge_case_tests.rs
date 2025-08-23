use hft_service::{app_router, store::Store, SharedState};

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use tower::ServiceExt;

/// Test floating point edge cases that could cause numerical issues
#[tokio::test]
async fn test_floating_point_edge_cases() {
    let state = SharedState::new(Store::new());
    let app = app_router(state);

    // Test with very large numbers
    let large_values = vec![1e15, 1e16, 1e17];
    let request_body = json!({ "symbol": "LARGE-NUM", "values": large_values });
    let request = Request::builder()
        .uri("/add_batch/")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&request_body).unwrap()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Verify stats are calculated correctly
    let stats_request = Request::builder()
        .uri("/stats/?symbol=LARGE-NUM&exponent=1")
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(stats_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let stats: Value = serde_json::from_slice(&body).unwrap();
    assert!(stats["min"].as_f64().unwrap().is_finite());
    assert!(stats["max"].as_f64().unwrap().is_finite());
    assert!(stats["avg"].as_f64().unwrap().is_finite());
    assert!(stats["var"].as_f64().unwrap().is_finite());

    // Test with very small numbers
    let small_values = vec![1e-15, 1e-16, 1e-17];
    let request_body = json!({ "symbol": "SMALL-NUM", "values": small_values });
    let request = Request::builder()
        .uri("/add_batch/")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&request_body).unwrap()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

/// Test rejection of infinite and NaN values
#[tokio::test]
async fn test_infinite_and_nan_rejection() {
    let state = SharedState::new(Store::new());
    let app = app_router(state);

    // Test with infinity - JSON doesn't support infinity, so this tests the API layer
    let request_body = r#"{"symbol": "INF-TEST", "values": [100.0, "Infinity", 200.0]}"#;
    let request = Request::builder()
        .uri("/add_batch/")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(request_body))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    // Should fail due to invalid JSON (422 Unprocessable Entity is also acceptable)
    assert!(
        response.status() == StatusCode::BAD_REQUEST
            || response.status() == StatusCode::UNPROCESSABLE_ENTITY,
        "Expected 400 or 422, got {}",
        response.status()
    );

    // Test with NaN - JSON doesn't support NaN either
    let request_body = r#"{"symbol": "NAN-TEST", "values": [100.0, "NaN", 200.0]}"#;
    let request = Request::builder()
        .uri("/add_batch/")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(request_body))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    // Should fail due to invalid JSON (422 Unprocessable Entity is also acceptable)
    assert!(
        response.status() == StatusCode::BAD_REQUEST
            || response.status() == StatusCode::UNPROCESSABLE_ENTITY,
        "Expected 400 or 422, got {}",
        response.status()
    );
}

/// Test with exact batch size limits
#[tokio::test]
async fn test_exact_batch_size_limits() {
    let state = SharedState::new(Store::new());
    let app = app_router(state);

    // Test with exactly 10,000 values (should succeed)
    let max_values: Vec<f64> = vec![100.0; 10_000];
    let request_body = json!({ "symbol": "MAX-BATCH", "values": max_values });
    let request = Request::builder()
        .uri("/add_batch/")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&request_body).unwrap()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Test with exactly 10,001 values (should fail)
    let over_max_values: Vec<f64> = vec![100.0; 10_001];
    let request_body = json!({ "symbol": "OVER-BATCH", "values": over_max_values });
    let request = Request::builder()
        .uri("/add_batch/")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&request_body).unwrap()))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// Test with single value batches
#[tokio::test]
async fn test_single_value_batches() {
    let state = SharedState::new(Store::new());
    let app = app_router(state);

    // Add a single value
    let request_body = json!({ "symbol": "SINGLE", "values": [42.5] });
    let request = Request::builder()
        .uri("/add_batch/")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&request_body).unwrap()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Query stats for single value
    let stats_request = Request::builder()
        .uri("/stats/?symbol=SINGLE&exponent=1")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(stats_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let stats: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(stats["min"].as_f64().unwrap(), 42.5);
    assert_eq!(stats["max"].as_f64().unwrap(), 42.5);
    assert_eq!(stats["last"].as_f64().unwrap(), 42.5);
    assert_eq!(stats["avg"].as_f64().unwrap(), 42.5);
    assert_eq!(stats["var"].as_f64().unwrap(), 0.0); // Variance of single value is 0
}

/// Test with zero values (should be rejected)
#[tokio::test]
async fn test_zero_values_allowed() {
    let state = SharedState::new(Store::new());
    let app = app_router(state);

    // Zero should be allowed as it's not negative
    let request_body = json!({ "symbol": "ZERO-TEST", "values": [0.0, 1.0, 0.0] });
    let request = Request::builder()
        .uri("/add_batch/")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&request_body).unwrap()))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

/// Test with exactly negative values near zero
#[tokio::test]
async fn test_negative_boundary_values() {
    let state = SharedState::new(Store::new());
    let app = app_router(state);

    // Test with -0.0 (should be treated as 0.0 and allowed)
    let request_body = json!({ "symbol": "NEG-ZERO", "values": [-0.0, 1.0] });
    let request = Request::builder()
        .uri("/add_batch/")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&request_body).unwrap()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Test with very small negative number (should be rejected)
    let request_body = json!({ "symbol": "TINY-NEG", "values": [-0.000001, 1.0] });
    let request = Request::builder()
        .uri("/add_batch/")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&request_body).unwrap()))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// Test exponent boundary values
#[tokio::test]
async fn test_exponent_boundaries() {
    let state = SharedState::new(Store::new());
    let app = app_router(state);

    // Add some test data first
    let request_body = json!({ "symbol": "EXP-TEST", "values": (1..=100).map(|i| i as f64).collect::<Vec<f64>>() });
    let request = Request::builder()
        .uri("/add_batch/")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&request_body).unwrap()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Test exponent = 1 (minimum valid)
    let stats_request = Request::builder()
        .uri("/stats/?symbol=EXP-TEST&exponent=1")
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(stats_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Test exponent = 8 (maximum valid)
    let stats_request = Request::builder()
        .uri("/stats/?symbol=EXP-TEST&exponent=8")
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(stats_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Test exponent = 0 (should fail)
    let stats_request = Request::builder()
        .uri("/stats/?symbol=EXP-TEST&exponent=0")
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(stats_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Test exponent = 9 (should fail)
    let stats_request = Request::builder()
        .uri("/stats/?symbol=EXP-TEST&exponent=9")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(stats_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// Test with very long symbol names
#[tokio::test]
async fn test_long_symbol_names() {
    let state = SharedState::new(Store::new());
    let app = app_router(state);

    // Test with a very long symbol name
    let long_symbol = "A".repeat(1000);
    let request_body = json!({ "symbol": long_symbol, "values": [100.0] });
    let request = Request::builder()
        .uri("/add_batch/")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&request_body).unwrap()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    // Should still work - no explicit length limit in the code
    assert_eq!(response.status(), StatusCode::OK);

    // Verify we can query it back
    let encoded_symbol = urlencoding::encode(&long_symbol);
    let stats_request = Request::builder()
        .uri(format!("/stats/?symbol={}&exponent=1", encoded_symbol))
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(stats_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

/// Test with special characters in symbol names
#[tokio::test]
async fn test_special_character_symbols() {
    let state = SharedState::new(Store::new());
    let app = app_router(state);

    let special_symbols = vec![
        "BTC-USD",
        "EUR/USD",
        "SPY_INDEX",
        "SYMBOL.WITH.DOTS",
        "日本円-USD", // Unicode characters
        "SYMBOL WITH SPACES",
    ];

    for symbol in special_symbols {
        let request_body = json!({ "symbol": symbol, "values": [100.0] });
        let request = Request::builder()
            .uri("/add_batch/")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&request_body).unwrap()))
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "Failed for symbol: {}",
            symbol
        );

        // Verify we can query it back
        let encoded_symbol = urlencoding::encode(symbol);
        let stats_request = Request::builder()
            .uri(format!("/stats/?symbol={}&exponent=1", encoded_symbol))
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(stats_request).await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "Query failed for symbol: {}",
            symbol
        );
    }
}

/// Test with empty symbol name
#[tokio::test]
async fn test_empty_symbol_name() {
    let state = SharedState::new(Store::new());
    let app = app_router(state);

    // Test with empty string symbol
    let request_body = json!({ "symbol": "", "values": [100.0] });
    let request = Request::builder()
        .uri("/add_batch/")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&request_body).unwrap()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    // Should work - empty string is a valid symbol
    assert_eq!(response.status(), StatusCode::OK);

    // Verify we can query it back
    let stats_request = Request::builder()
        .uri("/stats/?symbol=&exponent=1")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(stats_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

/// Test malformed JSON requests
#[tokio::test]
async fn test_malformed_json() {
    let state = SharedState::new(Store::new());
    let app = app_router(state);

    let malformed_requests = vec![
        r#"{"symbol": "TEST", "values": [1,2,3"#, // Missing closing bracket
        r#"{"symbol": "TEST", "values": [1,2,]}"#, // Trailing comma
        r#"{"symbol": "TEST", "values": [1,2,3.}}"#, // Invalid number format
        r#"{"symbol": "TEST" "values": [1,2,3]}"#, // Missing comma
        r#"{"symbol": "TEST", "values": "not_an_array"}"#, // Wrong type
    ];

    for (i, malformed_json) in malformed_requests.iter().enumerate() {
        let request = Request::builder()
            .uri("/add_batch/")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(*malformed_json))
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();

        // Should return 400 or 422 for malformed JSON
        assert!(
            response.status() == StatusCode::BAD_REQUEST
                || response.status() == StatusCode::UNPROCESSABLE_ENTITY,
            "Request {} should have failed with 400 or 422, got {}: {}",
            i,
            response.status(),
            malformed_json
        );
    }
}

/// Test missing required fields in JSON
#[tokio::test]
async fn test_missing_json_fields() {
    let state = SharedState::new(Store::new());
    let app = app_router(state);

    // Missing symbol field
    let request = Request::builder()
        .uri("/add_batch/")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"values": [1,2,3]}"#))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert!(
        response.status() == StatusCode::BAD_REQUEST
            || response.status() == StatusCode::UNPROCESSABLE_ENTITY,
        "Missing symbol should fail with 400 or 422, got {}",
        response.status()
    );

    // Missing values field
    let request = Request::builder()
        .uri("/add_batch/")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"symbol": "TEST"}"#))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert!(
        response.status() == StatusCode::BAD_REQUEST
            || response.status() == StatusCode::UNPROCESSABLE_ENTITY,
        "Missing values should fail with 400 or 422, got {}",
        response.status()
    );

    // Empty JSON object
    let request = Request::builder()
        .uri("/add_batch/")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(r#"{}"#))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert!(
        response.status() == StatusCode::BAD_REQUEST
            || response.status() == StatusCode::UNPROCESSABLE_ENTITY,
        "Empty JSON should fail with 400 or 422, got {}",
        response.status()
    );
}
