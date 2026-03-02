//! Criterion benchmark harness: measures full tick simulation latency for both
//! SQLite schema strategies at multiple population levels.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use rusqlite::Connection;
use sqlite_bench::populate::generate_synthetic;
use sqlite_bench::schema::blob::BlobSchema;
use sqlite_bench::schema::normalized::NormalizedSchema;
use sqlite_bench::schema::{configure_connection, BenchSchema, PopulationParams};
use sqlite_bench::tick_sim::simulate_tick;
use std::time::Duration;

/// Population levels to benchmark.
fn population_levels() -> Vec<(&'static str, PopulationParams)> {
    vec![
        ("std", PopulationParams::standard()), // 50 players, 500 NPCs
        ("stressed", PopulationParams::stressed()), // 250 players, 2000 NPCs
    ]
}

/// Create an in-memory SQLite database, populate it, and return the connection.
fn setup_db(schema: &dyn BenchSchema, params: &PopulationParams) -> Connection {
    let conn = Connection::open_in_memory().expect("Failed to open in-memory SQLite");
    configure_connection(&conn).expect("Failed to configure connection");
    schema
        .create_tables(&conn)
        .expect("Failed to create tables");

    let data = generate_synthetic(params);
    schema
        .populate(
            &conn,
            &data.characters,
            &data.items,
            &data.map,
            &data.effects,
            &data.globals,
        )
        .expect("Failed to populate");

    conn
}

fn bench_tick_blob(c: &mut Criterion) {
    let schema = BlobSchema::new();
    let mut group = c.benchmark_group("tick/blob");
    group.measurement_time(Duration::from_secs(30));
    group.sample_size(50);

    for (label, params) in population_levels() {
        let conn = setup_db(&schema, &params);
        let mut tick = 0u32;

        group.bench_with_input(BenchmarkId::from_parameter(label), &params, |b, params| {
            b.iter(|| {
                tick += 1;
                simulate_tick(&conn, &schema, params, tick).expect("tick failed");
            });
        });
    }
    group.finish();
}

fn bench_tick_normalized(c: &mut Criterion) {
    let schema = NormalizedSchema::new();
    let mut group = c.benchmark_group("tick/normalized");
    group.measurement_time(Duration::from_secs(30));
    group.sample_size(50);

    for (label, params) in population_levels() {
        let conn = setup_db(&schema, &params);
        let mut tick = 0u32;

        group.bench_with_input(BenchmarkId::from_parameter(label), &params, |b, params| {
            b.iter(|| {
                tick += 1;
                simulate_tick(&conn, &schema, params, tick).expect("tick failed");
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_tick_blob, bench_tick_normalized);
criterion_main!(benches);
