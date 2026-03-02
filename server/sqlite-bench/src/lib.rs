//! SQLite In-Memory Latency Benchmark
//!
//! Evaluates whether SQLite (`:memory:` with WAL mode) can serve as a viable
//! replacement for the server's `.dat` files loaded into flat arrays, while
//! meeting the 36 TPS (27.78 ms/tick) latency budget.
//!
//! Two schema strategies are tested:
//! - **BLOB schema**: array fields stored as fixed-size BLOBs in flat tables
//! - **Normalized schema**: array fields broken out into relational sub-tables
//!
//! Run benchmarks: `cargo bench`
//! Run tests: `cargo test`

pub mod populate;
pub mod report;
pub mod schema;
pub mod tick_sim;
