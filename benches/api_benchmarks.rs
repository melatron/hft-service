use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use hft_service::{app_router, store::Store, SharedState};
use tokio::runtime::Runtime;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::json;
use tower::ServiceExt;

// demonstrating O(log N) complexity
fn bench_get_stats_complexity(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("GET /stats complexity");

    // Test across different data sizes (N)
    for n_points in [1_000, 10_000, 100_000, 1_000_000].iter() {
        group.throughput(Throughput::Elements(*n_points as u64));

        // For each N, create a fresh app and pre-load it with data
        let state = SharedState::new(Store::new());
        let app = app_router(state.clone());
        rt.block_on(async {
            let values: Vec<f64> = (0..*n_points).map(|i| 150.0 + (i % 10) as f64).collect();
            let request = Request::builder()
                .uri("/add_batch/")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({ "symbol": "COMPLEXITY", "values": values }))
                        .unwrap(),
                ))
                .unwrap();
            let response = app.clone().oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        });

        // Benchmark the query time for this N
        group.bench_with_input(
            criterion::BenchmarkId::from_parameter(n_points),
            n_points,
            |b, _| {
                b.to_async(&rt).iter(|| async {
                    let request = Request::builder()
                        .uri("/stats/?symbol=COMPLEXITY&exponent=2") // Always query for last 100
                        .body(Body::empty())
                        .unwrap();

                    let response = black_box(app.clone().oneshot(request).await.unwrap());
                    assert_eq!(response.status(), StatusCode::OK);
                });
            },
        );
    }
    group.finish();
}

fn bench_add_batch(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let state = SharedState::new(Store::new());
    let app = app_router(state);

    let values: Vec<f64> = (0..10_000).map(|i| 150.0 + (i % 10) as f64).collect();
    let request_body = serde_json::to_string(&json!({
        "symbol": "BENCH-SYM",
        "values": values,
    }))
    .unwrap();

    c.bench_function("POST /add_batch (10k points)", |b| {
        b.to_async(&rt).iter(|| async {
            let request = Request::builder()
                .uri("/add_batch/")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(request_body.clone()))
                .unwrap();

            let response = black_box(app.clone().oneshot(request).await.unwrap());
            assert_eq!(response.status(), StatusCode::OK);
        });
    });
}

fn bench_get_stats(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let state = SharedState::new(Store::new());
    let app = app_router(state);

    rt.block_on(async {
        let batch_size = 10_000;
        for i in 0..(1_000_000 / batch_size) {
            let values: Vec<f64> = (0..batch_size)
                .map(|j| 150.0 + ((i * batch_size + j) % 10) as f64)
                .collect();
            let request = Request::builder()
                .uri("/add_batch/")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_string(&json!({ "symbol": "BENCH-SYM", "values": values }))
                        .unwrap(),
                ))
                .unwrap();
            let response = app.clone().oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }
    });

    c.bench_function("GET /stats (1M points, exponent=6)", |b| {
        b.to_async(&rt).iter(|| async {
            let request = Request::builder()
                .uri("/stats/?symbol=BENCH-SYM&exponent=6") // Query for all 1M points
                .body(Body::empty())
                .unwrap();

            let response = black_box(app.clone().oneshot(request).await.unwrap());
            assert_eq!(response.status(), StatusCode::OK);
        });
    });
}

criterion_group!(
    benches,
    bench_add_batch,
    bench_get_stats,
    bench_get_stats_complexity
);
criterion_main!(benches);
