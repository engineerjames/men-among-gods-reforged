use crate::{driver_special, god::God, repository::Repository, state::State};
use core::constants::*;
use crate::helpers;

// Helper functions

pub fn get_frust_x_off(f: i32) -> i32 {
    match f % 5 {
        0 => 0,
        1 => 1,
        2 => -1,
        3 => 2,
        4 => -2,
        _ => 0,
    }
}

pub fn get_frust_y_off(f: i32) -> i32 {
    match (f / 5) % 5 {
        0 => 0,
        1 => 1,
        2 => -1,
        3 => 2,
        4 => -2,
        _ => 0,
    }
}

pub fn npc_dist(cn: usize, co: usize) -> i32 {
    Repository::with_characters(|characters| {
        let ch_cn = &characters[cn];
        let ch_co = &characters[co];
        std::cmp::max((ch_cn.x - ch_co.x).abs(), (ch_cn.y - ch_co.y).abs()) as i32
    })
}

pub struct Driver {}

impl Driver {
    pub fn msg(character_id: u32, notify_type: i32, dat1: i32, dat2: i32, dat3: i32, dat4: i32) {
        npc_msg(character_id as usize, notify_type, dat1, dat2, dat3, dat4);
    }
}

// ****************************************************
// NPC Message Handling and AI Functions
// ****************************************************

pub fn npc_add_enemy(cn: usize, co: usize, always: bool) -> bool {
    Repository::with_characters_mut(|characters| {
        
        // Don't attack anyone of the same group
        if characters[cn].data[42] == characters[co].data[42] {
            return false;
        }
        
        // Group 1 mobs shall not attack ghost companions
        if !always && characters[cn].data[42] == 1 && (characters[co].data[42] & 0x10000) != 0 {
            return false;
        }
        
        if !always && (characters[cn].points_tot + 500) * 25 < characters[co].points_tot {
            return false;
        }
        
        let ticker = Repository::with_globals(|globals| globals.ticker);
        characters[cn].data[76] = characters[co].x as i32 + characters[co].y as i32 * SERVER_MAPX;
        characters[cn].data[77] = ticker as i32;
        
        let cc = characters[cn].attack_cn;
        let d1 = if cc > 0 && usize::from(cc) < MAXCHARS { npc_dist(cn, cc as usize) } else { i32::MAX };
        let d2 = npc_dist(cn, co);
        
        let flags = Repository::with_globals(|globals| globals.flags);
        if characters[cn].attack_cn == 0 || 
           (d1 > d2 && (flags & 0x04) != 0) ||  // GF_CLOSEENEMY
           (d1 == d2 && (cc == 0 || characters[cc as usize].attack_cn != cn as u16) && 
            characters[co].attack_cn == cn as u16) {
            characters[cn].attack_cn = co as u16;
            characters[cn].goto_x = 0;
            characters[cn].data[58] = 2;
        }
        
        let idx = co as i32 | ((helpers::char_id(co) as i32) << 16);
        
        // Check if already in enemy list
        for n in 80..92 {
            if characters[cn].data[n] == idx {
                return false;
            }
        }
        
        // Shift enemy list and add new enemy
        for n in (81..92).rev() {
            characters[cn].data[n] = characters[cn].data[n - 1];
        }
        characters[cn].data[80] = idx;
        
        true
    })
}

pub fn npc_is_enemy(cn: usize, co: usize) -> bool {
    Repository::with_characters(|characters| {
        let idx = co as i32 | ((helpers::char_id(co) as i32) << 16);
        
        for n in 80..92 {
            if characters[cn].data[n] == idx {
                return true;
            }
        }
        false
    })
}

pub fn npc_list_enemies(npc: usize, cn: usize) -> i32 {
    State::with(|state| {
    Repository::with_characters(|characters| {
        let mut none = true;
        state.do_character_log(cn, core::types::FontColor::Green, &format!("Enemies of {}:", String::from_utf8_lossy(&characters[npc].name)));
        
        for n in 80..92 {
            let cv = (characters[npc].data[n] & 0xFFFF) as usize;
            if cv > 0 && cv < MAXCHARS {
                state.do_character_log(cn, core::types::FontColor::Green, &format!("  {}", String::from_utf8_lossy(&characters[cv].name)));
                none = false;
            }
        }
        
        if none {
            state.do_character_log(cn, core::types::FontColor::Green, "-none-");
            0
        } else {
            1
        }
    })
})
}

pub fn npc_remove_enemy(npc: usize, enemy: usize) -> bool {
    Repository::with_characters_mut(|characters| {
        let mut found = false;
        
        for n in 80..92 {
            if (characters[npc].data[n] & 0xFFFF) as usize == enemy {
                found = true;
            }
            if found {
                if n < 91 {
                    characters[npc].data[n] = characters[npc].data[n + 1];
                } else {
                    characters[npc].data[n] = 0;
                }
            }
        }
        
        found
    })
}

pub fn npc_saytext_n(npc: usize, n: usize, name: Option<&str>) {
    Repository::with_characters(|characters| {
        let ch_npc = &characters[npc];
        
        if (ch_npc.flags & CharacterFlags::CF_SHUTUP.bits()) != 0 {
            return;
        }
        
        if !ch_npc.text[n].is_empty() {
            let temp = ch_npc.temp;
            let talkative = ch_npc.data[71]; // CHD_TALKATIVE
            
            if temp == CT_COMPANION as u16 {
                if talkative == -10 {
                    let text = if let Some(name_str) = name {
                        String::from_utf8_lossy(&ch_npc.text[n]).replace("%1", name_str)
                    } else {
                        String::from_utf8_lossy(&ch_npc.text[n]).to_string()
                    };
                    State::with(|state| {
                        state.do_sayx(npc, &text);
                    });
                }
            } else {
                let text = if let Some(name_str) = name {
                    String::from_utf8_lossy(&ch_npc.text[n]).replace("%1", name_str)
                } else {
                    String::from_utf8_lossy(&ch_npc.text[n]).to_string()
                };

                State::with(|state| {
                    state.do_sayx(npc, &text);
                });
            }
        }
    });
}

pub fn npc_gotattack(cn: usize, co: usize, _dam: i32) -> i32 {
    Repository::with_characters_mut(|characters| {
        characters[cn].data[92] = (TICKS * 60) as i32;
        
        let ticker = Repository::with_globals(|globals| globals.ticker);
        
        // Special handling for high alignment NPCs being attacked by players
        if co > 0 && co < MAXCHARS && 
           (characters[co].flags & CharacterFlags::CF_PLAYER.bits()) != 0 &&
           characters[cn].alignment == 10000 &&
           (String::from_utf8_lossy(&characters[cn].name) != "Peacekeeper" || 
            characters[cn].a_hp < (characters[cn].hp[5] * 500) as i32) &&
           characters[cn].data[70] < ticker as i32 {
            
            State::with(|state| {
                state.do_sayx(cn, "Skua! Protect the innocent! Send me a Peacekeeper!");
            });
            // TODO: Add fx_add_effect(6, 0, characters[cn].x, characters[cn].y, 0);
            characters[cn].data[70] = (ticker + (TICKS * 60) ) as i32;
            
            let cc = God::create_char(80, true);
            if cc.is_some() && cc.unwrap() > 0 && cc.unwrap() < MAXCHARS as i32 {
                let cc = cc.unwrap() as usize;
                characters[cc].temp = CT_COMPANION as u16;
                characters[cc].data[42] = 65536 + cn as i32;
                characters[cc].data[59] = 65536 + cn as i32;
                characters[cc].data[24] = 0;
                characters[cc].data[36] = 0;
                characters[cc].data[43] = 0;
                characters[cc].data[80] = co as i32 | ((helpers::char_id(co) as i32) << 16);
                characters[cc].data[63] = cn as i32;
                characters[cc].data[64] = (ticker + 120 * TICKS) as i32;
                characters[cc].data[70] = (ticker + (TICKS * 60)) as i32;
                
                characters[cc].set_name("Shadow of Peace");
                characters[cc].set_reference("Shadow of Peace");
                characters[cc].set_description("You see a Shadow of Peace.");


                
                if !God::drop_char_fuzzy(cc as usize, characters[co].x as usize, characters[co].y as usize) {
                    God::destroy_items(cc as usize);
                    characters[cc].used = 0;
                }
            }
        }
        
        // Help request for good aligned characters
        if characters[cn].alignment > 1000 && 
           characters[cn].data[70] < ticker as i32 && 
           characters[cn].a_mana < (characters[cn].mana[5] * 333) as i32 {
            State::with(|state| {
                state.do_sayx(cn, "Skua! Help me!");
            });
            characters[cn].data[70] = (ticker + (TICKS * 60 * 2)) as i32;
            characters[cn].a_mana = (characters[cn].mana[5] * 1000) as i32;
            // TODO: fx_add_effect(6, 0, characters[cn].x, characters[cn].y, 0);
        }
        
        // Shout for help
        if characters[cn].data[52] != 0 && characters[cn].a_hp < (characters[cn].hp[5] * 666) as i32 {
            if characters[cn].data[55] + (TICKS * 60) < ticker as i32 {
                characters[cn].data[54] = 0;
                characters[cn].data[55] = ticker as i32;
                if co < MAXCHARS {
                    let co_name = String::from_utf8_lossy(&characters[co].name).to_string();
                    npc_saytext_n(cn, 4, Some(&co_name));
                }
                State::with(|state| {
                    state.do_npc_shout(cn, NT_SHOUT as i32, cn as i32, characters[cn].data[52] as i32, 
                           characters[cn].x as i32, characters[cn].y as i32);
                });
            }
        }
        
        // Can't see attacker - panic mode
        let character_can_see = State::with_mut(|state| {
            state.do_character_can_see(cn, co)
        });
        if co >= MAXCHARS || !character_can_see {
            characters[cn].data[78] = (ticker + (TICKS * 30)) as i32;
            return 1;
        }
        
        // Fight back
        if co < MAXCHARS {
            let co_name = characters[co].name.clone();
            if npc_add_enemy(cn, co, true) {
                let co_name = String::from_utf8_lossy(&co_name).to_string();
                npc_saytext_n(cn, 1, Some(&co_name));
                log::info!("NPC {} added {} to enemy list for attacking", cn, co);
            }
        }
        
        1
    })
}

pub fn npc_gothit(cn: usize, co: usize, dam: i32) -> i32 {
    npc_gotattack(cn, co, dam)
}

pub fn npc_gotmiss(cn: usize, co: usize) -> i32 {
    npc_gotattack(cn, co, 0)
}

pub fn npc_didhit(_cn: usize, _co: usize, _dam: i32) -> i32 {
    0
}

pub fn npc_didmiss(_cn: usize, _co: usize) -> i32 {
    0
}

pub fn npc_killed(cn: usize, cc: usize, co: usize) -> i32 {
    Repository::with_characters_mut(|characters| {
        if characters[cn].attack_cn == co as u16 {
            characters[cn].attack_cn = 0;
        }
        characters[cn].data[76] = 0;
        characters[cn].data[77] = 0;
        characters[cn].data[78] = 0;
        
        let idx = co as i32 | ((helpers::char_id(co) as i32) << 16);
        
        for n in 80..92 {
            if characters[cn].data[n] == idx {
                if cn == cc && co < MAXCHARS {
                    let co_name = characters[co].name.clone();
                    npc_saytext_n(cn, 0, Some(&String::from_utf8_lossy(&co_name)));
                    Repository::with_characters_mut(|chars| {
                        chars[cn].data[n] = 0;
                    });
                } else {
                    characters[cn].data[n] = 0;
                }
                return 1;
            }
        }
        
        0
    })
}

pub fn npc_didkill(cn: usize, co: usize) -> i32 {
    npc_killed(cn, cn, co)
}

pub fn npc_gotexp(_cn: usize, _amount: i32) -> i32 {
    0
}

pub fn npc_seekill(cn: usize, cc: usize, co: usize) -> i32 {
    npc_killed(cn, cc, co)
}

pub fn npc_seeattack(cn: usize, cc: usize, co: usize) -> i32 {
    Repository::with_characters_mut(|characters| {
        characters[cn].data[92] = (TICKS * 60) as i32;
        
        let cn_can_see_co = State::with_mut(|state| {
            state.do_character_can_see(cn, co)
        });

        let cn_can_see_cc = State::with_mut(|state| {
            state.do_character_can_see(cn, cc)
        });

        if !cn_can_see_co || !cn_can_see_cc {
            return 1; // Processed - can't see participants
        }
        
        // Prevent fight mode logic
        if characters[cn].data[24] != 0 {
            let diff = (characters[cc].alignment - 50) - characters[co].alignment;
            let (ret, c2, c3) = if diff <= 0 {
                if characters[cn].data[24] > 0 {
                    (npc_add_enemy(cn, cc, true), cc, co)
                } else {
                    (npc_add_enemy(cn, co, true), co, cc)
                }
            } else {
                if characters[cn].data[24] > 0 {
                    (npc_add_enemy(cn, co, true), co, cc)
                } else {
                    (npc_add_enemy(cn, cc, true), cc, co)
                }
            };
            
            if ret {
                let c2_name = Repository::with_characters(|chars| chars[c2].name.clone());
                let c3_name = Repository::with_characters(|chars| chars[c3].name.clone());
                npc_saytext_n(cn, 1, Some(&String::from_utf8_lossy(&c2_name)));
                log::info!("NPC {} added {} to enemy list for attacking {}", cn, String::from_utf8_lossy(&c2_name), String::from_utf8_lossy(&c3_name));
            }
            return 1;
        }
        
        // Protect character by template
        if characters[cn].data[31] != 0 {
            if characters[co].temp == characters[cn].data[31] as u16 {
                if npc_add_enemy(cn, cc, true) {
                    let cc_name = Repository::with_characters(|chars| chars[cc].name.clone());
                    let co_name = Repository::with_characters(|chars| chars[co].name.clone());
                    npc_saytext_n(cn, 1, Some(&String::from_utf8_lossy(&cc_name)));
                    log::info!("NPC {} added {} to enemy list for attacking {} (protect char)", cn, String::from_utf8_lossy(&cc_name), String::from_utf8_lossy(&co_name));
                }
                Repository::with_characters_mut(|chars| {
                    if chars[cn].data[65] == 0 {
                        chars[cn].data[65] = co as i32;
                    }
                });
            }
        }
        
        // Additional protect logic continues...
        // (Truncated for brevity - similar pattern for other protect cases)
        // TODO: Fill out the rest of thus function...
        
        0
    })
}

pub fn npc_seehit(cn: usize, cc: usize, co: usize) -> i32 {
    if npc_seeattack(cn, cc, co) != 0 {
        return 1;
    }
    if npc_see(cn, cc) != 0 {
        return 1;
    }
    if npc_see(cn, co) != 0 {
        return 1;
    }
    0
}

pub fn npc_seemiss(cn: usize, cc: usize, co: usize) -> i32 {
    if npc_seeattack(cn, cc, co) != 0 {
        return 1;
    }
    if npc_see(cn, cc) != 0 {
        return 1;
    }
    if npc_see(cn, co) != 0 {
        return 1;
    }
    0
}

pub fn npc_give(_cn: usize, _co: usize, _in: usize, _money: i32) -> i32 {
    // TODO: Large function - implement item giving logic
    0
}

pub fn npc_died(cn: usize, co: usize) -> i32 {
    // TODO: Re-evaluate this function
    Repository::with_characters(|characters| {
        if characters[cn].data[48] != 0 && co > 0 {
            // TODO: Add RANDOM function
            if characters[cn].data[48] > 50 { // Simplified random check
                let co_name = if co < MAXCHARS {
                    String::from_utf8_lossy(&characters[co].name).to_string()
                } else {
                    String::new()
                };
                npc_saytext_n(cn, 3, if co_name.is_empty() { None } else { Some(&co_name) });
            }
            return 1;
        }
        0
    })
}

pub fn npc_shout(cn: usize, co: usize, code: i32, x: i32, y: i32) -> i32 {
    Repository::with_characters_mut(|characters| {
        if characters[cn].data[53] != 0 && characters[cn].data[53] == code {
            characters[cn].data[92] = (TICKS * 60) as i32;
            characters[cn].data[54] = x + y * SERVER_MAPX;
            characters[cn].data[55] = Repository::with_globals(|globals| globals.ticker) as i32;
            
            let co_name = if co < MAXCHARS {
                String::from_utf8_lossy(&characters[co].name).to_string()
            } else {
                String::new()
            };
            drop(characters);
            
            npc_saytext_n(cn, 5, if co_name.is_empty() { None } else { Some(&co_name) });
            
            // Cancel current actions
            Repository::with_characters_mut(|chars| {
                chars[cn].goto_x = 0;
                chars[cn].misc_action = 0;
            });
            
            return 1;
        }
        0
    })
}

pub fn npc_hitme(cn: usize, co: usize) -> i32 {
    let cn_can_see_co = State::with_mut(|state| {
        state.do_character_can_see(cn, co)
    });

    if !cn_can_see_co {
        return 1;
    }
    
    Repository::with_characters(|characters| {
        let data_26 = characters[cn].data[26];
        data_26
    });
    
    // TODO: Implement trap logic
    0
}

pub fn npc_msg(cn: usize, msg_type: i32, dat1: i32, dat2: i32, dat3: i32, dat4: i32) -> i32 {
    // Check for special driver
    let special_driver = Repository::with_characters(|chars| chars[cn].data[25]);
    
    if special_driver != 0 {
        return match special_driver {
            1 => driver_special::npc_stunrun_msg(cn, msg_type, dat1, dat2, dat3, dat4),
            2 => driver_special::npc_cityattack_msg(cn, msg_type, dat1, dat2, dat3, dat4),
            3 => driver_special::npc_malte_msg(cn, msg_type, dat1, dat2, dat3, dat4),
            _ => {
                log::error!("Unknown special driver {} for {}", special_driver, cn);
                0
            }
        };
    }
    
    match msg_type {
        x if x == NT_GOTHIT as i32 => npc_gothit(cn, dat1 as usize, dat2),
        x if x == NT_GOTMISS as i32 => npc_gotmiss(cn, dat1 as usize),
        x if x == NT_DIDHIT as i32 => npc_didhit(cn, dat1 as usize, dat2),
        x if x == NT_DIDMISS as i32 => npc_didmiss(cn, dat1 as usize),
        x if x == NT_DIDKILL as i32 => npc_didkill(cn, dat1 as usize),
        x if x == NT_GOTEXP as i32 => npc_gotexp(cn, dat1),
        x if x == NT_SEEKILL as i32 => npc_seekill(cn, dat1 as usize, dat2 as usize),
        x if x == NT_SEEHIT as i32 => npc_seehit(cn, dat1 as usize, dat2 as usize),
        x if x == NT_SEEMISS as i32 => npc_seemiss(cn, dat1 as usize, dat2 as usize),
        x if x == NT_GIVE as i32 => npc_give(cn, dat1 as usize, dat2 as usize, dat3),
        x if x == NT_SEE as i32 => npc_see(cn, dat1 as usize),
        x if x == NT_DIED as i32 => npc_died(cn, dat1 as usize),
        x if x == NT_SHOUT as i32 => npc_shout(cn, dat1 as usize, dat2, dat3, dat4),
        x if x == NT_HITME as i32 => npc_hitme(cn, dat1 as usize),
        _ => {
            log::error!("Unknown NPC message for {}: {}", cn, msg_type);
            0
        }
    }
}

// ****************************************************
// Spell and Combat Functions
// ****************************************************

pub fn get_spellcost(cn: usize, spell: usize) -> i32 {
    Repository::with_characters(|characters| {
        match spell {
            SK_BLAST => characters[cn].skill[SK_BLAST][5] / 5,
            SK_IDENT => 50,
            SK_CURSE => 35,
            SK_BLESS => 35,
            SK_ENHANCE => 15,
            SK_PROTECT => 15,
            SK_LIGHT => 5,
            SK_STUN => 20,
            SK_HEAL => 25,
            SK_GHOST => 45,
            SK_MSHIELD => 25,
            SK_RECALL => 15,
            _ => 255, // Originally was 9999 which is invalid for a u8
        }
    }) as i32
}

pub fn spellflag(spell: usize) -> u32 {
    match spell {
        SK_LIGHT => SP_LIGHT,
        SK_PROTECT => SP_PROTECT,
        SK_ENHANCE => SP_ENHANCE,
        SK_BLESS => SP_BLESS,
        SK_HEAL => SP_HEAL,
        SK_CURSE => SP_CURSE,
        SK_STUN => SP_STUN,
        SK_DISPEL => SP_DISPEL,
        _ => 0,
    }
}

pub fn npc_check_target( x: usize, y: usize) -> bool {
    if x < 1 || x >= SERVER_MAPX as usize || y < 1 || y >= SERVER_MAPY as usize {
        return false;
    }

    let m = x + y * SERVER_MAPX as usize;

    Repository::with_map(|map| {
        let map_item = Repository::with_items(|items| {
            if map[m].it == 0 {
                return None;
            }

            Some(items[map[m].it as usize])
        } );

        if map_item.is_none() {
            return false;
        }

        let map_item = map_item.unwrap();
        if map[m].flags & (core::constants::MF_MOVEBLOCK as u64 | core::constants::MF_NOMONST as u64) != 0 || map[m].ch != 0 || map[m].to_ch != 0 || (map_item.flags & ItemFlags::IF_MOVEBLOCK.bits() != 0 && map_item.driver != 2) {
            return false;
        }

        true
    })
}

pub fn npc_is_stunned(cn: usize) -> bool {
    for n in 0..20 {
        let active_spell = Repository::with_characters(|characters| characters[cn].spell[n]);
        if active_spell != 0 && Repository::with_items(|items| items[active_spell as usize].temp) == SK_STUN as u16 {
            return true;
        }
    }

    return false;
}

// TODO: Combine with npc_is_stunned?
pub fn npc_is_blessed(cn: usize) -> bool {
    for n in 0..20 {
        let active_spell = Repository::with_characters(|characters| characters[cn].spell[n]);
        if active_spell != 0 && Repository::with_items(|items| items[active_spell as usize].temp) == SK_BLESS as u16 {
            return true;
        }
    }

    return false;
}

pub fn npc_try_spell(cn: usize, co: usize, spell: usize) -> bool {
    // TODO: Implement full spell casting logic
    // This is a complex function that needs item and spell system integration
    false
}

pub fn npc_can_spell(cn: usize, co: usize, spell: usize) -> bool {
    Repository::with_characters(|characters| {
        if characters[cn].a_mana / 1000 < get_spellcost(cn, spell) {
            return false;
        }
        if characters[cn].skill[spell][0] == 0 {
            return false;
        }
        if characters[co].skill[spell][5] > characters[cn].skill[spell][5] {
            return false;
        }
        true
    })
}

pub fn npc_quaff_potion(cn: usize, itemp: i32, stemp: i32) -> i32 {
    // TODO: Implement potion quaffing logic
    0
}

pub fn die_companion(cn: usize) {
    Repository::with_characters(|characters| {
        // TODO: fx_add_effect(7, 0, characters[cn].x, characters[cn].y, 0);
    });
    God::destroy_items(cn);
    Repository::with_characters_mut(|characters| {
        characters[cn].gold = 0;
    });

    State::with(|state| {
        state.do_character_killed(0, cn);
    });
}

// ****************************************************
// High Priority NPC Driver
// ****************************************************

pub fn npc_driver_high(cn: usize) -> i32 {
    // Check for special driver
    let special_driver = Repository::with_characters(|chars| chars[cn].data[25]);
    
    if special_driver != 0 {
        return match special_driver {
            1 => driver_special::npc_stunrun_high(cn),
            2 => driver_special::npc_cityattack_high(cn),
            3 => driver_special::npc_malte_high(cn),
            _ => {
                log::error!("Unknown special driver {} for {}", special_driver, cn);
                0
            }
        };
    }
    
    // TODO: Implement full high priority driver logic
    // This is a very large function with many subsystems
    0
}

// ****************************************************
// Low Priority NPC Driver
// ****************************************************

pub fn npc_driver_low(cn: usize) {
    // Check for special driver
    let special_driver = Repository::with_characters(|chars| chars[cn].data[25]);
    
    if special_driver != 0 {
        match special_driver {
            1 => driver_special::npc_stunrun_low(cn),
            2 => driver_special::npc_cityattack_low(cn),
            3 => driver_special::npc_malte_low(cn),
            _ => {log::error!("Unknown special driver {} for {}", special_driver, cn) ;
        -1
        }
            
        };
        return;
    }
    
    // TODO: Implement full low priority driver logic
    // This is a very large function with patrol, random walk, etc.
}

// ****************************************************
// Grave Looting and Equipment Functions
// ****************************************************

pub fn npc_check_placement(in_idx: usize, n: usize) -> bool {
    Repository::with_items(|items| {
        let placement = items[in_idx].placement;
        
        match n {
            WN_HEAD => (placement & PL_HEAD) != 0,
            WN_NECK => (placement & PL_NECK) != 0,
            WN_BODY => (placement & PL_BODY) != 0,
            WN_ARMS => (placement & PL_ARMS) != 0,
            WN_BELT => (placement & PL_BELT) != 0,
            WN_LEGS => (placement & PL_LEGS) != 0,
            WN_FEET => (placement & PL_FEET) != 0,
            WN_LHAND => (placement & PL_SHIELD) != 0,
            WN_RHAND => (placement & PL_WEAPON) != 0,
            WN_CLOAK => (placement & PL_CLOAK) != 0,
            WN_LRING | WN_RRING => (placement & PL_RING) != 0,
            _ => false,
        }
    })
}

pub fn npc_can_wear_item(cn: usize, in_idx: usize) -> bool {
    if (in_idx & 0x80000000) != 0 {
        return false;
    }
    
    Repository::with_characters(|characters| {
        Repository::with_items(|items| {
            let ch = &characters[cn];
            let it = &items[in_idx];
            
            // Check attribute requirements
            for m in 0..5 {
                if it.attrib[m][2] > ch.attrib[m][0] as i8 {
                    return false;
                }
            }
            
            // Check skill requirements
            for m in 0..50 {
                if it.skill[m][2] > ch.skill[m][0] as i8 {
                    return false;
                }
            }
            
            // Check other requirements
            if it.hp[2] > ch.hp[0] as i16{
                return false;
            }
            
            if it.end[2] > ch.end[0] as i16 {
                return false;
            }
            
            if it.mana[2] > ch.mana[0] as i16 {
                return false;
            }
            
            true
        })
    })
}

pub fn npc_item_value(in_idx: usize) -> i32 {
    Repository::with_items(|items| {
        let it = &items[in_idx];
        let mut score = 0;
        
        for n in 0..50 {
            score += it.attrib[n][0] * 5;
        }
        
        score += (it.value / 10) as i8;
        score += it.weapon[0] * 50;
        score += it.armor[0] * 50;
        score -= it.damage_state as i8;
        
        score
    }) as i32
}

pub fn npc_want_item(cn: usize, in_idx: usize) -> bool {
    Repository::with_characters(|characters| {
        if characters[cn].item[38] != 0 {
            return false; // Inventory almost full
        }
        
        if characters[cn].citem != 0 {
            Repository::with_items(|items| {
                log::info!("have {} in citem",String::from_utf8_lossy(&items[in_idx].name));
                
            });

            let do_store_item = State::with(|state| {
                state.do_store_item(cn)
            });
            if do_store_item == -1 {
                Repository::with_items_mut(|items| {
                    items[characters[cn].citem as usize].used = 0;
                });
                Repository::with_characters_mut(|chars| {
                    chars[cn].citem = 0;
                });
            }
        }
        
        Repository::with_items(|items| {
            let temp = items[in_idx].temp;
            temp == 833 || temp == 267
        })
    });
    
    // TODO: Complete implementation
    false
}

pub fn npc_equip_item(cn: usize, in_idx: usize) -> bool {
    // TODO: Implement equipment logic
    false
}

pub fn npc_loot_grave(cn: usize, in_idx: usize) -> bool {
    // TODO: Implement grave looting logic
    false
}

pub fn npc_already_searched_grave(cn: usize, in_idx: usize) -> bool {
    // TODO: Implement grave search tracking
    false
}

pub fn npc_add_searched_grave(cn: usize, in_idx: usize) {
    // TODO: Implement grave search tracking
}

pub fn npc_grave_logic(cn: usize) -> bool {
    // TODO: Implement grave scanning logic
    false
}

// ****************************************************
// Shop Functions
// ****************************************************

pub fn update_shop(cn: usize) {
    // TODO: Implement shop update logic
    // This manages shop inventory, repairs items, etc.
}

// ****************************************************
// Special Functions
// ****************************************************

pub fn shiva_activate_candle(cn: usize, in_idx: usize) -> i32 {
    // TODO: Implement special Shiva candle activation
    0
}

pub fn npc_see(cn: usize, co: usize) -> i32 {
    // TODO: Implement full NPC see logic
    // This is a very large function with many special cases
    0
}
