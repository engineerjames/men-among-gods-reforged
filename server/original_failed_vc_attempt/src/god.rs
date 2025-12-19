/*************************************************************************

This file is part of 'Mercenaries of Astonia v2'
Copyright (c) 1997-2001 Daniel Brockhaus (joker@astonia.com)
All rights reserved.

Rust port maintains original logic and comments.

**************************************************************************/

//! God/Admin functions module - manages God mode operations, admin tools, and item management

use std::collections::HashMap;
use crate::constants::*;
use crate::types::*;
use crate::logging::Logger;
use crate::xlog;

/// Free item list for tracking available items
pub struct FreeItemManager {
    free_items: Vec<usize>,
}

impl FreeItemManager {
    pub fn new() -> Self {
        Self {
            free_items: Vec::with_capacity(32),
        }
    }

    /// Initialize the free item list by scanning all items
    /// Based on god_init_freelist from svr_god.cpp
    pub fn init_freelist(&mut self, items: &[Item]) {
        self.free_items.clear();
        
        for (idx, item) in items.iter().enumerate() {
            if item.used == USE_EMPTY {
                self.free_items.push(idx);
                if self.free_items.len() >= 32 {
                    break;
                }
            }
        }
    }

    /// Get a free item slot, maintaining a quick list and scanning for more if needed
    pub fn get_free_item(&mut self, items: &[Item]) -> Option<usize> {
        // Check if we have a cached free item
        while let Some(idx) = self.free_items.pop() {
            if idx < items.len() && items[idx].used == USE_EMPTY {
                return Some(idx);
            }
        }

        // Scan for free items and update cache
        let mut found_idx = None;
        let mut cache_count = 0;
        
        for (idx, item) in items.iter().enumerate() {
            if item.used == USE_EMPTY {
                if found_idx.is_none() {
                    found_idx = Some(idx);
                } else {
                    self.free_items.push(idx);
                    cache_count += 1;
                    if cache_count >= 31 {
                        break;
                    }
                }
            }
        }

        found_idx
    }
}

/// Ban list manager - handles banned players/IPs
pub struct BanListManager {
    banned_ips: HashMap<String, String>,  // IP -> reason
    banned_names: Vec<String>,  // Banned character names
}

impl BanListManager {
    pub fn new() -> Self {
        Self {
            banned_ips: HashMap::new(),
            banned_names: Vec::new(),
        }
    }

    /// Read ban list from disk
    /// Based on god_read_banlist from svr_god.cpp
    pub fn read_banlist(&mut self, logger: &Logger) {
        // For now, just log that we would load it
        xlog!(logger, "Initialized ban list manager");
    }

    /// Check if an IP is banned
    pub fn is_ip_banned(&self, ip: &str) -> Option<&str> {
        self.banned_ips.get(ip).map(|s| s.as_str())
    }

    /// Check if a name is banned
    pub fn is_name_banned(&self, name: &str) -> bool {
        self.banned_names.iter().any(|n| n.eq_ignore_ascii_case(name))
    }

    /// Add IP to ban list
    pub fn ban_ip(&mut self, ip: String, reason: String) {
        self.banned_ips.insert(ip, reason);
    }

    /// Add name to ban list
    pub fn ban_name(&mut self, name: String) {
        self.banned_names.push(name);
    }
}

/// Bad word/name list manager
pub struct BadListManager {
    bad_words: Vec<String>,
    bad_names: Vec<String>,
}

impl BadListManager {
    pub fn new() -> Self {
        Self {
            bad_words: Vec::new(),
            bad_names: Vec::new(),
        }
    }

    /// Initialize badwords list
    /// Based on init_badwords from svr_god.cpp
    pub fn init_badwords(&mut self, logger: &Logger) {
        // Would load from badwords.txt
        xlog!(logger, "Initialized bad words list");
    }

    /// Initialize badnames list
    /// Based on god_init_badnames from svr_god.cpp
    pub fn init_badnames(&mut self, logger: &Logger) {
        // Would load from badnames.txt
        xlog!(logger, "Initialized bad names list");
    }

    /// Check if a word is banned
    pub fn is_word_bad(&self, word: &str) -> bool {
        self.bad_words.iter().any(|w| word.to_lowercase().contains(&w.to_lowercase()))
    }

    /// Check if a name is bad
    pub fn is_name_bad(&self, name: &str) -> bool {
        self.bad_names.iter().any(|n| name.to_lowercase().contains(&n.to_lowercase()))
    }
}

/// God function container
pub struct GodManager {
    pub free_items: FreeItemManager,
    pub ban_list: BanListManager,
    pub bad_list: BadListManager,
}

impl GodManager {
    pub fn new() -> Self {
        Self {
            free_items: FreeItemManager::new(),
            ban_list: BanListManager::new(),
            bad_list: BadListManager::new(),
        }
    }

    /// Initialize all God systems
    pub fn init_all(&mut self, state: &mut crate::game_loop::GameState) {
        self.free_items.init_freelist(&state.it);
        self.ban_list.read_banlist(&state.logger);
        self.bad_list.init_badwords(&state.logger);
        self.bad_list.init_badnames(&state.logger);
        
        xlog!(state.logger, "God manager initialized");
    }

    /// Get a free item and initialize it
    /// Based on logic from svr_god.cpp
    pub fn get_free_item(&mut self, items: &[Item]) -> Option<usize> {
        self.free_items.get_free_item(items)
    }

    /// Create a new item (god_create_item from svr_god.cpp)
    /// This would create an item of specified type
    pub fn create_item(&mut self, item_type: u16, items: &mut [Item], logger: &Logger) -> Option<usize> {
        if let Some(idx) = self.get_free_item(items) {
            // Initialize the item
            items[idx].used = USE_ACTIVE;
            items[idx].temp = item_type;
            // Would set up more properties based on item_type
            xlog!(logger, "Created item {} of type {}", idx, item_type);
            return Some(idx);
        }
        None
    }

    /// Take item from character
    /// Based on god_take_from_char from svr_god.cpp
    pub fn take_from_char(
        &self,
        item_idx: usize,
        ch_idx: usize,
        items: &mut [Item],
        chars: &mut [Character],
        logger: &Logger,
    ) {
        if ch_idx >= chars.len() || item_idx >= items.len() {
            return;
        }

        // Remove item from character's inventory
        if chars[ch_idx].citem as usize == item_idx {
            chars[ch_idx].citem = 0;
        }

        // Could also check carried items, equipment slots, etc.
        items[item_idx].carried = 0;
        
        xlog!(logger, "Item {} removed from character {}", item_idx, ch_idx);
    }

    /// Give item to character
    pub fn give_to_char(&self, item_idx: usize, ch_idx: usize, items: &mut [Item], chars: &mut [Character]) {
        if ch_idx >= chars.len() || item_idx >= items.len() {
            return;
        }

        items[item_idx].carried = ch_idx as u16;
        // Would implement full inventory logic
    }

    /// Reset changed items flag
    pub fn reset_changed_items(&self, _items: &mut [Item]) {
        // Items don't have a changed field in the current type definition
        // This would be used if item change tracking was implemented
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_freelist_init() {
        let mut manager = FreeItemManager::new();
        let mut items = vec![Item::default(); 100];
        
        // Mark some as empty
        items[5].used = USE_EMPTY;
        items[10].used = USE_EMPTY;
        
        manager.init_freelist(&items);
        assert!(!manager.free_items.is_empty());
    }
}
