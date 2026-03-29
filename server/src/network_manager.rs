use core::constants::{SV_SETMAP3, SV_SETMAP4, SV_SETMAP5, SV_SETMAP6};
use core::logout_reasons::LogoutReason;
use std::net::Shutdown;
use std::sync::{OnceLock, RwLock};

use crate::{game_state::GameState, player};

static PACKET_STATS: OnceLock<RwLock<PacketStats>> = OnceLock::new();

struct PacketStats {
    cnt: [usize; 256],
    pkt_mapshort: usize,
    pkt_light: usize,
}

impl PacketStats {
    fn new() -> Self {
        PacketStats {
            cnt: [0usize; 256],
            pkt_mapshort: 0,
            pkt_light: 0,
        }
    }
}

pub fn initialize_packet_stats() -> Result<(), String> {
    PACKET_STATS
        .set(RwLock::new(PacketStats::new()))
        .map_err(|_| "PacketStats already initialized".to_string())
}

/// Send bytes to a player's tick buffer.
///
/// Copies up to `length` bytes from `data` into the player's `tbuf`.
/// If the buffer overflows the player is disconnected.
pub fn xsend(gs: &mut GameState, player_id: usize, data: &[u8], length: u8) {
    let send_len = std::cmp::min(length as usize, data.len());

    if player_id < 1 || player_id >= gs.players.len() {
        log::warn!("xsend: invalid player id {}", player_id);
        return;
    }

    if gs.players[player_id].sock.is_none() {
        log::warn!("xsend: no socket for player {}", player_id);
        return;
    }

    if gs.players[player_id].tptr + send_len >= gs.players[player_id].tbuf.len() {
        log::error!(
            "#INTERNAL ERROR# ticksize too large for player {}, terminating connection",
            player_id
        );
        let cn = gs.players[player_id].usnr;
        player::plr_logout(gs, cn, player_id, LogoutReason::Unknown);
        if let Some(s) = gs.players[player_id].sock.take() {
            let _ = s.shutdown(Shutdown::Both);
        }
        gs.players[player_id].ltick = 0;
        gs.players[player_id].rtick = 0;
        if let Some(z) = gs.players[player_id].zs.take().as_mut() {
            let _ = z.try_finish();
        }
        return;
    }

    let start = gs.players[player_id].tptr;
    let end = start + send_len;
    if end <= gs.players[player_id].tbuf.len() {
        gs.players[player_id].tbuf[start..end].copy_from_slice(&data[..send_len]);
        gs.players[player_id].tptr = end;

        if let Some(stats_lock) = PACKET_STATS.get() {
            let mut stats = stats_lock.write().unwrap();
            let pnr = if !data.is_empty() {
                data[0] as usize
            } else {
                0
            };
            if pnr < stats.cnt.len() {
                stats.cnt[pnr] = stats.cnt[pnr].saturating_add(send_len);
                if pnr > 128 {
                    stats.pkt_mapshort = stats.pkt_mapshort.saturating_add(send_len);
                } else if pnr == SV_SETMAP3 as usize
                    || pnr == SV_SETMAP4 as usize
                    || pnr == SV_SETMAP5 as usize
                    || pnr == SV_SETMAP6 as usize
                {
                    stats.pkt_light = stats.pkt_light.saturating_add(send_len);
                }
            }
        }
    } else {
        log::warn!(
            "xsend: computed end {} out of bounds for player {} tbuf len {}",
            end,
            player_id,
            gs.players[player_id].tbuf.len()
        );
    }
}

/// Send bytes into the player's circular output buffer.
///
/// Enqueues up to `length` bytes from `data` into the player's `obuf`.
/// If the buffer is full the player is disconnected (client too slow).
pub fn csend(gs: &mut GameState, player_id: usize, data: &[u8], length: u8) {
    let send_len = std::cmp::min(length as usize, data.len());

    if player_id < 1 || player_id >= gs.players.len() {
        log::warn!("csend: invalid player id {}", player_id);
        return;
    }

    if gs.players[player_id].sock.is_none() {
        return;
    }

    let mut written = 0usize;
    while written < send_len {
        let mut tmp = gs.players[player_id].iptr + 1;
        if tmp == gs.players[player_id].obuf.len() {
            tmp = 0;
        }

        if tmp == gs.players[player_id].optr {
            log::warn!("Connection too slow for player {}, terminating", player_id);
            let cn = gs.players[player_id].usnr;
            player::plr_logout(gs, cn, player_id, LogoutReason::ClientTooSlow);
            if let Some(s) = gs.players[player_id].sock.take() {
                let _ = s.shutdown(Shutdown::Both);
            }
            gs.players[player_id].ltick = 0;
            gs.players[player_id].rtick = 0;
            if let Some(z) = gs.players[player_id].zs.take().as_mut() {
                let _ = z.try_finish();
            }
            gs.players[player_id].zs = None;
            return;
        }

        let iptr = gs.players[player_id].iptr;
        gs.players[player_id].obuf[iptr] = data[written];
        gs.players[player_id].iptr = tmp;
        written += 1;
    }
}
