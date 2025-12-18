/*************************************************************************

This file is part of 'Mercenaries of Astonia v2'
Copyright (c) 1997-2001 Daniel Brockhaus (joker@astonia.com)
All rights reserved.

Rust port maintains original logic and comments.

**************************************************************************/

//! Network module - handles socket operations and data transfer

use flate2::write::ZlibEncoder;
use flate2::Compression;
use socket2::{Domain, Socket, Type};
use std::io::{self, ErrorKind, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener};
use std::os::unix::io::AsRawFd;

use crate::constants::*;
use crate::logging::Logger;
use crate::player::{Player, PlayerManager};
use crate::types::*;
use crate::xlog;

/// Packet statistics
pub struct PacketStats {
    pub pkt_cnt: [i32; 256],
    pub pkt_mapshort: i32,
    pub pkt_light: i32,
}

impl Default for PacketStats {
    fn default() -> Self {
        Self {
            pkt_cnt: [0; 256],
            pkt_mapshort: 0,
            pkt_light: 0,
        }
    }
}

impl PacketStats {
    /// Print packet statistics (pkt_list from original)
    pub fn pkt_list(&self, logger: &Logger) {
        let mut m = 0;
        let mut tot = 0;
        
        for n in 0..256 {
            tot += self.pkt_cnt[n];
            if self.pkt_cnt[n] > m {
                m = self.pkt_cnt[n];
            }
        }
        
        for n in 0..256 {
            if self.pkt_cnt[n] > m / 16 {
                xlog!(logger, "pkt type {:2}: {:5} ({:.2}%)", 
                    n, self.pkt_cnt[n], 100.0 / tot as f64 * self.pkt_cnt[n] as f64);
            }
        }
        xlog!(logger, "pkt type {:2}: {:5} ({:.2}%)", 
            256, self.pkt_mapshort, 100.0 / tot as f64 * self.pkt_mapshort as f64);
        xlog!(logger, "pkt type {:2}: {:5} ({:.2}%)", 
            257, self.pkt_light, 100.0 / tot as f64 * self.pkt_light as f64);
    }
}

/// Network manager for handling connections
pub struct NetworkManager {
    listener: TcpListener,
    pub pkt_stats: PacketStats,
}

impl NetworkManager {
    /// Create a new network manager bound to port 5555
    pub fn new() -> io::Result<Self> {
        let socket = Socket::new(Domain::IPV4, Type::STREAM, None)?;
        
        // Set socket options
        socket.set_reuse_address(true)?;
        socket.set_nonblocking(true)?;
        
        // Bind to 0.0.0.0:5555
        let addr = SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 5555);
        socket.bind(&addr.into())?;
        
        // Listen with backlog of 5
        socket.listen(5)?;
        
        let listener = TcpListener::from(socket);
        
        Ok(Self {
            listener,
            pkt_stats: PacketStats::default(),
        })
    }
    
    /// Process a new connection by finding, initializing and connecting a player entry to a new socket
    pub fn new_player(
        &self,
        players: &mut PlayerManager,
        globs: &Global,
        logger: &Logger,
    ) -> Option<usize> {
        let (stream, addr) = match self.listener.accept() {
            Ok(result) => result,
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => return None,
            Err(e) => {
                xlog!(logger, "new_player (server.c): accept() failed: {}", e);
                return None;
            }
        };
        
        // Set non-blocking mode
        if let Err(e) = stream.set_nonblocking(true) {
            xlog!(logger, "new_player: failed to set non-blocking: {}", e);
            return None;
        }
        
        // Set socket options
        // setsockopt(nsock,SOL_SOCKET,SO_SNDBUF,(const char *)&onek,sizeof(int));
        // setsockopt(nsock,SOL_SOCKET,SO_RCVBUF,(const char *)&onek,sizeof(int));
        // setsockopt(nsock,SOL_SOCKET,SO_LINGER,(const char *)&zero,sizeof(int));
        // setsockopt(nsock,SOL_SOCKET,SO_KEEPALIVE,(const char *)&one,sizeof(int));
        let _ = stream.set_nodelay(true);
        
        // Find empty player slot
        let n = match players.find_empty_slot() {
            Some(slot) => slot,
            None => {
                xlog!(logger, "new_player (server.c): MAXPLAYER reached");
                return None;
            }
        };
        
        // Convert socket address to u32
        let addr_u32 = match addr {
            SocketAddr::V4(v4) => {
                let octets = v4.ip().octets();
                u32::from_le_bytes(octets)
            }
            SocketAddr::V6(_) => 0, // IPv6 not supported in original
        };
        
        // Initialize player
        let player = &mut players.players[n];
        player.sock = Some(stream);
        player.initialize(addr_u32, globs.ticker);
        
        // Initialize zlib compressor
        player.zs = Some(ZlibEncoder::new(Vec::new(), Compression::best()));
        
        xlog!(logger, "New connection to slot {}", n);
        
        Some(n)
    }
    
    /// Send data to a player
    pub fn send_player(
        &self,
        nr: usize,
        players: &mut PlayerManager,
        ch: &mut [Character],
        globs: &mut Global,
        logger: &Logger,
        plr_logout_fn: impl FnOnce(usize, usize, u8, &mut [Character], &mut PlayerManager),
    ) {
        let player = &mut players.players[nr];
        
        let sock = match &mut player.sock {
            Some(s) => s,
            None => return,
        };
        
        // Determine how much data to send
        let (ptr, len) = if player.iptr < player.optr {
            let len = OBUFSIZE - player.optr;
            (&player.obuf[player.optr..player.optr + len], len)
        } else {
            let len = player.iptr - player.optr;
            (&player.obuf[player.optr..player.optr + len], len)
        };
        
        if len == 0 {
            return;
        }
        
        match sock.write(ptr) {
            Ok(ret) => {
                globs.send += ret as i64;
                player.optr += ret;
                if player.optr == OBUFSIZE {
                    player.optr = 0;
                }
            }
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                // Would block, try again later
            }
            Err(e) => {
                // send failure
                xlog!(logger, "Connection closed (send, {})", e);
                let usnr = player.usnr;
                player.disconnect();
                plr_logout_fn(usnr, nr, 0, ch, players);
            }
        }
    }
    
    /// Receive data from a player
    pub fn rec_player(
        &self,
        nr: usize,
        players: &mut PlayerManager,
        ch: &mut [Character],
        globs: &mut Global,
        logger: &Logger,
        plr_logout_fn: impl FnOnce(usize, usize, u8, &mut [Character], &mut PlayerManager),
    ) {
        let player = &mut players.players[nr];
        
        let sock = match &mut player.sock {
            Some(s) => s,
            None => return,
        };
        
        let remaining = 256 - player.in_len;
        if remaining == 0 {
            return;
        }
        
        let mut buf = vec![0u8; remaining];
        match sock.read(&mut buf) {
            Ok(0) => {
                // Connection closed
                xlog!(logger, "Connection closed (recv, EOF)");
                let usnr = player.usnr;
                player.disconnect();
                plr_logout_fn(usnr, nr, 0, ch, players);
            }
            Ok(len) => {
                player.inbuf[player.in_len..player.in_len + len].copy_from_slice(&buf[..len]);
                player.in_len += len;
                globs.recv += len as i64;
            }
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                // No data available, that's fine
            }
            Err(e) => {
                // receive failure
                xlog!(logger, "Connection closed (recv, {})", e);
                let usnr = player.usnr;
                player.disconnect();
                plr_logout_fn(usnr, nr, 0, ch, players);
            }
        }
    }
    
    /// Get the raw file descriptor for select()
    pub fn get_fd(&self) -> i32 {
        self.listener.as_raw_fd()
    }
}

/// Queue data to be sent to a specific player (internal helper that works on Player directly)
fn csend_player(
    player: &mut Player,
    buf: &[u8],
    logger: &Logger,
) -> i32 {
    if player.sock.is_none() {
        return -1;
    }
    
    for &byte in buf {
        let mut tmp = player.iptr + 1;
        if tmp == OBUFSIZE {
            tmp = 0;
        }
        
        if tmp == player.optr {
            xlog!(logger, "Connection too slow, terminated");
            player.disconnect();
            return -1;
        }
        
        player.obuf[player.iptr] = byte;
        player.iptr = tmp;
    }
    
    0
}

/// Send data to the circular output buffer
/// Returns -1 on error, 0 on success
pub fn csend(nr: usize, buf: &[u8], players: &mut PlayerManager, logger: &Logger) -> i32 {
    if nr < 1 || nr >= MAXPLAYER {
        return -1;
    }
    
    csend_player(&mut players.players[nr], buf, logger)
}

/// Queue data to be sent at the end of the tick
pub fn xsend(
    nr: usize,
    buf: &[u8],
    players: &mut PlayerManager,
    pkt_stats: &mut PacketStats,
    logger: &Logger,
) {
    if nr < 1 || nr >= MAXPLAYER {
        return;
    }
    
    let player = &mut players.players[nr];
    
    if player.sock.is_none() {
        return;
    }
    
    if player.tptr + buf.len() >= TBUFSIZE {
        xlog!(logger, "#INTERNAL ERROR# ticksize too large, terminated");
        player.disconnect();
        return;
    }
    
    player.tbuf[player.tptr..player.tptr + buf.len()].copy_from_slice(buf);
    player.tptr += buf.len();
    
    // Track packet statistics
    if !buf.is_empty() {
        let pnr = buf[0] as usize;
        pkt_stats.pkt_cnt[pnr] += buf.len() as i32;
        
        if pnr > 128 {
            pkt_stats.pkt_mapshort += buf.len() as i32;
        } else if pnr == SV_SETMAP3 as usize 
               || pnr == SV_SETMAP4 as usize 
               || pnr == SV_SETMAP5 as usize 
               || pnr == SV_SETMAP6 as usize 
        {
            pkt_stats.pkt_light += buf.len() as i32;
        }
    }
}

/// Compress and send tick data to all connected players
pub fn compress_ticks(
    players: &mut PlayerManager,
    ch: &mut [Character],
    logger: &Logger,
) {
    for n in 1..MAXPLAYER {
        let player = &mut players.players[n];
        
        if player.sock.is_none() {
            continue;
        }
        if player.ticker_started == 0 {
            continue;
        }
        if player.usnr >= MAXCHARS {
            player.usnr = 0;
        }
        
        let ilen = player.tptr;
        let mut olen = player.tptr + 2;
        
        if olen > 16 {
            // Compress the tick data
            let input = &player.tbuf[..ilen];
            
            // Use a new encoder for each compression
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
            if let Err(e) = encoder.write_all(input) {
                xlog!(logger, "ARGH compression error: {}", e);
                continue;
            }
            
            let compressed = match encoder.finish() {
                Ok(data) => data,
                Err(e) => {
                    xlog!(logger, "ARGH compression finish error: {}", e);
                    continue;
                }
            };
            
            let csize = compressed.len();
            olen = (csize + 2) | 0x8000;
            
            // Send compressed data
            let olen_bytes = (olen as u16).to_le_bytes();
            if csend_player(player, &olen_bytes, logger) == -1 {
                continue;
            }
            if csend_player(player, &compressed, logger) == -1 {
                continue;
            }
        } else {
            // Send uncompressed
            let olen_bytes = (olen as u16).to_le_bytes();
            if csend_player(player, &olen_bytes, logger) == -1 {
                continue;
            }
            if ilen > 0 {
                let data = player.tbuf[..ilen].to_vec();
                if csend_player(player, &data, logger) == -1 {
                    continue;
                }
            }
        }
        
        // Update character volume stats
        let usnr = player.usnr;
        if usnr > 0 && usnr < MAXCHARS {
            ch[usnr].comp_volume += olen as u32;
            ch[usnr].raw_volume += ilen as u32;
        }
        
        player.tptr = 0;
        
        // xlog("uncompressed tick size=%d byte",ilen+4);
        // xlog("compressed tick size=%d byte",olen+4);
    }
}
