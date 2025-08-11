# High-Frequency Trading Stats Service

This project is a high-performance RESTful service built in Rust, designed to handle the rigorous demands of high-frequency trading (HFT) systems. It allows for the real-time ingestion of trading data and provides near-instantaneous statistical analysis on variable-sized windows of that data.

---

## Table of Contents
- [Core Design & Technology](#core-design--technology)
- [Production-Ready Features](#production-ready-features)
- [Testing Strategy](#testing-strategy)
- [Setup and Usage](#setup-and-usage)
- [API Reference](#api-reference)

---

## Core Design & Technology

The service was engineered with a primary focus on performance, safety, and robustness.

### Language: Rust 🦀
**Rust** was chosen as it provides a unique combination of strengths ideal for HFT systems:

-   **Peak Performance**: Offers C-level performance with zero-cost abstractions, ensuring minimal and predictable latency.
-   **Guaranteed Memory Safety**: The ownership and borrow checker eliminates entire classes of bugs (e.g., data races, null pointer dereferencing) at compile time.
-   **Fearless Concurrency**: Rust's safety guarantees make it easier to write correct, efficient, and highly parallel code.

### Data Structure: Segment Tree
To meet the requirement of performing statistical analysis (`min`, `max`, `avg`, `var`) in **better than O(n) time**, this service uses a **Segment Tree**.

This data structure is optimal for this use case, as it can calculate all required statistics for any given range in **`O(log N)` time**, where `N` is the total number of data points for a symbol. For maximum performance, this service uses a non-recursive **iterative implementation**, avoiding function call overhead and any risk of stack overflow.

### Concurrency Model: DashMap
To handle a high volume of concurrent requests, the service uses **`DashMap`** as its central data store. Unlike a standard `HashMap` protected by a single global lock, `DashMap` provides fine-grained, sharded locking. This allows requests for *different* symbols to be processed in parallel, dramatically increasing throughput.

### Numeric Type: `f64`
This service deliberately uses the native `f64` type over a fixed-precision library like `rust_decimal`. In HFT, **raw computational speed is the highest priority**, and hardware-accelerated `f64` operations are orders of magnitude faster. This is a conscious trade-off where a massive performance gain is prioritized for this latency-sensitive application.

---

## Production-Ready Features

This service includes several features essential for deployment in a production environment.

-   **Configuration Management**: Server behavior is configured via `Config.toml` and can be overridden with environment variables (e.g., `APP_SERVER__PORT=9090`), managed by the **`figment`** crate.
-   **Structured Logging**: Uses the **`tracing`** framework to emit structured (JSON) logs to both the console and a daily rotating file (`logs/app.log`), making them easy to analyze.
-   **Graceful Shutdown**: Listens for termination signals (`Ctrl+C` or `SIGTERM`) and shuts down gracefully, allowing in-flight requests to complete.
-   **Health Check**: Provides a `GET /health` endpoint for load balancers and container orchestrators (like Kubernetes) to verify service health.

---

## Testing Strategy

The project employs a comprehensive, multi-layered testing strategy to ensure reliability and correctness.

-   **Unit Tests**: Located alongside the source code in `src/`, these test individual components like the `SegmentTree` in isolation.
-   **Integration Tests**: Located in the `tests/` directory, these validate the entire service's API, including error handling and edge cases.
-   **Stress Test**: A dedicated, resource-intensive integration test (marked as `#[ignore]`) verifies correctness under a full load of 100 million data points.
-   **Performance Benchmarks**: Located in the `benches/` directory, these use the **`Criterion`** framework to provide statistically rigorous performance measurements of key API endpoints.

---

## Setup and Usage

### Prerequisites
- The Rust toolchain (install via [rustup.rs](https://rustup.rs/))

### Build & Run
1.  Clone the repository and navigate to the root directory.
2.  Build the service in release mode for maximum optimization:
    ```sh
    cargo build --release
    ```
3.  Run the compiled binary:
    ```sh
    ./target/release/hft-service
    ```
    The service will start on the port specified in `Config.toml` (default `8080`).

### Running Tests & Benchmarks
```sh
# Run all standard unit and integration tests
cargo test --release

# Run the ignored, resource-intensive stress test
cargo test --release -- --ignored

# Run the performance benchmarks
cargo bench
````

-----

## API Reference

### 1\. Health Check

Verifies that the service is running and ready to accept traffic.

  - **Endpoint**: `GET /health`
  - **Success Response** (`200 OK`):
    ```json
    {
      "status": "ok"
    }
    ```

### 2\. Add Data Batch

Adds a batch of consecutive trading prices for a specific symbol. All prices must be non-negative.

  - **Endpoint**: `POST /add_batch/`
  - **Body**: A JSON object containing a `symbol` and an array of `values`.
  - **Example `curl`**:
    ```sh
    curl -X POST http://localhost:8080/add_batch/ \
    -H "Content-Type: application/json" \
    -d '{"symbol": "ABC-USD", "values": [150.1, 150.5, 151.0, 149.8, 150.2, 151.1, 151.2, 152.0, 151.5, 151.9]}'
    ```

### 3\. Get Statistics

Provides statistical analysis on the last `1e{exponent}` data points for a given symbol.

  - **Endpoint**: `GET /stats/`
  - **Query Parameters**:
      - `symbol` (string): The financial instrument's identifier.
      - `exponent` (integer): A number from 1 to 8.
  - **Example `curl`**:
    *Get stats for the last 1e1 (10) data points of "ABC-USD" added in the previous example.*
    ```sh
    curl "http://localhost:8080/stats/?symbol=ABC-USD&exponent=1"
    ```
  - **Success Response** (`200 OK`):
    *The following values are calculated from the 10 data points in the `add_batch` example above.*
    ```json
    {
      "min": 149.8,
      "max": 152.0,
      "last": 151.9,
      "avg": 150.93,
      "var": 0.5380099999999984
    }
    ```
