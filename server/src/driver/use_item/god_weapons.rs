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

    // Check if character has Skua's kindred (KIN_SKUA = 0x00000002)
    let has_skua_kindred = (gs.characters[cn].kindred & 0x00000002) != 0;

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

    // Check if character has Purple One's kindred (KIN_PURPLE = 0x00000001)
    let has_purple_kindred = (gs.characters[cn].kindred & 0x00000001) != 0;

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
