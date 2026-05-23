//! Static client-side definitions for NPC quests.
//!
//! The server ships a one-shot [`SV_SETQUESTCATALOG`] packet at login time
//! enumerating every NPC quest that exists in the world. Per-player
//! completion progress is then streamed as
//! [`SV_SETQUESTCOMPLETION`](crate::server_commands::ServerCommandType::SetQuestCompletion)
//! snapshots and deltas. The actual quest title, description and
//! step-by-step walkthrough are authored statically and shipped with the
//! client; the entries in [`QUEST_DEFS`] are looked up by
//! `npc_template_id` to render the quest log panel.
//!
//! When a quest giver is reported by the server but no matching definition
//! exists in [`QUEST_DEFS`], the client falls back to a generic
//! [`fallback_title`] so the entry remains visible.

/// Maximum number of distinct quests carried in a single
/// [`SV_SETQUESTCATALOG`](crate::server_commands::ServerCommandType::SetQuestCatalog)
/// packet. Matches the width of `Character::future2` so per-player
/// completion counts have a guaranteed home.
pub const MAX_QUEST_CATALOG: usize = 49;

/// One entry in the static quest catalog.
///
/// Built once on the server immediately after world load and broadcast to
/// each connecting client as a one-shot snapshot. Catalog index is the
/// position inside the broadcast `Vec` and doubles as the key into the
/// per-player completion vector stored in `Character::future2`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QuestCatalogEntry {
    /// Template ID of the NPC quest giver.
    pub template_id: u16,
    /// Template ID of the item the NPC wants.
    pub item_template_id: u16,
    /// World tile X of the NPC quest giver at the time the catalog was
    /// built.
    pub npc_x: u16,
    /// World tile Y of the NPC quest giver at the time the catalog was
    /// built.
    pub npc_y: u16,
    /// Number of successful turn-ins required to mark the quest fully
    /// completed (defaults to `1`; e.g. Seyan Du quest-givers set this to
    /// `2` to model the SK_STUN → SK_IMMUN progression).
    pub stages: u8,
    /// `true` for infinitely repeatable quests (e.g. black candle): the
    /// row never disappears and the server stops incrementing the
    /// completion counter once it reaches `1`.
    pub repeatable: bool,
    /// Display name of the NPC quest giver.
    pub npc_name: String,
    /// Display name of the wanted item.
    pub item_name: String,
}

/// One step in a quest walkthrough.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestStep {
    /// Step that points at a fixed world tile (e.g. "Search the cave at
    /// (1234, 5678)"). The coordinates are also used for the minimap pin.
    FixedLocation {
        /// World tile X.
        x: u16,
        /// World tile Y.
        y: u16,
        /// Human-readable instruction shown in the quest panel.
        desc: &'static str,
    },
    /// Step that says "return the item to the quest giver". The minimap pin
    /// is driven by the NPC's catalog-recorded `(npc_x, npc_y)` tile.
    ReturnToQuestGiver {
        /// Human-readable instruction shown in the quest panel.
        desc: &'static str,
    },
}

/// Static definition of a single NPC quest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestDefinition {
    /// Template ID of the NPC that hands out this quest.
    pub npc_template_id: u16,
    /// Title shown in the quest log panel header.
    pub title: &'static str,
    /// Short description shown under the title.
    pub description: &'static str,
    /// Ordered walkthrough steps.
    pub steps: &'static [QuestStep],
}

/// Authored quest definitions, indexed by `npc_template_id` via
/// [`find_quest_def`]. New quests are added here as content is written.
pub static QUEST_DEFS: &[QuestDefinition] = &[];

/// Look up a quest definition by NPC template ID.
///
/// # Arguments
///
/// * `npc_template_id` - Template ID of the NPC quest giver.
///
/// # Returns
///
/// * `Some(&QuestDefinition)` if a matching entry exists in [`QUEST_DEFS`].
/// * `None` otherwise, in which case the caller should fall back to
///   [`fallback_title`] for display.
pub fn find_quest_def(npc_template_id: u16) -> Option<&'static QuestDefinition> {
    QUEST_DEFS
        .iter()
        .find(|d| d.npc_template_id == npc_template_id)
}

/// Build a generic quest title for an NPC that has no entry in [`QUEST_DEFS`].
///
/// # Arguments
///
/// * `item_template_ref` - Template reference string of the quest item the
///   server expects the player to bring.
///
/// # Returns
///
/// * A human-readable string of the form `"Bring <item_template_ref> to NPC"`.
pub fn fallback_title(item_template_ref: &str) -> String {
    format!("Bring {item_template_ref} to NPC")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_quest_def_returns_none_for_unknown_npc() {
        assert!(find_quest_def(0).is_none());
        assert!(find_quest_def(u16::MAX).is_none());
    }

    #[test]
    fn fallback_title_formats_item_ref() {
        assert_eq!(
            fallback_title("rusty_sword"),
            "Bring rusty_sword to NPC".to_owned()
        );
    }

    #[test]
    fn fallback_title_handles_empty_ref() {
        assert_eq!(fallback_title(""), "Bring  to NPC".to_owned());
    }

    #[test]
    fn quest_step_variants_compile() {
        let _fixed = QuestStep::FixedLocation {
            x: 100,
            y: 200,
            desc: "go here",
        };
        let _ret = QuestStep::ReturnToQuestGiver { desc: "come back" };
    }
}
