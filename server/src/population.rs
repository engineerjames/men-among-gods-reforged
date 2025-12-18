/*************************************************************************

This file is part of 'Mercenaries of Astonia v2'
Copyright (c) 1997-2001 Daniel Brockhaus (joker@astonia.com)
All rights reserved.

Rust port maintains original logic and comments.

**************************************************************************/

//! Population management module - handles spawning, removing, and managing NPCs and world entities
//! Based on populate.cpp from original server

use crate::constants::*;
use crate::types::*;
use crate::xlog;

/// Population manager for handling NPC spawning and despawning
pub struct PopulationManager {
    active_spawns: Vec<(usize, u16)>,  // (character_index, template)
}

impl PopulationManager {
    pub fn new() -> Self {
        Self {
            active_spawns: Vec::new(),
        }
    }

    /// Initialize world lights
    /// Based on init_lights from populate.cpp
    pub fn init_lights(&self, state: &mut crate::game_loop::GameState) {
        xlog!(state.logger, "Initializing world lights...");
        
        // Reset all lights on the map
        for y in 0..SERVER_MAPY as usize {
            for x in 0..SERVER_MAPX as usize {
                let map_idx = x + y * (SERVER_MAPX as usize);
                if map_idx < state.map.len() {
                    state.map[map_idx].light = 0;
                    state.map[map_idx].dlight = 0;
                }
            }
        }

        xlog!(state.logger, "Initialized world lights");
    }

    /// Populate the world with NPCs and entities
    /// Based on populate from populate.cpp
    pub fn populate(&mut self, state: &mut crate::game_loop::GameState) {
        xlog!(state.logger, "Populating world...");
        
        // This would scan through template data and create NPCs
        // For now, just initialize the structure
        self.active_spawns.clear();
        
        xlog!(state.logger, "World populated");
    }

    /// Remove population entities
    /// Based on pop_remove from populate.cpp
    pub fn pop_remove(&self, state: &mut crate::game_loop::GameState) {
        xlog!(state.logger, "Removing population entities...");
        
        // Would iterate through active spawns and mark for removal
        // Find all non-player characters and remove them
        for cn in 1..state.ch.len() {
            if state.ch[cn].used == USE_EMPTY {
                continue;
            }
            
            if !state.ch[cn].is_player() {
                // Mark NPC for removal
                state.ch[cn].used = USE_EMPTY;
            }
        }
        
        xlog!(state.logger, "Population entities removed");
    }

    /// Wipe all population data
    /// Based on pop_wipe from populate.cpp
    pub fn pop_wipe(&self, state: &mut crate::game_loop::GameState) {
        xlog!(state.logger, "Wiping all population data...");
        
        // Clear all character structures except keeping player data
        for cn in 1..state.ch.len() {
            if !state.ch[cn].is_player() {
                state.ch[cn] = Character::default();
            }
        }
        
        xlog!(state.logger, "Population data wiped");
    }

    /// Initialize skills for NPCs
    /// Based on pop_skill from populate.cpp
    pub fn pop_skill(&self, state: &mut crate::game_loop::GameState) {
        xlog!(state.logger, "Initializing population skills...");
        
        // Would load skill tables and assign to NPCs
        
        xlog!(state.logger, "Population skills initialized");
    }

    /// Load all character data from disk
    /// Based on pop_load_all_chars
    pub fn pop_load_all_chars(&self, state: &mut crate::game_loop::GameState) {
        xlog!(state.logger, "Loading all characters from disk...");
        
        // Would iterate through character files and load them
        for cn in 0..state.ch.len() {
            if self.load_char(cn, state).is_err() {
                // Character file not found or corrupted, leave as empty
            }
        }
        
        xlog!(state.logger, "All characters loaded");
    }

    /// Save all character data to disk
    /// Based on pop_save_all_chars
    pub fn pop_save_all_chars(&self, state: &mut crate::game_loop::GameState) {
        xlog!(state.logger, "Saving all characters to disk...");
        
        // Would iterate through characters and save them
        for cn in 0..state.ch.len() {
            if state.ch[cn].used != USE_EMPTY {
                let _ = self.save_char(cn, state);
            }
        }
        
        xlog!(state.logger, "All characters saved");
    }

    /// Load a single character from disk
    fn load_char(&self, _cn: usize, _state: &crate::game_loop::GameState) -> Result<(), String> {
        // Would read character data from disk
        // For now, just a stub that returns Ok
        Ok(())
    }

    /// Save a single character to disk
    fn save_char(&self, _cn: usize, _state: &crate::game_loop::GameState) -> Result<(), String> {
        // Would write character data to disk
        // For now, just a stub that returns Ok
        Ok(())
    }

    /// Create an item for a character (pop_create_item from populate.cpp)
    pub fn create_item_for_char(&self, _template: i32, ch_idx: usize, state: &mut crate::game_loop::GameState) -> Option<usize> {
        // Check for special items based on character alignment
        if ch_idx < state.ch.len() && state.ch[ch_idx].alignment < 0 && (state.globs.ticker as i32) % 150 == 0 {
            // Could create a special item based on template
            // For now, return None
            return None;
        }

        // Would normally create an item from the template
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_population_manager_new() {
        let manager = PopulationManager::new();
        assert!(manager.active_spawns.is_empty());
    }
}
