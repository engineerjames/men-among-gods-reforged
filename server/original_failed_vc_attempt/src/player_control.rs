/*************************************************************************

This file is part of 'Mercenaries of Astonia v2'
Copyright (c) 1997-2001 Daniel Brockhaus (joker@astonia.com)
All rights reserved.

Rust port maintains original logic and comments.

**************************************************************************/

//! Player control and logout module - handles player disconnections and state management
//! Based on plr_logout and related functions from svr_tick.cpp

use crate::constants::*;
use crate::xlog;

/// Player control manager
pub struct PlayerControlManager;

impl PlayerControlManager {
    /// Handle player logout
    /// Based on plr_logout from svr_tick.cpp
    /// 
    /// cn: character index being logged out
    /// nr: player number (connection number)
    /// reason: logout reason (LO_EXIT, LO_IDLE, LO_SHUTDOWN, etc.)
    pub fn plr_logout(cn: usize, nr: usize, reason: u8, state: &mut crate::game_loop::GameState) {
        // Validate parameters
        if nr >= MAXPLAYER {
            return;
        }

        // Check for USURP flag (god mode takeover)
        if cn > 0 && cn < state.ch.len() && 
           (state.ch[cn].player as usize == nr || nr == 0) && 
           (state.ch[cn].flags & CharacterFlags::CF_USURP.bits()) != 0 {
            
            // Clear usurp and related flags
            state.ch[cn].flags &= !(CharacterFlags::CF_CCP.bits() | CharacterFlags::CF_USURP.bits() | CharacterFlags::CF_STAFF.bits() | CharacterFlags::CF_IMMORTAL.bits() | CharacterFlags::CF_GOD.bits() | CharacterFlags::CF_CREATOR.bits());
            
            // Get the controlled character and logout the controller
            let co = state.ch[cn].data[97] as usize;
            if co < state.ch.len() {
                Self::plr_logout(co, 0, LO_SHUTDOWN as u8, state);
            }
        }

        // Handle player character logout
        if cn > 0 && cn < state.ch.len() && 
           (state.ch[cn].player as usize == nr || nr == 0) && 
           (state.ch[cn].flags & CharacterFlags::CF_PLAYER.bits()) != 0 &&
           (state.ch[cn].flags & CharacterFlags::CF_CCP.bits()) == 0 {

            // Handle exit punishment
            if reason == LO_EXIT {
                xlog!(state.logger, "Punishing {} for F12 exit", cn);
                
                // Apply demon punishment
                let hp_loss = state.ch[cn].hp[5] as i32 * 8 / 10;
                state.ch[cn].a_hp -= hp_loss * 100;  // hp is in hundredths
                
                if state.ch[cn].a_hp < 500 {
                    // Character killed by demon
                    xlog!(state.logger, "Character {} killed by demon", cn);
                    // Would call do_char_killed
                } else {
                    // Lose some gold too
                    let gold_loss = state.ch[cn].gold / 10;
                    state.ch[cn].gold -= gold_loss;
                }
            }

            // Remove character from map
            Self::remove_from_map(cn, state);

            // Give lag scroll if player was away from temple
            if reason == LO_IDLE || reason == LO_SHUTDOWN || reason == 0 {
                Self::give_lag_scroll(cn, state);
            }

            // Clear position
            state.ch[cn].x = 0;
            state.ch[cn].y = 0;
            state.ch[cn].tox = 0;
            state.ch[cn].toy = 0;
            state.ch[cn].frx = 0;
            state.ch[cn].fry = 0;
        }

        // Disconnect the player socket if valid
        if nr > 0 && nr < MAXPLAYER {
            if let Some(ref mut player) = state.players.players.get_mut(nr) {
                player.disconnect();
            }
        }
    }

    /// Remove character from map structures
    fn remove_from_map(cn: usize, state: &mut crate::game_loop::GameState) {
        if cn >= state.ch.len() {
            return;
        }

        let map_x = state.ch[cn].x as usize;
        let map_y = state.ch[cn].y as usize;
        
        // Check bounds
        if map_x < SERVER_MAPX as usize && map_y < SERVER_MAPY as usize {
            let map_idx = map_x + map_y * (SERVER_MAPX as usize);
            
            if map_idx < state.map.len() {
                // Remove from main map position
                if state.map[map_idx].ch as usize == cn {
                    state.map[map_idx].ch = 0;
                    
                    // Remove light contribution if any
                    if state.ch[cn].light != 0 {
                        // Would call do_add_light(x, y, -light)
                    }
                }
            }
        }

        // Remove from 'to' position if different
        let to_map_x = state.ch[cn].tox as usize;
        let to_map_y = state.ch[cn].toy as usize;
        
        if to_map_x < SERVER_MAPX as usize && to_map_y < SERVER_MAPY as usize {
            let to_map_idx = to_map_x + to_map_y * (SERVER_MAPX as usize);
            
            if to_map_idx < state.map.len() {
                if state.map[to_map_idx].to_ch as usize == cn {
                    state.map[to_map_idx].to_ch = 0;
                }
            }
        }

        // Would also call remove_enemy(cn) to clear enemy lists
    }

    /// Give lag scroll to player at logout location
    fn give_lag_scroll(cn: usize, state: &mut crate::game_loop::GameState) {
        if cn >= state.ch.len() {
            return;
        }

        let char_x = state.ch[cn].x;
        let char_y = state.ch[cn].y;
        let temple_x = state.ch[cn].temple_x;
        let temple_y = state.ch[cn].temple_y;

        // Check if player is far from temple and not in NOLAG zone
        let distance = ((char_x as i32 - temple_x as i32).abs() + 
                       (char_y as i32 - temple_y as i32).abs()) as u16;
        
        if distance > 10 {
            let map_idx = (char_x as usize) + (char_y as usize) * (SERVER_MAPX as usize);
            
            if map_idx < state.map.len() && (state.map[map_idx].flags & (MF_NOLAG as u64)) == 0 {
                xlog!(state.logger, "Giving lag scroll to character {} at ({}, {})", cn, char_x, char_y);
                
                // Would create lag scroll item:
                // let in = god_create_item(IT_LAGSCROLL);
                // it[in].data[0] = char_x;
                // it[in].data[1] = char_y;
                // it[in].data[2] = globs.ticker;
                // god_give_char(in, cn);
            }
        }
    }

    /// Mark a character as changed
    pub fn mark_char_changed(_cn: usize, _state: &mut crate::game_loop::GameState) {
        // Characters don't have a changed field in current implementation
        // This would be used to track which characters need syncing
    }

    /// Check if character is valid and alive
    pub fn check_char_valid(cn: usize, state: &crate::game_loop::GameState) -> bool {
        if cn >= state.ch.len() {
            return false;
        }

        // Would check:
        // - Character is used
        // - Character is in a valid state
        // - Character has valid position
        // - etc.
        
        state.ch[cn].used != USE_EMPTY
    }

    /// Check if a location is in a lab area
    pub fn is_lab_area(x: u16, _y: u16) -> bool {
        // Known lab temple positions
        x == 512 || x == 558 || x == 813
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_lab_area() {
        assert!(PlayerControlManager::is_lab_area(512, 0));
        assert!(PlayerControlManager::is_lab_area(558, 0));
        assert!(PlayerControlManager::is_lab_area(813, 0));
        assert!(!PlayerControlManager::is_lab_area(100, 100));
    }
}
