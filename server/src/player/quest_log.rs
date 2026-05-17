//! Quest catalog & per-player completion dispatch.
//!
//! At server boot (lazily on first use) we scan the world for every NPC
//! that has an outstanding quest item assigned (`Character::data[49] != 0`)
//! and build an immutable [`QuestCatalog`] mapping catalog index → quest
//! metadata. The catalog is broadcast once per session via
//! `SV_SETQUESTCATALOG` (opcode 100).
//!
//! Per-player completion progress lives in `Character::future2[idx]` (a
//! signed 16-bit counter, saturating-added) and is broadcast either as a
//! full snapshot at login or as a single-entry delta whenever the server
//! bumps a counter on a turn-in (`SV_SETQUESTCOMPLETION`, opcode 101).
//!
//! Heuristics for the catalog:
//! * `repeatable = true` when the NPC is the black-candle guard
//!   (`temp == 518 && data[49] == 740`).
//! * `stages = 2` when the NPC teaches `SK_STUN` or `SK_CURSE` (Seyan Du
//!   / Templar progression turns into `SK_IMMUN` / `SK_SURROUND` on the
//!   second turn-in).
//! * Everything else defaults to `stages = 1`, `repeatable = false`.

use core::constants::{MAXCHARS, MAXTITEM, USE_ACTIVE};
use core::quest_defs::{MAX_QUEST_CATALOG, QuestCatalogEntry};
use core::server_commands::{
    QUEST_CATALOG_ENTRY_LEN, QUEST_CATALOG_ITEM_NAME_LEN, QUEST_CATALOG_NPC_NAME_LEN,
    QUEST_CATALOG_PACKET_LEN, QUEST_COMPLETION_DELTA_LEN, QUEST_COMPLETION_FULL_LEN,
    ServerCommandType,
};
use core::skills;
use std::sync::{Mutex, OnceLock};

use crate::game_state::GameState;
use crate::network_manager::xsend;

/// Slot on `Character::data` that legacy NPC quests use to record the
/// template ID of the quest item the NPC is currently waiting for.
pub const QUEST_ITEM_DATA_SLOT: usize = 49;

/// Slot on `Character::data` that records the skill an NPC teaches when
/// the quest item is handed over (0 = no skill teach).
const QUEST_SKILL_DATA_SLOT: usize = 50;

/// Immutable per-server static catalog of NPC quests.
#[derive(Debug, Default, Clone)]
pub struct QuestCatalog {
    /// Catalog entries in stable order (sorted by NPC template id). Index
    /// is the key into per-player completion vectors.
    pub entries: Vec<QuestCatalogEntry>,
}

impl QuestCatalog {
    /// Look up the catalog index for an NPC template id.
    ///
    /// # Arguments
    ///
    /// * `template_id` - NPC template id to look up.
    ///
    /// # Returns
    ///
    /// * `Some(idx)` if the NPC is a known quest giver, `None` otherwise.
    pub fn index_of(&self, template_id: u16) -> Option<u8> {
        self.entries
            .iter()
            .position(|e| e.template_id == template_id)
            .map(|i| i as u8)
    }

    /// Borrow the entry at a given catalog index.
    ///
    /// # Arguments
    ///
    /// * `idx` - Catalog index.
    ///
    /// # Returns
    ///
    /// * `Some(&entry)` if `idx` is in range, `None` otherwise.
    pub fn entry(&self, idx: u8) -> Option<&QuestCatalogEntry> {
        self.entries.get(idx as usize)
    }

    /// Build the catalog by scanning `gs` for active NPCs with a quest
    /// item assignment.
    ///
    /// # Arguments
    ///
    /// * `gs` - Game state (read-only).
    ///
    /// # Returns
    ///
    /// * Deterministic catalog (sorted by `temp`) capped at
    ///   [`MAX_QUEST_CATALOG`] entries. Duplicates by `temp` are folded.
    pub fn build_from_gamestate(gs: &GameState) -> Self {
        let mut seen: std::collections::BTreeMap<u16, QuestCatalogEntry> =
            std::collections::BTreeMap::new();
        for cn in 1..MAXCHARS {
            let ch = &gs.characters[cn];
            if ch.used != USE_ACTIVE {
                continue;
            }
            if ch.temp == 0 {
                continue;
            }
            let quest_item = ch.data[QUEST_ITEM_DATA_SLOT];
            if quest_item == 0 {
                continue;
            }
            if seen.contains_key(&ch.temp) {
                continue;
            }
            let item_template_id = quest_item as u16;
            let item_name = if (quest_item as usize) < MAXTITEM
                && gs.item_templates[quest_item as usize].used != core::constants::USE_EMPTY
            {
                gs.item_templates[quest_item as usize].get_name().to_owned()
            } else {
                String::new()
            };
            let repeatable = ch.temp == 518 && quest_item == 740;
            let stages = stage_count_for(ch.data[QUEST_SKILL_DATA_SLOT]);
            seen.insert(
                ch.temp,
                QuestCatalogEntry {
                    template_id: ch.temp,
                    item_template_id,
                    npc_x: ch.x.max(0) as u16,
                    npc_y: ch.y.max(0) as u16,
                    stages,
                    repeatable,
                    npc_name: ch.get_name().to_owned(),
                    item_name,
                },
            );
        }
        let mut entries: Vec<QuestCatalogEntry> = seen.into_values().collect();
        if entries.len() > MAX_QUEST_CATALOG {
            log::warn!(
                "QuestCatalog::build: world has {} quest givers but cap is {}; truncating",
                entries.len(),
                MAX_QUEST_CATALOG
            );
            entries.truncate(MAX_QUEST_CATALOG);
        }
        Self { entries }
    }
}

/// Compute the `stages` field for a catalog entry from the NPC's
/// `data[50]` (skill-teach slot).
///
/// # Arguments
///
/// * `data50` - Raw value of the NPC's `data[50]`.
///
/// # Returns
///
/// * `2` for SK_STUN / SK_CURSE teachers (kindred-based follow-ups), else
///   `1`.
fn stage_count_for(data50: i32) -> u8 {
    if data50 <= 0 {
        return 1;
    }
    let canonical = skills::canonicalize_weapon_skill(data50 as usize);
    if canonical == skills::SK_STUN || canonical == skills::SK_CURSE {
        2
    } else {
        1
    }
}

static QUEST_CATALOG: OnceLock<Mutex<QuestCatalog>> = OnceLock::new();

/// Initialise (once) the global quest catalog from `gs`. Subsequent calls
/// are no-ops.
///
/// # Arguments
///
/// * `gs` - Game state used as the source of NPC data.
pub fn ensure_initialized(gs: &GameState) {
    if QUEST_CATALOG.get().is_some() {
        return;
    }
    let catalog = QuestCatalog::build_from_gamestate(gs);
    let _ = QUEST_CATALOG.set(Mutex::new(catalog));
}

/// Borrow the global quest catalog for the duration of `f`.
///
/// # Arguments
///
/// * `f` - Closure that receives a shared reference to the catalog.
///
/// # Returns
///
/// * Whatever `f` returns.
///
/// # Panics
///
/// * Panics if [`ensure_initialized`] has never been called.
pub fn with_catalog<R>(f: impl FnOnce(&QuestCatalog) -> R) -> R {
    let mutex = QUEST_CATALOG
        .get()
        .expect("quest catalog accessed before initialisation");
    let guard = mutex.lock().expect("quest catalog mutex poisoned");
    f(&guard)
}

/// Read the per-player completion counter at `idx`.
///
/// # Arguments
///
/// * `ch` - Character to read from.
/// * `idx` - Catalog index.
///
/// # Returns
///
/// * Counter value, or `0` if `idx` is out of range.
pub fn get_completion(ch: &core::types::Character, idx: u8) -> i16 {
    *ch.future2.get(idx as usize).unwrap_or(&0)
}

/// Increment the per-player completion counter at `idx` per the rules of
/// `entry`. Repeatable quests are clamped at `1`; multi-stage quests
/// saturate at `entry.stages`; everything else saturates at `1`.
///
/// # Arguments
///
/// * `ch` - Mutable character to bump.
/// * `idx` - Catalog index.
/// * `entry` - Catalog entry describing the quest.
///
/// # Returns
///
/// * `true` if the stored counter changed (caller should emit a delta),
///   `false` if it was already at the cap.
pub fn bump_completion(
    ch: &mut core::types::Character,
    idx: u8,
    entry: &QuestCatalogEntry,
) -> bool {
    let Some(slot) = ch.future2.get_mut(idx as usize) else {
        return false;
    };
    let cap: i16 = if entry.repeatable {
        1
    } else {
        i16::from(entry.stages.max(1))
    };
    if *slot >= cap {
        return false;
    }
    *slot = slot.saturating_add(1).min(cap);
    true
}

/// Send `SV_SETQUESTCATALOG` to player `nr`.
///
/// # Arguments
///
/// * `gs` - Mutable game state.
/// * `nr` - Player slot index.
pub fn plr_send_quest_catalog(gs: &mut GameState, nr: usize) {
    ensure_initialized(gs);
    let mut buf = [0u8; QUEST_CATALOG_PACKET_LEN];
    buf[0] = ServerCommandType::SetQuestCatalog as u8;
    with_catalog(|cat| {
        let count = cat.entries.len().min(MAX_QUEST_CATALOG) as u8;
        buf[1] = count;
        for (i, e) in cat.entries.iter().take(MAX_QUEST_CATALOG).enumerate() {
            let off = 2 + i * QUEST_CATALOG_ENTRY_LEN;
            buf[off..off + 2].copy_from_slice(&e.template_id.to_le_bytes());
            buf[off + 2..off + 4].copy_from_slice(&e.item_template_id.to_le_bytes());
            buf[off + 4..off + 6].copy_from_slice(&e.npc_x.to_le_bytes());
            buf[off + 6..off + 8].copy_from_slice(&e.npc_y.to_le_bytes());
            buf[off + 8] = e.stages;
            buf[off + 9] = u8::from(e.repeatable);
            write_padded_name(
                &mut buf[off + 10..off + 10 + QUEST_CATALOG_NPC_NAME_LEN],
                &e.npc_name,
            );
            let item_off = off + 10 + QUEST_CATALOG_NPC_NAME_LEN;
            write_padded_name(
                &mut buf[item_off..item_off + QUEST_CATALOG_ITEM_NAME_LEN],
                &e.item_name,
            );
        }
    });
    xsend(gs, nr, &buf, QUEST_CATALOG_PACKET_LEN);
}

/// Send a full `SV_SETQUESTCOMPLETION` snapshot to player `nr`.
///
/// # Arguments
///
/// * `gs` - Mutable game state.
/// * `nr` - Player slot index.
pub fn plr_send_quest_completion_full(gs: &mut GameState, nr: usize) {
    let cn = gs.players[nr].usnr;
    if cn == 0 || cn >= MAXCHARS {
        return;
    }
    let mut buf = [0u8; QUEST_COMPLETION_FULL_LEN];
    buf[0] = ServerCommandType::SetQuestCompletion as u8;
    buf[1] = 0;
    let counts = gs.characters[cn].future2;
    for (i, c) in counts.iter().enumerate() {
        let off = 2 + i * 2;
        buf[off..off + 2].copy_from_slice(&c.to_le_bytes());
    }
    xsend(gs, nr, &buf, QUEST_COMPLETION_FULL_LEN);
}

/// Send a single-entry `SV_SETQUESTCOMPLETION` delta to player `nr`.
///
/// # Arguments
///
/// * `gs` - Mutable game state.
/// * `nr` - Player slot index.
/// * `idx` - Catalog index whose counter changed.
/// * `count` - New absolute counter value.
pub fn plr_send_quest_completion_delta(gs: &mut GameState, nr: usize, idx: u8, count: i16) {
    let mut buf = [0u8; QUEST_COMPLETION_DELTA_LEN];
    buf[0] = ServerCommandType::SetQuestCompletion as u8;
    buf[1] = 1;
    buf[2] = idx;
    buf[3..5].copy_from_slice(&count.to_le_bytes());
    xsend(gs, nr, &buf, QUEST_COMPLETION_DELTA_LEN);
}

/// Record a quest turn-in on character `cn` against NPC template
/// `npc_template_id`. Looks up the catalog index, bumps the counter, and
/// transmits a delta to the player (if one is attached).
///
/// # Arguments
///
/// * `gs` - Mutable game state.
/// * `cn` - Character receiving credit for the turn-in.
/// * `npc_template_id` - Template id of the NPC that accepted the item.
pub fn record_turn_in(gs: &mut GameState, cn: usize, npc_template_id: u16) {
    if cn == 0 || cn >= MAXCHARS {
        return;
    }
    if QUEST_CATALOG.get().is_none() {
        return;
    }
    let (idx, entry) = match with_catalog(|cat| {
        cat.index_of(npc_template_id)
            .and_then(|i| cat.entry(i).map(|e| (i, e.clone())))
    }) {
        Some(pair) => pair,
        None => return,
    };
    if !bump_completion(&mut gs.characters[cn], idx, &entry) {
        return;
    }
    let count = get_completion(&gs.characters[cn], idx);
    let player_slot = gs.characters[cn].player as usize;
    if player_slot != 0 && player_slot < gs.players.len() {
        plr_send_quest_completion_delta(gs, player_slot, idx, count);
    }
}

/// Copy `name` into `dst`, truncating to `dst.len() - 1` to leave at least one
/// trailing NUL. Bytes past the copied prefix remain zero-initialized.
///
/// # Arguments
///
/// * `dst`  - Destination slice (assumed to be zero-initialized).
/// * `name` - Source UTF-8 string; the byte slice is copied verbatim.
fn write_padded_name(dst: &mut [u8], name: &str) {
    let max = dst.len().saturating_sub(1);
    let bytes = name.as_bytes();
    let n = bytes.len().min(max);
    dst[..n].copy_from_slice(&bytes[..n]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(stages: u8, repeatable: bool) -> QuestCatalogEntry {
        QuestCatalogEntry {
            template_id: 1,
            item_template_id: 1,
            npc_x: 0,
            npc_y: 0,
            stages,
            repeatable,
            npc_name: "n".into(),
            item_name: "i".into(),
        }
    }

    #[test]
    fn bump_increments_then_caps_for_single_stage() {
        let mut ch = core::types::Character::default();
        let e = entry(1, false);
        assert!(bump_completion(&mut ch, 0, &e));
        assert_eq!(ch.future2[0], 1);
        assert!(!bump_completion(&mut ch, 0, &e));
        assert_eq!(ch.future2[0], 1);
    }

    #[test]
    fn bump_multi_stage_saturates_at_stages() {
        let mut ch = core::types::Character::default();
        let e = entry(2, false);
        assert!(bump_completion(&mut ch, 5, &e));
        assert!(bump_completion(&mut ch, 5, &e));
        assert!(!bump_completion(&mut ch, 5, &e));
        assert_eq!(ch.future2[5], 2);
    }

    #[test]
    fn bump_repeatable_clamps_at_one() {
        let mut ch = core::types::Character::default();
        let e = entry(99, true);
        assert!(bump_completion(&mut ch, 3, &e));
        assert!(!bump_completion(&mut ch, 3, &e));
        assert_eq!(ch.future2[3], 1);
    }

    #[test]
    fn bump_out_of_range_returns_false() {
        let mut ch = core::types::Character::default();
        let e = entry(1, false);
        assert!(!bump_completion(&mut ch, 49, &e));
    }

    #[test]
    fn stage_count_uses_skill_canonicalisation() {
        assert_eq!(stage_count_for(0), 1);
        assert_eq!(stage_count_for(skills::SK_STUN as i32), 2);
        assert_eq!(stage_count_for(skills::SK_CURSE as i32), 2);
        assert_eq!(stage_count_for(skills::SK_RECALL as i32), 1);
    }
}
