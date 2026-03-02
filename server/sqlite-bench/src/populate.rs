//! Data population: generates realistic game state for SQLite benchmarking.
//!
//! Can load actual `.dat` files from disk (via `MAG_DAT_PATH` env var) or
//! generate synthetic data with realistic occupancy patterns.

use crate::schema::PopulationParams;
use mag_core::constants::{MAXCHARS, MAXEFFECT, MAXITEM, SERVER_MAPX, SERVER_MAPY};
use mag_core::types::{Character, Effect, Global, Item, Map};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// All game data needed to populate a database.
pub struct GameData {
    pub characters: Vec<Character>,
    pub items: Vec<Item>,
    pub map: Vec<Map>,
    pub effects: Vec<Effect>,
    pub globals: Global,
}

/// Generate synthetic game data matching the given population parameters.
///
/// Uses a fixed seed for deterministic, reproducible benchmarks.
pub fn generate_synthetic(params: &PopulationParams) -> GameData {
    let mut rng = StdRng::seed_from_u64(0xDEAD_BEEF_CAFE_1337);
    let mapx = SERVER_MAPX as usize;
    let mapy = SERVER_MAPY as usize;

    // ── Characters ──────────────────────────────────────────────────
    let mut characters = vec![zeroed_character(); params.max_characters];

    let total_active = params.total_active_characters();
    for i in 1..=total_active.min(params.max_characters - 1) {
        let ch = &mut characters[i];
        ch.used = 1; // USE_ACTIVE
        ch.x = rng.gen_range(50..974) as i16;
        ch.y = rng.gen_range(50..974) as i16;
        ch.sprite = rng.gen_range(1..500);
        ch.speed = rng.gen_range(1..20);
        ch.a_hp = rng.gen_range(10..500);
        ch.a_end = rng.gen_range(10..300);
        ch.a_mana = rng.gen_range(0..200);
        ch.status = 0;
        ch.light = rng.gen_range(0..10);
        ch.flags = if i <= params.active_players {
            1 // Player flag
        } else {
            0
        };

        // Fill name with something
        let name = format!("char_{:04}", i);
        let name_bytes = name.as_bytes();
        let len = name_bytes.len().min(39);
        ch.name[..len].copy_from_slice(&name_bytes[..len]);

        // Fill some attribs
        for attr in ch.attrib.iter_mut() {
            for v in attr.iter_mut() {
                *v = rng.gen_range(1..50);
            }
        }

        // Fill hp/end/mana stats
        for v in ch.hp.iter_mut() {
            *v = rng.gen_range(50..500);
        }
        for v in ch.end.iter_mut() {
            *v = rng.gen_range(50..300);
        }
        for v in ch.mana.iter_mut() {
            *v = rng.gen_range(0..200);
        }

        // Fill some skills
        for si in 0..50 {
            ch.skill[si][0] = rng.gen_range(0..100);
            ch.skill[si][5] = ch.skill[si][0]; // total = base
        }

        // Fill some item/worn/spell slots with valid-looking item ids
        for slot in 0..10 {
            ch.item[slot] = rng.gen_range(1..params.active_items as u32);
        }
        for slot in 0..5 {
            ch.worn[slot] = rng.gen_range(1..params.active_items as u32);
        }

        // Fill some driver data
        ch.data[19] = rng.gen_range(0..10);
        ch.data[25] = rng.gen_range(0..5);
        ch.data[36] = rng.gen_range(0..1000);

        ch.attack_cn = if rng.gen_bool(0.3) {
            rng.gen_range(1..total_active as u16)
        } else {
            0
        };
        ch.goto_x = if rng.gen_bool(0.5) {
            rng.gen_range(50..974)
        } else {
            0
        };
        ch.goto_y = if ch.goto_x != 0 {
            rng.gen_range(50..974)
        } else {
            0
        };
    }

    // ── Items ────────────────────────────────────────────────────────
    let mut items = vec![zeroed_item(); params.max_items];

    for i in 1..=params.active_items.min(params.max_items - 1) {
        let it = &mut items[i];
        it.used = 1;
        it.value = rng.gen_range(1..10000);
        it.sprite[0] = rng.gen_range(1..500) as i16;
        it.status[0] = 0;
        it.flags = rng.gen_range(0..u64::MAX);

        let name = format!("item_{:05}", i);
        let name_bytes = name.as_bytes();
        let len = name_bytes.len().min(39);
        it.name[..len].copy_from_slice(&name_bytes[..len]);

        // Some items are on the map, some are carried
        if rng.gen_bool(0.3) {
            it.x = rng.gen_range(50..974);
            it.y = rng.gen_range(50..974);
            it.carried = 0;
        } else {
            it.carried = rng.gen_range(1..total_active as u16);
        }

        // Fill modifiers
        for attr in it.attrib.iter_mut() {
            for v in attr.iter_mut() {
                *v = rng.gen_range(-5..10);
            }
        }
        for sk in it.skill.iter_mut() {
            if rng.gen_bool(0.1) {
                sk[0] = rng.gen_range(-3..5);
            }
        }
        it.armor[0] = rng.gen_range(0..20);
        it.weapon[0] = rng.gen_range(0..30);
        it.light[0] = rng.gen_range(0..5);
    }

    // ── Map ─────────────────────────────────────────────────────────
    let mut map = vec![Map::default(); mapx * mapy];

    // Place characters on map
    for i in 1..=total_active.min(params.max_characters - 1) {
        let ch = &characters[i];
        if ch.used != 0 {
            let idx = ch.x as usize + ch.y as usize * mapx;
            if idx < map.len() {
                map[idx].ch = i as u32;
            }
        }
    }

    // Place items on map
    for i in 1..=params.active_items.min(params.max_items - 1) {
        let it = &items[i];
        if it.used != 0 && it.carried == 0 && it.x > 0 && it.y > 0 {
            let idx = it.x as usize + it.y as usize * mapx;
            if idx < map.len() && map[idx].it == 0 {
                map[idx].it = i as u32;
            }
        }
    }

    // Fill some random tile sprites and flags
    for tile in map.iter_mut() {
        tile.sprite = rng.gen_range(1..200);
        tile.fsprite = if rng.gen_bool(0.3) {
            rng.gen_range(1..100)
        } else {
            0
        };
        tile.dlight = 100;
        tile.light = rng.gen_range(0..50);
    }

    // ── Effects ─────────────────────────────────────────────────────
    let mut effects = vec![Effect::default(); params.max_effects];

    for i in 1..=params.active_effects.min(params.max_effects - 1) {
        let eff = &mut effects[i];
        eff.used = 1;
        eff.effect_type = rng.gen_range(1..10);
        eff.duration = rng.gen_range(10..1000);
        eff.data[0] = rng.gen_range(50..974); // x
        eff.data[1] = rng.gen_range(50..974); // y
    }

    // ── Global ──────────────────────────────────────────────────────
    let globals = Global {
        ticker: 1000,
        uptime: 360000,
        players_online: params.active_players as i32,
        character_cnt: total_active as i32,
        item_cnt: params.active_items as i32,
        effect_cnt: params.active_effects as i32,
        dlight: 100,
        ..Default::default()
    };

    GameData {
        characters,
        items,
        map,
        effects,
        globals,
    }
}

/// Try to load real `.dat` files. Returns `None` if path not available or files missing.
pub fn try_load_dat_files(dat_path: &str) -> Option<GameData> {
    use bincode::Decode;
    use std::fs;
    use std::path::Path;

    let base = Path::new(dat_path);
    if !base.exists() {
        return None;
    }

    #[derive(Debug, Decode)]
    struct NormalizedDataSet<T> {
        #[allow(dead_code)]
        magic: [u8; 4],
        #[allow(dead_code)]
        version: u32,
        #[allow(dead_code)]
        source_file: String,
        #[allow(dead_code)]
        source_record_size: usize,
        records: Vec<T>,
    }

    fn load_dat<T: Decode<()>>(path: &Path, expected_count: usize) -> Option<Vec<T>> {
        let data = fs::read(path).ok()?;
        let config = bincode::config::standard();
        let (dataset, _): (NormalizedDataSet<T>, usize) =
            bincode::decode_from_slice(&data, config).ok()?;
        if dataset.records.len() == expected_count {
            Some(dataset.records)
        } else {
            eprintln!(
                "Warning: {} has {} records, expected {}",
                path.display(),
                dataset.records.len(),
                expected_count
            );
            // Pad or truncate to expected count
            let mut records = dataset.records;
            records.resize_with(expected_count, || unsafe { core::mem::zeroed() });
            Some(records)
        }
    }

    let characters: Vec<Character> = load_dat(&base.join("char.dat"), MAXCHARS)?;
    let items: Vec<Item> = load_dat(&base.join("item.dat"), MAXITEM)?;
    let map: Vec<Map> = load_dat(
        &base.join("map.dat"),
        SERVER_MAPX as usize * SERVER_MAPY as usize,
    )?;
    let effects: Vec<Effect> = load_dat(&base.join("effect.dat"), MAXEFFECT)?;
    let globals_vec: Vec<Global> = load_dat(&base.join("global.dat"), 1)?;

    Some(GameData {
        characters,
        items,
        map,
        effects,
        globals: globals_vec.into_iter().next().unwrap_or_default(),
    })
}

/// Load game data: tries real files first (via MAG_DAT_PATH), falls back to synthetic.
pub fn load_or_generate(params: &PopulationParams) -> GameData {
    if let Ok(path) = std::env::var("MAG_DAT_PATH") {
        if let Some(data) = try_load_dat_files(&path) {
            eprintln!("Loaded real .dat files from {}", path);
            return data;
        }
        eprintln!(
            "Warning: MAG_DAT_PATH={} but could not load files, falling back to synthetic data",
            path
        );
    }

    eprintln!(
        "Generating synthetic data: {} players, {} NPCs, {} items, {} effects",
        params.active_players, params.active_npcs, params.active_items, params.active_effects
    );
    generate_synthetic(params)
}

/// Create a zeroed Character (matches USE_EMPTY semantics).
fn zeroed_character() -> Character {
    // SAFETY: Character is a plain-old-data struct (all fields are numeric/array).
    unsafe { std::mem::zeroed() }
}

/// Create a zeroed Item (matches USE_EMPTY semantics).
fn zeroed_item() -> Item {
    // SAFETY: Item is a plain-old-data struct (all fields are numeric/array).
    unsafe { std::mem::zeroed() }
}
