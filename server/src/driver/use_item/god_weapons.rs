use super::item_damage_worn;
use crate::driver;
use crate::effect::EffectManager;
use crate::game_state::GameState;
use core::constants::{CharacterFlags, WN_RHAND};

/// Handles the legacy `skua_protect` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `skua_protect` hook.
///
/// # Returns
///
/// * `true` when `skua_protect` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `skua_protect` is outside the corresponding game-state collection.
pub fn skua_protect(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    // Check if the weapon is wielded
    let is_wielded = gs.characters[cn].worn[WN_RHAND] == item_idx as u32;

    if !is_wielded {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "You cannot use Skua's weapon if you're not wielding it.\n",
        );
        return false;
    }

    // Skua's followers are everyone not sworn to the Purple One.
    let has_skua_kindred = (gs.characters[cn].kindred as u32 & core::traits::KIN_PURPLE) == 0;

    if !has_skua_kindred {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "How dare you to call on Skua to help you? Slave of the Purple One!\n",
        );
        gs.do_character_log(cn, core::types::FontColor::Green, "Your weapon vanished.\n");

        {
            gs.characters[cn].worn[WN_RHAND] = 0;
        };

        {
            gs.items[item_idx].used = core::constants::USE_EMPTY;
        };
    } else {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "You feel Skua's presence protect you.\n",
        );
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "He takes away His weapon and replaces it by a common one.\n",
        );

        driver::spell_from_item(gs, cn, item_idx);

        // Get replacement weapon template from data[2]
        let replacement_template = gs.items[item_idx].data[2] as usize;

        // Remove the Skua weapon
        {
            gs.items[item_idx].used = core::constants::USE_EMPTY;
        };

        // Create replacement weapon
        if let Some(new_weapon) = crate::god::God::create_item(gs, replacement_template) {
            {
                gs.items[new_weapon].carried = cn as u16;
                gs.items[new_weapon].flags |= core::constants::ItemFlags::IF_UPDATE.bits();
            };

            {
                gs.characters[cn].worn[WN_RHAND] = new_weapon as u32;
            };
        }
    }

    true
}

/// Handles the legacy `purple_protect` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `purple_protect` hook.
///
/// # Returns
///
/// * `true` when `purple_protect` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `purple_protect` is outside the corresponding game-state collection.
pub fn purple_protect(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    // Check if the weapon is wielded
    let is_wielded = { gs.characters[cn].worn[core::constants::WN_RHAND] == item_idx as u32 };

    if !is_wielded {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "You cannot use the Purple One's weapon if you're not wielding it.\n",
        );
        return false;
    }

    let has_purple_kindred = (gs.characters[cn].kindred as u32 & core::traits::KIN_PURPLE) != 0;

    if !has_purple_kindred {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "How dare you to call on the Purple One to help you? Slave of Skua!\n",
        );
        gs.do_character_log(cn, core::types::FontColor::Green, "Your weapon vanished.\n");

        {
            gs.characters[cn].worn[core::constants::WN_RHAND] = 0;
        };

        {
            gs.items[item_idx].used = core::constants::USE_EMPTY;
        };
    } else {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "You feel the Purple One's presence protect you.\n",
        );
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "He takes away His weapon and replaces it by a common one.\n",
        );

        driver::spell_from_item(gs, cn, item_idx);

        // Get replacement weapon template from data[2]
        let replacement_template = gs.items[item_idx].data[2] as usize;

        // Remove the Purple One's weapon
        {
            gs.items[item_idx].used = core::constants::USE_EMPTY;
        };

        // Create replacement weapon
        if let Some(new_weapon) = crate::god::God::create_item(gs, replacement_template) {
            {
                gs.items[new_weapon].carried = cn as u16;
                gs.items[new_weapon].flags |= core::constants::ItemFlags::IF_UPDATE.bits();
            };

            {
                gs.characters[cn].worn[core::constants::WN_RHAND] = new_weapon as u32;
            };
        }
    }

    true
}

/// Handles the legacy `use_kill_undead` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_kill_undead` hook.
///
/// # Returns
///
/// * `true` when `use_kill_undead` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_kill_undead` is outside the corresponding game-state collection.
pub fn use_kill_undead(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    if cn == 0 {
        return false;
    }

    // Check if wielding the item
    let is_wielded = { gs.characters[cn].worn[core::constants::WN_RHAND] as usize == item_idx };

    if !is_wielded {
        return false;
    }

    EffectManager::fx_add_effect(
        gs,
        7,
        0,
        i32::from(gs.characters[cn].x),
        i32::from(gs.characters[cn].y),
        0,
    );

    // Get character position
    let (ch_x, ch_y) = {
        (
            i32::from(gs.characters[cn].x),
            i32::from(gs.characters[cn].y),
        )
    };

    // Damage all undead in 8x8 area
    for y in (ch_y - 8)..(ch_y + 8) {
        if !(1..core::constants::SERVER_MAPY).contains(&y) {
            continue;
        }
        for x in (ch_x - 8)..(ch_x + 8) {
            if !(1..core::constants::SERVER_MAPX).contains(&x) {
                continue;
            }

            let co = { gs.map[(x + y * core::constants::SERVER_MAPX) as usize].ch as usize };

            if co != 0 {
                let is_undead = { (gs.characters[co].flags & CharacterFlags::Undead.bits()) != 0 };

                if is_undead {
                    gs.do_hurt(cn, co, 500, 2);
                    EffectManager::fx_add_effect(gs, 5, 0, x, y, 0);
                }
            }
        }
    }

    item_damage_worn(gs, cn, core::constants::WN_RHAND, 500);

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{add_test_player, with_test_gs};
    use core::constants::{ItemFlags, USE_ACTIVE, USE_EMPTY, WN_RHAND};
    use core::traits::KIN_PURPLE;

    /// Places a used item in the player's right-hand weapon slot.
    fn set_worn_weapon(gs: &mut GameState, cn: usize, item_idx: usize) {
        gs.items[item_idx].used = USE_ACTIVE;
        gs.items[item_idx].carried = cn as u16;
        gs.characters[cn].worn[WN_RHAND] = item_idx as u32;
    }

    /// Marks a template usable by `God::create_item` in protection tests.
    fn enable_template(gs: &mut GameState, template_id: usize) {
        gs.item_templates[template_id].used = USE_ACTIVE;
    }

    #[test]
    fn skua_protect_refuses_unwielded_weapon() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            gs.items[item_idx].used = USE_ACTIVE;
            gs.items[item_idx].carried = cn as u16;

            assert!(!skua_protect(gs, cn, item_idx));
            assert_eq!(gs.items[item_idx].used, USE_ACTIVE);
            assert_eq!(gs.characters[cn].worn[WN_RHAND], 0);
        });
    }

    #[test]
    fn skua_protect_removes_weapon_for_wrong_kindred() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            set_worn_weapon(gs, cn, item_idx);
            gs.characters[cn].kindred = KIN_PURPLE as i32;

            assert!(skua_protect(gs, cn, item_idx));
            assert_eq!(gs.characters[cn].worn[WN_RHAND], 0);
            assert_eq!(gs.items[item_idx].used, USE_EMPTY);
        });
    }

    #[test]
    fn skua_protect_replaces_weapon_for_non_purple_follower() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            let replacement_template = 77;
            set_worn_weapon(gs, cn, item_idx);
            gs.characters[cn].kindred = 0;
            gs.items[item_idx].data[2] = replacement_template as u32;
            enable_template(gs, 1);
            enable_template(gs, replacement_template);

            assert!(skua_protect(gs, cn, item_idx));
            let replacement = gs.characters[cn].worn[WN_RHAND] as usize;
            assert_ne!(replacement, 0);
            assert_eq!(gs.items[replacement].temp as usize, replacement_template);
            assert_eq!(gs.items[replacement].carried as usize, cn);
            assert_ne!(gs.items[replacement].flags & ItemFlags::IF_UPDATE.bits(), 0);
        });
    }

    #[test]
    fn purple_protect_refuses_unwielded_weapon() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            gs.items[item_idx].used = USE_ACTIVE;
            gs.items[item_idx].carried = cn as u16;

            assert!(!purple_protect(gs, cn, item_idx));
            assert_eq!(gs.items[item_idx].used, USE_ACTIVE);
            assert_eq!(gs.characters[cn].worn[WN_RHAND], 0);
        });
    }

    #[test]
    fn purple_protect_removes_weapon_for_wrong_kindred() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            set_worn_weapon(gs, cn, item_idx);
            gs.characters[cn].kindred = 0;

            assert!(purple_protect(gs, cn, item_idx));
            assert_eq!(gs.characters[cn].worn[WN_RHAND], 0);
            assert_eq!(gs.items[item_idx].used, USE_EMPTY);
        });
    }

    #[test]
    fn purple_protect_replaces_weapon_for_purple_follower() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            let replacement_template = 88;
            set_worn_weapon(gs, cn, item_idx);
            gs.characters[cn].kindred = KIN_PURPLE as i32;
            gs.items[item_idx].data[2] = replacement_template as u32;
            enable_template(gs, 1);
            enable_template(gs, replacement_template);

            assert!(purple_protect(gs, cn, item_idx));
            let replacement = gs.characters[cn].worn[WN_RHAND] as usize;
            assert_ne!(replacement, 0);
            assert_eq!(gs.items[replacement].temp as usize, replacement_template);
            assert_eq!(gs.items[replacement].carried as usize, cn);
            assert_ne!(gs.items[replacement].flags & ItemFlags::IF_UPDATE.bits(), 0);
        });
    }

    #[test]
    fn use_kill_undead_refuses_zero_character() {
        with_test_gs(|gs| {
            assert!(!use_kill_undead(gs, 0, 1));
        });
    }

    #[test]
    fn use_kill_undead_refuses_unwielded_weapon() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            gs.items[item_idx].used = USE_ACTIVE;

            assert!(!use_kill_undead(gs, cn, item_idx));
        });
    }
}
