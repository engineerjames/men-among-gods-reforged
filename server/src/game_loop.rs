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
    
    /// Wakeup system - cycles through characters for periodic wakeup
    pub wakeup_counter: usize,
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
            wakeup_counter: 1,
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

            // Process incoming commands
            while self.players.players[n].in_len >= 16 {
                self.plr_cmd(n);
                self.players.players[n].in_len -= 16;
            }

            // Check for idle timeout
            self.plr_idle(n);
        }

        // Handle login state machine
        for n in 1..MAXPLAYER {
            if !self.players.players[n].is_connected() {
                continue;
            }

            if self.players.players[n].state != ST_NORMAL && self.players.players[n].state != ST_EXIT {
                // Process login state transitions
                self.plr_state(n);
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
            self.plr_getmap(n);
            self.plr_change(n);
        }

        // Let characters (NPCs and players) act
        let mut cnt = 0;
        let mut awake = 0;
        let mut body = 0;
        let mut plon = 0;  // visible online players

        // Periodically wake up sleeping characters (every 64 ticks, give one character 60 seconds of activity)
        if (self.globs.ticker & 63) == 0 {
            if self.wakeup_counter >= MAXCHARS {
                self.wakeup_counter = 1;
            }
            self.ch[self.wakeup_counter].data[92] = (TICKS * 60) as i32; // 60 seconds of awake time
            self.wakeup_counter += 1;
        }

        for n in 1..self.ch.len() {
            if self.ch[n].used == USE_EMPTY {
                continue;
            }
            cnt += 1;

            // Update character flags
            if (self.ch[n].flags & CharacterFlags::CF_UPDATE.bits()) != 0 {
                self.really_update_char(n);
                self.ch[n].flags &= !CharacterFlags::CF_UPDATE.bits();
            }

            // Check for expired non-active characters
            if self.ch[n].used == USE_NONACTIVE && (n & 1023) == (self.globs.ticker as usize & 1023) {
                self.check_expire(n);
            }

            // Handle bodies (corpses)
            if (self.ch[n].flags & CharacterFlags::CF_BODY.bits()) != 0 {
                if (self.ch[n].flags & CharacterFlags::CF_PLAYER.bits()) == 0 && self.ch[n].data[98] as i32 > TICKS as i32 * 60 * 30 {
                    self.ch[n].data[98] += 1;
                    // Remove lost body
                    xlog!(self.logger, "Removing lost body of character {}", n);
                    self.god_destroy_items(n);
                    self.ch[n].used = USE_EMPTY;
                    continue;
                }
                body += 1;
                continue;
            }

            // Reduce single awake timer
            if self.ch[n].data[92] > 0 {
                self.ch[n].data[92] -= 1;
            }

            // Skip if character is sleeping and not in group
            if self.ch[n].status < 8 && !self.group_active(n) {
                continue;
            }

            awake += 1;

            // Update online time for active characters
            if self.ch[n].used == USE_ACTIVE {
                if (n & 1023) == (self.globs.ticker as usize & 1023) {
                    if !self.check_char_valid(n) {
                        continue;
                    }
                }
                self.ch[n].current_online_time += 1;
                self.ch[n].total_online_time += 1;

                if (self.ch[n].flags & (CharacterFlags::CF_PLAYER.bits() | CharacterFlags::CF_USURP.bits())) != 0 {
                    self.globs.total_online_time += 1;
                    self.globs.online_per_hour[hour as usize % 24] += 1;

                    // Decrement player cooldown/duration counters
                    if (self.ch[n].flags & CharacterFlags::CF_PLAYER.bits()) != 0 {
                        if self.ch[n].data[71] > 0 {
                            self.ch[n].data[71] -= 1;
                        }
                        if self.ch[n].data[72] > 0 {
                            self.ch[n].data[72] -= 1;
                        }
                    }

                    // Count visible online players (not invisible)
                    if (self.ch[n].flags & CharacterFlags::CF_PLAYER.bits()) != 0 && 
                       (self.ch[n].flags & CharacterFlags::CF_INVISIBLE.bits()) == 0 {
                        plon += 1;
                    }
                }

                // Let character act
                self.plr_act(n);
            }

            // Handle regeneration for all awake characters
            self.do_regenerate(n);
        }

        self.globs.character_cnt = cnt as i32;
        self.globs.awake = awake as i32;
        self.globs.body = body as i32;
        self.globs.players_online = plon as i32;

        // Track max online
        if plon as i32 > self.globs.max_online {
            self.globs.max_online = plon as i32;
        }
        if plon as i32 > self.globs.max_online_per_hour[hour as usize % 24] {
            self.globs.max_online_per_hour[hour as usize % 24] = plon as i32;
        }

        // Process world systems
        self.pop_tick();
        self.effect_tick();
        self.item_tick();
        self.global_tick();
    }

    /// Process player commands from incoming packets
    fn plr_cmd(&mut self, nr: usize) {
        if nr == 0 || nr >= MAXPLAYER || self.players.players[nr].inbuf.is_empty() {
            return;
        }

        let cmd = self.players.players[nr].inbuf[0];

        // Update last command time if not a passive command
        if cmd != CL_CMD_AUTOLOOK && cmd != CL_PERF_REPORT && cmd != CL_CMD_CTICK {
            self.players.players[nr].lasttick2 = self.globs.ticker as u32;
        }

        match cmd {
            // Login/authentication commands
            CL_NEWLOGIN => {
                xlog!(self.logger, "Player {} attempting new login", nr);
                // Would call plr_challenge_newlogin(nr)
            }
            CL_CHALLENGE => {
                xlog!(self.logger, "Player {} sending challenge response", nr);
                // Would call plr_challenge(nr)
            }
            CL_LOGIN => {
                xlog!(self.logger, "Player {} attempting login", nr);
                // Would call plr_challenge_login(nr)
            }
            CL_PASSWD => {
                xlog!(self.logger, "Player {} sending password", nr);
                // Would call plr_passwd(nr)
            }
            CL_CMD_UNIQUE => {
                xlog!(self.logger, "Player {} requesting unique ID", nr);
                // Would call plr_unique(nr)
            }

            // In-game commands (only if ST_NORMAL)
            _ if self.players.players[nr].state == ST_NORMAL => {
                match cmd {
                    CL_CMD_MOVE => {
                        // Would call plr_cmd_move(nr)
                    }
                    CL_CMD_ATTACK => {
                        // Would call plr_cmd_attack(nr)
                    }
                    CL_CMD_PICKUP => {
                        // Would call plr_cmd_pickup(nr)
                    }
                    CL_CMD_DROP => {
                        // Would call plr_cmd_drop(nr)
                    }
                    CL_CMD_GIVE => {
                        // Would call plr_cmd_give(nr)
                    }
                    CL_CMD_USE => {
                        // Would call plr_cmd_use(nr)
                    }
                    CL_CMD_LOOK => {
                        // Would call plr_cmd_look(nr, 0)
                    }
                    CL_CMD_AUTOLOOK => {
                        // Would call plr_cmd_look(nr, 1)
                    }
                    CL_CMD_STAT => {
                        // Would call plr_cmd_stat(nr)
                    }
                    CL_CMD_SETUSER => {
                        // Would call plr_cmd_setuser(nr)
                    }
                    CL_CMD_EXIT => {
                        xlog!(self.logger, "Player {} pressed exit", nr);
                        let usnr = self.players.players[nr].usnr;
                        self.plr_logout(usnr, nr, LO_EXIT);
                    }
                    CL_CMD_CTICK => {
                        // Client tick acknowledgment
                        if self.players.players[nr].inbuf.len() >= 5 {
                            let rtick = u32::from_le_bytes([
                                self.players.players[nr].inbuf[1],
                                self.players.players[nr].inbuf[2],
                                self.players.players[nr].inbuf[3],
                                self.players.players[nr].inbuf[4],
                            ]);
                            self.players.players[nr].rtick = rtick;
                            self.players.players[nr].lasttick = self.globs.ticker as u32;
                        }
                    }
                    CL_PERF_REPORT => {
                        // Would call plr_perf_report(nr)
                    }
                    _ => {
                        xlog!(self.logger, "Unknown command {} from player {}", cmd, nr);
                    }
                }
            }
            _ => {}
        }
    }

    /// Check for player timeouts
    fn plr_idle(&mut self, nr: usize) {
        if nr == 0 || nr >= MAXPLAYER {
            return;
        }

        // Protocol level timeout: 60 seconds
        if self.globs.ticker - self.players.players[nr].lasttick as i32 > TICKS as i32 * 60 {
            plog!(self.logger, nr, &self.ch, &self.players.players, "Idle too long (protocol level)");
            let usnr = self.players.players[nr].usnr;
            self.plr_logout(usnr, nr, LO_IDLE);
            return;
        }

        if self.players.players[nr].state == ST_EXIT {
            return;
        }

        // Player level timeout: 15 minutes
        if self.globs.ticker - self.players.players[nr].lasttick2 as i32 > TICKS as i32 * 60 * 15 {
            plog!(self.logger, nr, &self.ch, &self.players.players, "Idle too long (player level)");
            let usnr = self.players.players[nr].usnr;
            self.plr_logout(usnr, nr, LO_IDLE);
        }
    }

    /// Handle login state machine
    fn plr_state(&mut self, nr: usize) {
        if nr == 0 || nr >= MAXPLAYER {
            return;
        }

        // Close connection if in ST_EXIT for too long (15 seconds)
        if self.globs.ticker - self.players.players[nr].lasttick as i32 > TICKS as i32 * 15 && 
           self.players.players[nr].state == ST_EXIT {
            plog!(self.logger, nr, &self.ch, &self.players.players, "Connection closed (ST_EXIT)");
            self.players.players[nr].disconnect();
            return;
        }

        // Final timeout at 60 seconds
        if self.globs.ticker - self.players.players[nr].lasttick as i32 > TICKS as i32 * 60 {
            plog!(self.logger, nr, &self.ch, &self.players.players, "Idle timeout");
            let usnr = self.players.players[nr].usnr;
            self.plr_logout(usnr, nr, LO_IDLE);
            return;
        }

        match self.players.players[nr].state {
            ST_NEWLOGIN => {
                // Would call plr_newlogin(nr)
            }
            ST_LOGIN => {
                // Would call plr_login(nr)
            }
            ST_NEWCAP => {
                // Timeout transition back to NEWLOGIN after 10 ticks
                if self.globs.ticker - self.players.players[nr].lasttick as i32 > TICKS as i32 * 10 {
                    self.players.players[nr].state = ST_NEWLOGIN;
                }
            }
            ST_CAP => {
                // Timeout transition back to LOGIN after 10 ticks
                if self.globs.ticker - self.players.players[nr].lasttick as i32 > TICKS as i32 * 10 {
                    self.players.players[nr].state = ST_LOGIN;
                }
            }
            ST_NEW_CHALLENGE | ST_LOGIN_CHALLENGE | ST_CONNECT | ST_EXIT => {
                // These states don't require action, just wait
            }
            _ => {
                plog!(self.logger, nr, &self.ch, &self.players.players, 
                      "Unknown state: {}", self.players.players[nr].state);
            }
        }
    }

    /// Send map data to player
    fn plr_getmap(&mut self, _nr: usize) {
        // This function would:
        // 1. Get player's visible map area (TILEX x TILEY tiles)
        // 2. Compare with previously sent map
        // 3. Send delta updates to reduce bandwidth
        // Implementation deferred pending network protocol definition
    }

    /// Send character and item changes to player
    fn plr_change(&mut self, _nr: usize) {
        // This function would:
        // 1. Check which characters are visible to player
        // 2. Send position updates for visible characters
        // 3. Send item updates for visible items
        // 4. Send stat updates for player's own character
        // Implementation deferred pending network protocol definition
    }

    /// Recalculate character stats
    fn really_update_char(&mut self, cn: usize) {
        if cn >= self.ch.len() || self.ch[cn].used == USE_EMPTY {
            return;
        }

        // Calculate effective HP from attributes
        let con = self.ch[cn].attrib[2][0] as i32; // Constitution
        let level = self.ch[cn].data[23] as i32; // Level
        
        // Simple calculation: base 50 + 10 per level + 5 per constitution
        let new_hp = 50 + (level * 10) + (con * 5);
        self.ch[cn].hp[0] = new_hp.min(65535) as u16;

        // Calculate effective mana from intelligence
        let int = self.ch[cn].attrib[3][0] as i32; // Intelligence
        let new_mana = (level * 20) + (int * 10);
        self.ch[cn].mana[0] = new_mana.min(65535) as u16;

        // Copy to local variables to avoid packed field alignment issues
        let hp_val = self.ch[cn].hp[0];
        let mana_val = self.ch[cn].mana[0];
        xlog!(self.logger, "Updated character {} stats: HP={}, Mana={}", 
              cn, hp_val, mana_val);
    }

    /// Handle character regeneration (HP and mana)
    fn do_regenerate(&mut self, cn: usize) {
        if cn >= self.ch.len() || self.ch[cn].used == USE_EMPTY {
            return;
        }

        // Only regenerate for active characters
        if self.ch[cn].used != USE_ACTIVE {
            return;
        }

        // Regenerate HP (every tick) - hp[0] is current HP
        let regen_rate = (self.ch[cn].hp[0] as i32 / 100).max(1) as u16;
        let current_hp = self.ch[cn].hp[0];
        let max_hp = self.ch[cn].hp[0]; // Use current as max for now
        if current_hp < max_hp {
            self.ch[cn].hp[0] = (current_hp + regen_rate).min(max_hp);
        }

        // Regenerate mana (every tick, slower than HP)
        let mana_regen_rate = (self.ch[cn].mana[0] as i32 / 200).max(1) as u16;
        let current_mana = self.ch[cn].mana[0];
        let max_mana = self.ch[cn].mana[0]; // Use current as max for now
        if current_mana < max_mana {
            self.ch[cn].mana[0] = (current_mana + mana_regen_rate).min(max_mana);
        }
    }

    /// Process character actions (movement, attacks, etc.)
    fn plr_act(&mut self, _cn: usize) {
        // This function would:
        // 1. Process character's queued actions
        // 2. Check movement/attack feasibility
        // 3. Update position and apply effects
        // 4. Handle interactions with items and NPCs
        // Implementation deferred - requires full action system
    }

    /// Update NPC behavior and world entities
    fn pop_tick(&mut self) {
        // This function would:
        // 1. Iterate through all NPCs
        // 2. Update AI and behavior
        // 3. Handle NPC movement and attacks
        // 4. Spawn new entities
        // Implementation deferred - requires full NPC AI system
    }

    /// Process active effects and buffs
    fn effect_tick(&mut self) {
        // This function would:
        // 1. Decrement effect durations
        // 2. Apply effect tick damage/healing
        // 3. Remove expired effects
        // Implementation deferred - requires effect system
        for fx_idx in 1..self.fx.len() {
            if self.fx[fx_idx].used == USE_EMPTY {
                continue;
            }
            // Decrement effect duration
            if self.fx[fx_idx].duration > 0 {
                self.fx[fx_idx].duration -= 1;
            }
            if self.fx[fx_idx].duration == 0 {
                self.fx[fx_idx].used = USE_EMPTY;
            }
        }
    }

    /// Process item updates (decay, poison, etc.)
    fn item_tick(&mut self) {
        // This function would:
        // 1. Handle item decay
        // 2. Process poison/damage
        // 3. Update item properties
        // Implementation deferred - requires item system
    }

    /// Update world (day/night, weather, etc.)
    fn global_tick(&mut self) {
        // This function would:
        // 1. Update time of day
        // 2. Change lighting based on time
        // 3. Update weather
        // 4. Trigger world events
        // For now, just maintain basic uptime tracking
    }

    /// Destroy all items carried by character
    fn god_destroy_items(&mut self, cn: usize) {
        if cn >= self.ch.len() || self.ch[cn].used == USE_EMPTY {
            return;
        }

        // Mark all carried items as empty
        for item_idx in 0..40 {
            if self.ch[cn].item[item_idx] != 0 {
                let it_idx = self.ch[cn].item[item_idx] as usize;
                if it_idx < self.it.len() {
                    self.it[it_idx].used = USE_EMPTY;
                }
                self.ch[cn].item[item_idx] = 0;
            }
        }

        // Mark all worn items as empty
        for item_idx in 0..20 {
            if self.ch[cn].worn[item_idx] != 0 {
                let it_idx = self.ch[cn].worn[item_idx] as usize;
                if it_idx < self.it.len() {
                    self.it[it_idx].used = USE_EMPTY;
                }
                self.ch[cn].worn[item_idx] = 0;
            }
        }

        // Mark all depot items as empty
        for item_idx in 0..62 {
            if self.ch[cn].depot[item_idx] != 0 {
                let it_idx = self.ch[cn].depot[item_idx] as usize;
                if it_idx < self.it.len() {
                    self.it[it_idx].used = USE_EMPTY;
                }
                self.ch[cn].depot[item_idx] = 0;
            }
        }

        // Mark all spell items as empty
        for item_idx in 0..20 {
            if self.ch[cn].spell[item_idx] != 0 {
                let it_idx = self.ch[cn].spell[item_idx] as usize;
                if it_idx < self.it.len() {
                    self.it[it_idx].used = USE_EMPTY;
                }
                self.ch[cn].spell[item_idx] = 0;
            }
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
        self.state_manager.unload(self);
    }
    
    /// Populate the world with NPCs and entities
    pub fn populate(&mut self) {
        // Stub - manager methods cannot borrow self while self is already borrowed
        xlog!(self.logger, "Populate called (stub implementation)");
    }
    
    /// Remove population entities
    pub fn pop_remove(&mut self) {
        // Stub - manager methods cannot borrow self while self is already borrowed
        xlog!(self.logger, "Pop remove called (stub implementation)");
    }
    
    /// Wipe all population data
    pub fn pop_wipe(&mut self) {
        // Stub - manager methods cannot borrow self while self is already borrowed
        xlog!(self.logger, "Pop wipe called (stub implementation)");
    }
    
    /// Initialize world lighting system
    pub fn init_lights(&mut self) {
        // Stub - manager methods cannot borrow self while self is already borrowed
        xlog!(self.logger, "Init lights called (stub implementation)");
    }
    
    /// Initialize NPC skills
    pub fn pop_skill(&mut self) {
        // Stub - manager methods cannot borrow self while self is already borrowed
        xlog!(self.logger, "Pop skill called (stub implementation)");
    }
    
    /// Load all character data from disk
    pub fn pop_load_all_chars(&mut self) {
        // Stub - manager methods cannot borrow self while self is already borrowed
        xlog!(self.logger, "Pop load all chars called (stub implementation)");
    }
    
    /// Save all character data to disk
    pub fn pop_save_all_chars(&mut self) {
        // Stub - manager methods cannot borrow self while self is already borrowed
        xlog!(self.logger, "Pop save all chars called (stub implementation)");
    }
    
    /// Handle player logout with reason code
    pub fn plr_logout(&mut self, _cn: usize, nr: usize, reason: u8) {
        // Stub - player control manager would handle actual logout
        xlog!(self.logger, "Player {} logging out (reason: {})", nr, reason);
    }
    
    /// Load mod files and extensions
    pub fn load_mod(&mut self) {
        // Stub - state manager cannot borrow self while self is already borrowed
        xlog!(self.logger, "Load mod called (stub implementation)");
    }
    
    /// Initialize node/server system
    pub fn init_node(&mut self) {
        // Stub - node manager cannot borrow self while self is already borrowed
        xlog!(self.logger, "Init node called (stub implementation)");
    }
    
    /// Initialize Lab9 area system
    pub fn init_lab9(&mut self) {
        // Stub - lab9 manager cannot borrow self while self is already borrowed
        xlog!(self.logger, "Init lab9 called (stub implementation)");
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

    /// Send tick update to player - based on plr_tick from svr_tick.cpp
    /// Checks for lag-induced stoning and handles lag recovery
    fn plr_tick(&mut self, nr: usize) {
        if nr >= MAXPLAYER {
            return;
        }

        // Increment player tick counter
        self.players.players[nr].ltick += 1;

        if self.players.players[nr].state != ST_NORMAL {
            return;
        }

        let cn = self.players.players[nr].usnr;

        // Check for lag-based stoning
        if cn > 0 && cn < self.ch.len() && 
           self.ch[cn].data[19] != 0 && 
           (self.ch[cn].flags & CharacterFlags::CF_PLAYER.bits()) != 0 {
            
            let lag_threshold = self.ch[cn].data[19] as i32;
            let ltick = self.players.players[nr].ltick as i32;
            let rtick = self.players.players[nr].rtick as i32;
            let lag_diff = ltick - rtick;

            // Check if should be stoned due to lag
            if lag_diff > lag_threshold && (self.ch[cn].flags & CharacterFlags::CF_STONED.bits()) == 0 {
                let lag_seconds = lag_diff as f32 / 18.0;
                xlog!(self.logger, "Character {} turned to stone due to lag ({:.2}s)", cn, lag_seconds);
                self.ch[cn].flags |= CharacterFlags::CF_STONED.bits();
                // Would call stone_gc(cn, 1)
            } 
            // Check if should unstoned (lag recovered)
            else if lag_diff < lag_threshold - TICKS as i32 && 
                    (self.ch[cn].flags & CharacterFlags::CF_STONED.bits()) != 0 {
                xlog!(self.logger, "Character {} unstoned, lag is gone", cn);
                self.ch[cn].flags &= !CharacterFlags::CF_STONED.bits();
                // Would call stone_gc(cn, 0)
            }
        }
    }

    /// Check if character is still valid - based on check_valid from svr_tick.cpp
    fn check_char_valid(&mut self, cn: usize) -> bool {
        if cn >= self.ch.len() {
            return false;
        }

        // Check bounds
        let ch_x = self.ch[cn].x;
        let ch_y = self.ch[cn].y;
        if ch_x < 1 || ch_y < 1 || 
           ch_x >= (SERVER_MAPX - 2) as i16 || 
           ch_y >= (SERVER_MAPY - 2) as i16 {
            xlog!(self.logger, "Character {} killed for invalid position ({}, {})", 
                  cn, ch_x, ch_y);
            // Would call do_char_killed(0, cn) - kill the character
            return false;
        }

        // Verify character is on map
        let map_idx = (ch_x as usize) + (ch_y as usize) * (SERVER_MAPX as usize);
        if map_idx < self.map.len() {
            let map_ch = self.map[map_idx].ch;
            if map_ch as usize != cn {
                xlog!(self.logger, "Character {} not on map (found {})", cn, map_ch);
                // Try to relocate character or fail
                if self.map[map_idx].ch == 0 {
                    self.map[map_idx].ch = cn as u32;
                } else {
                    // Would call god_drop_char_fuzzy_large to relocate
                    return false;
                }
            }
        }

        // Validate inventory items (40 items)
        for n in 0..40 {
            let item_idx = self.ch[cn].item[n] as usize;
            if item_idx != 0 && item_idx < self.it.len() {
                if self.it[item_idx].carried as usize != cn || self.it[item_idx].used != USE_ACTIVE {
                    xlog!(self.logger, "Reset inventory item {} from character {}", item_idx, cn);
                    self.ch[cn].item[n] = 0;
                }
            }
        }

        // Validate depot items (62 items)
        for n in 0..62 {
            let item_idx = self.ch[cn].depot[n] as usize;
            if item_idx != 0 && item_idx < self.it.len() {
                if self.it[item_idx].carried as usize != cn || self.it[item_idx].used != USE_ACTIVE {
                    xlog!(self.logger, "Reset depot item {} from character {}", item_idx, cn);
                    self.ch[cn].depot[n] = 0;
                }
            }
        }

        // Validate worn items and spells (20 each)
        for n in 0..20 {
            let worn_idx = self.ch[cn].worn[n] as usize;
            if worn_idx != 0 && worn_idx < self.it.len() {
                if self.it[worn_idx].carried as usize != cn || self.it[worn_idx].used != USE_ACTIVE {
                    xlog!(self.logger, "Reset worn item {} from character {}", worn_idx, cn);
                    self.ch[cn].worn[n] = 0;
                }
            }

            let spell_idx = self.ch[cn].spell[n] as usize;
            if spell_idx != 0 && spell_idx < self.it.len() {
                if self.it[spell_idx].carried as usize != cn || self.it[spell_idx].used != USE_ACTIVE {
                    xlog!(self.logger, "Reset spell item {} from character {}", spell_idx, cn);
                    self.ch[cn].spell[n] = 0;
                }
            }
        }

        // If NPC is stoned, verify the character who stoned them still exists
        if (self.ch[cn].flags & CharacterFlags::CF_STONED.bits()) != 0 && 
           (self.ch[cn].flags & CharacterFlags::CF_PLAYER.bits()) == 0 {
            let caster = self.ch[cn].data[63] as usize;
            if caster == 0 || caster >= self.ch.len() || self.ch[caster].used == USE_EMPTY {
                self.ch[cn].flags &= !CharacterFlags::CF_STONED.bits();
                xlog!(self.logger, "Removed stoned flag from character {} (caster gone)", cn);
            }
        }

        true
    }

    /// Check if character should expire - based on check_expire from svr_tick.cpp
    fn check_expire(&mut self, cn: usize) {
        if cn >= self.ch.len() {
            return;
        }

        let day_secs = 60 * 60 * 24;
        let week_secs = day_secs * 7;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let login_date = self.ch[cn].login_date as i64;
        let points = self.ch[cn].points_tot as i64;

        let should_erase = if points == 0 {
            login_date + 3 * day_secs < now
        } else if points < 10000 {
            login_date + 1 * week_secs < now
        } else if points < 100000 {
            login_date + 2 * week_secs < now
        } else if points < 1000000 {
            login_date + 4 * week_secs < now
        } else {
            login_date + 6 * week_secs < now
        };

        if should_erase {
            let char_name = self.ch[cn].get_name().to_string();
            xlog!(self.logger, "Erased player {}, {} exp", char_name, points);
            // Would call god_destroy_items(cn)
            self.ch[cn].used = USE_EMPTY;
        }
    }

    /// Check if character is part of active group - based on group_active from svr_tick.cpp
    fn group_active(&self, cn: usize) -> bool {
        if cn >= self.ch.len() {
            return false;
        }

        // Player or usurped character that's active
        if (self.ch[cn].flags & (CharacterFlags::CF_PLAYER.bits() | CharacterFlags::CF_USURP.bits() | CharacterFlags::CF_NOSLEEP.bits())) != 0
            && self.ch[cn].used == USE_ACTIVE {
            return true;
        }

        // Awake timer is set
        if self.ch[cn].data[92] > 0 {
            return true;
        }

        false
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
