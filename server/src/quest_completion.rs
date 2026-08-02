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
    pub label: &'static str,
}

/// Hand-authored quest catalog. Add new entries as more quests are wired up;
/// leave existing `id`s untouched since the client markdown references them.
pub const QUEST_DEFS: &[QuestDef] = &[QuestDef {
    id: 0,
    npc_temp: 518,
    label: "Black Candle (Cityguard)",
}];

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_known_npc_temp() {
        let quest = find_by_npc_temp(518).expect("expected a quest for temp 518");
        assert_eq!(quest.id, 0);
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
