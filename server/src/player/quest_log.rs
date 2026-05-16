//! Per-player quest log dispatch.
//!
//! The server scans all active characters for those that have a quest item
//! assigned to the calling player (encoded in `Character::data[49]` as a
//! template ID) and reports up to 16 such NPCs to the client every tick.
//!
//! State is purely cosmetic — no quest progress lives on the server beyond
//! the per-character quest-item slot the legacy game already maintained.
//! The cached `last_sent_quest_log` / `last_sent_active_quest` fields on
//! [`crate::types::server_player::ServerPlayer`] let us skip retransmission
//! when nothing changed, mirroring the weather subsystem.

use core::constants::{MAXCHARS, MAXTITEM, USE_ACTIVE};
use core::server_commands::{
    QUEST_LOG_ENTRY_LEN, QUEST_LOG_ITEM_NAME_LEN, QUEST_LOG_MAX_ENTRIES, QUEST_LOG_NPC_NAME_LEN,
    QUEST_LOG_PACKET_LEN, QuestLogEntry, ServerCommandType,
};

use crate::game_state::GameState;
use crate::network_manager::xsend;

/// Slot on `Character::data` that legacy NPC quests use to record the
/// template ID of the quest item the NPC is currently waiting for.
const QUEST_ITEM_DATA_SLOT: usize = 49;

/// Scan the world for quests assigned to player `nr`, build the
/// `SV_SETQUESTLOG` packet, and transmit it via `xsend` only when the
/// snapshot or the player's currently focused quest changed since the
/// previous send.
///
/// # Arguments
///
/// * `gs` - Mutable game state.
/// * `nr` - Player slot index.
pub fn plr_send_quest_log(gs: &mut GameState, nr: usize) {
    if nr >= gs.players.len() {
        return;
    }

    let mut entries: Vec<QuestLogEntry> = Vec::with_capacity(QUEST_LOG_MAX_ENTRIES);
    for cn in 1..MAXCHARS {
        if entries.len() >= QUEST_LOG_MAX_ENTRIES {
            break;
        }
        let ch = &gs.characters[cn];
        if ch.used != USE_ACTIVE {
            continue;
        }
        let quest_item = ch.data[QUEST_ITEM_DATA_SLOT];
        if quest_item == 0 {
            continue;
        }
        let template = ch.temp;
        if template == 0 {
            continue;
        }
        let x = ch.x.max(0) as u16;
        let y = ch.y.max(0) as u16;
        let npc_name = ch.get_name().to_owned();

        let item_template_id = quest_item as u16;
        let item_name = if (quest_item as usize) < MAXTITEM
            && gs.item_templates[quest_item as usize].used != core::constants::USE_EMPTY
        {
            gs.item_templates[quest_item as usize].get_name().to_owned()
        } else {
            String::new()
        };

        entries.push(QuestLogEntry {
            npc_template_id: template,
            npc_x: x,
            npc_y: y,
            item_template_id,
            npc_name,
            item_name,
        });
    }

    let active_template_id = gs.players[nr].active_quest_template_id;
    let mut active_npc_x: u16 = 0;
    let mut active_npc_y: u16 = 0;
    if active_template_id != 0
        && let Some(e) = entries
            .iter()
            .find(|e| e.npc_template_id == active_template_id)
    {
        active_npc_x = e.npc_x;
        active_npc_y = e.npc_y;
    }
    let active_step_idx: u8 = 0;

    let cached_changed = gs.players[nr].last_sent_quest_log != entries
        || gs.players[nr].last_sent_active_quest != active_template_id;
    if !cached_changed {
        return;
    }

    let mut buf = [0u8; QUEST_LOG_PACKET_LEN];
    buf[0] = ServerCommandType::SetQuestLog as u8;
    let count = entries.len().min(QUEST_LOG_MAX_ENTRIES) as u8;
    buf[1] = count;
    for (i, e) in entries.iter().take(QUEST_LOG_MAX_ENTRIES).enumerate() {
        let off = 2 + i * QUEST_LOG_ENTRY_LEN;
        buf[off..off + 2].copy_from_slice(&e.npc_template_id.to_le_bytes());
        buf[off + 2..off + 4].copy_from_slice(&e.npc_x.to_le_bytes());
        buf[off + 4..off + 6].copy_from_slice(&e.npc_y.to_le_bytes());
        buf[off + 6..off + 8].copy_from_slice(&e.item_template_id.to_le_bytes());
        write_padded_name(
            &mut buf[off + 8..off + 8 + QUEST_LOG_NPC_NAME_LEN],
            &e.npc_name,
        );
        let item_off = off + 8 + QUEST_LOG_NPC_NAME_LEN;
        write_padded_name(
            &mut buf[item_off..item_off + QUEST_LOG_ITEM_NAME_LEN],
            &e.item_name,
        );
    }
    let trailer_off = 2 + QUEST_LOG_MAX_ENTRIES * QUEST_LOG_ENTRY_LEN;
    buf[trailer_off..trailer_off + 2].copy_from_slice(&active_template_id.to_le_bytes());
    buf[trailer_off + 2] = active_step_idx;
    buf[trailer_off + 3..trailer_off + 5].copy_from_slice(&active_npc_x.to_le_bytes());
    buf[trailer_off + 5..trailer_off + 7].copy_from_slice(&active_npc_y.to_le_bytes());

    {
        let p = &mut gs.players[nr];
        p.last_sent_quest_log = entries;
        p.last_sent_active_quest = active_template_id;
    }

    xsend(gs, nr, &buf, QUEST_LOG_PACKET_LEN as u8);
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
