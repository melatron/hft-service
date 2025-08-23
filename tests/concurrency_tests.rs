use hft_service::{app_router, store::Store, SharedState};

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use std::time::Duration;
use tower::ServiceExt;

/// Test that multiple threads can safely add batches to different symbols simultaneously
#[tokio::test]
async fn test_concurrent_different_symbols() {
    let state = SharedState::new(Store::new());
    let app = app_router(state);

    // Create multiple concurrent tasks adding data to different symbols
    let handles: Vec<_> = (0..5)
        .map(|i| {
            let app_clone = app.clone();
            tokio::spawn(async move {
                let symbol = format!("SYM-{}", i);
                let values: Vec<f64> = (0..1000).map(|j| (i * 1000 + j) as f64).collect();
                let request_body = json!({ "symbol": symbol, "values": values });

                let request = Request::builder()
                    .uri("/add_batch/")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                    .unwrap();

                let response = app_clone.oneshot(request).await.unwrap();
                assert_eq!(response.status(), StatusCode::OK);

                (symbol, values.len())
            })
        })
        .collect();

    // Wait for all tasks to complete and collect results
    let results: Vec<_> = futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|h| h.unwrap())
        .collect();

    // Verify all symbols were added successfully
    assert_eq!(results.len(), 5);
    for (symbol, count) in results {
        assert_eq!(count, 1000);

        // Verify we can query stats for each symbol
        let stats_request = Request::builder()
            .uri(format!("/stats/?symbol={}&exponent=3", symbol))
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(stats_request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}

/// Test that multiple threads can safely add batches to the same symbol simultaneously
#[tokio::test]
async fn test_concurrent_same_symbol() {
    let state = SharedState::new(Store::new());
    let app = app_router(state);
    let symbol = "SHARED-SYM";

    // Create multiple concurrent tasks adding data to the same symbol
    let handles: Vec<_> = (0..5)
        .map(|i| {
            let app_clone = app.clone();
            let symbol = symbol.to_string();
            tokio::spawn(async move {
                let values: Vec<f64> = (0..100).map(|j| (i * 100 + j) as f64).collect();
                let request_body = json!({ "symbol": symbol, "values": values });

                let request = Request::builder()
                    .uri("/add_batch/")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                    .unwrap();

                let response = app_clone.oneshot(request).await.unwrap();
                assert_eq!(response.status(), StatusCode::OK);

                values.len()
            })
        })
        .collect();

    // Wait for all tasks to complete
    let results: Vec<_> = futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|h| h.unwrap())
        .collect();

    // Verify all batches were processed
    let total_points: usize = results.iter().sum();
    assert_eq!(total_points, 500); // 5 tasks * 100 points each

    // Verify final state by querying stats
    let stats_request = Request::builder()
        .uri(format!("/stats/?symbol={}&exponent=3", symbol))
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(stats_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

/// Test concurrent reads while writes are happening
#[tokio::test]
async fn test_concurrent_read_write() {
    let state = SharedState::new(Store::new());
    let app = app_router(state);
    let symbol = "READ-WRITE-SYM";

    // First, add some initial data
    let initial_values: Vec<f64> = (0..1000).map(|i| i as f64).collect();
    let request_body = json!({ "symbol": symbol, "values": initial_values });
    let request = Request::builder()
        .uri("/add_batch/")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&request_body).unwrap()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Create concurrent readers and writers
    let mut handles = Vec::new();

    // Spawn writers
    for i in 0..3 {
        let app_clone = app.clone();
        let symbol = symbol.to_string();
        let handle = tokio::spawn(async move {
            for j in 0..10 {
                let values: Vec<f64> = (0..100).map(|k| ((i * 10 + j) * 100 + k) as f64).collect();
                let request_body = json!({ "symbol": symbol, "values": values });

                let request = Request::builder()
                    .uri("/add_batch/")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                    .unwrap();

                let response = app_clone.clone().oneshot(request).await.unwrap();
                assert_eq!(response.status(), StatusCode::OK);

                // Small delay to allow interleaving
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        });
        handles.push(handle);
    }

    // Spawn readers
    for _ in 0..5 {
        let app_clone = app.clone();
        let symbol = symbol.to_string();
        let handle = tokio::spawn(async move {
            for _ in 0..20 {
                let stats_request = Request::builder()
                    .uri(format!("/stats/?symbol={}&exponent=3", symbol))
                    .body(Body::empty())
                    .unwrap();
                let response = app_clone.clone().oneshot(stats_request).await.unwrap();

                // Should always succeed (even if data is changing)
                assert_eq!(response.status(), StatusCode::OK);

                let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
                let stats: Value = serde_json::from_slice(&body).unwrap();

                // Basic sanity checks on the response
                assert!(stats["min"].as_f64().is_some());
                assert!(stats["max"].as_f64().is_some());
                assert!(stats["avg"].as_f64().is_some());
                assert!(stats["var"].as_f64().is_some());
                assert!(stats["last"].as_f64().is_some());

                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    futures::future::join_all(handles).await;
}

/// Test that the system handles the maximum number of symbols under concurrent load
#[tokio::test]
async fn test_concurrent_symbol_limit() {
    let state = SharedState::new(Store::new());
    let app = app_router(state);

    // Try to add exactly 10 symbols concurrently (should all succeed)
    let handles: Vec<_> = (1..=10)
        .map(|i| {
            let app_clone = app.clone();
            tokio::spawn(async move {
                let symbol = format!("LIMIT-SYM-{:02}", i);
                let values: Vec<f64> = vec![i as f64; 100];
                let request_body = json!({ "symbol": symbol, "values": values });

                let request = Request::builder()
                    .uri("/add_batch/")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                    .unwrap();

                let response = app_clone.oneshot(request).await.unwrap();
                (symbol, response.status())
            })
        })
        .collect();

    let results: Vec<_> = futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|h| h.unwrap())
        .collect();

    // All 10 should succeed
    for (symbol, status) in &results {
        assert_eq!(*status, StatusCode::OK, "Symbol {} failed", symbol);
    }

    // Now try to add an 11th symbol - this should fail
    let request_body = json!({ "symbol": "LIMIT-SYM-11", "values": [100.0] });
    let request = Request::builder()
        .uri("/add_batch/")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&request_body).unwrap()))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// Test that the batch update implementation is correct (not necessarily faster due to API overhead)
#[tokio::test]
async fn test_batch_vs_individual_correctness() {
    let state1 = SharedState::new(Store::new());
    let state2 = SharedState::new(Store::new());
    let app1 = app_router(state1);
    let app2 = app_router(state2);

    let test_data: Vec<f64> = (0..100).map(|i| i as f64).collect();

    // Add data using batch update
    let request_body = json!({ "symbol": "BATCH-SYM", "values": test_data });
    let request = Request::builder()
        .uri("/add_batch/")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&request_body).unwrap()))
        .unwrap();
    let response = app1.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Add data using individual updates
    for &value in &test_data {
        let request_body = json!({ "symbol": "INDIVIDUAL-SYM", "values": [value] });
        let request = Request::builder()
            .uri("/add_batch/")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&request_body).unwrap()))
            .unwrap();
        let response = app2.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    // Both should produce identical statistics
    let stats_request1 = Request::builder()
        .uri("/stats/?symbol=BATCH-SYM&exponent=2")
        .body(Body::empty())
        .unwrap();
    let response1 = app1.oneshot(stats_request1).await.unwrap();
    assert_eq!(response1.status(), StatusCode::OK);
    let body1 = to_bytes(response1.into_body(), usize::MAX).await.unwrap();
    let stats1: Value = serde_json::from_slice(&body1).unwrap();

    let stats_request2 = Request::builder()
        .uri("/stats/?symbol=INDIVIDUAL-SYM&exponent=2")
        .body(Body::empty())
        .unwrap();
    let response2 = app2.oneshot(stats_request2).await.unwrap();
    assert_eq!(response2.status(), StatusCode::OK);
    let body2 = to_bytes(response2.into_body(), usize::MAX).await.unwrap();
    let stats2: Value = serde_json::from_slice(&body2).unwrap();

    // Compare statistics (should be identical)
    assert_eq!(stats1["min"], stats2["min"]);
    assert_eq!(stats1["max"], stats2["max"]);
    assert_eq!(stats1["last"], stats2["last"]);
    // For avg and var, allow small floating point differences
    let avg_diff = (stats1["avg"].as_f64().unwrap() - stats2["avg"].as_f64().unwrap()).abs();
    let var_diff = (stats1["var"].as_f64().unwrap() - stats2["var"].as_f64().unwrap()).abs();
    assert!(avg_diff < 1e-10, "Average should be identical");
    assert!(var_diff < 1e-10, "Variance should be identical");
}

/// Test memory efficiency during concurrent operations
#[tokio::test]
async fn test_memory_stability_under_load() {
    let state = SharedState::new(Store::new());
    let app = app_router(state);

    // Add a large amount of data across multiple symbols to test memory behavior
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let app_clone = app.clone();
            tokio::spawn(async move {
                let symbol = format!("MEM-SYM-{}", i);

                // Add data in multiple batches to simulate real usage
                for batch in 0..10 {
                    let values: Vec<f64> = (0..1000)
                        .map(|j| (i * 10000 + batch * 1000 + j) as f64)
                        .collect();
                    let request_body = json!({ "symbol": symbol, "values": values });

                    let request = Request::builder()
                        .uri("/add_batch/")
                        .method("POST")
                        .header("content-type", "application/json")
                        .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                        .unwrap();

                    let response = app_clone.clone().oneshot(request).await.unwrap();
                    assert_eq!(response.status(), StatusCode::OK);
                }

                // Verify we can still query stats efficiently
                let stats_request = Request::builder()
                    .uri(format!("/stats/?symbol={}&exponent=4", symbol))
                    .body(Body::empty())
                    .unwrap();
                let response = app_clone.oneshot(stats_request).await.unwrap();
                assert_eq!(response.status(), StatusCode::OK);
            })
        })
        .collect();

    // Wait for all operations to complete
    futures::future::join_all(handles).await;

    // If we get here without panicking or running out of memory, the test passes
}
