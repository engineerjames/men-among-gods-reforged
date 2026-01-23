use crate::core;
use crate::driver;
use crate::effect::EffectManager;
use crate::helpers;
use crate::player;
use crate::populate;
use crate::{god::God, repository::Repository, state::State};
use core::constants::*;
use core::string_operations::c_string_to_str;
use core::types::skilltab;
use rand::Rng;

// Helper functions

/// Returns the X offset for a given frustration value.
///
/// # Arguments
///
/// * `f` - Frustration value
///
/// # Returns
///
/// The X offset as an i32.
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

/// Returns the Y offset for a given frustration value.
///
/// # Arguments
///
/// * `f` - Frustration value
///
/// # Returns
///
/// The Y offset as an i32.
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

/// Calculates the maximum of the absolute X or Y distance between two characters.
///
/// # Arguments
///
/// * `cn` - First character number (index)
/// * `co` - Second character number (index)
///
/// # Returns
///
/// The maximum of the absolute X or Y distance as i32.
pub fn npc_dist(cn: usize, co: usize) -> i32 {
    Repository::with_characters(|characters| {
        let ch_cn = &characters[cn];
        let ch_co = &characters[co];
        std::cmp::max((ch_cn.x - ch_co.x).abs(), (ch_cn.y - ch_co.y).abs()) as i32
    })
}

// ****************************************************
// NPC Message Handling and AI Functions
// ****************************************************

/// Adds an enemy to the NPC's enemy list if conditions are met.
///
/// # Arguments
///
/// * `cn` - NPC character number
/// * `co` - Enemy character number
/// * `always` - If true, always add as enemy regardless of some conditions
///
/// # Returns
///
/// `true` if the enemy was added, `false` otherwise.
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
        characters[cn].data[77] = ticker;

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

        let idx = co as i32 | (helpers::char_id(co) << 16);

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
        let idx = co as i32 | (helpers::char_id(co) << 16);

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
                &format!("Enemies of {}:", c_string_to_str(&characters[npc].name)),
            );

            for n in 80..92 {
                let cv = (characters[npc].data[n] & 0xFFFF) as usize;
                if cv > 0 && cv < MAXCHARS {
                    state.do_character_log(
                        cn,
                        core::types::FontColor::Green,
                        &format!("  {}", c_string_to_str(&characters[cv].name)),
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

        if (ch_npc.flags & CharacterFlags::ShutUp.bits()) != 0 {
            return;
        }

        if n >= ch_npc.text.len() {
            return;
        }

        let base_text = c_string_to_str(&ch_npc.text[n]);
        if base_text.is_empty() {
            return;
        }

        let text = if let Some(name_str) = name {
            base_text.replace("%s", name_str)
        } else {
            base_text.to_string()
        };

        let temp = ch_npc.temp;
        let talkative = ch_npc.data[71]; // CHD_TALKATIVE

        if temp == CT_COMPANION as u16 {
            if talkative == -10 {
                State::with(|state| {
                    state.do_sayx(npc, &text);
                });
            }
        } else {
            State::with(|state| {
                state.do_sayx(npc, &text);
            });
        }
    });
}

pub fn npc_gotattack(cn: usize, co: usize, _dam: i32) -> i32 {
    Repository::with_characters_mut(|characters| {
        characters[cn].data[92] = TICKS * 60;

        let ticker = Repository::with_globals(|globals| globals.ticker);

        // Special handling for high alignment NPCs being attacked by players
        if co > 0
            && co < MAXCHARS
            && (characters[co].flags & CharacterFlags::Player.bits()) != 0
            && characters[cn].alignment == 10000
            && (characters[cn].get_name() != "Peacekeeper"
                || characters[cn].a_hp < (characters[cn].hp[5] * 500) as i32)
            && characters[cn].data[70] < ticker
        {
            State::with(|state| {
                state.do_sayx(cn, "Skua! Protect the innocent! Send me a Peacekeeper!");
            });
            Repository::with_characters(|ch| {
                EffectManager::fx_add_effect(6, 0, ch[cn].x as i32, ch[cn].y as i32, 0)
            });
            characters[cn].data[70] = ticker + (TICKS * 60);

            let cc = God::create_char(80, true);
            if cc.is_some() && cc.unwrap() > 0 && cc.unwrap() < MAXCHARS as i32 {
                let cc = cc.unwrap() as usize;
                characters[cc].temp = CT_COMPANION as u16;
                characters[cc].data[42] = 65536 + cn as i32;
                characters[cc].data[59] = 65536 + cn as i32;
                characters[cc].data[24] = 0;
                characters[cc].data[36] = 0;
                characters[cc].data[43] = 0;
                characters[cc].data[80] = co as i32 | (helpers::char_id(co) << 16);
                characters[cc].data[63] = cn as i32;
                characters[cc].data[64] = ticker + 120 * TICKS;
                characters[cc].data[70] = ticker + (TICKS * 60);

                characters[cc].set_name("Shadow of Peace");
                characters[cc].set_reference("Shadow of Peace");
                characters[cc].set_description("You see a Shadow of Peace.");

                if !God::drop_char_fuzzy(cc, characters[co].x as usize, characters[co].y as usize) {
                    God::destroy_items(cc);
                    characters[cc].used = 0;
                }
            }
        }

        // Help request for good aligned characters
        if characters[cn].alignment > 1000
            && characters[cn].data[70] < ticker
            && characters[cn].a_mana < (characters[cn].mana[5] * 333) as i32
        {
            State::with(|state| {
                state.do_sayx(cn, "Skua! Help me!");
            });
            characters[cn].data[70] = ticker + (TICKS * 60 * 2);
            characters[cn].a_mana = (characters[cn].mana[5] * 1000) as i32;
            EffectManager::fx_add_effect(6, 0, characters[cn].x as i32, characters[cn].y as i32, 0);
        }

        // Shout for help
        if characters[cn].data[52] != 0 && characters[cn].a_hp < (characters[cn].hp[5] * 666) as i32
        {
            if characters[cn].data[55] + (TICKS * 60) < ticker {
                characters[cn].data[54] = 0;
                characters[cn].data[55] = ticker;
                if co < MAXCHARS {
                    let co_name = characters[co].get_name();
                    npc_saytext_n(cn, 4, Some(co_name));
                }
                State::with(|state| {
                    state.do_npc_shout(
                        cn,
                        NT_SHOUT as i32,
                        cn as i32,
                        characters[cn].data[52],
                        characters[cn].x as i32,
                        characters[cn].y as i32,
                    );
                });
            }
        }

        // Can't see attacker - panic mode
        let character_can_see = State::with_mut(|state| state.do_char_can_see(cn, co));
        if co >= MAXCHARS || character_can_see == 0 {
            characters[cn].data[78] = ticker + (TICKS * 30);
            return 1;
        }

        // Fight back
        if co < MAXCHARS {
            let co_name = characters[co].get_name();
            let cn_name = characters[cn].get_name();
            if npc_add_enemy(cn, co, true) {
                npc_saytext_n(cn, 1, Some(co_name));
                log::info!(
                    "NPC {} ({}) added {} ({}) to enemy list for attacking",
                    cn,
                    cn_name,
                    co,
                    co_name
                );
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

        let idx = co as i32 | (helpers::char_id(co) << 16);

        for n in 80..92 {
            if characters[cn].data[n] == idx {
                if cn == cc && co < MAXCHARS {
                    let co_name = characters[co].get_name().to_string();
                    npc_saytext_n(cn, 0, Some(&co_name));
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
        characters[cn].data[92] = TICKS * 60;

        let cn_can_see_co = State::with_mut(|state| state.do_char_can_see(cn, co));

        let cn_can_see_cc = State::with_mut(|state| state.do_char_can_see(cn, cc));

        if cn_can_see_co == 0 || cn_can_see_cc == 0 {
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
                let c2_name = Repository::with_characters(|chars| chars[c2].get_name().to_string());
                let c3_name = Repository::with_characters(|chars| chars[c3].get_name().to_string());
                npc_saytext_n(cn, 1, Some(&c2_name));
                log::info!(
                    "NPC {} added {} to enemy list for attacking {}",
                    cn,
                    c2_name,
                    c3_name
                );
            }
            return 1;
        }

        // Protect character by template
        if characters[cn].data[31] != 0 {
            if characters[co].temp == characters[cn].data[31] as u16 {
                if npc_add_enemy(cn, cc, true) {
                    let cc_name =
                        Repository::with_characters(|chars| chars[cc].get_name().to_string());
                    let co_name =
                        Repository::with_characters(|chars| chars[co].get_name().to_string());
                    npc_saytext_n(cn, 1, Some(&cc_name));
                    log::info!(
                        "NPC {} added {} to enemy list for attacking {} (protect char)",
                        cn,
                        cc_name,
                        co_name
                    );
                }
                Repository::with_characters_mut(|chars| {
                    if chars[cn].data[65] == 0 {
                        chars[cn].data[65] = co as i32;
                    }
                });
            }
        }

        // Protect character by number (CHD_MASTER)
        if characters[cn].data[63] != 0 {
            if co == characters[cn].data[63] as usize {
                if npc_add_enemy(cn, cc, true) {
                    let cc_name =
                        Repository::with_characters(|chars| chars[cc].get_name().to_string());
                    let co_name =
                        Repository::with_characters(|chars| chars[co].get_name().to_string());
                    npc_saytext_n(cn, 1, Some(&cc_name));
                    log::info!(
                        "NPC {} added {} to enemy list for attacking {} (protect char)",
                        cn,
                        cc_name,
                        co_name
                    );
                }
                if characters[cn].data[65] == 0 {
                    characters[cn].data[65] = co as i32;
                }
            }
            if cc == characters[cn].data[63] as usize {
                if npc_add_enemy(cn, co, true) {
                    let co_name =
                        Repository::with_characters(|chars| chars[co].get_name().to_string());
                    let cc_name =
                        Repository::with_characters(|chars| chars[cc].get_name().to_string());
                    npc_saytext_n(cn, 1, Some(&co_name));
                    log::info!(
                        "NPC {} added {} to enemy list for being attacked by {} (protect char)",
                        cn,
                        co_name,
                        cc_name
                    );
                }
                if characters[cn].data[65] == 0 {
                    characters[cn].data[65] = cc as i32;
                }
            }
        }

        // Protect by group (CHD_HELPGROUP)
        if characters[cn].data[59] != 0 {
            if characters[cn].data[59] == characters[co].data[42] {
                if npc_add_enemy(cn, cc, true) {
                    let cc_name =
                        Repository::with_characters(|chars| chars[cc].get_name().to_string());
                    let co_name =
                        Repository::with_characters(|chars| chars[co].get_name().to_string());
                    npc_saytext_n(cn, 1, Some(&cc_name));
                    log::info!(
                        "NPC {} added {} to enemy list for attacking {} (protect group)",
                        cn,
                        cc_name,
                        co_name
                    );
                }
                if characters[cn].data[65] == 0 {
                    characters[cn].data[65] = co as i32;
                }
            }
            if characters[cn].data[59] == characters[cc].data[42] {
                if npc_add_enemy(cn, co, true) {
                    let co_name =
                        Repository::with_characters(|chars| chars[co].get_name().to_string());
                    let cc_name =
                        Repository::with_characters(|chars| chars[cc].get_name().to_string());
                    npc_saytext_n(cn, 1, Some(&co_name));
                    log::info!(
                        "NPC {} added {} to enemy list for being attacked by {} (protect group)",
                        cn,
                        co_name,
                        cc_name
                    );
                }
                if characters[cn].data[65] == 0 {
                    characters[cn].data[65] = cc as i32;
                }
            }
        }

        // If one of the participants is my companion and its master is me, register the helper index
        if characters[co].temp == core::constants::CT_COMPANION as u16
            && characters[co].data[63] == cn as i32
        {
            if characters[cn].data[65] == 0 {
                characters[cn].data[65] = co as i32;
            }
        }

        if characters[cc].temp == core::constants::CT_COMPANION as u16
            && characters[cc].data[63] == cn as i32
        {
            if characters[cn].data[65] == 0 {
                characters[cn].data[65] = cc as i32;
            }
        }

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

pub fn npc_give(cn: usize, co: usize, in_item: usize, money: i32) -> i32 {
    Repository::with_characters_mut(|characters| {
        // If giver is a player/usurp, set active timer; otherwise ensure group active
        if (characters[co].flags & (CharacterFlags::Player.bits() | CharacterFlags::Usurp.bits()))
            != 0
        {
            characters[cn].data[92] = TICKS * 60;
        } else if !characters[cn].group_active() {
            return 0;
        }

        // Item given and matches what NPC wants
        if in_item != 0
            && Repository::with_items(|items| items[in_item].temp as i32) == characters[cn].data[49]
        {
            // Black candle special-case
            if characters[cn].data[49] == 740 && characters[cn].temp == 518 {
                characters[co].data[43] += 1;
                // Remove item from NPC and destroy it
                God::take_from_char(in_item, cn);
                Repository::with_items_mut(|items| {
                    items[in_item].used = core::constants::USE_EMPTY;
                });

                State::with(|state| {
                    state.do_sayx(
                        cn,
                        &format!(
                            "Ah, a black candle! Great work, {}! Now we will have peace for a while...",
                            characters[co].get_name()
                        ),
                    );
                    state.do_area_log(
                        cn,
                        0,
                        characters[cn].x as i32,
                        characters[cn].y as i32,
                        core::types::FontColor::Yellow,
                        &format!(
                            "The Cityguard is impressed by {}'s deed.\n",
                            characters[co].get_name()
                        ),
                    );
                });
            } else {
                // Thank you message
                let ref_name = Repository::with_items(|items| {
                    c_string_to_str(&items[in_item].reference).to_string()
                });
                State::with(|state| {
                    state.do_sayx(
                        cn,
                        &format!(
                            "Thank you {}. That's the {} I wanted.",
                            characters[co].get_name(),
                            ref_name
                        ),
                    );
                });
            }

            // Quest-requested items: teach skill / give exp
            let nr = characters[cn].data[50];
            if nr != 0 {
                let nr_usize = nr as usize;
                let skill_name = skilltab::get_skill_name(nr_usize);
                State::with(|state| {
                    state.do_sayx(cn, &format!("Now I'll teach you {}.", skill_name));
                });

                if characters[co].skill[nr_usize][0] != 0 {
                    State::with(|state| {
                        state.do_sayx(
                            cn,
                            &format!(
                                "But you already know {}, {}!",
                                skill_name,
                                characters[co].get_name()
                            ),
                        );
                    });
                    // give item back to player
                    God::take_from_char(in_item, cn);
                    God::give_character_item(co, in_item);
                    State::with(|state| {
                        state.do_character_log(
                            co,
                            core::types::FontColor::Green,
                            &format!(
                                "{} did not accept the {}.\n",
                                characters[cn].get_name(),
                                Repository::with_items(|items| items[in_item]
                                    .get_name()
                                    .to_string())
                            ),
                        );
                    });
                } else {
                    // teach skill
                    characters[co].skill[nr_usize][0] = 1;
                    State::with(|state| {
                        state.do_character_log(
                            co,
                            core::types::FontColor::Green,
                            &format!("You learned {}!\n", skill_name),
                        );
                        characters[co].set_do_update_flags();
                    });

                    let give_exp = characters[cn].data[51];
                    if give_exp != 0 {
                        State::with_mut(|state| {
                            state.do_sayx(cn, &format!("Now I'll teach you a bit about life, the world and everything, {}.", characters[co].get_name()));
                            state.do_give_exp(co, give_exp, 0, -1);
                        });
                    }

                    // take and destroy the offered item
                    God::take_from_char(in_item, cn);
                    Repository::with_items_mut(|items| {
                        items[in_item].used = core::constants::USE_EMPTY;
                    });
                }
            }

            // Return-gift
            let give_temp = characters[cn].data[66];
            if give_temp != 0 {
                State::with(|state| {
                    state.do_sayx(
                        cn,
                        &format!(
                            "Here is your {} in exchange.",
                            Repository::with_item_templates(|t| c_string_to_str(
                                &t[give_temp as usize].reference
                            )
                            .to_string())
                        ),
                    );
                });
                God::take_from_char(in_item, cn);
                Repository::with_items_mut(|items| {
                    items[in_item].used = core::constants::USE_EMPTY
                });
                if let Some(new_item) = God::create_item(give_temp as usize) {
                    God::give_character_item(co, new_item);
                }
            }

            // Riddle-giver special
            let ar = characters[cn].data[72];
            if characters[co].is_player()
                && (core::constants::RIDDLE_MIN_AREA..=core::constants::RIDDLE_MAX_AREA)
                    .contains(&ar)
            {
                let idx = (ar - core::constants::RIDDLE_MIN_AREA) as usize;
                // check Lab9 guesser
                let still = crate::lab9::Labyrinth9::with(|lab9| lab9.get_guesser(idx));
                if still != 0 && still as usize != co {
                    State::with(|state| {
                        state.do_sayx(
                            cn,
                            &format!(
                                "I'm still riddling {}; please come back later!\n",
                                Repository::with_characters(|ch| ch[still as usize]
                                    .get_name()
                                    .to_string())
                            ),
                        );
                    });
                    God::take_from_char(in_item, cn);
                    God::give_character_item(co, in_item);
                    return 0;
                }

                // Destroy gift from player and pose riddle
                God::take_from_char(in_item, co);
                Repository::with_items_mut(|items| {
                    items[in_item].used = core::constants::USE_EMPTY
                });
                crate::lab9::Labyrinth9::with_mut(|lab9| lab9.lab9_pose_riddle(cn, co));
            }

            return 0;
        } else if in_item == 0 && money != 0 {
            // NPC doesn't take money
            State::with(|state| {
                state.do_sayx(cn, "I don't take money from you!");
            });
            characters[co].gold += money;
            characters[cn].gold -= money;
        } else {
            // Not accepted - return item to giver
            God::take_from_char(in_item, cn);
            God::give_character_item(co, in_item);
            State::with(|state| {
                state.do_character_log(
                    co,
                    core::types::FontColor::Green,
                    &format!(
                        "{} did not accept the {}.\n",
                        characters[cn].get_name(),
                        Repository::with_items(|items| items[in_item].get_name().to_string())
                    ),
                );
            });
        }

        0
    })
}

pub fn npc_died(cn: usize, co: usize) -> i32 {
    // Mirror C++ behavior: chance = characters[cn].data[48]
    Repository::with_characters(|characters| {
        let chance = characters[cn].data[48];
        if chance != 0 && co > 0 {
            // random 0..99 < chance
            let roll = rand::thread_rng().gen_range(0..100) as i32;
            if roll < chance {
                let co_name = if co < MAXCHARS {
                    characters[co].get_name().to_string()
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
            characters[cn].data[92] = TICKS * 60;
            characters[cn].data[54] = x + y * SERVER_MAPX;
            characters[cn].data[55] = Repository::with_globals(|globals| globals.ticker);

            let co_name = if co < MAXCHARS {
                characters[co].get_name().to_string()
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
    let cn_can_see_co = State::with_mut(|state| state.do_char_can_see(cn, co));

    if cn_can_see_co == 0 {
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
            1 => driver::npc_stunrun_msg(cn, msg_type as u8, dat1, dat2, dat3, dat4),
            2 => driver::npc_cityattack_msg(cn, msg_type, dat1, dat2, dat3, dat4),
            3 => driver::npc_malte_msg(cn, msg_type, dat1, dat2, dat3, dat4),
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

    false
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

    false
}

pub fn npc_try_spell(cn: usize, co: usize, spell: usize) -> bool {
    Repository::with_characters_mut(|ch| {
        if ch[cn].flags & CharacterFlags::NoMagic.bits() != 0 {
            return false;
        }

        if ch[co].used != core::constants::USE_ACTIVE {
            return false;
        }

        if ch[co].flags & CharacterFlags::Body.bits() != 0 {
            return false;
        }

        if ch[cn].skill[spell][0] == 0 {
            return false;
        }

        if ch[co].flags & CharacterFlags::Stoned.bits() != 0 {
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
            && 10 * ch[cn].skill[core::constants::SK_CURSE][5] as i32
                / (std::cmp::max(1, ch[co].skill[core::constants::SK_RESIST][5]) as i32)
                < 7
        {
            return false;
        }

        // Don't stun if the chance of success is bad
        if spell == core::constants::SK_STUN
            && 10 * ch[cn].skill[core::constants::SK_STUN][5] as i32
                / (std::cmp::max(1, ch[co].skill[core::constants::SK_RESIST][5]) as i32)
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

        // C++ logic: scan target's active spells; if we find the same spell with
        // sufficient power and still > 50% duration remaining, we do NOT cast it again.
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

        // Match C++: only cast if such a spell was NOT found on the target.
        if !found {
            let tmp = spellflag(spell);

            if mana >= get_spellcost(cn, spell) && ch[co].data[96] as u32 & tmp == 0 {
                ch[cn].skill_nr = spell as u16;
                ch[cn].skill_target1 = co as u16;
                ch[co].data[96] |= tmp as i32;
                // Match C++ parameter semantics: effect[11].data[0]=target character id,
                // effect[11].data[1]=spellflag bitmask to clear later.
                EffectManager::fx_add_effect(11, 8, co as i32, tmp as i32, 0);
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
        let (should_quaff, name, item_index): (bool, String, usize) =
            Repository::with_items(|it| {
                for n in 0..40 {
                    let item_index = ch[cn].item[n];

                    if item_index == 0 {
                        continue;
                    }

                    if it[item_index as usize].temp == itemp as u16 {
                        return (
                            true,
                            it[item_index as usize].get_name().to_string(),
                            item_index as usize,
                        );
                    }
                }

                (false, String::new().into(), 0)
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
                &format!("The {} uses a {}.\n", ch[cn].get_name(), name),
            )
        });

        driver::use_driver(cn, item_index, true);

        true
    })
}

pub fn die_companion(cn: usize) {
    Repository::with_characters(|characters| {
        EffectManager::fx_add_effect(7, 0, characters[cn].x as i32, characters[cn].y as i32, 0);
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
            1 => driver::npc_stunrun_high(cn),
            2 => driver::npc_cityattack_high(cn),
            3 => driver::npc_malte_high(cn),
            _ => {
                log::error!("Unknown special driver {} for {}", special_driver, cn);
                0
            }
        };
    }

    let ticker = Repository::with_globals(|g| g.ticker);
    let _flags = Repository::with_globals(|g| g.flags);

    // reset panic mode if expired
    Repository::with_characters_mut(|characters| {
        if characters[cn].data[78] < ticker {
            characters[cn].data[78] = 0;
        }
    });

    // self destruct
    {
        let mut do_die = false;
        Repository::with_characters_mut(|characters| {
            let d64 = characters[cn].data[64];
            if d64 != 0 {
                if d64 < (TICKS * 60 * 15) {
                    characters[cn].data[64] = d64 + ticker;
                }
                if characters[cn].data[64] < ticker {
                    // NPC should self-destruct
                    // TODO: Port do_sayx(cn, "Free!")
                    characters[cn].used = USE_EMPTY;
                    do_die = true;
                }
            }
        });
        if do_die {
            God::destroy_items(cn);
            player::plr_map_remove(cn);
            npc_remove_enemy(cn, 0);
            return 1;
        }
    }

    // Count down master-no-see timer for player ghost companions
    {
        let (temp, data64) = Repository::with_characters(|characters| {
            (characters[cn].temp, characters[cn].data[64])
        });
        if temp == CT_COMPANION as u16 && data64 == 0 {
            let co = Repository::with_characters(|characters| characters[cn].data[CHD_MASTER]);
            let master_ok = Repository::with_characters(|characters| {
                let co = co as usize;
                if co >= characters.len() {
                    return false;
                }
                characters[co].used != USE_EMPTY && characters[co].data[64] == cn as i32
            });
            if !master_ok {
                log::warn!("{} killed for bad master({})", cn, co);
                die_companion(cn);
                return 1;
            }

            let should_self_destruct = Repository::with_globals(|g| g.ticker)
                > Repository::with_characters(|characters| characters[cn].data[98]);
            if should_self_destruct {
                Repository::with_characters_mut(|characters| {
                    let co = characters[cn].data[CHD_MASTER] as usize;
                    if co < characters.len() {
                        characters[co].luck -= 1;
                    }
                });
                log::info!("{} Self-destructed because of neglect by master", cn);
                die_companion(cn);
                return 1;
            }
        }
    }

    // heal us if we're hurt
    {
        let (a_hp, hp5) =
            Repository::with_characters(|characters| (characters[cn].a_hp, characters[cn].hp[5]));
        if a_hp < hp5 as i32 * 600 {
            if npc_try_spell(cn, cn, SK_HEAL) {
                return 1;
            }
        }
    }

    // donate/destroy citem if that's our job
    {
        let citem = Repository::with_characters(|characters| characters[cn].citem as usize);
        let donate_dest = Repository::with_characters(|characters| characters[cn].data[47]);
        if citem != 0 && donate_dest != 0 {
            let take_action = Repository::with_items(|items| {
                let it = &items[citem];
                it.damage_state != 0
                    || (it.flags & ItemFlags::IF_SHOPDESTROY.bits() != 0)
                    || (it.flags & ItemFlags::IF_DONATE.bits() == 0)
            });
            if take_action {
                Repository::with_items_mut(|items| items[citem].used = USE_EMPTY);
                Repository::with_characters_mut(|characters| characters[cn].citem = 0);
            } else {
                // reset ages/damage
                Repository::with_items_mut(|items| {
                    items[citem].current_age[0] = 0;
                    items[citem].current_age[1] = 0;
                    items[citem].current_damage = 0;
                });
                God::donate_item(citem, donate_dest);
                Repository::with_characters_mut(|characters| characters[cn].citem = 0);
            }
        }
    }

    // donate item[39]
    {
        let it39 = Repository::with_characters(|characters| characters[cn].item[39] as usize);
        let donate_dest = Repository::with_characters(|characters| characters[cn].data[47]);
        if it39 != 0 && donate_dest != 0 {
            let take_action = Repository::with_items(|items| {
                let it = &items[it39];
                it.damage_state != 0
                    || (it.flags & ItemFlags::IF_SHOPDESTROY.bits() != 0)
                    || (it.flags & ItemFlags::IF_DONATE.bits() == 0)
            });
            if take_action {
                Repository::with_items_mut(|items| items[it39].used = USE_EMPTY);
                Repository::with_characters_mut(|characters| characters[cn].item[39] = 0);
            } else {
                Repository::with_items_mut(|items| {
                    items[it39].current_age[0] = 0;
                    items[it39].current_age[1] = 0;
                    items[it39].current_damage = 0;
                });
                God::donate_item(it39, donate_dest);
                Repository::with_characters_mut(|characters| characters[cn].item[39] = 0);
            }
        }
    }

    // generic spell management
    {
        let (a_mana, med_skill) = Repository::with_characters(|characters| {
            (characters[cn].a_mana, characters[cn].skill[SK_MEDIT][0])
        });
        if a_mana > (Repository::with_characters(|characters| characters[cn].mana[5]) as i32) * 850
            && med_skill != 0
        {
            if a_mana > 75000 && npc_try_spell(cn, cn, SK_BLESS) {
                return 1;
            }
            if npc_try_spell(cn, cn, SK_PROTECT) {
                return 1;
            }
            if npc_try_spell(cn, cn, SK_MSHIELD) {
                return 1;
            }
            if npc_try_spell(cn, cn, SK_ENHANCE) {
                return 1;
            }
            if npc_try_spell(cn, cn, SK_BLESS) {
                return 1;
            }
        }
    }

    // generic endurance management (mode switching)
    {
        let data58 = Repository::with_characters(|characters| characters[cn].data[58]);
        let a_end = Repository::with_characters(|characters| characters[cn].a_end);
        if data58 > 1 && a_end > 10000 {
            Repository::with_characters_mut(|characters| {
                if characters[cn].mode != 2 {
                    characters[cn].mode = 2;
                    State::with(|s| s.do_update_char(cn));
                }
            });
        } else if data58 == 1 && a_end > 10000 {
            Repository::with_characters_mut(|characters| {
                if characters[cn].mode != 1 {
                    characters[cn].mode = 1;
                    State::with(|s| s.do_update_char(cn));
                }
            });
        } else {
            Repository::with_characters_mut(|characters| {
                if characters[cn].mode != 0 {
                    characters[cn].mode = 0;
                    State::with(|s| s.do_update_char(cn));
                }
            });
        }
    }

    // create light (approximation: attempt spell if conditions met)
    {
        let (data62, _data58) = Repository::with_characters(|characters| {
            (characters[cn].data[62], characters[cn].data[58])
        });
        if data62 > _data58 {
            let (cx, cy) = Repository::with_characters(|characters| {
                (characters[cn].x as usize, characters[cn].y as usize)
            });
            let light = State::check_dlight(cx, cy);
            if light < 20 {
                if npc_try_spell(cn, cn, SK_LIGHT) {
                    return 1;
                }
            }
        }
    }

    // make sure protected character survives
    {
        let co = Repository::with_characters(|characters| characters[cn].data[63] as usize);
        if co != 0 {
            let (a_hp, hp5) = Repository::with_characters(|characters| {
                (characters[co].a_hp, characters[co].hp[5])
            });
            if a_hp < hp5 as i32 * 600 {
                if npc_try_spell(cn, co, SK_HEAL) {
                    return 1;
                }
            }
        }
    }

    // help friend
    {
        let co = Repository::with_characters(|characters| characters[cn].data[65] as usize);
        if co != 0 {
            let cc = Repository::with_characters(|characters| characters[co].attack_cn as usize);

            if Repository::with_characters(|characters| characters[cn].a_mana)
                > (get_spellcost(cn, SK_BLESS) * 2
                    + get_spellcost(cn, SK_PROTECT)
                    + get_spellcost(cn, SK_ENHANCE))
            {
                if npc_try_spell(cn, cn, SK_BLESS) {
                    return 1;
                }
            }

            if Repository::with_characters(|characters| characters[co].a_hp)
                < Repository::with_characters(|characters| characters[co].hp[5]) as i32 * 600
            {
                if npc_try_spell(cn, co, SK_HEAL) {
                    return 1;
                }
            }

            if !npc_can_spell(co, cn, SK_PROTECT) && npc_try_spell(cn, co, SK_PROTECT) {
                return 1;
            }
            if !npc_can_spell(co, cn, SK_ENHANCE) && npc_try_spell(cn, co, SK_ENHANCE) {
                return 1;
            }
            if !npc_can_spell(co, cn, SK_BLESS) && npc_try_spell(cn, co, SK_BLESS) {
                return 1;
            }

            if cc != 0
                && Repository::with_characters(|characters| characters[co].a_hp)
                    < Repository::with_characters(|characters| characters[co].hp[5]) as i32 * 650
                && npc_is_enemy(cn, cc)
            {
                if npc_try_spell(cn, cc, SK_BLAST) {
                    return 1;
                }
            }
            Repository::with_characters_mut(|characters| characters[cn].data[65] = 0);
        }
    }

    // generic fight-magic management
    {
        let co = Repository::with_characters(|characters| characters[cn].attack_cn as usize);
        let in_fight =
            co != 0 || Repository::with_characters(|characters| characters[cn].data[78]) != 0;
        if in_fight {
            if npc_quaff_potion(cn, 833, 254) {
                return 1;
            }
            if npc_quaff_potion(cn, 267, 254) {
                return 1;
            }

            if co != 0
                && (Repository::with_characters(|characters| characters[cn].a_hp)
                    < Repository::with_characters(|characters| characters[cn].hp[5]) as i32 * 600
                    || rand::thread_rng().gen_range(0..10) == 0)
            {
                if npc_try_spell(cn, co, SK_BLAST) {
                    return 1;
                }
            }

            if co != 0
                && Repository::with_globals(|g| g.ticker)
                    > Repository::with_characters(|characters| characters[cn].data[75])
            {
                if npc_try_spell(cn, co, SK_STUN) {
                    Repository::with_characters_mut(|characters| {
                        characters[cn].data[75] =
                            Repository::with_characters(|chars| chars[cn].skill[SK_STUN][5]) as i32
                                + 18 * 8
                    });
                    return 1;
                }
            }

            if Repository::with_characters(|characters| characters[cn].a_mana) > 75000
                && npc_try_spell(cn, cn, SK_BLESS)
            {
                return 1;
            }
            if npc_try_spell(cn, cn, SK_PROTECT) {
                return 1;
            }
            if npc_try_spell(cn, cn, SK_MSHIELD) {
                return 1;
            }
            if npc_try_spell(cn, cn, SK_ENHANCE) {
                return 1;
            }
            if npc_try_spell(cn, cn, SK_BLESS) {
                return 1;
            }
            if co != 0 && npc_try_spell(cn, co, SK_CURSE) {
                return 1;
            }
            if co != 0
                && Repository::with_globals(|g| g.ticker)
                    > Repository::with_characters(|characters| characters[cn].data[74])
                        + (TICKS * 10)
                && npc_try_spell(cn, co, SK_GHOST)
            {
                Repository::with_characters_mut(|characters| {
                    characters[cn].data[74] = Repository::with_globals(|g| g.ticker)
                });
                return 1;
            }

            if co != 0
                && Repository::with_characters(|characters| characters[co].armor) + 5
                    > Repository::with_characters(|characters| characters[cn].weapon)
            {
                if npc_try_spell(cn, co, SK_BLAST) {
                    return 1;
                }
            }
        }
    }

    // did we panic?
    if Repository::with_characters(|characters| characters[cn].data[78]) != 0
        && Repository::with_characters(|characters| characters[cn].attack_cn) == 0
        && Repository::with_characters(|characters| characters[cn].goto_x) == 0
    {
        let (x, y) = Repository::with_characters(|characters| (characters[cn].x, characters[cn].y));
        let rx = rand::thread_rng().gen_range(0..10) as i32;
        let ry = rand::thread_rng().gen_range(0..10) as i32;
        Repository::with_characters_mut(|characters| {
            characters[cn].goto_x = (x as i32 + 5 - rx) as u16;
            characters[cn].goto_y = (y as i32 + 5 - ry) as u16;
        });
        return 1;
    }

    // are we on protect and want to follow our master?
    {
        let co = Repository::with_characters(|characters| characters[cn].data[69] as usize);
        if Repository::with_characters(|characters| characters[cn].attack_cn) == 0 && co != 0 {
            if driver::follow_driver(cn, co) {
                Repository::with_characters_mut(|characters| characters[cn].data[58] = 2);
                return 1;
            }
        }
    }

    // don't scan if we don't use the information
    if Repository::with_characters(|characters| characters[cn].data[41]) == 0
        && Repository::with_characters(|characters| characters[cn].data[47]) == 0
    {
        return 0;
    }

    // save some work
    if Repository::with_characters(|characters| characters[cn].data[41]) != 0
        && Repository::with_characters(|characters| characters[cn].misc_action) == DR_USE as u16
    {
        return 0;
    }
    if Repository::with_characters(|characters| characters[cn].data[47]) != 0
        && Repository::with_characters(|characters| characters[cn].misc_action) == DR_PICKUP as u16
    {
        return 0;
    }
    if Repository::with_characters(|characters| characters[cn].data[47]) != 0
        && Repository::with_characters(|characters| characters[cn].misc_action) == DR_USE as u16
    {
        return 0;
    }

    // scan nearby map for items of interest
    let ch_pos = Repository::with_characters(|characters| (characters[cn].x, characters[cn].y));
    // indoor detection
    let indoor1 = Repository::with_map(|map| {
        let idx = ch_pos.0 as usize + ch_pos.1 as usize * SERVER_MAPX as usize;
        (map[idx].flags & MF_INDOORS as u64) != 0
    });
    let min_y = std::cmp::max(ch_pos.1 as i32 - 8, 1) as usize;
    let max_y = std::cmp::min(ch_pos.1 as i32 + 8, SERVER_MAPY - 1) as usize;
    let min_x = std::cmp::max(ch_pos.0 as i32 - 8, 1) as usize;
    let max_x = std::cmp::min(ch_pos.0 as i32 + 8, SERVER_MAPX - 1) as usize;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let m = x + y * SERVER_MAPX as usize;
            let map_it = Repository::with_map(|map| map[m].it as usize);
            if map_it == 0 {
                continue;
            }

            let indoor2 = Repository::with_map(|map| (map[m].flags & MF_INDOORS as u64) != 0);
            let it_temp = Repository::with_items(|items| items[map_it].temp as i32);

            if it_temp == Repository::with_characters(|characters| characters[cn].data[41]) {
                // check active and light conditions - TODO: check actual map light/dlight
                let active = Repository::with_items(|items| items[map_it].active);
                if active == 0 {
                    Repository::with_characters_mut(|characters| {
                        characters[cn].misc_action = DR_USE as u16;
                        characters[cn].misc_target1 = x as u16;
                        characters[cn].misc_target2 = y as u16;
                        characters[cn].goto_x = 0u16;
                        characters[cn].data[58] = 1;
                    });
                    return 1;
                }
                // TODO: handle case when active and dlight > 200 and !indoor2
            }

            if Repository::with_characters(|characters| characters[cn].data[47]) != 0
                && indoor1 == indoor2
            {
                let flags = Repository::with_items(|items| items[map_it].flags);
                if flags & ItemFlags::IF_TAKE.bits() != 0 {
                    // TODO: check can_go and do_char_can_see_item
                    Repository::with_characters_mut(|characters| {
                        characters[cn].misc_action = DR_PICKUP as u16;
                        characters[cn].misc_target1 = x as u16;
                        characters[cn].misc_target2 = y as u16;
                        characters[cn].goto_x = 0u16;
                        characters[cn].data[58] = 1;
                    });
                    return 1;
                }
                if Repository::with_items(|items| items[map_it].driver) == 7 {
                    let map_idx = m;
                    if player::plr_check_target(map_idx) {
                        Repository::with_characters_mut(|characters| {
                            characters[cn].misc_action = DR_PICKUP as u16;
                            characters[cn].misc_target1 = x as u16;
                            characters[cn].misc_target2 = y as u16;
                            characters[cn].goto_x = 0u16;
                            characters[cn].data[58] = 1;
                        });
                        return 1;
                    }
                }
            }
        }
    }

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
            1 => driver::npc_stunrun_low(cn),
            2 => driver::npc_cityattack_low(cn),
            3 => driver::npc_malte_low(cn),
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
            || (character_flags & CharacterFlags::IsLooting.bits()) != 0)
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

    if data_55 != 0 && data_55 + (TICKS * 120) > ticker && data_54 != 0 {
        let m = data_54;
        Repository::with_characters_mut(|characters| {
            characters[cn].goto_x = (m % SERVER_MAPX) as u16 + get_frust_x_off(ticker) as u16;
            characters[cn].goto_y = (m / SERVER_MAPX) as u16 + get_frust_y_off(ticker) as u16;
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

    if data_77 != 0 && data_77 + (TICKS * 30) > ticker {
        let m = data_76;
        Repository::with_characters_mut(|characters| {
            characters[cn].goto_x = ((m % SERVER_MAPX) + get_frust_x_off(data_36)) as u16;
            characters[cn].goto_y = ((m / SERVER_MAPX) + get_frust_y_off(data_36)) as u16;
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

        if m != 0 && m < (SERVER_MAPX * SERVER_MAPY) {
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

        if !(10..=18).contains(&n) {
            n = 10;
            Repository::with_characters_mut(|characters| {
                characters[cn].data[19] = n;
            });
        }

        let data_57 = Repository::with_characters(|characters| characters[cn].data[57]);
        if data_57 > ticker {
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

        let x = (m % SERVER_MAPX) + get_frust_x_off(data_36);
        let y = (m / SERVER_MAPX) + get_frust_y_off(data_36);

        if data_36 > 20 || ((ch_x as i32 - x).abs() + (ch_y as i32 - y).abs()) < 4 {
            if data_36 <= 20 && data_79 != 0 {
                Repository::with_characters_mut(|characters| {
                    characters[cn].data[57] = ticker + data_79;
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

        let data_61 = Repository::with_characters(|characters| characters[cn].data[61]);
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
                // TODO: Call RANDOM function (doesn't exist yet, use placeholder)
                x = ch_x as i32 - 5 + (ticker % 11); // RANDOM(11)
                y = ch_y as i32 - 5 + ((ticker / 11) % 11); // RANDOM(11)

                if !(1..SERVER_MAPX).contains(&x) || !(1..=SERVER_MAPX).contains(&y) {
                    panic = attempt + 1;
                    continue;
                }

                if data_73 != 0 {
                    // Too far away from origin?
                    let xo = data_29 % SERVER_MAPX;
                    let yo = data_29 / SERVER_MAPX;

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

                if State::with_mut(|state| state.can_go(ch_x as i32, ch_y as i32, x, y)) == 0 {
                    panic = attempt + 1;
                    continue;
                }

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
        let x = (m % SERVER_MAPX) + get_frust_x_off(data_36);
        let y = (m / SERVER_MAPX) + get_frust_y_off(data_36);

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

                // Turn toward an adjacent tile based on desired direction.
                // (misc_target1/misc_target2 are coordinates, not the direction value.)
                let mut target_x = x;
                let mut target_y = y;

                match data_30 {
                    d if d == DX_UP as i32 => target_y -= 1,
                    d if d == DX_DOWN as i32 => target_y += 1,
                    d if d == DX_LEFT as i32 => target_x -= 1,
                    d if d == DX_RIGHT as i32 => target_x += 1,
                    d if d == DX_LEFTUP as i32 => {
                        target_x -= 1;
                        target_y -= 1;
                    }
                    d if d == DX_LEFTDOWN as i32 => {
                        target_x -= 1;
                        target_y += 1;
                    }
                    d if d == DX_RIGHTUP as i32 => {
                        target_x += 1;
                        target_y -= 1;
                    }
                    d if d == DX_RIGHTDOWN as i32 => {
                        target_x += 1;
                        target_y += 1;
                    }
                    _ => {
                        characters[cn].misc_action = DR_IDLE as u16;
                        return;
                    }
                }

                if !(0..SERVER_MAPX).contains(&target_x) || !(0..SERVER_MAPY).contains(&target_y) {
                    characters[cn].misc_action = core::constants::DR_IDLE as u16;
                    return;
                }

                characters[cn].misc_target1 = target_x as u16;
                characters[cn].misc_target2 = target_y as u16;
            });
            return;
        }
    }

    // Reset talked-to list
    let data_67 = Repository::with_characters(|characters| characters[cn].data[67]);
    if data_67 + (TICKS * 60 * 5) < ticker {
        let data_37 = Repository::with_characters(|characters| characters[cn].data[37]);
        if data_37 != 0 {
            Repository::with_characters_mut(|characters| {
                for n in 37..41 {
                    characters[cn].data[n] = 1; // Hope we never have a character nr 1!
                }
            });
        }
        Repository::with_characters_mut(|characters| {
            characters[cn].data[67] = ticker;
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
            if (flags & (CharacterFlags::Body.bits() | CharacterFlags::Respawn.bits())) != 0 {
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
                    let co = match populate::pop_create_char(503 + m_idx, false) {
                        Some(co) => co,
                        None => {
                            State::with(|state| {
                                state.do_sayx(cn, &format!("create char ({})", m_idx));
                            });
                            break;
                        }
                    };

                    if !God::drop_char_fuzzy(co, 452, 345) {
                        State::with(|state| {
                            state.do_sayx(cn, &format!("drop char ({})", m_idx));
                        });
                        God::destroy_items(co);
                        Repository::with_characters_mut(|characters| {
                            characters[co].used = 0;
                        });
                        break;
                    }

                    Repository::with_characters(|ch| {
                        EffectManager::fx_add_effect(6, 0, ch[co].x as i32, ch[co].y as i32, 0);
                    });
                }

                Repository::with_characters(|ch| {
                    EffectManager::fx_add_effect(7, 0, ch[cn].x as i32, ch[cn].y as i32, 0);
                });
                State::with(|state| {
                    state.do_sayx(cn, "Khuzak gurawin duskar!");
                });

                Repository::with_characters_mut(|characters| {
                    characters[cn].a_mana -= (n * 100 * 1000) as i32;
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
            // TODO: Do a deeper dive into what this is doing -- originally
            // the C code has it.attrib here which is clearly wrong since attrib
            // only has 5 entries.
            score += it.skill[n][0] as i32 * 5;
        }

        score += (it.value / 10) as i32;
        score += it.weapon[0] as i32 * 50;
        score += it.armor[0] as i32 * 50;
        score -= it.damage_state as i32;

        score
    })
}

pub fn npc_want_item(cn: usize, in_idx: usize) -> bool {
    let item_38 = Repository::with_characters(|characters| characters[cn].item[38]);

    if item_38 != 0 {
        return false; // hack: don't take more stuff if inventory is almost full
    }

    let citem = Repository::with_characters(|characters| characters[cn].citem);

    if citem != 0 {
        Repository::with_items(|items| {
            log::info!("have {} in citem", items[in_idx].get_name());
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
    let citem = Repository::with_characters(|characters| characters[cn].citem);

    if citem != 0 {
        Repository::with_items(|items| {
            log::info!("have {} in citem", items[in_idx].get_name());
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

    for n in 0..20 {
        let worn_n = Repository::with_characters(|characters| characters[cn].worn[n]);

        if worn_n == 0 || npc_item_value(in_idx) > npc_item_value(worn_n as usize) {
            if npc_check_placement(in_idx, n) {
                if npc_can_wear_item(cn, in_idx) {
                    Repository::with_items(|items| {
                        log::info!("now wearing {}", items[in_idx].get_name());
                    });

                    // Remove old item if any
                    if worn_n != 0 {
                        log::info!("storing item");
                        Repository::with_characters_mut(|characters| {
                            characters[cn].citem = worn_n;
                        });

                        let do_store_item = State::with(|state| state.do_store_item(cn));
                        if do_store_item == -1 {
                            return false; // Stop looting if our backpack is full
                        }
                    }

                    Repository::with_characters_mut(|characters| {
                        characters[cn].worn[n] = in_idx as u32;
                        characters[cn].set_do_update_flags();
                    });
                    Repository::with_items_mut(|items| {
                        items[in_idx].carried = cn as u16;
                    });

                    return true;
                }
            }
        }
    }

    false
}

pub fn npc_loot_grave(cn: usize, in_idx: usize) -> bool {
    let (ch_x, ch_y, ch_dir) = Repository::with_characters(|characters| {
        (characters[cn].x, characters[cn].y, characters[cn].dir)
    });

    let (it_x, it_y) = Repository::with_items(|items| (items[in_idx].x, items[in_idx].y));

    // Check if we're adjacent and facing the grave
    if ((ch_x as i32 - it_x as i32).abs() + (ch_y as i32 - it_y as i32).abs()) > 1
        || helpers::drv_dcoor2dir(it_x as i32 - ch_x as i32, it_y as i32 - ch_y as i32)
            != ch_dir as i32
    {
        Repository::with_characters_mut(|characters| {
            characters[cn].misc_action = DR_USE as u16;
            characters[cn].misc_target1 = it_x;
            characters[cn].misc_target2 = it_y;
        });
        return true;
    }

    let co = Repository::with_items(|items| items[in_idx].data[0]) as usize;

    // Try to loot worn items
    for n in 0..20 {
        let worn_item = Repository::with_characters(|characters| characters[co].worn[n]);

        if worn_item != 0 {
            let in_item = worn_item as usize;
            if npc_equip_item(cn, in_item) {
                let (item_name, co_name) = Repository::with_items(|items| {
                    Repository::with_characters(|characters| {
                        (
                            items[in_item].get_name().to_string(),
                            characters[co].get_name().to_string(),
                        )
                    })
                });
                log::info!("got {} from {}'s grave", item_name, co_name);
                Repository::with_characters_mut(|characters| {
                    characters[co].worn[n] = 0;
                });
                return true;
            }
        }
    }

    // Try to loot inventory items
    for n in 0..40 {
        let inv_item = Repository::with_characters(|characters| characters[co].item[n]);

        if inv_item != 0 {
            let in_item = inv_item as usize;

            if npc_equip_item(cn, in_item) {
                let (item_name, co_name) = Repository::with_items(|items| {
                    Repository::with_characters(|characters| {
                        (
                            items[in_item].get_name().to_string(),
                            characters[co].get_name().to_string(),
                        )
                    })
                });
                log::info!("got {} from {}'s grave", item_name, co_name);
                Repository::with_characters_mut(|characters| {
                    characters[co].item[n] = 0;
                });
                return true;
            }

            if npc_want_item(cn, in_item) {
                let (item_name, co_name) = Repository::with_items(|items| {
                    Repository::with_characters(|characters| {
                        (
                            items[in_item].get_name().to_string(),
                            characters[co].get_name().to_string(),
                        )
                    })
                });
                log::info!("got {} from {}'s grave", item_name, co_name);
                Repository::with_characters_mut(|characters| {
                    characters[co].item[n] = 0;
                });
                return true;
            }
        }
    }

    // Try to loot gold
    let co_gold = Repository::with_characters(|characters| characters[co].gold);
    if co_gold != 0 {
        let co_name =
            Repository::with_characters(|characters| characters[co].get_name().to_string());
        log::info!(
            "got {:.2}G from {}'s grave",
            co_gold as f32 / 100.0,
            co_name
        );
        Repository::with_characters_mut(|characters| {
            characters[cn].gold += co_gold;
            characters[co].gold = 0;
        });
        return true;
    }

    false
}

pub fn npc_already_searched_grave(cn: usize, in_idx: usize) -> bool {
    Repository::with_characters(|characters| {
        let text_9 = &characters[cn].text[9];

        // Search through text[9] in 4-byte (sizeof(int)) chunks
        let mut n = 0;
        while n < 160 {
            // Read 4 bytes as an i32 (little-endian)
            if n + 4 <= text_9.len() {
                let value =
                    i32::from_le_bytes([text_9[n], text_9[n + 1], text_9[n + 2], text_9[n + 3]]);

                if value == in_idx as i32 {
                    return true;
                }
            }
            n += std::mem::size_of::<i32>();
        }

        false
    })
}

pub fn npc_add_searched_grave(cn: usize, in_idx: usize) {
    Repository::with_characters_mut(|characters| {
        let int_size = std::mem::size_of::<i32>();
        let text_9_len = characters[cn].text[9].len();

        // Shift existing data right by sizeof(int) bytes
        // memmove(dest + sizeof(int), src, len - sizeof(int))
        if text_9_len > int_size {
            characters[cn].text[9].copy_within(0..(text_9_len - int_size), int_size);
        }

        // Write the new grave index at the start
        let bytes = (in_idx as i32).to_le_bytes();
        if text_9_len >= int_size {
            characters[cn].text[9][0..int_size].copy_from_slice(&bytes);
        }
    });
}

pub fn npc_grave_logic(cn: usize) -> bool {
    let (ch_x, ch_y) =
        Repository::with_characters(|characters| (characters[cn].x, characters[cn].y));

    // Scan area around NPC (within 8 tiles)
    let min_y = std::cmp::max(ch_y as i32 - 8, 1);
    let max_y = std::cmp::min(ch_y as i32 + 8, SERVER_MAPY - 1);
    let min_x = std::cmp::max(ch_x as i32 - 8, 1);
    let max_x = std::cmp::min(ch_x as i32 + 8, SERVER_MAPX - 1);

    for y in min_y..max_y {
        for x in min_x..max_x {
            let map_idx = (x + y * SERVER_MAPX) as usize;
            let in_idx = Repository::with_map(|map| map[map_idx].it);

            if in_idx != 0 {
                let in_idx = in_idx as usize;

                // Check if it's a grave (temp == 170)
                let is_grave = Repository::with_items(|items| items[in_idx].temp == 170);

                if is_grave {
                    let (it_x, it_y) =
                        Repository::with_items(|items| (items[in_idx].x, items[in_idx].y));

                    // Check if we can reach the grave and haven't searched it yet
                    let can_reach = State::with_mut(|state| {
                        state.can_go(ch_x as i32, ch_y as i32, it_x as i32, it_y as i32) != 0
                    });

                    let can_see =
                        State::with_mut(|state| state.do_char_can_see_item(cn, in_idx)) != 0;

                    if can_reach && can_see && !npc_already_searched_grave(cn, in_idx) {
                        if !npc_loot_grave(cn, in_idx) {
                            // Grave is empty, mark as searched
                            npc_add_searched_grave(cn, in_idx);
                            Repository::with_characters_mut(|characters| {
                                characters[cn].flags &= !CharacterFlags::IsLooting.bits();
                            });
                        } else {
                            // Still looting
                            Repository::with_characters_mut(|characters| {
                                characters[cn].flags |= CharacterFlags::IsLooting.bits();
                            });
                        }
                        return true;
                    }
                }
            }
        }
    }

    false
}

// ****************************************************
// Shop Functions
// ****************************************************

pub fn update_shop(cn: usize) {
    let mut sale = [0i32; 10];

    // Copy shop inventory template from data[0..9]
    Repository::with_characters(|characters| {
        let data_copy = characters[cn].data;
        sale.copy_from_slice(&data_copy[0..10]);
    });

    // Check if we have free space (at least 10 slots)
    State::with(|state| {
        state.do_sort(cn, "v");
    });

    let mut m = 0; // Free slots
    let mut x = 0; // Last non-sale item position

    for n in 0..40 {
        let in_idx = Repository::with_characters(|characters| characters[cn].item[n]);

        if in_idx == 0 {
            m += 1;
        } else {
            let temp = Repository::with_items(|items| items[in_idx as usize].temp);

            // Check if this item is part of our shop inventory
            let mut found = false;
            for z in 0..10 {
                if temp == sale[z] as u16 {
                    sale[z] = 0; // Mark as found
                    found = true;
                    break;
                }
            }

            if !found {
                x = n; // This is not a sale item
            }
        }
    }

    // If we have less than 2 free slots, remove a non-sale item
    if m < 2 {
        let in_idx = Repository::with_characters(|characters| characters[cn].item[x]);

        if in_idx != 0 {
            let flags = Repository::with_items(|items| items[in_idx as usize].flags);

            // TODO: Add RANDOM function call
            // For now, use a simple check
            if (flags & ItemFlags::IF_DONATE.bits()) != 0 {
                // Call god_donate_item (doesn't exist yet)
                God::donate_item(in_idx as usize, 0);
                Repository::with_items_mut(|items| {
                    items[in_idx as usize].used = USE_EMPTY;
                });
            } else {
                Repository::with_items_mut(|items| {
                    items[in_idx as usize].used = USE_EMPTY;
                });
            }

            Repository::with_characters_mut(|characters| {
                characters[cn].item[x] = 0;
            });
        }
    }

    // Check if our store is complete - create missing items
    for n in 0..10 {
        let temp = sale[n];
        if temp == 0 {
            continue;
        }

        let in_idx = God::create_item(temp as usize);

        if in_idx.is_some() {
            // Call god_give_char
            if !God::give_character_item(cn, in_idx.unwrap()) {
                Repository::with_items_mut(|items| {
                    items[in_idx.unwrap()].used = USE_EMPTY;
                });
            }
        }
    }

    // Small-repair all items (reset damage and age)
    // Junk all items needing serious repair
    for n in 0..40 {
        let in_idx = Repository::with_characters(|characters| characters[cn].item[n]);

        if in_idx != 0 {
            let (damage_state, flags) = Repository::with_items(|items| {
                (
                    items[in_idx as usize].damage_state,
                    items[in_idx as usize].flags,
                )
            });

            if damage_state != 0 || (flags & ItemFlags::IF_SHOPDESTROY.bits()) != 0 {
                // Item needs serious repair or should be destroyed
                Repository::with_items_mut(|items| {
                    items[in_idx as usize].used = USE_EMPTY;
                });
                Repository::with_characters_mut(|characters| {
                    characters[cn].item[n] = 0;
                });
            } else {
                // Small repair - reset current damage and age
                Repository::with_items_mut(|items| {
                    items[in_idx as usize].current_damage = 0;
                    items[in_idx as usize].current_age[0] = 0;
                    items[in_idx as usize].current_age[1] = 0;
                });
            }
        }
    }

    State::with(|state| {
        state.do_sort(cn, "v");
    });
}

// ****************************************************
// Special Functions
// ****************************************************

pub fn shiva_activate_candle(cn: usize, in_idx: usize) -> i32 {
    let (mdtime, mdday) = Repository::with_globals(|globals| (globals.mdtime, globals.mdday));

    // Only allow during night time (mdtime <= 2000)
    if mdtime > 2000 {
        return 0;
    }

    // Check if character can create another candle (cooldown check)
    let data_0 = Repository::with_characters(|characters| characters[cn].data[0]);
    if data_0 >= mdday {
        return 0;
    }

    log::info!(
        "Created new candle, time={}, day={}, last day={}",
        mdtime,
        mdday,
        data_0
    );

    // Set cooldown: can create another candle in 9 days
    Repository::with_characters_mut(|characters| {
        characters[cn].data[0] = mdday + 9;
    });

    // Deactivate the candle item
    Repository::with_items_mut(|items| {
        items[in_idx].active = 0;
    });

    // Update lighting if the candle provides light
    let (light_0, light_1, it_x, it_y) = Repository::with_items(|items| {
        (
            items[in_idx].light[0],
            items[in_idx].light[1],
            items[in_idx].x,
            items[in_idx].y,
        )
    });

    if light_0 != light_1 && it_x > 0 {
        State::with_mut(|state| {
            state.do_add_light(it_x as i32, it_y as i32, light_0 as i32 - light_1 as i32);
        });
    }

    // Add visual effects
    EffectManager::fx_add_effect(6, 0, it_x as i32, it_y as i32, 0);

    Repository::with_characters(|ch| {
        EffectManager::fx_add_effect(7, 0, ch[cn].x as i32, ch[cn].y as i32, 0);
    });

    // Character says the magic words
    State::with(|state| {
        state.do_sayx(cn, "Shirak ishagur gorweran dulak!");
    });

    // Consume mana
    Repository::with_characters_mut(|characters| {
        characters[cn].a_mana -= 800 * 1000;
    });

    1
}

// ****************************************************
// Helper Functions for npc_see
// ****************************************************

pub fn is_unique(in_idx: usize) -> bool {
    const UNIQUE_TEMPS: [u16; 60] = [
        280, 281, 282, 283, 284, 285, 286, 287, 288, 289, 290, 291, 292, 525, 526, 527, 528, 529,
        530, 531, 532, 533, 534, 535, 536, 537, 538, 539, 540, 541, 542, 543, 544, 545, 546, 547,
        548, 549, 550, 551, 552, 553, 554, 555, 556, 572, 573, 574, 575, 576, 577, 578, 579, 580,
        581, 582, 583, 584, 585, 586,
    ];

    Repository::with_items(|items| {
        let temp = items[in_idx].temp;
        UNIQUE_TEMPS.contains(&temp)
    })
}

pub fn count_uniques(cn: usize) -> i32 {
    let mut cnt = 0;

    // Check citem
    let citem = Repository::with_characters(|characters| characters[cn].citem);
    if citem != 0 && (citem & 0x80000000) == 0 && is_unique(citem as usize) {
        cnt += 1;
    }

    // Check inventory items
    for n in 0..40 {
        let in_idx = Repository::with_characters(|characters| characters[cn].item[n]);
        if in_idx != 0 && is_unique(in_idx as usize) {
            cnt += 1;
        }
    }

    // Check worn items
    for n in 0..20 {
        let in_idx = Repository::with_characters(|characters| characters[cn].worn[n]);
        if in_idx != 0 && is_unique(in_idx as usize) {
            cnt += 1;
        }
    }

    // Check depot items
    for n in 0..62 {
        let in_idx = Repository::with_characters(|characters| characters[cn].depot[n]);
        if in_idx != 0 && is_unique(in_idx as usize) {
            cnt += 1;
        }
    }

    cnt
}

pub fn npc_cityguard_see(cn: usize, co: usize, flag: i32) -> i32 {
    let co_group = Repository::with_characters(|characters| characters[co].data[42]);

    // Check if enemy is from group 27 (monsters)
    if co_group == 27 {
        let ticker = Repository::with_globals(|globals| globals.ticker);
        let (data_55, data_52, ch_x, ch_y) = Repository::with_characters(|characters| {
            (
                characters[cn].data[55],
                characters[cn].data[52],
                characters[cn].x,
                characters[cn].y,
            )
        });

        // Shout every 180 seconds
        if data_55 + (TICKS * 180) < ticker {
            Repository::with_characters_mut(|characters| {
                characters[cn].data[54] = 0;
                characters[cn].data[55] = ticker;
            });

            let co_name =
                Repository::with_characters(|characters| characters[co].get_name().to_string());

            // Say text and shout
            npc_saytext_n(cn, 4, Some(&co_name));
            State::with(|state| {
                state.do_npc_shout(
                    cn,
                    NT_SHOUT as i32,
                    cn as i32,
                    data_52,
                    ch_x as i32,
                    ch_y as i32,
                );
            });

            // Shout for players too
            for n in 1..MAXCHARS {
                let (is_player, used, no_shout) = Repository::with_characters(|characters| {
                    if n >= characters.len() {
                        return (false, USE_EMPTY, true);
                    }
                    (
                        (characters[n].flags
                            & (CharacterFlags::Player.bits() | CharacterFlags::Usurp.bits()))
                            != 0,
                        characters[n].used,
                        (characters[n].flags & CharacterFlags::NoShout.bits()) != 0,
                    )
                });

                if is_player && used == USE_ACTIVE && !no_shout {
                    let message = if flag != 0 {
                        "Cityguard: \"The monsters are approaching the city! Alert!\""
                    } else {
                        "Cityguard: \"The monsters are approaching the outpost! Alert!\""
                    };
                    log::info!("[char {}] {}", n, message);
                }
            }
        }
    }

    0
}

// ****************************************************
// NPC See Function
// ****************************************************

pub fn npc_see(cn: usize, co: usize) -> i32 {
    let ticker = Repository::with_globals(|globals| globals.ticker);

    // Update no-sleep bonus if target is player
    let co_flags = Repository::with_characters(|characters| characters[co].flags);
    if (co_flags & (CharacterFlags::Player.bits() | CharacterFlags::Usurp.bits())) != 0 {
        Repository::with_characters_mut(|characters| {
            characters[cn].data[92] = TICKS * 60;
        });
    } else {
        // For non-player targets we only refresh the NPC's awake timer if
        // its action group is currently active (matching the original logic).
        if Repository::with_characters(|characters| characters[cn].group_active()) {
            Repository::with_characters_mut(|characters| {
                characters[cn].data[92] = TICKS * 60;
            });
        }
    }

    // Check if we can see the character
    let can_see = State::with_mut(|state| state.do_char_can_see(cn, co));
    if can_see == 0 {
        return 1; // Processed: we cannot see them, so ignore
    }

    // Check for Ghost Companion seeing their master
    let (temp, data_63) =
        Repository::with_characters(|characters| (characters[cn].temp, characters[cn].data[63]));

    if temp == CT_COMPANION as u16 && co == data_63 as usize {
        // Happy to see master, reset timeout
        Repository::with_characters_mut(|characters| {
            characters[cn].data[98] = ticker + COMPANION_TIMEOUT;
        });
    }

    // Special sub driver
    let data_26 = Repository::with_characters(|characters| characters[cn].data[26]);
    if data_26 != 0 {
        let ret = match data_26 {
            1 => npc_cityguard_see(cn, co, 0),
            3 => npc_cityguard_see(cn, co, 1),
            _ => 0,
        };
        if ret != 0 {
            return 1;
        }
    }

    // Check indoor status
    let (cn_x, cn_y, co_x, co_y) = Repository::with_characters(|characters| {
        (
            characters[cn].x,
            characters[cn].y,
            characters[co].x,
            characters[co].y,
        )
    });

    let indoor1 = Repository::with_map(|map| {
        let idx = cn_x as usize + cn_y as usize * SERVER_MAPX as usize;
        (map[idx].flags & MF_INDOORS as u64) != 0
    });

    let indoor2 = Repository::with_map(|map| {
        let idx = co_x as usize + co_y as usize * SERVER_MAPX as usize;
        (map[idx].flags & MF_INDOORS as u64) != 0
    });

    // Check if this is an enemy we added to our list earlier
    let attack_cn = Repository::with_characters(|characters| characters[cn].attack_cn);
    if attack_cn == 0 {
        // Only attack if we aren't fighting already
        let co_id = helpers::char_id(co) as u32;
        let idx = co as i32 | ((co_id as i32) << 16);

        let mut found = false;
        for n in 80..92 {
            let data_n = Repository::with_characters(|characters| characters[cn].data[n]);
            if data_n == idx {
                found = true;
                break;
            }
        }

        if found {
            Repository::with_characters_mut(|characters| {
                characters[cn].attack_cn = co as u16;
                characters[cn].goto_x = 0; // Cancel goto (patrol)
                characters[cn].data[58] = 2;
            });
            return 1;
        }
    }

    // Check if we need to attack by group
    let data_43 = Repository::with_characters(|characters| characters[cn].data[43]);
    if data_43 != 0 {
        let co_group = Repository::with_characters(|characters| characters[co].data[42]);
        let co_temp = Repository::with_characters(|characters| characters[co].temp);

        let mut found = false;
        for n in 43..47 {
            let data_n = Repository::with_characters(|characters| characters[cn].data[n]);
            if data_n != 0 && co_group == data_n {
                found = true;
                break;
            }
            if data_n == 65536
                && ((co_flags & CharacterFlags::Player.bits()) != 0
                    || co_temp == CT_COMPANION as u16)
            {
                found = true;
                break;
            }
        }

        if !found {
            let mut should_attack = true;

            // Check attack distance
            let (data_95, data_93, data_29) = Repository::with_characters(|characters| {
                (
                    characters[cn].data[95],
                    characters[cn].data[93],
                    characters[cn].data[29],
                )
            });

            if data_95 == 2 && data_93 != 0 {
                let rest_x = (data_29 % SERVER_MAPX) as i16;
                let rest_y = (data_29 / SERVER_MAPX) as i16;
                let dist =
                    std::cmp::max((rest_x - co_x).abs() as i32, (rest_y - co_y).abs() as i32);

                if dist > data_93 {
                    should_attack = false;
                }
            }

            if should_attack && npc_add_enemy(cn, co, false) {
                let co_name =
                    Repository::with_characters(|characters| characters[co].get_name().to_string());
                npc_saytext_n(cn, 1, Some(&co_name));
                log::info!(
                    "Added {} to kill list because he's not in my group",
                    co_name
                );
                return 1;
            }
        }
    }

    // Attack with warning
    let (data_95, data_93, data_27, data_29, data_94) = Repository::with_characters(|characters| {
        (
            characters[cn].data[95],
            characters[cn].data[93],
            characters[cn].data[27],
            characters[cn].data[29],
            characters[cn].data[94],
        )
    });

    if data_95 == 1
        && (co_flags & CharacterFlags::Player.bits()) != 0
        && ticker > data_27 + (TICKS * 120)
    {
        let x1 = co_x as i32;
        let x2 = data_29 % SERVER_MAPX;
        let y1 = co_y as i32;
        let y2 = data_29 / SERVER_MAPX;
        let dist = (x1 - x2).abs() + (y1 - y2).abs();

        if dist <= data_93 {
            if npc_add_enemy(cn, co, false) {
                let co_name =
                    Repository::with_characters(|characters| characters[co].get_name().to_string());
                npc_saytext_n(cn, 1, Some(&co_name));
                log::info!(
                    "Added {} to kill list because he didn't say the password",
                    co_name
                );
                return 1;
            }
        } else if dist <= data_93 * 2 && data_94 + (TICKS * 15) < ticker {
            npc_saytext_n(cn, 8, None);
            Repository::with_characters_mut(|characters| {
                characters[cn].data[94] = ticker;
            });
            return 1;
        }
    }

    // Check if we need to talk to them
    let (attack_cn, data_37, data_56) = Repository::with_characters(|characters| {
        (
            characters[cn].attack_cn,
            characters[cn].data[37],
            characters[cn].data[56],
        )
    });

    if attack_cn == 0
        && (co_flags & CharacterFlags::Player.bits()) != 0
        && data_37 != 0
        && indoor1 == indoor2
        && data_56 < ticker
    {
        // Check if we've already talked to this character
        let mut already_talked = false;
        for n in 37..41 {
            let data_n = Repository::with_characters(|characters| characters[cn].data[n]);
            if data_n == co as i32 {
                already_talked = true;
                break;
            }
        }

        if !already_talked {
            let text_2 = Repository::with_characters(|characters| characters[cn].text[2]);
            let text_2_str = c_string_to_str(&text_2).to_string();
            let co_name =
                Repository::with_characters(|characters| characters[co].get_name().to_string());

            let (co_kindred, co_skill_19) = Repository::with_characters(|characters| {
                (characters[co].kindred as u32, characters[co].skill[19][0])
            });

            // Special greeting logic
            if text_2_str == "#stunspec\0" || text_2_str.starts_with("#stunspec") {
                let message = if (co_kindred & (KIN_TEMPLAR | KIN_ARCHTEMPLAR)) != 0
                    || ((co_kindred & KIN_SEYAN_DU) != 0 && co_skill_19 != 0)
                {
                    format!("Hello, {}. I'll teach you Immunity, if you bring me the potion from the Skeleton Lord.", co_name)
                } else {
                    format!(
                        "Hello, {}. I'll teach you Stun, if you bring me the potion from the Skeleton Lord.",
                        co_name
                    )
                };
                State::with(|state| {
                    state.do_sayx(cn, &message);
                });
            } else if text_2_str == "#cursespec\0" || text_2_str.starts_with("#cursespec") {
                let message = if (co_kindred & (KIN_TEMPLAR | KIN_ARCHTEMPLAR)) != 0
                    || ((co_kindred & KIN_SEYAN_DU) != 0 && co_skill_19 != 0)
                {
                    format!(
                        "Hi, {}. Bring me a Potion of Life and I'll teach you Surround Hit.",
                        co_name
                    )
                } else {
                    format!(
                        "Hi, {}. Bring me a Potion of Life and I'll teach you Curse.",
                        co_name
                    )
                };
                State::with(|state| {
                    state.do_sayx(cn, &message);
                });
            } else {
                // Check if this is a priest (temp 180) greeting a PURPLE player
                let cn_temp = Repository::with_characters(|characters| characters[cn].temp);
                if cn_temp == 180 && (co_kindred & KIN_PURPLE) != 0 {
                    State::with(|state| {
                        state.do_sayx(cn, &format!("Greetings, {}!", co_name));
                    });
                } else {
                    // Normal greeting
                    npc_saytext_n(cn, 2, Some(&co_name));
                }
            }

            // Update talked-to list (FIFO queue)
            Repository::with_characters_mut(|characters| {
                characters[cn].data[40] = characters[cn].data[39];
                characters[cn].data[39] = characters[cn].data[38];
                characters[cn].data[38] = characters[cn].data[37];
                characters[cn].data[37] = co as i32;
                characters[cn].data[56] = ticker + (TICKS * 30);
            });

            // Special proc for unique warning
            let data_26 = Repository::with_characters(|characters| characters[cn].data[26]);
            if data_26 == 5 {
                let cnt = count_uniques(co);

                if cnt == 1 {
                    State::with(|state| {
                        state.do_sayx(
                            cn,
                            &format!(
                                "I see you have a sword dedicated to the gods. Make good use of it, {}.\n",
                                co_name
                            ),
                        );
                    });
                } else if cnt > 1 {
                    State::with(|state| {
                        state.do_sayx(
                            cn,
                            &format!(
                                "I see you have several swords dedicated to the gods. They will get angry if you keep more than one, {}.\n",
                                co_name
                            ),
                        );
                    });
                }
            }
        }
    }

    0
}
