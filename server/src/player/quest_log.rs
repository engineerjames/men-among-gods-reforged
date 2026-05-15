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

use core::constants::{MAXCHARS, USE_ACTIVE};
use core::server_commands::ServerCommandType;

use crate::game_state::GameState;
use crate::network_manager::xsend;

/// Maximum number of quest entries the wire format carries in a single
/// `SV_SETQUESTLOG` packet.
const MAX_QUEST_ENTRIES: usize = 16;

/// Total `SV_SETQUESTLOG` packet size in bytes (matches
/// `ServerCommandType::SetQuestLog` in `core::server_commands`).
const QUEST_LOG_PACKET_LEN: usize = 105;

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

    let mut entries: Vec<(u16, u16, u16)> = Vec::with_capacity(MAX_QUEST_ENTRIES);
    for cn in 1..MAXCHARS {
        if entries.len() >= MAX_QUEST_ENTRIES {
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
        entries.push((template, x, y));
    }

    let active_template_id = gs.players[nr].active_quest_template_id;
    let mut active_npc_x: u16 = 0;
    let mut active_npc_y: u16 = 0;
    if active_template_id != 0 {
        if let Some((_, x, y)) = entries.iter().find(|(t, _, _)| *t == active_template_id) {
            active_npc_x = *x;
            active_npc_y = *y;
        }
    }
    let active_step_idx: u8 = 0;

    let cached_changed = gs.players[nr].last_sent_quest_log != entries
        || gs.players[nr].last_sent_active_quest != active_template_id;
    if !cached_changed {
        return;
    }

    let mut buf = [0u8; QUEST_LOG_PACKET_LEN];
    buf[0] = ServerCommandType::SetQuestLog as u8;
    let count = entries.len().min(MAX_QUEST_ENTRIES) as u8;
    buf[1] = count;
    for (i, (t, x, y)) in entries.iter().take(MAX_QUEST_ENTRIES).enumerate() {
        let off = 2 + i * 6;
        buf[off..off + 2].copy_from_slice(&t.to_le_bytes());
        buf[off + 2..off + 4].copy_from_slice(&x.to_le_bytes());
        buf[off + 4..off + 6].copy_from_slice(&y.to_le_bytes());
    }
    buf[98..100].copy_from_slice(&active_template_id.to_le_bytes());
    buf[100] = active_step_idx;
    buf[101..103].copy_from_slice(&active_npc_x.to_le_bytes());
    buf[103..105].copy_from_slice(&active_npc_y.to_le_bytes());

    {
        let p = &mut gs.players[nr];
        p.last_sent_quest_log = entries;
        p.last_sent_active_quest = active_template_id;
    }

    xsend(gs, nr, &buf, QUEST_LOG_PACKET_LEN as u8);
}
