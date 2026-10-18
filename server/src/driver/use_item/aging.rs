use crate::game_state::GameState;
use crate::helpers::{self};
use core::constants::{
    CharacterFlags, ItemFlags, SERVER_MAPX, SERVER_MAPY, TICKS, USE_EMPTY, WN_LHAND, WN_RHAND,
};
use core::string_operations::c_string_to_str;
use core::types::FontColor;

/// Handles the legacy `item_age` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `item_age` hook.
///
/// # Returns
///
/// * `true` when `item_age` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `item_age` is outside the corresponding game-state collection.
pub fn item_age(gs: &mut GameState, item_idx: usize) -> bool {
    let (current_age_act, max_age_act, current_damage, max_damage) = {
        let act = if gs.items[item_idx].active != 0 { 1 } else { 0 };
        (
            gs.items[item_idx].current_age[act],
            gs.items[item_idx].max_age[act],
            gs.items[item_idx].current_damage,
            gs.items[item_idx].max_damage,
        )
    };

    if (max_age_act != 0 && current_age_act > max_age_act)
        || (max_damage != 0 && current_damage > max_damage)
    {
        gs.items[item_idx].flags |= core::constants::ItemFlags::IF_UPDATE.bits();
        gs.items[item_idx].current_damage = 0;
        gs.items[item_idx].current_age[0] = 0;
        gs.items[item_idx].current_age[1] = 0;
        gs.items[item_idx].damage_state += 1;
        gs.items[item_idx].value /= 2;

        if gs.items[item_idx].damage_state > 1 {
            let st = std::cmp::max(0, 4 - i32::from(gs.items[item_idx].damage_state));

            if gs.items[item_idx].armor[0] > st as i8 {
                gs.items[item_idx].armor[0] -= 1;
            }
            if gs.items[item_idx].armor[1] > st as i8 {
                gs.items[item_idx].armor[1] -= 1;
            }

            if gs.items[item_idx].weapon[0] > st as i8 * 2 {
                gs.items[item_idx].weapon[0] -= 1;
                if gs.items[item_idx].weapon[0] > 0 {
                    gs.items[item_idx].weapon[0] -= 1;
                }
            }
            if gs.items[item_idx].weapon[1] > st as i8 * 2 {
                gs.items[item_idx].weapon[1] -= 1;
                if gs.items[item_idx].weapon[1] > 0 {
                    gs.items[item_idx].weapon[1] -= 1;
                }
            }
        }

        if gs.items[item_idx].max_age[0] != 0 {
            gs.items[item_idx].sprite[0] += 1;
        }
        if gs.items[item_idx].max_age[1] != 0 {
            gs.items[item_idx].sprite[1] += 1;
        }

        return true;
    }

    // Expire no-age items after 30 minutes (lag scrolls after 2 minutes)
    if max_age_act == 0 {
        let is_lag_scroll = gs.items[item_idx].temp == 500;
        let expire_time = if is_lag_scroll {
            TICKS * 60 * 2
        } else {
            TICKS * 60 * 30
        };

        if current_age_act > expire_time as u32 {
            gs.items[item_idx].damage_state = 5;
            return true;
        }
    }

    false
}

/// Handles the legacy `item_damage_worn` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `n` - Value passed through to the legacy `item_damage_worn` hook.
/// * `damage` - Value passed through to the legacy `item_damage_worn` hook.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `item_damage_worn` is outside the corresponding game-state collection.
pub fn item_damage_worn(gs: &mut GameState, cn: usize, n: usize, damage: i32) {
    let worn_idx = gs.characters[cn].worn[n] as usize;
    if worn_idx == 0 {
        return;
    }

    let has_max_damage = gs.items[worn_idx].max_damage != 0;
    if !has_max_damage {
        return;
    }

    // Callers derive `damage` from a combat roll that can go negative when a
    // large to-hit penalty outweighs the weapon value. The C original let the
    // unsigned accumulator wrap; clamp instead so a negative swing simply
    // inflicts no wear, and saturate so repeated huge values cannot overflow.
    gs.items[worn_idx].current_damage = gs.items[worn_idx]
        .current_damage
        .saturating_add(damage.max(0) as u32);

    if item_age(gs, worn_idx) {
        let (damage_state, reference) = {
            (
                gs.items[worn_idx].damage_state,
                c_string_to_str(&gs.items[worn_idx].reference).to_owned(),
            )
        };

        match damage_state {
            1 => {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("The {} you are using is showing signs of use.\n", reference),
                );
            }
            2 => {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("The {} you are using was slightly damaged.\n", reference),
                );
            }
            3 => {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("The {} you are using was damaged.\n", reference),
                );
            }
            4 => {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("The {} you are using was badly damaged.\n", reference),
                );
            }
            5 => {
                gs.characters[cn].worn[n] = 0;
                gs.items[worn_idx].used = USE_EMPTY;
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("The {} you were using was destroyed.\n", reference),
                );
            }
            _ => {}
        }
        gs.do_update_char(cn);
    }
}

/// Handles the legacy `item_damage_citem` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `damage` - Value passed through to the legacy `item_damage_citem` hook.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `item_damage_citem` is outside the corresponding game-state collection.
pub fn item_damage_citem(gs: &mut GameState, cn: usize, damage: i32) {
    let citem = gs.characters[cn].citem;
    if citem == 0 || (citem & 0x80000000) != 0 {
        return;
    }

    let citem_idx = citem as usize;
    let has_max_damage = gs.items[citem_idx].max_damage != 0;
    if !has_max_damage {
        return;
    }

    // See `item_damage_worn`: negative wear inflicts nothing and the
    // accumulator saturates rather than overflowing.
    gs.items[citem_idx].current_damage = gs.items[citem_idx]
        .current_damage
        .saturating_add(damage.max(0) as u32);

    if item_age(gs, citem_idx) {
        let (damage_state, reference) = {
            (
                gs.items[citem_idx].damage_state,
                c_string_to_str(&gs.items[citem_idx].reference).to_owned(),
            )
        };

        match damage_state {
            1 => {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("The {} you are using is showing signs of use.\n", reference),
                );
            }
            2 => {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("The {} you are using was slightly damaged.\n", reference),
                );
            }
            3 => {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("The {} you are using was damaged.\n", reference),
                );
            }
            4 => {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("The {} you are using was badly damaged.\n", reference),
                );
            }
            5 => {
                {
                    gs.characters[cn].citem = 0;
                };
                {
                    gs.items[citem_idx].used = USE_EMPTY;
                };
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("The {} you were using was destroyed.\n", reference),
                );
            }
            _ => {}
        }
    }
}

/// Handles the legacy `item_damage_armor` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `damage` - Value passed through to the legacy `item_damage_armor` hook.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `item_damage_armor` is outside the corresponding game-state collection.
pub fn item_damage_armor(gs: &mut GameState, cn: usize, damage: i32) {
    let dam = damage / 4 + 1;
    for n in 0..20 {
        if n != WN_RHAND && n != WN_LHAND && helpers::random_mod_i32(3) != 0 {
            item_damage_worn(gs, cn, n, dam);
        }
    }
}

/// Handles the legacy `item_damage_weapon` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `damage` - Value passed through to the legacy `item_damage_weapon` hook.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `item_damage_weapon` is outside the corresponding game-state collection.
pub fn item_damage_weapon(gs: &mut GameState, cn: usize, damage: i32) {
    let dam = damage / 4 + 1;
    item_damage_worn(gs, cn, WN_RHAND, dam);
}

/// Handles the legacy `lightage` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `lightage` hook.
/// * `multi` - Value passed through to the legacy `lightage` hook.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `lightage` is outside the corresponding game-state collection.
pub fn lightage(gs: &mut GameState, item_idx: usize, multi: i32) {
    // Read basic item info
    let (carried, it_x, it_y, active) = {
        (
            gs.items[item_idx].carried,
            i32::from(gs.items[item_idx].x),
            i32::from(gs.items[item_idx].y),
            gs.items[item_idx].active,
        )
    };

    // Determine map coordinates: if carried by a character, use that character's position
    let (mx, my) = if carried != 0 {
        let cn = carried as usize;
        if cn >= core::constants::MAXCHARS || gs.characters[cn].used == USE_EMPTY {
            (it_x, it_y)
        } else {
            (
                i32::from(gs.characters[cn].x),
                i32::from(gs.characters[cn].y),
            )
        }
    } else {
        (it_x, it_y)
    };

    // Validate coordinates
    if mx < 0 || my < 0 || mx >= SERVER_MAPX || my >= SERVER_MAPY {
        return;
    }

    let m = (mx + my * SERVER_MAPX) as usize;

    // Read map light
    let mut light = i32::from(gs.map[m].light);
    if light < 1 {
        return;
    }
    if light > 250 {
        light = 250;
    }

    light *= multi;

    let act = if active != 0 { 1usize } else { 0usize };

    gs.items[item_idx].current_age[act] =
        gs.items[item_idx].current_age[act].wrapping_add((light as u32) * 2);
}

/// Handles the legacy `age_message` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `age_message` hook.
/// * `where_is` - Value passed through to the legacy `age_message` hook.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `age_message` is outside the corresponding game-state collection.
pub fn age_message(gs: &mut GameState, cn: usize, item_idx: usize, where_is: &str) {
    let (driver, damage_state, reference) = {
        (
            gs.items[item_idx].driver,
            gs.items[item_idx].damage_state,
            c_string_to_str(&gs.items[item_idx].reference).to_owned(),
        )
    };

    let (msg, font) = if driver == 60 {
        // Ice egg or cloak
        match damage_state {
            1 => ("The {ref} {where} is beginning to melt.\n", FontColor::Red),
            2 => (
                "The {ref} {where} is melting fairly rapidly.\n",
                FontColor::Red,
            ),
            3 => (
                "The {ref} {where} is melting down as you look and dripping water everywhere.\n",
                FontColor::Red,
            ),
            4 => (
                "The {ref} {where} has melted down to a small icy lump and large puddles of water.\n",
                FontColor::Yellow,
            ),
            5 => (
                "The {ref} {where} has completely melted away, leaving you all wet.\n",
                FontColor::Yellow,
            ),
            _ => ("The {ref} {where} is changing.\n", FontColor::Red),
        }
    } else {
        // Anything else
        match damage_state {
            1 => (
                "The {ref} {where} is showing signs of age.\n",
                FontColor::Red,
            ),
            2 => ("The {ref} {where} is getting fairly old.\n", FontColor::Red),
            3 => ("The {ref} {where} is getting old.\n", FontColor::Red),
            4 => (
                "The {ref} {where} is getting very old and battered.\n",
                FontColor::Yellow,
            ),
            5 => (
                "The {ref} {where} was so old and battered that it finally vanished.\n",
                FontColor::Yellow,
            ),
            _ => ("The {ref} {where} is aging.\n", FontColor::Red),
        }
    };

    let formatted_msg = msg
        .replace("{ref}", &reference)
        .replace("{where}", where_is);

    gs.do_character_log(cn, font, &formatted_msg);
}

/// Handles the legacy `char_item_expire` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `char_item_expire` is outside the corresponding game-state collection.
pub fn char_item_expire(gs: &mut GameState, cn: usize) {
    if (gs.characters[cn].flags & CharacterFlags::BuildMode.bits()) != 0 {
        return;
    }

    let mut must_update = false;

    let current_ice_cloak_clock = gs.ice_cloak_clock;
    gs.ice_cloak_clock = current_ice_cloak_clock + 1;

    // Age items in backpack (40 slots)
    for n in 0..40 {
        let item_idx = gs.characters[cn].item[n] as usize;
        if item_idx == 0 {
            continue;
        }

        let (active, has_alwaysexp1, has_alwaysexp2, driver, has_lightage) = {
            let act = if gs.items[item_idx].active != 0 { 1 } else { 0 };
            (
                act,
                (gs.items[item_idx].flags & core::constants::ItemFlags::IF_ALWAYSEXP1.bits()) != 0,
                (gs.items[item_idx].flags & core::constants::ItemFlags::IF_ALWAYSEXP2.bits()) != 0,
                gs.items[item_idx].driver,
                (gs.items[item_idx].flags & core::constants::ItemFlags::IF_LIGHTAGE.bits()) != 0,
            )
        };

        let should_age = (active == 0 && has_alwaysexp1) || (active == 1 && has_alwaysexp2);
        if !should_age {
            continue;
        }

        // Ice cloak ages more slowly when not worn or held
        if driver == 60 && gs.ice_cloak_clock.is_multiple_of(4) {
            continue;
        }

        gs.items[item_idx].current_age[active] += 1;

        if has_lightage {
            lightage(gs, item_idx, 1);
        }

        if item_age(gs, item_idx) {
            must_update = true;
            age_message(gs, cn, item_idx, "in your backpack");

            let damage_state = gs.items[item_idx].damage_state;
            if damage_state == 5 {
                gs.characters[cn].item[n] = 0;
                gs.items[item_idx].used = USE_EMPTY;
            }
        }
    }

    // Age items in worn slots (20 slots)
    for n in 0..20 {
        let item_idx = gs.characters[cn].worn[n] as usize;
        if item_idx == 0 {
            continue;
        }

        let (active, has_alwaysexp1, has_alwaysexp2, has_lightage) = {
            let act = if gs.items[item_idx].active != 0 { 1 } else { 0 };
            (
                act,
                (gs.items[item_idx].flags & core::constants::ItemFlags::IF_ALWAYSEXP1.bits()) != 0,
                (gs.items[item_idx].flags & core::constants::ItemFlags::IF_ALWAYSEXP2.bits()) != 0,
                (gs.items[item_idx].flags & core::constants::ItemFlags::IF_LIGHTAGE.bits()) != 0,
            )
        };

        let should_age = (active == 0 && has_alwaysexp1) || (active == 1 && has_alwaysexp2);
        if !should_age {
            continue;
        }

        gs.items[item_idx].current_age[active] += 1;

        if has_lightage {
            lightage(gs, item_idx, 1);
        }

        if item_age(gs, item_idx) {
            must_update = true;
            let damage_state = gs.items[item_idx].damage_state;

            if damage_state == 5 {
                age_message(gs, cn, item_idx, "you were using");
                gs.characters[cn].worn[n] = 0;
                gs.items[item_idx].used = USE_EMPTY;
            } else {
                age_message(gs, cn, item_idx, "you are using");
            }
        }
    }

    // Age item under mouse cursor (citem)
    let citem = gs.characters[cn].citem;
    if citem != 0 && (citem & 0x80000000) == 0 {
        let item_idx = citem as usize;
        let (active, has_alwaysexp1, has_alwaysexp2, has_lightage) = {
            let act = if gs.items[item_idx].active != 0 { 1 } else { 0 };
            (
                act,
                (gs.items[item_idx].flags & ItemFlags::IF_ALWAYSEXP1.bits()) != 0,
                (gs.items[item_idx].flags & ItemFlags::IF_ALWAYSEXP2.bits()) != 0,
                (gs.items[item_idx].flags & ItemFlags::IF_LIGHTAGE.bits()) != 0,
            )
        };

        let should_age = (active == 0 && has_alwaysexp1) || (active == 1 && has_alwaysexp2);
        if should_age {
            gs.items[item_idx].current_age[active] += 1;

            if has_lightage {
                lightage(gs, item_idx, 1);
            }

            if item_age(gs, item_idx) {
                must_update = true;
                let damage_state = gs.items[item_idx].damage_state;

                if damage_state == 5 {
                    age_message(gs, cn, item_idx, "you were using");
                    gs.characters[cn].citem = 0;
                    gs.items[item_idx].used = USE_EMPTY;
                } else {
                    age_message(gs, cn, item_idx, "you are using");
                }
            }
        }
    }

    if must_update {
        gs.do_update_char(cn);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_state::GameState;
    use crate::test_helpers::{add_test_player, with_test_gs};
    use core::constants::{ItemFlags, USE_ACTIVE, WN_LHAND, WN_RHAND};

    /// Puts a damageable item in a worn slot and returns its index.
    fn wear_damageable_item(gs: &mut GameState, cn: usize, slot: usize) -> usize {
        let item_idx = 1;
        gs.items[item_idx] = core::types::Item::default();
        gs.items[item_idx].used = USE_ACTIVE;
        gs.items[item_idx].max_damage = 1_000;
        gs.items[item_idx].current_damage = 10;
        gs.items[item_idx].carried = cn as u16;
        gs.characters[cn].worn[slot] = item_idx as u32;
        item_idx
    }

    #[test]
    fn item_damage_worn_ignores_negative_wear() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = wear_damageable_item(gs, cn, WN_RHAND);

            // A combat roll can go negative when the to-hit penalty outweighs
            // the weapon value; that must not wrap the unsigned accumulator.
            item_damage_worn(gs, cn, WN_RHAND, -3);

            assert_eq!(gs.items[item_idx].current_damage, 10);
            assert_eq!(gs.characters[cn].worn[WN_RHAND], item_idx as u32);
        });
    }

    #[test]
    fn item_damage_worn_saturates_instead_of_overflowing() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = wear_damageable_item(gs, cn, WN_RHAND);
            gs.items[item_idx].current_damage = u32::MAX;
            gs.items[item_idx].max_damage = u32::MAX;

            item_damage_worn(gs, cn, WN_RHAND, i32::MAX);

            assert_eq!(gs.items[item_idx].current_damage, u32::MAX);
        });
    }

    #[test]
    fn item_damage_weapon_with_negative_damage_does_not_panic() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = wear_damageable_item(gs, cn, WN_RHAND);

            item_damage_weapon(gs, cn, -19);

            assert!(gs.items[item_idx].current_damage <= 11);
        });
    }

    #[test]
    fn item_damage_citem_ignores_negative_wear() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            gs.items[item_idx] = core::types::Item::default();
            gs.items[item_idx].used = USE_ACTIVE;
            gs.items[item_idx].max_damage = 1_000;
            gs.items[item_idx].current_damage = 10;
            gs.characters[cn].citem = item_idx as u32;

            item_damage_citem(gs, cn, -100);

            assert_eq!(gs.items[item_idx].current_damage, 10);
            assert_eq!(gs.characters[cn].citem, item_idx as u32);
        });
    }

    #[test]
    fn item_age_advances_damage_state_and_resets_counters() {
        with_test_gs(|gs| {
            let item_idx = 1;
            gs.items[item_idx] = core::types::Item::default();
            gs.items[item_idx].used = USE_ACTIVE;
            gs.items[item_idx].max_age[0] = 10;
            gs.items[item_idx].current_age[0] = 11;
            gs.items[item_idx].current_damage = 25;
            gs.items[item_idx].max_damage = 100;
            gs.items[item_idx].value = 80;
            gs.items[item_idx].sprite[0] = 4;

            assert!(item_age(gs, item_idx));

            assert_eq!(gs.items[item_idx].damage_state, 1);
            assert_eq!(gs.items[item_idx].current_age, [0, 0]);
            assert_eq!(gs.items[item_idx].current_damage, 0);
            assert_eq!(gs.items[item_idx].value, 40);
            assert_eq!(gs.items[item_idx].sprite[0], 5);
            assert_ne!(gs.items[item_idx].flags & ItemFlags::IF_UPDATE.bits(), 0);
        });
    }

    #[test]
    fn item_age_reduces_armor_and_weapon_after_second_damage_state() {
        with_test_gs(|gs| {
            let item_idx = 1;
            gs.items[item_idx] = core::types::Item::default();
            gs.items[item_idx].used = USE_ACTIVE;
            gs.items[item_idx].max_damage = 10;
            gs.items[item_idx].current_damage = 11;
            gs.items[item_idx].damage_state = 1;
            gs.items[item_idx].armor = [5, 4];
            gs.items[item_idx].weapon = [8, 7];
            gs.items[item_idx].value = 100;

            assert!(item_age(gs, item_idx));

            assert_eq!(gs.items[item_idx].damage_state, 2);
            assert_eq!(gs.items[item_idx].armor, [4, 3]);
            assert_eq!(gs.items[item_idx].weapon, [6, 5]);
            assert_eq!(gs.items[item_idx].value, 50);
        });
    }

    #[test]
    fn item_age_expires_no_age_items_after_timeout() {
        with_test_gs(|gs| {
            let item_idx = 1;
            gs.items[item_idx] = core::types::Item::default();
            gs.items[item_idx].used = USE_ACTIVE;
            gs.items[item_idx].current_age[0] = (TICKS * 60 * 30 + 1) as u32;

            assert!(item_age(gs, item_idx));
            assert_eq!(gs.items[item_idx].damage_state, 5);
        });
    }

    #[test]
    fn lightage_uses_carriers_position_and_clamps_bright_tiles() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            gs.items[item_idx] = core::types::Item::default();
            gs.items[item_idx].used = USE_ACTIVE;
            gs.items[item_idx].carried = cn as u16;
            gs.items[item_idx].active = 1;
            let map_idx =
                gs.characters[cn].x as usize + gs.characters[cn].y as usize * SERVER_MAPX as usize;
            gs.map[map_idx].light = 300;

            lightage(gs, item_idx, 2);

            assert_eq!(gs.items[item_idx].current_age[1], 1_000);
            assert_eq!(gs.items[item_idx].current_age[0], 0);
        });
    }

    #[test]
    fn char_item_expire_build_mode_leaves_items_and_clock_unchanged() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            gs.items[item_idx] = core::types::Item::default();
            gs.items[item_idx].used = USE_ACTIVE;
            gs.items[item_idx].flags = ItemFlags::IF_ALWAYSEXP1.bits();
            gs.characters[cn].item[0] = item_idx as u32;
            gs.characters[cn].flags |= CharacterFlags::BuildMode.bits();

            char_item_expire(gs, cn);

            assert_eq!(gs.ice_cloak_clock, 0);
            assert_eq!(gs.items[item_idx].current_age[0], 0);
            assert_eq!(gs.characters[cn].item[0], item_idx as u32);
        });
    }

    #[test]
    fn char_item_expire_removes_destroyed_backpack_item() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            gs.items[item_idx] = core::types::Item::default();
            gs.items[item_idx].used = USE_ACTIVE;
            gs.items[item_idx].flags = ItemFlags::IF_ALWAYSEXP1.bits();
            gs.items[item_idx].damage_state = 4;
            gs.items[item_idx].max_age[0] = 0;
            gs.items[item_idx].current_age[0] = (TICKS * 60 * 30 + 1) as u32;
            gs.characters[cn].item[0] = item_idx as u32;

            char_item_expire(gs, cn);

            assert_eq!(gs.items[item_idx].damage_state, 5);
            assert_eq!(gs.characters[cn].item[0], 0);
            assert_eq!(gs.items[item_idx].used, USE_EMPTY);
        });
    }

    #[test]
    fn item_damage_armor_never_damages_hand_slots() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let right = wear_damageable_item(gs, cn, WN_RHAND);
            let left = 2;
            gs.items[left] = gs.items[right];
            gs.characters[cn].worn[WN_LHAND] = left as u32;

            item_damage_armor(gs, cn, 1_000);

            assert_eq!(gs.items[right].current_damage, 10);
            assert_eq!(gs.items[left].current_damage, 10);
        });
    }
}
