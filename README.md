# High-Frequency Trading Stats Service

This project is a high-performance RESTful service built in Rust, designed to handle the rigorous demands of high-frequency trading (HFT) systems. It allows for the real-time ingestion of trading data and provides near-instantaneous statistical analysis on variable-sized windows of that data.

-----

## Core Technology & Design Choices

The service was engineered with a focus on performance, safety, and robustness.

### Language: Rust

**Rust** was chosen for its unique combination of strengths, making it ideal for HFT systems:

  - **Peak Performance**: Rust offers C-level performance with zero-cost abstractions, ensuring minimal latency.
  - **Memory Safety**: The ownership and borrow checker guarantees memory safety at compile time, eliminating entire classes of bugs like data races.
  - **Fearless Concurrency**: Rust's safety guarantees make it easier to write correct and highly efficient concurrent code.

### Data Structure: Segment Tree

The core requirement is to perform statistical analysis (`min`, `max`, `avg`, `var`) on the last `n` data points with a time complexity **better than O(n)**. The **Segment Tree** was chosen as the optimal data structure, as it can calculate all required statistics for any given range in **`O(log N)` time**, where `N` is the total number of data points.

### Implementation: Iterative vs. Recursive

This service uses a non-recursive **iterative implementation** of the Segment Tree for maximum performance. This approach avoids function call overhead and the risk of stack overflow, providing a meaningful performance advantage in a latency-sensitive HFT system.

### Concurrency Model: DashMap

To handle high volumes of concurrent requests efficiently, the service uses a **`DashMap`**. Unlike a standard `HashMap` protected by a single `RwLock`, `DashMap` provides fine-grained locking on a per-symbol basis. This eliminates a global bottleneck and allows requests for different symbols to be processed in parallel, significantly increasing throughput.

### Numeric Type: `f64`

This service deliberately uses the native `f64` type over a precision library like `rust_decimal`. In the context of HFT, **raw speed is the highest priority**, and hardware-accelerated `f64` operations are significantly faster. This is a conscious trade-off where a massive performance gain is prioritized for this specific, latency-sensitive use case.

### Error Handling

The service uses a custom `AppError` enum combined with the **`thiserror`** crate. This pattern provides compile-time, type-safe error handling and allows for precise control over the HTTP status codes and error messages returned to the client.

-----

## Production Features

This service includes several features essential for running in a production environment:

### Configuration Management

The server's behavior is configured via a `Config.toml` file and can be overridden with environment variables (e.g., `APP_SERVER__PORT=9090`). This is managed by the **`figment`** crate.

### Structured Logging

The service uses the **`tracing`** framework to emit structured (JSON-formatted) logs. The configuration writes logs to both the console and a daily rotating file in the `logs/` directory, making them easy to collect and analyze.

### Graceful Shutdown

The server listens for termination signals (`Ctrl+C` or `SIGTERM`) and will shut down gracefully, allowing any in-flight requests to complete before exiting.

### Health Check Endpoint

A `GET /health` endpoint is provided to allow load balancers and container orchestrators (like Kubernetes) to verify that the service is running and ready to accept traffic.

-----

## Testing Strategy

The project employs a comprehensive testing strategy to ensure reliability and correctness.

  - **Unit Tests**: Located in `src/`, these test individual components like the `SegmentTree` in isolation.
  - **Integration Tests**: Located in `tests/`, these validate the entire service's API, including error handling and edge cases. A dedicated stress test, marked as `#[ignore]`, verifies correctness under a load of 100 million data points.
  - **Performance Benchmarks**: Located in `benches/`, these use the **Criterion** framework to provide statistically rigorous performance measurements of the key API endpoints.

-----

## How to Build and Launch

### Prerequisites

  - The Rust toolchain (install via [rustup.rs](https://rustup.rs/))

### Build

Navigate to the project's root directory and run the build command with the `--release` flag for optimizations.

```sh
cargo build --release
```

The executable will be at `target/release/hft-service`.

### Launch

Run the service directly with Cargo or by executing the compiled binary.

```sh
# Option 1: Run using Cargo
cargo run --release

# Option 2: Run the compiled binary
./target/release/hft-service
```

The service will start on the port specified in `Config.toml` (default `8080`).

### Running Tests

```sh
# Run all standard tests
cargo test --release

# Run the ignored, resource-intensive stress test
cargo test --release -- --ignored

# Run the performance benchmarks
cargo bench
```

-----

## API Endpoints

### 1\. Health Check

Verifies that the service is running.

  - **Endpoint**: `GET /health`
  - **Success Response**:
    ```json
    {
      "status": "ok"
    }
    ```

### 2\. Add Data Batch

Adds a batch of consecutive (and non-negative) trading prices for a specific symbol.

  - **Endpoint**: `POST /add_batch/`
  - **Content-Type**: `application/json`
  - **Example (`curl`)**:
    ```sh
    curl -X POST http://localhost:8080/add_batch/ \
    -H "Content-Type: application/json" \
    -d '{"symbol": "ABC-USD", "values": [150.1, 150.5, 151.0, 149.8, 150.2, 151.1, 151.2, 152.0, 151.5, 151.9]}'
    ```

### 3\. Get Statistics

Provides statistical analysis on the last `1e{exponent}` data points. If you run the two examples below in order, you will get the exact response shown.

  - **Endpoint**: `GET /stats/`
  - **Query Parameters**:
      - `symbol`: The financial instrument's identifier.
      - `exponent`: An integer from 1 to 8.
  - **Example (`curl`)**:
    ```sh
    # Get stats for the last 1e1 (10) data points of ABC-USD
    curl "http://localhost:8080/stats/?symbol=ABC-USD&exponent=1"
    ```
  - **Success Response (Calculated from the 10 values in the batch above)**:
    ```json
    {
      "min": 149.8,
      "max": 152.0,
      "last": 151.9,
      "avg": 150.93,
      "var": 0.538009
    }
    ```
