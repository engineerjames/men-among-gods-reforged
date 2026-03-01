//! Integration tests: verify schema creation, population, and tick simulation.

use rusqlite::Connection;
use sqlite_bench::populate::generate_synthetic;
use sqlite_bench::schema::blob::BlobSchema;
use sqlite_bench::schema::normalized::NormalizedSchema;
use sqlite_bench::schema::{configure_connection, BenchSchema, PopulationParams};
use sqlite_bench::tick_sim::simulate_tick;

fn small_params() -> PopulationParams {
    PopulationParams {
        active_players: 5,
        active_npcs: 20,
        max_characters: 100,
        max_items: 500,
        max_effects: 50,
        active_items: 50,
        active_effects: 5,
    }
}

fn setup_and_populate(schema: &dyn BenchSchema, params: &PopulationParams) -> Connection {
    let conn = Connection::open_in_memory().expect("open");
    configure_connection(&conn).expect("configure");
    schema.create_tables(&conn).expect("create_tables");

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
        .expect("populate");
    conn
}

// ── BLOB schema tests ───────────────────────────────────────────────

#[test]
fn blob_create_and_populate() {
    let schema = BlobSchema::new();
    let params = small_params();
    let conn = setup_and_populate(&schema, &params);

    // Verify row counts
    let char_count: u32 = conn
        .query_row("SELECT COUNT(*) FROM characters", [], |r| r.get(0))
        .unwrap();
    assert_eq!(char_count, params.max_characters as u32);

    let item_count: u32 = conn
        .query_row("SELECT COUNT(*) FROM items", [], |r| r.get(0))
        .unwrap();
    assert_eq!(item_count, params.max_items as u32);

    let map_count: u32 = conn
        .query_row("SELECT COUNT(*) FROM map", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        map_count,
        (mag_core::constants::SERVER_MAPX * mag_core::constants::SERVER_MAPY) as u32
    );
}

#[test]
fn blob_character_triage() {
    let schema = BlobSchema::new();
    let params = small_params();
    let conn = setup_and_populate(&schema, &params);

    let active = schema.character_triage(&conn).unwrap();
    assert_eq!(active.len(), params.total_active_characters());
}

#[test]
fn blob_read_character() {
    let schema = BlobSchema::new();
    let params = small_params();
    let conn = setup_and_populate(&schema, &params);

    let ch = schema.read_character(&conn, 1).unwrap();
    assert_eq!(ch.id, 1);
    assert_eq!(ch.used, 1);
    assert!(ch.x > 0);
}

#[test]
fn blob_read_viewport() {
    let schema = BlobSchema::new();
    let params = small_params();
    let conn = setup_and_populate(&schema, &params);

    let tiles = schema.read_viewport(&conn, 100, 100).unwrap();
    assert!(!tiles.is_empty());
    // Should get up to 34×34 = 1156 tiles
    assert!(tiles.len() <= 34 * 34);
}

#[test]
fn blob_update_character_stats() {
    let schema = BlobSchema::new();
    let params = small_params();
    let conn = setup_and_populate(&schema, &params);

    schema
        .update_character_stats(&conn, 1, 999, 888, 777, 42)
        .unwrap();

    let ch = schema.read_character(&conn, 1).unwrap();
    assert_eq!(ch.a_hp, 999);
    assert_eq!(ch.a_end, 888);
    assert_eq!(ch.a_mana, 777);
    assert_eq!(ch.status, 42);
}

#[test]
fn blob_simulate_tick() {
    let schema = BlobSchema::new();
    let params = small_params();
    let conn = setup_and_populate(&schema, &params);

    let stats = simulate_tick(&conn, &schema, &params, 1).unwrap();
    assert!(stats.reads > 0, "Expected >0 reads, got {}", stats.reads);
    assert!(stats.writes > 0, "Expected >0 writes, got {}", stats.writes);
}

// ── Normalized schema tests ─────────────────────────────────────────

#[test]
fn normalized_create_and_populate() {
    let schema = NormalizedSchema::new();
    let params = small_params();
    let conn = setup_and_populate(&schema, &params);

    let char_count: u32 = conn
        .query_row("SELECT COUNT(*) FROM characters", [], |r| r.get(0))
        .unwrap();
    assert_eq!(char_count, params.max_characters as u32);
}

#[test]
fn normalized_character_triage() {
    let schema = NormalizedSchema::new();
    let params = small_params();
    let conn = setup_and_populate(&schema, &params);

    let active = schema.character_triage(&conn).unwrap();
    assert_eq!(active.len(), params.total_active_characters());
}

#[test]
fn normalized_read_character_with_subtables() {
    let schema = NormalizedSchema::new();
    let params = small_params();
    let conn = setup_and_populate(&schema, &params);

    let ch = schema.read_character(&conn, 1).unwrap();
    assert_eq!(ch.id, 1);
    assert_eq!(ch.used, 1);
    // Normalized schema should populate attrib sub-table data
    assert!(!ch.attrib.is_empty(), "Expected attrib data from sub-table");
}

#[test]
fn normalized_read_character_slots() {
    let schema = NormalizedSchema::new();
    let params = small_params();
    let conn = setup_and_populate(&schema, &params);

    let slots = schema.read_character_slots(&conn, 1).unwrap();
    assert_eq!(slots.item.len(), 40 * 4);
    assert_eq!(slots.worn.len(), 20 * 4);
    assert_eq!(slots.spell.len(), 20 * 4);
    assert_eq!(slots.depot.len(), 62 * 4);
}

#[test]
fn normalized_simulate_tick() {
    let schema = NormalizedSchema::new();
    let params = small_params();
    let conn = setup_and_populate(&schema, &params);

    let stats = simulate_tick(&conn, &schema, &params, 1).unwrap();
    assert!(stats.reads > 0);
    assert!(stats.writes > 0);
}

// ── Cross-schema consistency ────────────────────────────────────────

#[test]
fn both_schemas_produce_same_triage_count() {
    let params = small_params();

    let blob = BlobSchema::new();
    let blob_conn = setup_and_populate(&blob, &params);
    let blob_active = blob.character_triage(&blob_conn).unwrap();

    let norm = NormalizedSchema::new();
    let norm_conn = setup_and_populate(&norm, &params);
    let norm_active = norm.character_triage(&norm_conn).unwrap();

    assert_eq!(blob_active.len(), norm_active.len());
}
