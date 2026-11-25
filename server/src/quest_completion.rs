//! Hand-authored table of quests that count toward the Journal's "Quests
//! Completable" checklist.
//!
//! Deliberately **not** an auto-scanning catalog (see
//! `docs/server/DESIGN.md` history / repo memory `quest-catalog.md` for why
//! the old dynamic quest-catalog system was removed). Each entry is matched
//! against the *giving* NPC's template number (`Character.temp`) at the
//! generic "quest-requested item" turn-in acceptance point in
//! `server::driver::npc::npc_give`. When a match is found, the corresponding
//! bit is set in the completing player's `Character::future3[1]` bitset (bit
//! index = `QuestDef::id`, up to 32 quests).
//!
//! Two more per-quest bitsets (same bit index) let the client's helper window
//! walk each quest chain step by step:
//!
//! * [`QUEST_STARTED_IDX`] - the quest NPC greeted the player with its request
//!   ([`record_quest_started`]).
//! * [`QUEST_ITEM_OBTAINED_IDX`] - the player has held the NPC's required item
//!   at least once; sticky, never cleared ([`record_quest_items_obtained`]).
//!
//! All three are sent to the client in `SetCompletionData`.

use core::constants::CharacterFlags;

use crate::game_state::GameState;

/// `Character::future3` index of the "quest started" bitset.
pub const QUEST_STARTED_IDX: usize = 7;
/// `Character::future3` index of the sticky "quest item obtained" bitset.
pub const QUEST_ITEM_OBTAINED_IDX: usize = 8;

/// A single hand-authored quest definition.
pub struct QuestDef {
    /// Bit index (0..32) set in `Character::future3[1]` when this quest is
    /// completed. Must be unique and stable (the client's Journal markdown
    /// checklist references these ids by hand, so don't renumber existing
    /// entries).
    pub id: u8,
    /// Template number (`Character.temp`) of the NPC that accepts the
    /// quest-completing item turn-in.
    pub npc_temp: u16,
    /// Human-readable label shown on the client's Journal quest checklist.
    #[allow(dead_code)]
    pub label: &'static str,
}

/// Hand-authored quest catalog. Add new entries as more quests are wired up;
/// leave existing `id`s untouched since the client markdown references them.
pub const QUEST_DEFS: &[QuestDef] = &[
    QuestDef {
        id: 0,
        npc_temp: 518,
        label: "Black Candle (Cityguard)",
    },
    // Skill turn-in quests (NPC `data[49]` is the required item, `data[50]`
    // is the taught skill). Identified by scanning `world_seed.wsnap` for
    // character templates with `data[49] != 0` and `data[50] != 0`.
    QuestDef {
        id: 1,
        npc_temp: 25,
        label: "Barter (Jamil)",
    },
    QuestDef {
        id: 2,
        npc_temp: 28,
        label: "Enhance Weapon (Sirjan)",
    },
    QuestDef {
        id: 3,
        npc_temp: 50,
        label: "Recall (Inga)",
    },
    QuestDef {
        id: 4,
        npc_temp: 64,
        label: "Repair (Jefferson)",
    },
    QuestDef {
        id: 5,
        npc_temp: 90,
        label: "Stun (Ingrid)",
    },
    QuestDef {
        id: 6,
        npc_temp: 91,
        label: "Lock (Steven)",
    },
    QuestDef {
        id: 7,
        npc_temp: 107,
        label: "Bless (Cirrus)",
    },
    QuestDef {
        id: 8,
        npc_temp: 108,
        label: "Identify (Nasir)",
    },
    QuestDef {
        id: 9,
        npc_temp: 109,
        label: "Resist (Serena)",
    },
    QuestDef {
        id: 10,
        npc_temp: 111,
        label: "Curse (Gordon)",
    },
    QuestDef {
        id: 11,
        npc_temp: 342,
        label: "Sense (Manfred)",
    },
    QuestDef {
        id: 12,
        npc_temp: 343,
        label: "Rest (Leopold)",
    },
    QuestDef {
        id: 13,
        npc_temp: 363,
        label: "Heal (Gunther)",
    },
];

/// Looks up the quest definition (if any) whose giving NPC template matches
/// `npc_temp`.
///
/// # Arguments
/// * `npc_temp` - Template number of the NPC that just accepted a
///   quest-completing item turn-in.
///
/// # Returns
/// * `Some(&QuestDef)` when `npc_temp` matches a known quest, otherwise
///   `None`.
pub fn find_by_npc_temp(npc_temp: u16) -> Option<&'static QuestDef> {
    QUEST_DEFS.iter().find(|q| q.npc_temp == npc_temp)
}

/// Sets quest bit `id` in `future3[slot]` of character `cn` and resends the
/// completion snapshot when the bit was newly set.
///
/// # Arguments
///
/// * `gs` - Active game state.
/// * `cn` - Player character index.
/// * `slot` - `future3` index of the bitset to update.
/// * `id` - Quest bit index (`QuestDef::id`).
fn set_quest_bit(gs: &mut GameState, cn: usize, slot: usize, id: u8) {
    let bit = 1i32 << id;
    if gs.characters[cn].future3[slot] & bit == 0 {
        gs.characters[cn].future3[slot] |= bit;
        crate::player::commands::resend_completion_data_for_character(gs, cn);
    }
}

/// Records that quest NPC `npc` greeted player `co` with its request.
///
/// No-op unless `co` is a player and `npc`'s template is a known quest giver.
///
/// # Arguments
///
/// * `gs` - Active game state.
/// * `npc` - NPC character index that spoke the greeting.
/// * `co` - Character index that was greeted.
pub fn record_quest_started(gs: &mut GameState, npc: usize, co: usize) {
    if gs.characters[co].flags & CharacterFlags::Player.bits() == 0 {
        return;
    }
    if let Some(quest) = find_by_npc_temp(gs.characters[npc].temp) {
        set_quest_bit(gs, co, QUEST_STARTED_IDX, quest.id);
    }
}

/// Records that player `cn` now holds items with the given templates, setting
/// the sticky "item obtained" bit of every quest whose giving NPC wants one.
///
/// The required item is read from the giving NPC's template (`data[49]`), so
/// it is not duplicated in [`QUEST_DEFS`].
///
/// # Arguments
///
/// * `gs` - Active game state.
/// * `cn` - Player character index.
/// * `item_temps` - Templates of items that just entered the player's slots.
pub fn record_quest_items_obtained(gs: &mut GameState, cn: usize, item_temps: &[u16]) {
    if gs.characters[cn].flags & CharacterFlags::Player.bits() == 0 {
        return;
    }
    for quest in QUEST_DEFS {
        let Some(giver) = gs.character_templates.get(usize::from(quest.npc_temp)) else {
            continue;
        };
        let wanted = giver.data[49];
        if wanted > 0 && item_temps.iter().any(|&t| i32::from(t) == wanted) {
            set_quest_bit(gs, cn, QUEST_ITEM_OBTAINED_IDX, quest.id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::with_test_gs;

    const JAMIL_TEMP: u16 = 25;
    const AMULET_TEMP: u16 = 105;
    const JAMIL_QUEST_ID: u8 = 1;

    fn setup(gs: &mut GameState) -> (usize, usize) {
        let (npc, player) = (1, 2);
        gs.characters[npc].temp = JAMIL_TEMP;
        gs.characters[player].flags = CharacterFlags::Player.bits();
        gs.character_templates[usize::from(JAMIL_TEMP)].data[49] = i32::from(AMULET_TEMP);
        (npc, player)
    }

    #[test]
    fn greeting_from_quest_npc_marks_quest_started_for_players_only() {
        with_test_gs(|gs| {
            let (npc, player) = setup(gs);
            record_quest_started(gs, npc, player);
            assert_eq!(
                gs.characters[player].future3[QUEST_STARTED_IDX],
                1 << JAMIL_QUEST_ID
            );

            // NPC listeners and non-quest NPCs record nothing.
            let other_npc = 3;
            record_quest_started(gs, npc, other_npc);
            assert_eq!(gs.characters[other_npc].future3[QUEST_STARTED_IDX], 0);
            gs.characters[npc].temp = 0xFFFF;
            gs.characters[player].future3[QUEST_STARTED_IDX] = 0;
            record_quest_started(gs, npc, player);
            assert_eq!(gs.characters[player].future3[QUEST_STARTED_IDX], 0);
        });
    }

    #[test]
    fn holding_required_item_marks_item_obtained() {
        with_test_gs(|gs| {
            let (_, player) = setup(gs);
            record_quest_items_obtained(gs, player, &[7, 8]);
            assert_eq!(gs.characters[player].future3[QUEST_ITEM_OBTAINED_IDX], 0);

            record_quest_items_obtained(gs, player, &[7, AMULET_TEMP]);
            assert_eq!(
                gs.characters[player].future3[QUEST_ITEM_OBTAINED_IDX],
                1 << JAMIL_QUEST_ID
            );
        });
    }

    #[test]
    fn finds_known_npc_temp() {
        let quest = find_by_npc_temp(518).expect("expected a quest for temp 518");
        assert_eq!(quest.id, 0);

        let quest = find_by_npc_temp(107).expect("expected a quest for temp 107");
        assert_eq!(quest.id, 7);
    }

    #[test]
    fn unknown_npc_temp_returns_none() {
        assert!(find_by_npc_temp(0xFFFF).is_none());
    }

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<u8> = QUEST_DEFS.iter().map(|q| q.id).collect();
        ids.sort_unstable();
        let mut deduped = ids.clone();
        deduped.dedup();
        assert_eq!(ids, deduped, "duplicate quest ids found in QUEST_DEFS");
    }

    #[test]
    fn ids_fit_in_32_bit_bitset() {
        assert!(QUEST_DEFS.iter().all(|q| q.id < 32));
    }
}
