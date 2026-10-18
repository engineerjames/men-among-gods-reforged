use crate::area;
use crate::game_state::GameState;
use crate::god::God;
use crate::helpers::{self};
use crate::populate;
use core::constants::{CharacterFlags, DX_RIGHT, MAXITEM, MAXSKILL, TICKS, USE_EMPTY};

/// Handles the legacy `boost_char` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `divi` - Value passed through to the legacy `boost_char` hook.
///
/// # Returns
///
/// * `true` when `boost_char` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `boost_char` is outside the corresponding game-state collection.
pub fn boost_char(gs: &mut GameState, cn: usize, divi: usize) -> bool {
    // Boost attributes
    for n in 0..5 {
        if i32::from(gs.characters[cn].attrib[n][0]) > divi as i32 {
            let boost =
                crate::helpers::random_mod(u32::from(gs.characters[cn].attrib[n][0]) / divi as u32)
                    as u16;
            gs.characters[cn].attrib[n][0] = gs.characters[cn].attrib[n][0].saturating_add(boost);
        }
    }

    // Boost skills
    for n in 0..MAXSKILL {
        if i32::from(gs.characters[cn].skill[n][0]) > divi as i32 {
            let boost =
                crate::helpers::random_mod(u32::from(gs.characters[cn].skill[n][0]) / divi as u32)
                    as u16;
            gs.characters[cn].skill[n][0] = gs.characters[cn].skill[n][0].saturating_add(boost);
        }
    }

    // Update name
    let old_name = gs.characters[cn].get_name();
    let new_name = format!("Strong {}", old_name);
    let new_name_bytes = new_name.as_bytes();
    let len = new_name_bytes.len().min(39);
    gs.characters[cn].name[..len].copy_from_slice(&new_name_bytes[..len]);
    gs.characters[cn].name[len..].fill(0);
    gs.characters[cn].reference = gs.characters[cn].name;

    // Create soulstone
    if let Some(in_idx) = God::create_item(gs, 1146) {
        let (exp, rank) = {
            let exp = gs.characters[cn].points_tot as u32 / 10
                + crate::helpers::random_mod(gs.characters[cn].points_tot as u32 / 20 + 1);
            let rank = core::ranks::points2rank(exp);
            (exp, rank)
        };

        let name = b"Soulstone";
        gs.items[in_idx].name[..name.len()].copy_from_slice(name);
        gs.items[in_idx].name[name.len()..].fill(0);

        let reference = b"soulstone";
        gs.items[in_idx].reference[..reference.len()].copy_from_slice(reference);
        gs.items[in_idx].reference[reference.len()..].fill(0);

        let description = format!("Level {} soulstone, holding {} exp.", rank, exp);
        let desc_bytes = description.as_bytes();
        let len = desc_bytes.len().min(gs.items[in_idx].description.len());
        gs.items[in_idx].description[..len].copy_from_slice(&desc_bytes[..len]);
        gs.items[in_idx].description[len..].fill(0);

        gs.items[in_idx].data[0] = rank;
        gs.items[in_idx].data[1] = exp;
        gs.items[in_idx].temp = 0;
        gs.items[in_idx].driver = 68;

        God::give_character_item(gs, cn, in_idx);
    }

    false
}

/// Handles the legacy `spawn_penta_enemy` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `spawn_penta_enemy` hook.
///
/// # Returns
///
/// * Value produced by the legacy `spawn_penta_enemy` implementation.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `spawn_penta_enemy` is outside the corresponding game-state collection.
pub fn spawn_penta_enemy(gs: &mut GameState, item_idx: usize) -> i32 {
    // Determine enemy type from data[9]
    let data9 = gs.items[item_idx].data[9];

    let mut tmp = if data9 == 10 {
        crate::helpers::random_mod(2) + 9
    } else if data9 == 11 {
        crate::helpers::random_mod(2) + 11
    } else if data9 == 17 {
        crate::helpers::random_mod(2) + 17
    } else if data9 == 18 {
        crate::helpers::random_mod(2) + 18
    } else if data9 == 21 {
        22
    } else if data9 == 22 {
        23
    } else if data9 == 23 {
        24
    } else {
        // [data9 - 1, data9 + 1]: e.g. data9 7 -> Master, First and Sergeant Major Grolm.
        (crate::helpers::random_mod(3) + data9).saturating_sub(1)
    };

    // Create appropriate character template
    let spawned = if tmp >= 22 {
        tmp -= 22;
        if tmp > 3 {
            tmp = 3;
        }
        populate::pop_create_char(gs, (1094 + tmp) as usize, false)
    } else if tmp > 17 {
        tmp -= 17;
        if tmp > 4 {
            tmp = 4;
        }
        populate::pop_create_char(gs, (538 + tmp) as usize, false)
    } else {
        populate::pop_create_char(gs, (364 + tmp) as usize, false)
    };

    let cn = match spawned {
        Some(cn) => cn,
        None => return 0,
    };

    // Configure character
    gs.characters[cn].flags &= !CharacterFlags::Respawn.bits();

    let (x, y) = (gs.items[item_idx].x, gs.items[item_idx].y);

    gs.characters[cn].data[0] = item_idx as i32;
    gs.characters[cn].data[29] = i32::from(x) + i32::from(y) * core::constants::SERVER_MAPX;
    gs.characters[cn].data[60] = TICKS * 60 * 2;
    gs.characters[cn].data[73] = 8;
    gs.characters[cn].dir = DX_RIGHT;

    // Randomly boost character (1 in 25 chance)
    if crate::helpers::random_mod(25) == 0 {
        boost_char(gs, cn, 5);
    }

    // Try to drop character
    if !God::drop_char_fuzzy(gs, cn, x as usize, y as usize) {
        God::destroy_items(gs, cn);
        gs.characters[cn].used = USE_EMPTY;
        return 0;
    }

    cn as i32
}

/// Handles the legacy `solved_pentagram` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `solved_pentagram` hook.
///
/// # Returns
///
/// * `true` when `solved_pentagram` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `solved_pentagram` is outside the corresponding game-state collection.
pub fn solved_pentagram(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    // Calculate bonus
    let bonus = {
        let data0 = gs.items[item_idx].data[0];
        (data0 * data0 * 3) / 7 + 1
    };

    // Add bonus to character's pending exp
    gs.characters[cn].data[18] += bonus as i32;
    gs.characters[cn].future3[0] = gs.characters[cn].future3[0].saturating_add(1);

    crate::player::commands::resend_completion_data_for_character(gs, cn);

    // Log to character
    gs.do_character_log(
        cn,
        core::types::FontColor::Green,
        &format!(
            "You solved the pentagram quest. Congratulations! You will get {} bonus experience points.\n",
            bonus
        ),
    );

    log::info!("Character {} solved pentagram quest", cn);

    let cn_name = gs.characters[cn].get_name().to_owned();
    let mut characters_in_pents: usize = 0;

    // Notify all players and award pending exp
    for n in 1..core::constants::MAXCHARS {
        if n >= gs.characters.len() {
            continue;
        }
        let (used, flags, active, has_bonus) = (
            gs.characters[n].used,
            gs.characters[n].flags,
            if gs.characters[n].used == core::constants::USE_ACTIVE {
                1
            } else {
                0
            },
            gs.characters[n].data[18],
        );

        if used == core::constants::USE_EMPTY {
            continue;
        }
        if (flags & (CharacterFlags::Player.bits() | CharacterFlags::Usurp.bits())) == 0 {
            continue;
        }

        // Notify other active players
        if active != 0 && n != cn {
            gs.do_character_log(
                n,
                core::types::FontColor::Green,
                &format!("{} solved the pentagram quest!\n", cn_name),
            );
        }

        // Award pending bonus exp
        if has_bonus != 0 {
            gs.do_give_exp(n, has_bonus, 0, -1);
            gs.characters[n].data[18] = 0;
        }

        if area::is_in_pentagram_quest(gs, n) {
            characters_in_pents += 1;
        }
    }

    // Activate all pentagram items (driver 33)
    for n in 1..gs.items.len() {
        if gs.items[n].used == core::constants::USE_EMPTY {
            continue;
        }
        if gs.items[n].driver != 33 {
            continue;
        }
        if gs.items[n].active == 0
            && gs.items[n].light[0] != gs.items[n].light[1]
            && gs.items[n].x > 0
        {
            gs.do_add_light(
                i32::from(gs.items[n].x),
                i32::from(gs.items[n].y),
                i32::from(gs.items[n].light[1]) - i32::from(gs.items[n].light[0]),
            );
        }
        gs.items[n].duration = 10 * 60 + crate::helpers::random_mod(20 * 60);
        gs.items[n].active = gs.items[n].duration;
    }

    let new_solve = {
        let state = gs;
        state.penta_needed = characters_in_pents * 5 + crate::helpers::random_mod_usize(6);

        // Ensure at least 5 are needed
        state.penta_needed.max(5)
    };

    log::info!(
        "Pentagram quest solved. Characters in pents: {}, new penta_needed: {}",
        characters_in_pents,
        new_solve
    );

    false
}

/// Handles the legacy `use_pentagram` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_pentagram` hook.
///
/// # Returns
///
/// * `true` when `use_pentagram` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_pentagram` is outside the corresponding game-state collection.
pub fn use_pentagram(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    // Check if already active
    let active = gs.items[item_idx].active;
    if active != 0 {
        if cn != 0 {
            gs.do_character_log(
                cn,
                core::types::FontColor::Green,
                "This pentagram is already active.\n",
            );
        } else {
            // Respawn enemies if needed
            for m in 1..4 {
                let needs_spawn = {
                    let co = gs.items[item_idx].data[m] as usize;

                    if co == 0 {
                        true
                    } else {
                        co >= gs.characters.len()
                            || gs.characters[co].used == USE_EMPTY
                            || gs.characters[co].data[0] != item_idx as i32
                            || (gs.characters[co].flags & CharacterFlags::Body.bits()) != 0
                    }
                };

                if needs_spawn {
                    let new_enemy = spawn_penta_enemy(gs, item_idx);
                    {
                        gs.items[item_idx].data[m] = new_enemy as u32;
                    };
                }
            }
        }
        return false;
    }

    if cn == 0 {
        return false;
    }

    // Check rank restriction
    let (r1, r2) = {
        let r1 = core::ranks::points2rank(gs.characters[cn].points_tot as u32) as i32;
        let r2 = {
            let mut r2 = gs.items[item_idx].data[9];
            if r2 < 5 {
                r2 += 5;
            } else if r2 < 7 {
                r2 += 6;
            } else if r2 < 9 {
                r2 += 7;
            } else if r2 < 11 {
                r2 += 8;
            } else {
                r2 += 9;
            }
            r2
        };
        (r1, r2)
    };

    if r1 as u32 > r2 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!(
                "You cannot use this pentagram. It is reserved for rank '{}' and below.\n",
                core::ranks::rank_name_by_index(r2 as usize)
            ),
        );
        return false;
    }

    // Activate pentagram
    let v = {
        let v = gs.items[item_idx].data[0];
        gs.items[item_idx].data[8] = cn as u32;
        gs.items[item_idx].duration = u32::MAX; // TODO: What should this be? Max int?
        v
    };

    let exp_points = (v * v) / 7 + 1;
    gs.do_character_log(
        cn,
        core::types::FontColor::Yellow,
        &format!(
            "You activated the pentagram with the value {}. It is worth {} experience point{}.\n",
            v,
            exp_points,
            if v == 1 { "" } else { "s" }
        ),
    );

    // Count active pentagrams and find top 5
    let mut tot = 0;
    let mut act = 0;
    let mut exp = 0;
    let mut b = [0usize; 5];
    let mut bv = [0i32; 5];

    for n in 1..MAXITEM {
        if n >= gs.items.len() {
            break;
        }
        let (item_used, item_driver, item_active, item_data8, item_data0) = (
            gs.items[n].used,
            gs.items[n].driver,
            gs.items[n].active,
            gs.items[n].data[8],
            gs.items[n].data[0],
        );

        if item_used == USE_EMPTY {
            continue;
        }
        if item_driver != 33 {
            continue;
        }
        tot += 1;
        if n != item_idx && item_active != u32::MAX {
            // TODO: This was -1 vs. 0 (now u32::MAX) before but I'm not sure how it worked? Need to re-evaluate...
            continue;
        }
        act += 1;
        if item_data8 as usize != cn {
            continue;
        }

        let v = item_data0;
        // Insert into sorted top 5 list
        if v > bv[0] as u32 {
            bv[4] = bv[3];
            bv[3] = bv[2];
            bv[2] = bv[1];
            bv[1] = bv[0];
            bv[0] = v as i32;
            b[4] = b[3];
            b[3] = b[2];
            b[2] = b[1];
            b[1] = b[0];
            b[0] = n;
        } else if v > bv[1] as u32 {
            bv[4] = bv[3];
            bv[3] = bv[2];
            bv[2] = bv[1];
            bv[1] = v as i32;
            b[4] = b[3];
            b[3] = b[2];
            b[2] = b[1];
            b[1] = n;
        } else if v > bv[2] as u32 {
            bv[4] = bv[3];
            bv[3] = bv[2];
            bv[2] = v as i32;
            b[4] = b[3];
            b[3] = b[2];
            b[2] = n;
        } else if v > bv[3] as u32 {
            bv[4] = bv[3];
            bv[3] = v as i32;
            b[4] = b[3];
            b[3] = n;
        } else if v > bv[4] as u32 {
            bv[4] = v as i32;
            b[4] = n;
        }
    }

    // Display top 5 pentagrams
    if b[0] != 0 {
        gs.do_character_log(cn, core::types::FontColor::Yellow, "You're holding:\n");
    }

    for n in 0..5 {
        if b[n] != 0 {
            let points = (bv[n] * bv[n]) / 7 + 1;
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!(
                    "Pentagram {:3}, worth {:5} point{}.\n",
                    bv[n],
                    points,
                    if bv[n] == 1 { "" } else { "s" }
                ),
            );
            exp += points;
        }
    }

    gs.characters[cn].data[18] = exp;

    gs.do_character_log(
        cn,
        core::types::FontColor::Yellow,
        &format!(
            "Your pentagrammas are worth a total of {} experience points. Note that only the highest 5 pentagrammas count towards your experience bonus.\n",
            exp
        ),
    );
    gs.do_character_log(
        cn,
        core::types::FontColor::Yellow,
        &format!(
            "There are {} pentagrammas total, of which {} are active.\n",
            tot, act
        ),
    );

    log::info!(
        "Character {} activated pentagram {} ({} of needed)",
        cn,
        v,
        act
    );

    // Check if quest solved
    let penta_needed = gs.penta_needed;
    if act >= penta_needed {
        solved_pentagram(gs, cn, item_idx);
        return false;
    }

    // Spawn enemies
    for m in 1..4 {
        let needs_spawn = {
            let co = gs.items[item_idx].data[m] as usize;

            if co == 0 {
                true
            } else {
                co >= gs.characters.len()
                    || gs.characters[co].used == USE_EMPTY
                    || gs.characters[co].data[0] != item_idx as i32
                    || (gs.characters[co].flags & CharacterFlags::Body.bits()) != 0
            }
        };

        if needs_spawn {
            let new_enemy = spawn_penta_enemy(gs, item_idx);
            {
                gs.items[item_idx].data[m] = new_enemy as u32;
            };
        }
    }

    true
}

/// Handles the legacy `use_blook_pentagram` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_blook_pentagram` hook.
///
/// # Returns
///
/// * `true` when `use_blook_pentagram` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_blook_pentagram` is outside the corresponding game-state collection.
pub fn use_blook_pentagram(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    if cn == 0 {
        return false;
    }

    gs.do_character_log(
        cn,
        core::types::FontColor::Green,
        "You try to wipe off the blood, but it seems to be coming back slowly.\n",
    );

    // Set blood state and update sprite
    gs.items[item_idx].data[0] = 1;
    gs.items[item_idx].sprite[0] =
        gs.items[item_idx].data[1] as i16 + gs.items[item_idx].data[0] as i16;

    true
}

/// Handles the legacy `pentagram` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `pentagram` hook.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `pentagram` is outside the corresponding game-state collection.
pub fn pentagram(gs: &mut GameState, item_idx: usize) {
    let active = gs.items[item_idx].active;
    if active != 0 {
        return;
    }

    if helpers::random_mod_i32(18) != 0 {
        return;
    }

    // Check data[1-3] for spawned enemies
    for n in 1..4 {
        let stored_cn = gs.items[item_idx].data[n];

        // Check if slot is empty or enemy is dead
        let should_spawn = if stored_cn == 0 {
            true
        } else {
            let cn = stored_cn as usize;

            gs.characters[cn].data[0] != item_idx as i32
                || gs.characters[cn].used == USE_EMPTY
                || (gs.characters[cn].flags & CharacterFlags::Body.bits()) != 0
        };

        if should_spawn {
            // Use the dedicated spawn helper which encapsulates template selection
            let new_cn = spawn_penta_enemy(gs, item_idx);
            if new_cn != 0 {
                gs.items[item_idx].data[n] = new_cn as u32;
            }
            break;
        }
    }
}

/// Handles the legacy `expire_blood_penta` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `expire_blood_penta` hook.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `expire_blood_penta` is outside the corresponding game-state collection.
pub fn expire_blood_penta(gs: &mut GameState, item_idx: usize) {
    let item = &mut gs.items[item_idx];
    if item.data[0] != 0 {
        item.data[0] += 1;
        if item.data[0] > 7 {
            item.data[0] = 0;
        }
        item.sprite[0] = item.data[1] as i16 + item.data[0] as i16;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{add_test_player, with_test_gs};
    use core::{
        constants::{USE_ACTIVE, USE_EMPTY},
        string_operations::{c_string_to_str, write_ascii_into_fixed},
    };

    /// Seed the item template slot needed by `God::create_item`.
    fn seed_template(gs: &mut GameState, template_id: usize) {
        gs.item_templates[template_id] = core::types::Item::default();
        gs.item_templates[template_id].used = USE_ACTIVE;
        write_ascii_into_fixed(
            &mut gs.item_templates[template_id].name,
            &format!("template {template_id}"),
        );
        write_ascii_into_fixed(
            &mut gs.item_templates[template_id].reference,
            &format!("template {template_id}"),
        );
    }

    /// Mark a runtime item as an active pentagram driver item.
    fn active_pentagram(gs: &mut GameState, item_idx: usize, value: u32) {
        gs.items[item_idx] = core::types::Item::default();
        gs.items[item_idx].used = USE_ACTIVE;
        gs.items[item_idx].driver = 33;
        gs.items[item_idx].data[0] = value;
        gs.items[item_idx].x = 20 + item_idx as u16;
        gs.items[item_idx].y = 20;
    }

    #[test]
    fn boost_char_prefixes_name_saturates_stats_and_creates_soulstone() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            seed_template(gs, 1146);
            gs.characters[cn].points_tot = 0;
            for attr in &mut gs.characters[cn].attrib {
                attr[0] = u16::MAX;
            }
            for skill in &mut gs.characters[cn].skill {
                skill[0] = u16::MAX;
            }

            assert!(!boost_char(gs, cn, 1));

            assert_eq!(c_string_to_str(&gs.characters[cn].name), "Strong Tester");
            assert!(
                gs.characters[cn]
                    .attrib
                    .iter()
                    .all(|attr| attr[0] == u16::MAX)
            );
            assert!(
                gs.characters[cn]
                    .skill
                    .iter()
                    .all(|skill| skill[0] == u16::MAX)
            );
            let soulstone = gs.characters[cn].item[0] as usize;
            assert_ne!(soulstone, 0);
            assert_eq!(gs.items[soulstone].used, USE_ACTIVE);
            assert_eq!(gs.items[soulstone].driver, 68);
            assert_eq!(gs.items[soulstone].data[0], 0);
            assert_eq!(gs.items[soulstone].data[1], 0);
        });
    }

    #[test]
    fn solved_pentagram_awards_formula_bonus_and_activates_pentagrams() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            active_pentagram(gs, 1, 7);
            active_pentagram(gs, 2, 3);
            gs.items[2].light = [0, 3];

            assert!(!solved_pentagram(gs, cn, 1));

            assert_eq!(gs.characters[cn].points_tot, 22);
            assert_eq!(gs.characters[cn].points, 22);
            assert_eq!(gs.characters[cn].data[18], 0);
            assert_eq!(gs.characters[cn].future3[0], 1);
            assert!(gs.items[1].active >= 10 * 60);
            assert!(gs.items[2].active >= 10 * 60);
        });
    }

    #[test]
    fn use_pentagram_active_item_returns_false_without_changing_owner() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            active_pentagram(gs, 1, 5);
            gs.items[1].active = 1;
            gs.items[1].data[8] = 0;

            assert!(!use_pentagram(gs, cn, 1));

            assert_eq!(gs.items[1].data[8], 0);
            assert_eq!(gs.characters[cn].data[18], 0);
        });
    }

    #[test]
    fn pentagram_active_item_does_not_spawn_enemies() {
        with_test_gs(|gs| {
            active_pentagram(gs, 1, 5);
            gs.items[1].active = 1;

            pentagram(gs, 1);

            assert_eq!(&gs.items[1].data[1..4], &[0, 0, 0]);
            assert!(gs.characters.iter().all(|ch| ch.used == USE_EMPTY));
        });
    }

    #[test]
    fn blood_pentagram_use_sets_blood_state_and_sprite() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.items[1] = core::types::Item::default();
            gs.items[1].used = USE_ACTIVE;
            gs.items[1].data[1] = 40;
            gs.items[1].sprite[0] = 40;

            assert!(use_blook_pentagram(gs, cn, 1));

            assert_eq!(gs.items[1].data[0], 1);
            assert_eq!(gs.items[1].sprite[0], 41);
        });
    }

    #[test]
    fn expire_blood_penta_advances_and_wraps_sprite_offset() {
        with_test_gs(|gs| {
            gs.items[1] = core::types::Item::default();
            gs.items[1].used = USE_ACTIVE;
            gs.items[1].data[0] = 6;
            gs.items[1].data[1] = 100;

            expire_blood_penta(gs, 1);
            assert_eq!(gs.items[1].data[0], 7);
            assert_eq!(gs.items[1].sprite[0], 107);

            expire_blood_penta(gs, 1);
            assert_eq!(gs.items[1].data[0], 0);
            assert_eq!(gs.items[1].sprite[0], 100);
        });
    }

    #[test]
    fn expire_blood_penta_ignores_clean_pentagram() {
        with_test_gs(|gs| {
            gs.items[1] = core::types::Item::default();
            gs.items[1].used = USE_ACTIVE;
            gs.items[1].data[0] = 0;
            gs.items[1].data[1] = 100;
            gs.items[1].sprite[0] = 77;

            expire_blood_penta(gs, 1);

            assert_eq!(gs.items[1].data[0], 0);
            assert_eq!(gs.items[1].sprite[0], 77);
        });
    }
}
