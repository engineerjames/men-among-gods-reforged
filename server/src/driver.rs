use crate::helpers;
use crate::{driver_special, god::God, repository::Repository, state::State};
use core::{constants::*, types::Character};

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
        let d1 = if cc > 0 && usize::from(cc) < MAXCHARS {
            npc_dist(cn, cc as usize)
        } else {
            i32::MAX
        };
        let d2 = npc_dist(cn, co);

        let flags = Repository::with_globals(|globals| globals.flags);
        if characters[cn].attack_cn == 0
            || (d1 > d2 && (flags & 0x04) != 0)
            || (d1 == d2
                && (cc == 0 || characters[cc as usize].attack_cn != cn as u16)
                && characters[co].attack_cn == cn as u16)
        {
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
            state.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!(
                    "Enemies of {}:",
                    String::from_utf8_lossy(&characters[npc].name)
                ),
            );

            for n in 80..92 {
                let cv = (characters[npc].data[n] & 0xFFFF) as usize;
                if cv > 0 && cv < MAXCHARS {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Green,
                        &format!("  {}", String::from_utf8_lossy(&characters[cv].name)),
                    );
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
        if co > 0
            && co < MAXCHARS
            && (characters[co].flags & CharacterFlags::CF_PLAYER.bits()) != 0
            && characters[cn].alignment == 10000
            && (String::from_utf8_lossy(&characters[cn].name) != "Peacekeeper"
                || characters[cn].a_hp < (characters[cn].hp[5] * 500) as i32)
            && characters[cn].data[70] < ticker as i32
        {
            State::with(|state| {
                state.do_sayx(cn, "Skua! Protect the innocent! Send me a Peacekeeper!");
            });
            // TODO: Add fx_add_effect(6, 0, characters[cn].x, characters[cn].y, 0);
            characters[cn].data[70] = (ticker + (TICKS * 60)) as i32;

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

                if !God::drop_char_fuzzy(
                    cc as usize,
                    characters[co].x as usize,
                    characters[co].y as usize,
                ) {
                    God::destroy_items(cc as usize);
                    characters[cc].used = 0;
                }
            }
        }

        // Help request for good aligned characters
        if characters[cn].alignment > 1000
            && characters[cn].data[70] < ticker as i32
            && characters[cn].a_mana < (characters[cn].mana[5] * 333) as i32
        {
            State::with(|state| {
                state.do_sayx(cn, "Skua! Help me!");
            });
            characters[cn].data[70] = (ticker + (TICKS * 60 * 2)) as i32;
            characters[cn].a_mana = (characters[cn].mana[5] * 1000) as i32;
            // TODO: fx_add_effect(6, 0, characters[cn].x, characters[cn].y, 0);
        }

        // Shout for help
        if characters[cn].data[52] != 0 && characters[cn].a_hp < (characters[cn].hp[5] * 666) as i32
        {
            if characters[cn].data[55] + (TICKS * 60) < ticker as i32 {
                characters[cn].data[54] = 0;
                characters[cn].data[55] = ticker as i32;
                if co < MAXCHARS {
                    let co_name = String::from_utf8_lossy(&characters[co].name).to_string();
                    npc_saytext_n(cn, 4, Some(&co_name));
                }
                State::with(|state| {
                    state.do_npc_shout(
                        cn,
                        NT_SHOUT as i32,
                        cn as i32,
                        characters[cn].data[52] as i32,
                        characters[cn].x as i32,
                        characters[cn].y as i32,
                    );
                });
            }
        }

        // Can't see attacker - panic mode
        let character_can_see = State::with_mut(|state| state.do_character_can_see(cn, co));
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

        let cn_can_see_co = State::with_mut(|state| state.do_character_can_see(cn, co));

        let cn_can_see_cc = State::with_mut(|state| state.do_character_can_see(cn, cc));

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
                log::info!(
                    "NPC {} added {} to enemy list for attacking {}",
                    cn,
                    String::from_utf8_lossy(&c2_name),
                    String::from_utf8_lossy(&c3_name)
                );
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
                    log::info!(
                        "NPC {} added {} to enemy list for attacking {} (protect char)",
                        cn,
                        String::from_utf8_lossy(&cc_name),
                        String::from_utf8_lossy(&co_name)
                    );
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
            if characters[cn].data[48] > 50 {
                // Simplified random check
                let co_name = if co < MAXCHARS {
                    String::from_utf8_lossy(&characters[co].name).to_string()
                } else {
                    String::new()
                };
                npc_saytext_n(
                    cn,
                    3,
                    if co_name.is_empty() {
                        None
                    } else {
                        Some(&co_name)
                    },
                );
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

            npc_saytext_n(
                cn,
                5,
                if co_name.is_empty() {
                    None
                } else {
                    Some(&co_name)
                },
            );

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
    let cn_can_see_co = State::with_mut(|state| state.do_character_can_see(cn, co));

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

pub fn npc_check_target(x: usize, y: usize) -> bool {
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
        });

        if map_item.is_none() {
            return false;
        }

        let map_item = map_item.unwrap();
        if map[m].flags
            & (core::constants::MF_MOVEBLOCK as u64 | core::constants::MF_NOMONST as u64)
            != 0
            || map[m].ch != 0
            || map[m].to_ch != 0
            || (map_item.flags & ItemFlags::IF_MOVEBLOCK.bits() != 0 && map_item.driver != 2)
        {
            return false;
        }

        true
    })
}

pub fn npc_is_stunned(cn: usize) -> bool {
    for n in 0..20 {
        let active_spell = Repository::with_characters(|characters| characters[cn].spell[n]);
        if active_spell != 0
            && Repository::with_items(|items| items[active_spell as usize].temp) == SK_STUN as u16
        {
            return true;
        }
    }

    return false;
}

// TODO: Combine with npc_is_stunned?
pub fn npc_is_blessed(cn: usize) -> bool {
    for n in 0..20 {
        let active_spell = Repository::with_characters(|characters| characters[cn].spell[n]);
        if active_spell != 0
            && Repository::with_items(|items| items[active_spell as usize].temp) == SK_BLESS as u16
        {
            return true;
        }
    }

    return false;
}

pub fn npc_try_spell(cn: usize, co: usize, spell: usize) -> bool {
    Repository::with_characters_mut(|ch| {
        if ch[cn].flags & CharacterFlags::CF_NOMAGIC.bits() != 0 {
            return false;
        }

        if ch[co].used != core::constants::USE_ACTIVE {
            return false;
        }

        if ch[co].flags & CharacterFlags::CF_BODY.bits() != 0 {
            return false;
        }

        if ch[cn].skill[spell][0] == 0 {
            return false;
        }

        if ch[co].flags & CharacterFlags::CF_STONED.bits() != 0 {
            return false;
        }

        // Don't blast if the enemies armor is too high
        if spell == core::constants::SK_BLAST
            && (ch[cn].skill[core::constants::SK_BLAST][5] as i16 - ch[co].armor) < 10
        {
            return false;
        }

        // Don't stun if the chance of success is bad
        if spell == core::constants::SK_CURSE
            && 10 * ch[cn].skill[core::constants::SK_CURSE][5]
                / std::cmp::max(1, ch[co].skill[core::constants::SK_RESIST][5])
                < 7
        {
            return false;
        }

        // Don't stun if the chance of success is bad
        if spell == core::constants::SK_STUN
            && 10 * ch[cn].skill[core::constants::SK_STUN][5]
                / std::cmp::max(1, ch[co].skill[core::constants::SK_RESIST][5])
                < 5
        {
            return false;
        }

        let should_return_false_early = Repository::with_items(|it| {
            for n in 0..20 {
                let item_index = ch[cn].spell[n];
                if item_index == 0 {
                    continue;
                }

                if it[item_index as usize].temp as usize == core::constants::SK_BLAST {
                    return true;
                }
            }
            false
        });

        if should_return_false_early {
            return false;
        }

        let mana = ch[cn].a_mana / 1000;

        let mut found = false;
        for n in 0..20 {
            let item_index = ch[co].spell[n];
            if item_index == 0 {
                continue;
            }

            let should_break = Repository::with_items(|it| {
                if it[item_index as usize].temp as usize == spell
                    && it[item_index as usize].power + 10
                        >= spell_immunity(
                            ch[cn].skill[spell][5] as i32,
                            ch[co].skill[core::constants::SK_IMMUN][5] as i32,
                        ) as u32
                    && it[item_index as usize].active > it[item_index as usize].duration / 2
                {
                    return true;
                }
                false
            });

            if should_break {
                found = true;
                break;
            }
        }

        if found {
            let tmp = spellflag(spell);

            if mana >= get_spellcost(cn, spell) && ch[co].data[96] as u32 & tmp == 0 {
                ch[cn].skill_nr = spell as u16;
                ch[cn].skill_target1 = co as u16;
                ch[co].data[96] |= tmp as i32;
                // TODO: fx_add_effect(11, 8, co, tmp, 0);
                return true;
            }
        }

        false
    })
}

pub fn spell_immunity(power: i32, immunity: i32) -> i32 {
    let half_immunity = immunity / 2;
    if power <= half_immunity {
        return 1;
    }

    power - half_immunity
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

pub fn npc_quaff_potion(cn: usize, itemp: i32, stemp: i32) -> bool {
    Repository::with_characters(|ch| {
        for n in 0..20 {
            let item_index = ch[cn].spell[n];

            if item_index == 0 {
                continue;
            }

            let should_return_false = Repository::with_items(|it| {
                if it[item_index as usize].temp as i32 == stemp {
                    return true;
                }
                false
            });

            if should_return_false {
                return false;
            }
        }

        // Find potion and quaff it
        let (should_quaff, name): (bool, String) = Repository::with_items(|it| {
            for n in 0..40 {
                let item_index = ch[cn].item[n];

                if item_index == 0 {
                    continue;
                }

                if it[item_index as usize].temp == itemp as u16 {
                    return (
                        true,
                        String::from_utf8_lossy(&it[item_index as usize].name)
                            .into_owned()
                            .into(),
                    );
                }
            }

            (false, String::new().into())
        });

        if !should_quaff {
            return false;
        }

        State::with(|state| {
            state.do_area_log(
                cn,
                0,
                ch[cn].x as i32,
                ch[cn].y as i32,
                core::types::FontColor::Yellow,
                &format!(
                    "The {} uses a {}.\n",
                    String::from_utf8_lossy(&ch[cn].name),
                    name
                ),
            )
        });

        // TODO: use_driver(cn, in, 1);

        true
    })
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
            _ => {
                log::error!("Unknown special driver {} for {}", special_driver, cn);
                -1
            }
        };
        return;
    }

    let ticker = Repository::with_globals(|globals| globals.ticker);
    let flags = Repository::with_globals(|globals| globals.flags);

    // Handle action results
    Repository::with_characters_mut(|characters| {
        if characters[cn].last_action == ERR_SUCCESS as i8 {
            characters[cn].data[36] = 0; // Reset frust with successful action
        } else if characters[cn].last_action == ERR_FAILED as i8 {
            characters[cn].data[36] += 1; // Increase frust with failed action
        }
    });

    // Are we supposed to loot graves?
    let (alignment, temp, character_flags) = Repository::with_characters(|characters| {
        (
            characters[cn].alignment,
            characters[cn].temp,
            characters[cn].flags,
        )
    });

    if alignment < 0
        && (flags & GF_LOOTING) != 0
        && ((cn & 15) == (ticker as usize & 15)
            || (character_flags & CharacterFlags::CF_ISLOOTING.bits()) != 0)
        && temp != CT_COMPANION as u16
    {
        if npc_grave_logic(cn) {
            return;
        }
    }

    // Did someone call help? - high prio
    let (data_55, data_54) = Repository::with_characters(|characters| {
        (characters[cn].data[55], characters[cn].data[54])
    });

    if data_55 != 0 && data_55 + (TICKS * 120) > ticker as i32 && data_54 != 0 {
        let m = data_54;
        Repository::with_characters_mut(|characters| {
            characters[cn].goto_x =
                (m % SERVER_MAPX) as u16 + get_frust_x_off(ticker as i32) as u16;
            characters[cn].goto_y =
                (m / SERVER_MAPX) as u16 + get_frust_y_off(ticker as i32) as u16;
            characters[cn].data[58] = 2;
        });
        return;
    }

    // Go to last known enemy position and stay there for up to 30 seconds
    let (data_77, data_76, data_36) = Repository::with_characters(|characters| {
        (
            characters[cn].data[77],
            characters[cn].data[76],
            characters[cn].data[36],
        )
    });

    if data_77 != 0 && data_77 + (TICKS * 30) > ticker as i32 {
        let m = data_76;
        Repository::with_characters_mut(|characters| {
            characters[cn].goto_x = (m % SERVER_MAPX) as u16 + get_frust_x_off(data_36) as u16;
            characters[cn].goto_y = (m / SERVER_MAPX) as u16 + get_frust_y_off(data_36) as u16;
        });
        return;
    }

    // We're hurt: rest
    let (a_hp, hp_5) =
        Repository::with_characters(|characters| (characters[cn].a_hp, characters[cn].hp[5]));

    if a_hp < (hp_5 as i32 * 750) {
        return;
    }

    // Close door, medium prio
    for n in 20..24 {
        let m = Repository::with_characters(|characters| characters[cn].data[n]);

        if m != 0 {
            let m = m as usize;
            // Check if the door is free
            let is_free = Repository::with_map(|map| {
                map[m].ch == 0
                    && map[m].to_ch == 0
                    && map[m + 1].ch == 0
                    && map[m + 1].to_ch == 0
                    && map[m - 1].ch == 0
                    && map[m - 1].to_ch == 0
                    && map[m + SERVER_MAPX as usize].ch == 0
                    && map[m + SERVER_MAPX as usize].to_ch == 0
                    && map[m - SERVER_MAPX as usize].ch == 0
                    && map[m - SERVER_MAPX as usize].to_ch == 0
            });

            if is_free {
                let (it_idx, is_active) = Repository::with_map(|map| {
                    let it_idx = map[m].it;
                    if it_idx != 0 {
                        let is_active =
                            Repository::with_items(|items| items[it_idx as usize].active);
                        (it_idx, is_active)
                    } else {
                        (0, 0)
                    }
                });

                if it_idx != 0 && is_active != 0 {
                    Repository::with_characters_mut(|characters| {
                        characters[cn].misc_action = core::constants::DR_USE as u16;
                        characters[cn].misc_target1 = (m % SERVER_MAPX as usize) as u16;
                        characters[cn].misc_target2 = (m / SERVER_MAPX as usize) as u16;
                        characters[cn].data[58] = 1;
                    });
                    return;
                }
            }
        }
    }

    // Activate light, medium prio
    for n in 32..36 {
        let m = Repository::with_characters(|characters| characters[cn].data[n]);

        if m != 0 && m < (SERVER_MAPX * SERVER_MAPY) as i32 {
            let m = m as usize;
            let (it_idx, is_active) = Repository::with_map(|map| {
                let it_idx = map[m].it;
                if it_idx != 0 {
                    let is_active = Repository::with_items(|items| items[it_idx as usize].active);
                    (it_idx, is_active)
                } else {
                    (0, 1)
                }
            });

            if it_idx != 0 && is_active == 0 {
                Repository::with_characters_mut(|characters| {
                    characters[cn].misc_action = core::constants::DR_USE as u16;
                    characters[cn].misc_target1 = (m % SERVER_MAPX as usize) as u16;
                    characters[cn].misc_target2 = (m / SERVER_MAPX as usize) as u16;
                    characters[cn].data[58] = 1;
                });
                return;
            }
        }
    }

    // Patrol, low
    let data_10 = Repository::with_characters(|characters| characters[cn].data[10]);
    if data_10 != 0 {
        let mut n = Repository::with_characters(|characters| characters[cn].data[19]);

        if n < 10 || n > 18 {
            n = 10;
            Repository::with_characters_mut(|characters| {
                characters[cn].data[19] = n;
            });
        }

        let data_57 = Repository::with_characters(|characters| characters[cn].data[57]);
        if data_57 > ticker as i32 {
            return;
        }

        let (m, data_36, ch_x, ch_y, data_79) = Repository::with_characters(|characters| {
            (
                characters[cn].data[n as usize],
                characters[cn].data[36],
                characters[cn].x,
                characters[cn].y,
                characters[cn].data[79],
            )
        });

        let x = (m % SERVER_MAPX) as i32 + get_frust_x_off(data_36);
        let y = (m / SERVER_MAPX) as i32 + get_frust_y_off(data_36);

        if data_36 > 20 || ((ch_x as i32 - x).abs() + (ch_y as i32 - y).abs()) < 4 {
            if data_36 <= 20 && data_79 != 0 {
                Repository::with_characters_mut(|characters| {
                    characters[cn].data[57] = ticker as i32 + data_79;
                });
            }

            n += 1;
            if n > 18 {
                n = 10;
            }

            let data_n = Repository::with_characters(|characters| characters[cn].data[n as usize]);
            if data_n == 0 {
                n = 10;
            }

            Repository::with_characters_mut(|characters| {
                characters[cn].data[19] = n;
                characters[cn].data[36] = 0;
            });

            return;
        }

        Repository::with_characters_mut(|characters| {
            characters[cn].goto_x = x as u16;
            characters[cn].goto_y = y as u16;
            characters[cn].data[58] = 0;
        });
        return;
    }

    // Random walk, low
    let data_60 = Repository::with_characters(|characters| characters[cn].data[60]);
    if data_60 != 0 {
        Repository::with_characters_mut(|characters| {
            characters[cn].data[58] = 0;
        });

        let mut data_61 = Repository::with_characters(|characters| characters[cn].data[61]);
        if data_61 < 1 {
            Repository::with_characters_mut(|characters| {
                characters[cn].data[61] = data_60;
            });

            let (ch_x, ch_y, data_73, data_29) = Repository::with_characters(|characters| {
                (
                    characters[cn].x,
                    characters[cn].y,
                    characters[cn].data[73],
                    characters[cn].data[29],
                )
            });

            let mut panic = 0;
            let mut x = 0;
            let mut y = 0;

            for attempt in 0..5 {
                // Call RANDOM function (doesn't exist yet, use placeholder)
                x = ch_x as i32 - 5 + (ticker as i32 % 11); // RANDOM(11)
                y = ch_y as i32 - 5 + ((ticker as i32 / 11) % 11); // RANDOM(11)

                if x < 1 || x >= SERVER_MAPX as i32 || y < 1 || y > SERVER_MAPX as i32 {
                    panic = attempt + 1;
                    continue;
                }

                if data_73 != 0 {
                    // Too far away from origin?
                    let xo = (data_29 % SERVER_MAPX) as i32;
                    let yo = (data_29 / SERVER_MAPX) as i32;

                    if (x - xo).abs() + (y - yo).abs() > data_73 {
                        // Try to return to origin
                        let plr_check_target = |tx: i32, ty: i32| -> bool {
                            npc_check_target(tx as usize, ty as usize)
                        };

                        if plr_check_target(xo, yo) {
                            Repository::with_characters_mut(|characters| {
                                characters[cn].goto_x = xo as u16;
                                characters[cn].goto_y = yo as u16;
                            });
                            return;
                        } else if plr_check_target(xo + 1, yo) {
                            Repository::with_characters_mut(|characters| {
                                characters[cn].goto_x = (xo + 1) as u16;
                                characters[cn].goto_y = yo as u16;
                            });
                            return;
                        } else if plr_check_target(xo - 1, yo) {
                            Repository::with_characters_mut(|characters| {
                                characters[cn].goto_x = (xo - 1) as u16;
                                characters[cn].goto_y = yo as u16;
                            });
                            return;
                        } else if plr_check_target(xo, yo + 1) {
                            Repository::with_characters_mut(|characters| {
                                characters[cn].goto_x = xo as u16;
                                characters[cn].goto_y = (yo + 1) as u16;
                            });
                            return;
                        } else if plr_check_target(xo, yo - 1) {
                            Repository::with_characters_mut(|characters| {
                                characters[cn].goto_x = xo as u16;
                                characters[cn].goto_y = (yo - 1) as u16;
                            });
                            return;
                        } else {
                            panic = attempt + 1;
                            continue;
                        }
                    }
                }

                if !npc_check_target(x as usize, y as usize) {
                    panic = attempt + 1;
                    continue;
                }

                // Call can_go (doesn't exist yet)
                // if !can_go(ch_x as i32, ch_y as i32, x, y) {
                //     panic = attempt + 1;
                //     continue;
                // }

                panic = attempt;
                break;
            }

            if panic == 5 {
                return;
            }

            Repository::with_characters_mut(|characters| {
                characters[cn].goto_x = x as u16;
                characters[cn].goto_y = y as u16;
            });
            return;
        } else {
            Repository::with_characters_mut(|characters| {
                characters[cn].data[61] -= 1;
            });
            return;
        }
    }

    // Resting position, lowest prio
    let data_29 = Repository::with_characters(|characters| characters[cn].data[29]);
    if data_29 != 0 {
        let data_36 = Repository::with_characters(|characters| characters[cn].data[36]);
        let m = data_29;
        let x = (m % SERVER_MAPX) as i32 + get_frust_x_off(data_36);
        let y = (m / SERVER_MAPX) as i32 + get_frust_y_off(data_36);

        Repository::with_characters_mut(|characters| {
            characters[cn].data[58] = 0;
        });

        let (ch_x, ch_y, ch_dir, data_30) = Repository::with_characters(|characters| {
            (
                characters[cn].x,
                characters[cn].y,
                characters[cn].dir,
                characters[cn].data[30],
            )
        });

        if ch_x != x as i16 || ch_y != y as i16 {
            Repository::with_characters_mut(|characters| {
                characters[cn].goto_x = x as u16;
                characters[cn].goto_y = y as u16;
            });
            return;
        }

        if ch_dir as i32 != data_30 {
            Repository::with_characters_mut(|characters| {
                characters[cn].misc_action = core::constants::DR_TURN as u16;

                match data_30 {
                    x if x == core::constants::DX_UP as i32 => {
                        characters[cn].misc_target1 = x as u16;
                        characters[cn].misc_target2 = (y - 1) as u16;
                    }
                    x if x == core::constants::DX_DOWN as i32 => {
                        characters[cn].misc_target1 = x as u16;
                        characters[cn].misc_target2 = (y + 1) as u16;
                    }
                    x if x == core::constants::DX_LEFT as i32 => {
                        characters[cn].misc_target1 = (x - 1) as u16;
                        characters[cn].misc_target2 = y as u16;
                    }
                    x if x == core::constants::DX_RIGHT as i32 => {
                        characters[cn].misc_target1 = (x + 1) as u16;
                        characters[cn].misc_target2 = y as u16;
                    }
                    x if x == core::constants::DX_LEFTUP as i32 => {
                        characters[cn].misc_target1 = (x - 1) as u16;
                        characters[cn].misc_target2 = (y - 1) as u16;
                    }
                    x if x == core::constants::DX_LEFTDOWN as i32 => {
                        characters[cn].misc_target1 = (x - 1) as u16;
                        characters[cn].misc_target2 = (y + 1) as u16;
                    }
                    x if x == core::constants::DX_RIGHTUP as i32 => {
                        characters[cn].misc_target1 = (x + 1) as u16;
                        characters[cn].misc_target2 = (y - 1) as u16;
                    }
                    x if x == core::constants::DX_RIGHTDOWN as i32 => {
                        characters[cn].misc_target1 = (x + 1) as u16;
                        characters[cn].misc_target2 = (y + 1) as u16;
                    }
                    _ => {
                        characters[cn].misc_action = core::constants::DR_IDLE as u16;
                    }
                }
            });
            return;
        }
    }

    // Reset talked-to list
    let data_67 = Repository::with_characters(|characters| characters[cn].data[67]);
    if data_67 + (TICKS * 60 * 5) < ticker as i32 {
        let data_37 = Repository::with_characters(|characters| characters[cn].data[37]);
        if data_37 != 0 {
            Repository::with_characters_mut(|characters| {
                for n in 37..41 {
                    characters[cn].data[n] = 1; // Hope we never have a character nr 1!
                }
            });
        }
        Repository::with_characters_mut(|characters| {
            characters[cn].data[67] = ticker as i32;
        });
    }

    // Special sub-proc for Shiva (black stronghold mage)
    let (data_26, a_mana, mana_5) = Repository::with_characters(|characters| {
        (
            characters[cn].data[26],
            characters[cn].a_mana,
            characters[cn].mana[5],
        )
    });

    if data_26 == 2 && a_mana > (mana_5 as i32 * 900) {
        // Count active monsters of type 27
        let mut m = 0;
        for n in 1..MAXCHARS {
            let (used, flags, data_42) = Repository::with_characters(|characters| {
                if n >= characters.len() {
                    return (0, 0, 0);
                }
                (
                    characters[n].used,
                    characters[n].flags,
                    characters[n].data[42],
                )
            });

            if used != USE_ACTIVE {
                continue;
            }
            if (flags & (CharacterFlags::CF_BODY.bits() | CharacterFlags::CF_RESPAWN.bits())) != 0 {
                continue;
            }
            if data_42 == 27 {
                m += 1;
            }
        }

        if m < 15 {
            let mut n = 0;

            // Check candles
            let candle_positions = [(446, 347), (450, 351), (457, 348), (457, 340), (449, 340)];

            for (cx, cy) in &candle_positions {
                let map_idx = cx + cy * SERVER_MAPX as usize;
                let (it_idx, is_active) = Repository::with_map(|map| {
                    let it_idx = map[map_idx].it;
                    if it_idx != 0 {
                        let is_active =
                            Repository::with_items(|items| items[it_idx as usize].active);
                        (it_idx, is_active)
                    } else {
                        (0, 0)
                    }
                });

                if it_idx != 0 {
                    if is_active == 0 {
                        n += 1;
                    } else {
                        if shiva_activate_candle(cn, it_idx as usize) != 0 {
                            return;
                        }
                    }
                }
            }

            if n > 0 {
                for m_idx in 0..n {
                    // Call pop_create_char (doesn't exist yet)
                    let co = 0; // pop_create_char(503 + m_idx, 0);
                    if co == 0 {
                        State::with(|state| {
                            state.do_sayx(cn, &format!("create char ({})", m_idx));
                        });
                        break;
                    }

                    // Call god_drop_char_fuzzy (doesn't exist yet)
                    let drop_result = false; // !god_drop_char_fuzzy(co, 452, 345);
                    if drop_result {
                        State::with(|state| {
                            state.do_sayx(cn, &format!("drop char ({})", m_idx));
                        });
                        God::destroy_items(co);
                        Repository::with_characters_mut(|characters| {
                            characters[co].used = 0;
                        });
                        break;
                    }

                    // Call fx_add_effect (doesn't exist yet)
                    // fx_add_effect(6, 0, characters[co].x, characters[co].y, 0);
                }

                // fx_add_effect(7, 0, characters[cn].x, characters[cn].y, 0);
                State::with(|state| {
                    state.do_sayx(cn, "Khuzak gurawin duskar!");
                });

                Repository::with_characters_mut(|characters| {
                    characters[cn].a_mana -= n * 100 * 1000;
                });

                log::info!("created {} new monsters", n);
            }
        }

        Repository::with_characters_mut(|characters| {
            characters[cn].a_mana -= 100 * 1000;
        });
    }
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
            if it.hp[2] > ch.hp[0] as i16 {
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
    let item_38 = Repository::with_characters(|characters| characters[cn].item[38]);

    if item_38 != 0 {
        return false; // hack: don't take more stuff if inventory is almost full
    }

    let citem = Repository::with_characters(|characters| characters[cn].citem);

    if citem != 0 {
        Repository::with_items(|items| {
            log::info!(
                "have {} in citem",
                String::from_utf8_lossy(&items[in_idx].name)
            );
        });

        let do_store_item = State::with(|state| state.do_store_item(cn));
        if do_store_item == -1 {
            Repository::with_items_mut(|items| {
                items[citem as usize].used = USE_EMPTY;
            });
            Repository::with_characters_mut(|chars| {
                chars[cn].citem = 0;
            });
        }
    }

    let temp = Repository::with_items(|items| items[in_idx].temp);

    if temp == 833 || temp == 267 {
        Repository::with_characters_mut(|characters| {
            characters[cn].citem = in_idx as u32;
        });
        Repository::with_items_mut(|items| {
            items[in_idx].carried = cn as u16;
        });
        State::with(|state| state.do_store_item(cn));
        return true;
    }

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
