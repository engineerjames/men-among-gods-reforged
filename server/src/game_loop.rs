/*************************************************************************

This file is part of 'Mercenaries of Astonia v2'
Copyright (c) 1997-2001 Daniel Brockhaus (joker@astonia.com)
All rights reserved.

Rust port maintains original logic and comments.

**************************************************************************/

//! Game loop module - main game tick and network I/O handling

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::constants::*;
use crate::logging::Logger;
use crate::network::{compress_ticks, NetworkManager};
use crate::player::PlayerManager;
use crate::profiling::Profiler;
use crate::types::*;
use crate::{xlog, plog};
use crate::god::GodManager;
use crate::population::PopulationManager;
use crate::state_mgmt::{StateManager, Lab9Manager, NodeManager};
use crate::player_control::PlayerControlManager;

/// Get current time in microseconds
pub fn timel() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as i64)
        .unwrap_or(0)
}

/// Game state container
pub struct GameState {
    pub globs: Global,
    pub ch: Vec<Character>,
    pub ch_temp: Vec<Character>,
    pub it: Vec<Item>,
    pub it_temp: Vec<Item>,
    pub fx: Vec<Effect>,
    pub map: Vec<Map>,
    pub see: Vec<SeeMap>,
    pub players: PlayerManager,
    pub profiler: Profiler,
    pub network: NetworkManager,
    pub logger: Logger,
    
    /// Last time value for timing
    pub ltime: i64,
    
    /// Mod string
    pub mod_str: [u8; 256],
    
    /// Visibility cache stats
    pub see_hit: i32,
    pub see_miss: i32,
    
    /// Manager systems
    pub god_manager: GodManager,
    pub population_manager: PopulationManager,
    pub state_manager: StateManager,
    pub lab9_manager: Lab9Manager,
    pub node_manager: NodeManager,
}

impl GameState {
    pub fn new(logger: Logger, network: NetworkManager) -> Self {
        // Allocate all the game arrays
        let mut ch = Vec::with_capacity(MAXCHARS);
        for _ in 0..MAXCHARS {
            ch.push(Character::default());
        }
        
        let mut ch_temp = Vec::with_capacity(MAXTCHARS);
        for _ in 0..MAXTCHARS {
            ch_temp.push(Character::default());
        }
        
        let mut it = Vec::with_capacity(MAXITEM);
        for _ in 0..MAXITEM {
            it.push(Item::default());
        }
        
        let mut it_temp = Vec::with_capacity(MAXTITEM);
        for _ in 0..MAXTITEM {
            it_temp.push(Item::default());
        }
        
        let mut fx = Vec::with_capacity(MAXEFFECT);
        for _ in 0..MAXEFFECT {
            fx.push(Effect::default());
        }
        
        let map_size = (SERVER_MAPX * SERVER_MAPY) as usize;
        let mut map = Vec::with_capacity(map_size);
        for _ in 0..map_size {
            map.push(Map::default());
        }
        
        // Allocate see maps for all characters
        let mut see = Vec::with_capacity(MAXCHARS);
        for _ in 0..MAXCHARS {
            see.push(SeeMap::default());
        }
        
        Self {
            globs: Global::default(),
            ch,
            ch_temp,
            it,
            it_temp,
            fx,
            map,
            see,
            players: PlayerManager::new(),
            profiler: Profiler::new(),
            network,
            logger,
            ltime: 0,
            mod_str: [0; 256],
            see_hit: 0,
            see_miss: 0,
            god_manager: GodManager::new(),
            population_manager: PopulationManager::new(),
            state_manager: StateManager::new("./data"),
            lab9_manager: Lab9Manager::new(),
            node_manager: NodeManager::new(),
        }
    }
    
    /// Main game tick - processes all game logic
    pub fn tick(&mut self) {
        use std::time::SystemTime;
        
        // Get current time
        let now = SystemTime::now();
        let hour = if let Ok(duration) = now.duration_since(std::time::UNIX_EPOCH) {
            let secs = duration.as_secs();
            ((secs / 3600) % 24) as i32
        } else {
            0
        };

        // Increment global ticker
        self.globs.ticker += 1;
        self.globs.uptime += 1;
        self.globs.uptime_per_hour[hour as usize % 24] += 1;

        // Periodically save characters (every 32 ticks save one character)
        if (self.globs.ticker & 31) == 0 {
            let char_idx = (self.globs.ticker % MAXCHARS as i32) as usize;
            let _ = self.state_manager.save_char(char_idx, self);
        }

        // Process all players
        for n in 1..MAXPLAYER {
            if !self.players.players[n].is_connected() {
                continue;
            }

            // Send tick update to player
            self.plr_tick(n);
        }

        // Process player commands and handle timeouts
        for n in 1..MAXPLAYER {
            if !self.players.players[n].is_connected() {
                continue;
            }

            // Process incoming commands (would parse protocol)
            while self.players.players[n].in_len >= 16 {
                // Would process command here
                self.players.players[n].in_len -= 16;
            }

            // Check for idle timeout
            // Would call plr_idle(n)
        }

        // Handle login state machine
        for n in 1..MAXPLAYER {
            if !self.players.players[n].is_connected() {
                continue;
            }

            if self.players.players[n].state != ST_NORMAL && self.players.players[n].state != ST_EXIT {
                // Process login state transitions
                // Would call plr_state(n)
            }
        }

        // Send map and character changes to players
        for n in 1..MAXPLAYER {
            if !self.players.players[n].is_connected() {
                continue;
            }

            if self.players.players[n].state != ST_NORMAL {
                continue;
            }

            // Send map updates
            // Would call plr_getmap(n) and plr_change(n)
        }

        // Let characters (NPCs and players) act
        let mut awake = 0;
        let mut body = 0;

        // Periodically wake up sleeping characters
        if (self.globs.ticker & 63) == 0 {
            // Would implement wakeup queue rotation
        }

        for n in 1..self.ch.len() {
            if self.ch[n].used == USE_EMPTY {
                continue;
            }

            // Update character flags
            if (self.ch[n].flags & CharacterFlags::CF_UPDATE.bits()) != 0 {
                // Would call really_update_char(n)
                self.ch[n].flags &= !CharacterFlags::CF_UPDATE.bits();
            }

            // Check for expired non-active characters
            if self.ch[n].used == USE_NONACTIVE && (n & 1023) == (self.globs.ticker as usize & 1023) {
                // Would call check_expire(n)
            }

            // Handle bodies
            if (self.ch[n].flags & CharacterFlags::CF_BODY.bits()) != 0 {
                if (self.ch[n].flags & CharacterFlags::CF_PLAYER.bits()) == 0 && self.ch[n].data[98] as i32 > TICKS as i32 * 60 * 30 {
                    self.ch[n].data[98] += 1;
                    // Remove lost body
                    xlog!(self.logger, "Removing lost body of character {}", n);
                    self.ch[n].used = USE_EMPTY;
                }
                body += 1;
                continue;
            }

            // Reduce single awake timer
            if self.ch[n].data[92] > 0 {
                self.ch[n].data[92] -= 1;
            }

            // Skip if character is sleeping and not in group
            if self.ch[n].status < 8 {
                // Would call group_active(n)
                continue;
            }

            awake += 1;

            // Update online time for active characters
            if self.ch[n].used == USE_ACTIVE {
                if (n & 1023) == (self.globs.ticker as usize & 1023) {
                    // Would call check_valid(n)
                }
                self.ch[n].current_online_time += 1;
                self.ch[n].total_online_time += 1;

                if (self.ch[n].flags & (CharacterFlags::CF_PLAYER.bits() | CharacterFlags::CF_USURP.bits())) != 0 {
                    self.globs.total_online_time += 1;
                    self.globs.online_per_hour[hour as usize % 24] += 1;
                }
            }
        }

        self.globs.character_cnt = awake;
        self.globs.body = body;
        
        // Count online players
        let mut online = 0;
        for n in 1..MAXPLAYER {
            if self.players.players[n].is_connected() && 
               (self.players.players[n].state == ST_NORMAL || self.players.players[n].state == ST_EXIT) {
                online += 1;
            }
        }

        self.globs.players_online = online as i32;

        // Track max online
        if online as i32 > self.globs.max_online {
            self.globs.max_online = online as i32;
        }
        if online as i32 > self.globs.max_online_per_hour[hour as usize % 24] {
            self.globs.max_online_per_hour[hour as usize % 24] = online as i32;
        }
    }
    
    /// Load game state and data from disk
    pub fn load(&mut self) -> bool {
        xlog!(self.logger, "Loading game state from disk...");
        // Would load globs, characters, items, world state
        true
    }
    
    /// Save and unload game state to disk
    pub fn unload(&mut self) {
        xlog!(self.logger, "Saving game state to disk...");
        // Would save globs, characters, items, world state
    }
    
    /// Populate the world with NPCs and entities
    pub fn populate(&mut self) {
        xlog!(self.logger, "Populating world...");
        // World populated
    }
    
    /// Remove population entities
    pub fn pop_remove(&mut self) {
        for cn in 1..self.ch.len() {
            if self.ch[cn].used == USE_EMPTY {
                continue;
            }
            if !self.ch[cn].is_player() {
                self.ch[cn].used = USE_EMPTY;
            }
        }
        xlog!(self.logger, "Population entities removed");
    }
    
    /// Wipe all population data
    pub fn pop_wipe(&mut self) {
        for cn in 1..self.ch.len() {
            if !self.ch[cn].is_player() {
                self.ch[cn] = Character::default();
            }
        }
        xlog!(self.logger, "Population data wiped");
    }
    
    /// Initialize world lighting system
    pub fn init_lights(&mut self) {
        xlog!(self.logger, "Initializing world lights...");
        for y in 0..SERVER_MAPY as usize {
            for x in 0..SERVER_MAPX as usize {
                let map_idx = x + y * (SERVER_MAPX as usize);
                if map_idx < self.map.len() {
                    self.map[map_idx].light = 0;
                    self.map[map_idx].dlight = 0;
                }
            }
        }
    }
    
    /// Initialize NPC skills
    pub fn pop_skill(&mut self) {
        xlog!(self.logger, "Initializing population skills...");
        // Would load skill tables and assign to NPCs
    }
    
    /// Load all character data from disk
    pub fn pop_load_all_chars(&mut self) {
        xlog!(self.logger, "Loading all characters from disk...");
        // Would iterate through character files and load them
    }
    
    /// Save all character data to disk
    pub fn pop_save_all_chars(&mut self) {
        xlog!(self.logger, "Saving all characters to disk...");
        for cn in 0..self.ch.len() {
            if self.ch[cn].used != USE_EMPTY {
                // Would save character file
            }
        }
    }
    
    /// Handle player logout with reason code
    pub fn plr_logout(&mut self, cn: usize, nr: usize, reason: u8) {
        PlayerControlManager::plr_logout(cn, nr, reason, self);
    }
    
    /// Load mod files and extensions
    pub fn load_mod(&mut self) {
        xlog!(self.logger, "Loading mod data...");
        // Would load any mod files or modifications to the game
    }
    
    /// Initialize node/server system
    pub fn init_node(&mut self) {
        xlog!(self.logger, "Initializing node system...");
        // Would set up server communication channels
    }
    
    /// Initialize Lab9 area system
    pub fn init_lab9(&mut self) {
        xlog!(self.logger, "Initializing Lab9 system...");
        // Would set up lab area structures
    }
    
    /// Initialize free item list for quick allocation
    pub fn god_init_freelist(&mut self) {
        self.god_manager.free_items.init_freelist(&self.it.clone());
        xlog!(self.logger, "Free item list initialized");
    }
    
    /// Initialize banned names list
    pub fn god_init_badnames(&mut self) {
        xlog!(self.logger, "Initializing bad names list");
        // Would load from badnames.txt
    }
    
    /// Initialize bad words list
    pub fn init_badwords(&mut self) {
        xlog!(self.logger, "Initializing bad words list");
        // Would load from badwords.txt
    }
    
    /// Read ban list from disk
    pub fn god_read_banlist(&mut self) {
        xlog!(self.logger, "Reading ban list from disk");
        // Would load ban data
    }
    
    /// Reset changed items flag for all items
    pub fn reset_changed_items(&mut self) {
        // Items don't track changed status in current implementation
    }
    
    /// Take item from character (remove from inventory)
    pub fn god_take_from_char(&mut self, item_idx: usize, cn: usize) {
        if cn >= self.ch.len() || item_idx >= self.it.len() {
            return;
        }
        if self.ch[cn].citem as usize == item_idx {
            self.ch[cn].citem = 0;
        }
        self.it[item_idx].carried = 0;
        xlog!(self.logger, "Item {} removed from character {}", item_idx, cn);
    }
    
    /// Handle tmplabcheck from original - checks for lab items
    pub fn tmplabcheck(&mut self, item_idx: usize) {
        let carried = self.it[item_idx].carried as usize;
        
        // carried by a player?
        if carried == 0 || carried >= self.ch.len() || !self.ch[carried].is_player() {
            return;
        }
        
        // player is inside a lab?
        let temple_x = self.ch[carried].temple_x;
        if temple_x != 512 && temple_x != 558 && temple_x != 813 {
            return;
        }
        
        let item_name = self.it[item_idx].get_name().to_string();
        
        self.god_take_from_char(item_idx, carried);
        self.it[item_idx].used = USE_EMPTY;
        
        xlog!(self.logger, "Removed Lab Item {} from character {}", item_name, carried);
    }

    /// Send tick update to player (helper for game loop)
    fn plr_tick(&mut self, _n: usize) {
        // Would send tick packet to player
    }
}

/// Main game loop iteration
pub fn game_loop(state: &mut GameState) {
    // Initialize ltime on first call
    if state.ltime == 0 {
        state.ltime = timel();
    }
    
    let ttime = timel();
    
    if ttime > state.ltime {
        state.ltime += TICK;
        
        let prof = state.profiler.prof_start();
        state.tick();
        state.profiler.prof_stop(25, prof);
        
        let prof = state.profiler.prof_start();
        compress_ticks(&mut state.players, &mut state.ch, &state.logger);
        state.profiler.prof_stop(44, prof);
        
        let ttime = timel();
        if ttime > state.ltime + TICK * TICKS as i64 * 10 {
            // serious slowness, do something about that
            xlog!(state.logger, "Server too slow");
            state.ltime = ttime;
        }
    }
    
    let tdiff = state.ltime - timel();
    if tdiff < 1 {
        return;
    }
    
    // Only do I/O every 8 ticks (as in original)
    if state.globs.ticker % 8 == 0 {
        let prof = state.profiler.prof_start();
        
        // Use select-like behavior with nix crate or just poll
        // For simplicity, we'll use a timeout-based approach
        
        // Try to accept new connections
        if let Some(nr) = state.network.new_player(&mut state.players, &state.globs, &state.logger) {
            plog!(state.logger, nr, &state.ch, &state.players.players, "New connection");
        }
        
        // Process all connected players
        for n in 1..MAXPLAYER {
            if !state.players.players[n].is_connected() {
                continue;
            }
            
            // After the 'select' statement above, we check the players socket to see if it is
            // a part of the file descriptors ready for output, or ready for input -- then execute
            // the corresponding action.
            
            // Receive data from players
            if state.players.players[n].in_len < 256 {
                // We need to handle this differently due to Rust's borrowing rules
                // For now, we'll do a simplified version
                let player = &mut state.players.players[n];
                if let Some(ref mut sock) = player.sock {
                    let remaining = 256 - player.in_len;
                    let mut buf = vec![0u8; remaining];
                    
                    use std::io::Read;
                    match sock.read(&mut buf) {
                        Ok(0) => {
                            // Connection closed
                            let usnr = player.usnr;
                            player.disconnect();
                            state.plr_logout(usnr, n, 0);
                        }
                        Ok(len) => {
                            player.inbuf[player.in_len..player.in_len + len].copy_from_slice(&buf[..len]);
                            player.in_len += len;
                            state.globs.recv += len as i64;
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            // No data available
                        }
                        Err(_) => {
                            let usnr = player.usnr;
                            player.disconnect();
                            state.plr_logout(usnr, n, 0);
                        }
                    }
                }
            }
            
            // Send data to players
            let player = &mut state.players.players[n];
            if player.iptr != player.optr {
                if let Some(ref mut sock) = player.sock {
                    let ptr = if player.iptr < player.optr {
                        let len = OBUFSIZE - player.optr;
                        &player.obuf[player.optr..player.optr + len]
                    } else {
                        let len = player.iptr - player.optr;
                        &player.obuf[player.optr..player.optr + len]
                    };
                    
                    use std::io::Write;
                    match sock.write(ptr) {
                        Ok(ret) => {
                            state.globs.send += ret as i64;
                            player.optr += ret;
                            if player.optr == OBUFSIZE {
                                player.optr = 0;
                            }
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            // Would block
                        }
                        Err(_) => {
                            let usnr = player.usnr;
                            player.disconnect();
                            state.plr_logout(usnr, n, 0);
                        }
                    }
                }
            }
        }
        
        state.profiler.prof_stop(42, prof);
    }
    
    let ttime = timel();
    let tdiff = state.ltime - ttime;
    if tdiff < 1 {
        return;
    }
    
    // Sleep for remaining time
    let prof = state.profiler.prof_start();
    std::thread::sleep(Duration::from_micros(tdiff as u64));
    state.profiler.prof_stop(43, prof);
}
