use core::constants::{SV_SETMAP3, SV_SETMAP4, SV_SETMAP5, SV_SETMAP6};
use std::net::Shutdown;
use std::sync::{OnceLock, RwLock};

use crate::{enums, player, server::Server};

static NETWORK_MANAGER: OnceLock<RwLock<NetworkManager>> = OnceLock::new();
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

pub struct NetworkManager {
    // Network management fields and methods would go here.
}

impl NetworkManager {
    pub fn new() -> Self {
        Self {
            // Initialize fields here.
        }
    }

    pub fn initialize() -> Result<(), String> {
        let manager = NetworkManager::new();
        NETWORK_MANAGER
            .set(RwLock::new(manager))
            .map_err(|_| "NetworkManager already initialized".to_string())?;
        // Initialize packet stats
        PACKET_STATS
            .set(RwLock::new(PacketStats::new()))
            .map_err(|_| "PacketStats already initialized".to_string())?;
        Ok(())
    }

    pub fn with<F, R>(f: F) -> R
    where
        F: FnOnce(&NetworkManager) -> R,
    {
        let manager = NETWORK_MANAGER
            .get()
            .expect("NetworkManager not initialized")
            .read()
            .unwrap();
        f(&*manager)
    }

    pub fn with_mut<F, R>(f: F) -> R
    where
        F: FnOnce(&mut NetworkManager) -> R,
    {
        let mut manager = NETWORK_MANAGER
            .get()
            .expect("NetworkManager not initialized")
            .write()
            .unwrap();
        f(&mut *manager)
    }

    pub fn xsend(&self, player_id: usize, data: &[u8], length: u8) {
        use crate::server::Server;
        use crate::{enums, player};
        use log::{error, warn};

        // Determine number of bytes to send (don't exceed provided slice)
        let send_len = std::cmp::min(length as usize, data.len());

        Server::with_players_mut(|players| {
            // Bounds check: valid player slots are 1..players.len()-1
            if player_id < 1 || player_id >= players.len() {
                return;
            }

            let p = &mut players[player_id];

            // If no socket, nothing to do
            if p.sock.is_none() {
                return;
            }

            // Check tick buffer space
            if p.tptr + send_len >= p.tbuf.len() {
                error!(
                    "#INTERNAL ERROR# ticksize too large for player {}, terminating connection",
                    player_id
                );
                // Attempt to log out the associated character and clean up
                let cn = p.usnr;
                player::plr_logout(cn, player_id, enums::LogoutReason::Unknown);
                if let Some(s) = p.sock.take() {
                    let _ = s.shutdown(Shutdown::Both);
                }
                p.ltick = 0;
                p.rtick = 0;
                p.zs = None;
                return;
            }

            // Copy data into tick buffer
            let start = p.tptr;
            let end = start + send_len;
            if end <= p.tbuf.len() {
                p.tbuf[start..end].copy_from_slice(&data[..send_len]);
                p.tptr = end;

                // Update packet counters similar to C++ pkt_cnt logic
                if let Some(stats_lock) = PACKET_STATS.get() {
                    let mut stats = stats_lock.write().unwrap();
                    let pnr = if data.len() > 0 { data[0] as usize } else { 0 };
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
                warn!(
                    "xsend: computed end {} out of bounds for player {} tbuf len {}",
                    end,
                    player_id,
                    p.tbuf.len()
                );
            }
        });
    }

    pub fn csend(&self, player_id: usize, data: &[u8], length: u8) {
        let send_len = std::cmp::min(length as usize, data.len());

        Server::with_players_mut(|players| {
            if player_id < 1 || player_id >= players.len() {
                return;
            }

            let p = &mut players[player_id];

            if p.sock.is_none() {
                return;
            }

            // Write bytes into circular output buffer one by one
            let mut written = 0usize;
            while written < send_len {
                let mut tmp = p.iptr + 1;
                if tmp == p.obuf.len() {
                    tmp = 0;
                }

                if tmp == p.optr {
                    // Connection too slow, terminate
                    log::warn!("Connection too slow for player {}, terminating", player_id);
                    let cn = p.usnr;
                    player::plr_logout(cn, player_id, enums::LogoutReason::ClientTooSlow);
                    if let Some(s) = p.sock.take() {
                        let _ = s.shutdown(Shutdown::Both);
                    }
                    p.ltick = 0;
                    p.rtick = 0;
                    p.zs = None;
                    return;
                }

                p.obuf[p.iptr] = data[written];
                p.iptr = tmp;
                written += 1;
            }
        });
    }

    // Additional methods for network management would go here.
}
