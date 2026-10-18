use crate::driver;
use crate::game_state::GameState;
use crate::god::God;
use core::constants::{
    CharacterFlags, IT_FLASK, IT_HEALING_POTION, IT_MANA_POTION, IT_PURPLE_FLOWER, IT_RED_FLOWER,
    ItemFlags, POTION_TEMPLATE_IDS, USE_EMPTY,
};
use core::string_operations::c_string_to_str;

mod aging;
mod crafting;
mod doors;
mod god_weapons;
mod mining;
mod pentagram;
mod scrolls;
mod shrines;
mod spawners;
mod step;
mod teleport;
mod tick;

pub use aging::*;
pub use crafting::*;
pub use doors::*;
pub use god_weapons::*;
pub use mining::*;
pub use pentagram::*;
pub use scrolls::*;
pub use shrines::*;
pub use spawners::*;
pub use step::*;
pub use teleport::*;
pub use tick::*;

// Helper function to take an item from a character
pub(super) fn take_item_from_char(gs: &mut GameState, item_idx: usize, cn: usize) {
    {
        let ch = &mut gs.characters[cn];

        if ch.citem as usize == item_idx {
            ch.citem = 0;
        } else {
            for n in 0..40 {
                if ch.item[n] as usize == item_idx {
                    ch.item[n] = 0;
                    break;
                }
            }

            for n in 0..20 {
                if ch.worn[n] as usize == item_idx {
                    ch.worn[n] = 0;
                    break;
                }
            }
        }
    }

    gs.items[item_idx].x = 0;
    gs.items[item_idx].y = 0;
    gs.items[item_idx].carried = 0;
    gs.do_update_char(cn);
}

/// Replaces a consumed potion with an empty flask while preserving its slot.
///
/// # Arguments
///
/// * `gs` - Active game state containing the consumed potion.
/// * `cn` - Character carrying the consumed potion.
/// * `item_idx` - Item index of the consumed potion.
///
/// # Returns
///
/// * `true` when the item was a known potion and was replaced, otherwise `false`.
fn replace_consumed_potion_with_flask(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    if !POTION_TEMPLATE_IDS.contains(&(gs.items[item_idx].temp as usize)) {
        return false;
    }

    let mut flask = gs.item_templates[IT_FLASK];
    flask.temp = IT_FLASK as u16;
    flask.carried = cn as u16;
    flask.x = 0;
    flask.y = 0;
    flask.flags |= ItemFlags::IF_UPDATE.bits();
    gs.items[item_idx] = flask;
    true
}

/// Handles the legacy `use_driver` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_driver` hook.
/// * `carried` - Value passed through to the legacy `use_driver` hook.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_driver` is outside the corresponding game-state collection.
pub fn use_driver(gs: &mut GameState, cn: usize, item_idx: usize, carried: bool) {
    if item_idx == 0 || cn >= 10000 {
        return;
    }

    // Check if character is in build mode
    if cn != 0 {
        let in_build_mode = { (gs.characters[cn].flags & CharacterFlags::BuildMode.bits()) != 0 };
        if in_build_mode {
            return;
        }
    }

    // Default to failed action for non-carried use; will be updated on success
    if cn != 0 && !carried {
        gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
    }

    let has_use_flag =
        { (gs.items[item_idx].flags & core::constants::ItemFlags::IF_USE.bits()) != 0 };

    if !has_use_flag && cn != 0 {
        return;
    }

    // Soulstone on the cursor, clicking a potion/flask/flower: run the soulstone
    // logic with roles swapped so both click orders behave the same.
    if cn != 0 {
        let citem_idx = gs.characters[cn].citem as usize;
        if citem_idx != 0
            && (citem_idx & 0x8000_0000) == 0
            && gs.items[citem_idx].driver == 68
            && gs.items[item_idx].driver != 68
            && carried
            && matches!(
                gs.items[item_idx].temp as usize,
                IT_FLASK | IT_HEALING_POTION | IT_MANA_POTION | IT_RED_FLOWER | IT_PURPLE_FLOWER
            )
        {
            // Swap slots: soulstone into the clicked slot, clicked item onto the cursor.
            if let Some(slot) = (0..40).find(|&n| gs.characters[cn].item[n] as usize == item_idx) {
                gs.characters[cn].item[slot] = citem_idx as u32;
                gs.characters[cn].citem = item_idx as u32;
                if use_soulstone(gs, cn, citem_idx) {
                    gs.do_update_char(cn);
                    return;
                }
                gs.characters[cn].item[slot] = item_idx as u32;
                gs.characters[cn].citem = citem_idx as u32;
            }
        }
    }

    // If the character holds a flask (driver 4 + IF_USESPECIAL) in citem and is
    // using a potion ingredient, redirect to use_mix_potion with roles swapped so
    // that "flask on flower" and "flower on flask" both produce a potion.
    if cn != 0 {
        let citem_idx = gs.characters[cn].citem as usize;
        if citem_idx != 0
            && (citem_idx & 0x8000_0000) == 0
            && (gs.items[citem_idx].flags & core::constants::ItemFlags::IF_USESPECIAL.bits()) != 0
            && gs.items[citem_idx].driver == 4
            && gs.items[item_idx].driver != 4
            && gs.items[item_idx].driver != 68
        {
            log::info!(
                "Redirecting to use_mix_potion with item_idx={} and citem_idx={}",
                item_idx,
                citem_idx
            );
            // Temporarily point citem at the ingredient (item_idx) so that
            // use_mix_potion reads: item_idx=flask (base), citem=flower (ingredient).
            gs.characters[cn].citem = item_idx as u32;
            if use_mix_potion(gs, cn, citem_idx) {
                return;
            }
            // Restore on failure so normal driver dispatch can proceed.
            gs.characters[cn].citem = citem_idx as u32;
        }
    }

    // Check if tile is occupied (for non-carried items)
    if !carried {
        let (it_x, it_y) = (
            i32::from(gs.items[item_idx].x),
            i32::from(gs.items[item_idx].y),
        );
        if it_x > 0 {
            let m = (it_x + it_y * core::constants::SERVER_MAPX) as usize;
            let occupied = gs.map[m].ch != 0 || gs.map[m].to_ch != 0;
            if occupied {
                return;
            }
        }
    }

    let has_usespecial =
        { (gs.items[item_idx].flags & core::constants::ItemFlags::IF_USESPECIAL.bits()) != 0 };

    if has_usespecial {
        let driver = gs.items[item_idx].driver;
        let ret = match driver {
            1 => use_create_item(gs, cn, item_idx),
            2 => use_door(gs, cn, item_idx),
            3 => {
                // Lock-pick - special message
                if cn != 0 {
                    gs.do_character_log(
                        cn,
                        core::types::FontColor::Red,
                        "You use cannot the lock-pick directly. Hold it under your mouse cursor and click on the door...\n",
                    );
                }
                false
            }
            4 => use_mix_potion(gs, cn, item_idx),
            5 => stone_sword(gs, cn, item_idx),
            6 => teleport(gs, cn, item_idx),
            7 => use_bag(gs, cn, item_idx),
            8 => use_scroll(gs, cn, item_idx),
            9 => use_crystal(gs, cn, item_idx),
            10 => use_scroll2(gs, cn, item_idx),
            11 => use_scroll3(gs, cn, item_idx),
            12 => use_scroll4(gs, cn, item_idx),
            13 => use_scroll5(gs, cn, item_idx),
            14 => use_chain(gs, cn, item_idx),
            15 => use_labyrinth(gs, cn, item_idx),
            16 => use_ladder(gs, cn, item_idx),
            17 => rat_eye(gs, cn, item_idx),
            18 => skua_protect(gs, cn, item_idx),
            19 => use_lever(gs, cn, item_idx),
            20 => use_door(gs, cn, item_idx),
            21 => use_spawn(gs, cn, item_idx),
            22 => use_pile(gs, cn, item_idx),
            23 => teleport2(gs, cn, item_idx),
            24 => build_ring(gs, cn, item_idx),
            25 => use_mine(gs, cn, item_idx),
            26 => use_mine_fast(gs, cn, item_idx),
            27 => use_mine_respawn(gs, cn, item_idx),
            28 => use_gargoyle(gs, cn, item_idx),
            29 => use_grave(gs, cn, item_idx),
            30 => use_create_item2(gs, cn, item_idx),
            31 => false, // empty, hole water
            32 => build_amulet(gs, cn, item_idx),
            33 => use_pentagram(gs, cn, item_idx),
            34 => use_seyan_shrine(gs, cn, item_idx),
            35 => use_seyan_door(gs, cn, item_idx),
            36 => false, // magic portal 1 in lab13
            37 => false, // traps
            38 => false, // magic portal 2 in lab13
            39 => purple_protect(gs, cn, item_idx),
            40 => false, // seyan'du sword
            41 => use_shrine(gs, cn, item_idx),
            42 => use_create_item3(gs, cn, item_idx),
            43 => false, // spiderweb
            44 => use_kill_undead(gs, cn, item_idx),
            45 => use_seyan_portal(gs, cn, item_idx),
            46 => teleport3(gs, cn, item_idx),
            47 => false, // arena portal
            48 => spell_scroll(gs, cn, item_idx),
            49 => use_blook_pentagram(gs, cn, item_idx),
            50 => use_create_npc(gs, cn, item_idx),
            51 => use_rotate(gs, cn, item_idx),
            52 => false, // personal item
            53 => use_create_item(gs, cn, item_idx),
            54 => use_create_item(gs, cn, item_idx),
            55 => shrine_of_change(gs, cn, item_idx),
            56 => false, // greenling green ball
            57 => explorer_point(gs, cn, item_idx),
            58 => use_grolm(gs, cn, item_idx),
            59 => use_create_gold(gs, cn, item_idx),
            61 => use_lab8_key(gs, cn, item_idx),
            63 => use_lab8_shrine(gs, cn, item_idx),
            64 => use_lab8_moneyshrine(gs, cn, item_idx),
            65 => crate::lab9::lab9_use_switch(gs, cn, item_idx as i32),
            66 => crate::lab9::lab9_use_door(gs, cn, item_idx as i32),
            67 => use_garbage(gs, cn, item_idx),
            68 => use_soulstone(gs, cn, item_idx),
            69 => false,
            _ => {
                log::warn!(
                    "use_driver: Unknown use_driver {} for item {}",
                    driver,
                    item_idx
                );
                false
            }
        };

        if cn != 0 {
            if !ret {
                if !carried {
                    gs.characters[cn].cerrno = core::constants::ERR_FAILED as u16;
                }
                return;
            }

            if !carried {
                gs.characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
            }

            // Ensure client update for the acting character
            gs.do_update_char(cn);
        }
    }

    if cn == 0 {
        return; // item_tick does activate and deactivate as well
    }

    // Handle activation/deactivation
    let (active, has_usedeactivate, has_useactivate) = {
        (
            gs.items[item_idx].active,
            (gs.items[item_idx].flags & ItemFlags::IF_USEDEACTIVATE.bits()) != 0,
            (gs.items[item_idx].flags & ItemFlags::IF_USEACTIVATE.bits()) != 0,
        )
    };

    if active != 0 && has_usedeactivate {
        // deactivate: set active=0 and adjust lighting
        let (light0, light1, it_x, it_y) = {
            (
                gs.items[item_idx].light[0],
                gs.items[item_idx].light[1],
                gs.items[item_idx].x,
                gs.items[item_idx].y,
            )
        };

        gs.items[item_idx].active = 0;

        if light0 != light1 && it_x > 0 {
            gs.do_add_light(
                i32::from(it_x),
                i32::from(it_y),
                i32::from(light0) - i32::from(light1),
            );
        }

        if carried {
            gs.items[item_idx].flags |= core::constants::ItemFlags::IF_UPDATE.bits();
            gs.do_update_char(cn);
        }

        if cn != 0 && !carried {
            gs.characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
        }
    } else if active == 0 && has_useactivate {
        // activate: set active=duration and adjust lighting
        let duration = gs.items[item_idx].duration;
        let (light0, light1, it_x, it_y) = {
            (
                gs.items[item_idx].light[0],
                gs.items[item_idx].light[1],
                gs.items[item_idx].x,
                gs.items[item_idx].y,
            )
        };

        gs.items[item_idx].active = duration;

        if light0 != light1 && it_x > 0 {
            gs.do_add_light(
                i32::from(it_x),
                i32::from(it_y),
                i32::from(light1) - i32::from(light0),
            );
        }

        if carried {
            gs.items[item_idx].flags |= core::constants::ItemFlags::IF_UPDATE.bits();
            gs.do_update_char(cn);
        }

        if cn != 0 && !carried {
            gs.characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
        }
    }

    // Handle IF_USEDESTROY items (potions, etc.)
    let will_destroy_on_use =
        (gs.items[item_idx].flags & core::constants::ItemFlags::IF_USEDESTROY.bits()) != 0;

    // This is the last case--so if both of these flags aren't set
    // correctly then we want to exit early.
    if !carried || !will_destroy_on_use {
        return;
    }

    // Check min_rank requirement
    let min_rank = gs.items[item_idx].min_rank;
    let curr_rank = { core::ranks::points2rank(gs.characters[cn].points_tot as u32) };
    if i32::from(min_rank) > curr_rank as i32 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "You're not experienced enough to use this.\n",
        );
        return;
    }

    // Log usage
    let item_name = gs.items[item_idx].get_name().to_owned();
    let item_reference = c_string_to_str(&gs.items[item_idx].reference).to_owned();
    log::info!("Used {}", item_name);

    // Apply hp/end/mana changes
    let hp = gs.items[item_idx].hp[0];
    let end = gs.items[item_idx].end[0];
    let mana = gs.items[item_idx].mana[0];

    gs.characters[cn].a_hp += i32::from(hp) * 1000;
    if gs.characters[cn].a_hp < 0 {
        gs.characters[cn].a_hp = 0;
    }
    gs.characters[cn].a_end += i32::from(end) * 1000;
    if gs.characters[cn].a_end < 0 {
        gs.characters[cn].a_end = 0;
    }
    gs.characters[cn].a_mana += i32::from(mana) * 1000;
    if gs.characters[cn].a_mana < 0 {
        gs.characters[cn].a_mana = 0;
    }

    // If item grants a spell-like effect, apply it
    let duration = gs.items[item_idx].duration;
    if duration != 0 {
        driver::spell_from_item(gs, cn, item_idx);
    }

    // Potions leave their reusable container in the same cursor or inventory
    // slot. Other use-destroy items are removed normally.
    if !replace_consumed_potion_with_flask(gs, cn, item_idx) {
        God::take_from_char(gs, item_idx, cn);
        gs.items[item_idx].used = USE_EMPTY;
    }

    // If character died as a result, announce and handle death
    let a_hp = gs.characters[cn].a_hp;
    if a_hp < 500 {
        let (x, y) = {
            (
                i32::from(gs.characters[cn].x),
                i32::from(gs.characters[cn].y),
            )
        };
        gs.do_area_log(
            cn,
            0,
            x,
            y,
            core::types::FontColor::Yellow,
            &format!(
                "{} was killed by {}.\n",
                gs.characters[cn].get_name().to_owned(),
                item_reference
            ),
        );
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            &format!("You were killed by {}.\n", item_reference),
        );
        gs.do_character_killed(cn, 0, true);
    }

    gs.do_update_char(cn);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_state::GameState;
    use crate::test_helpers::{add_test_player, with_test_gs};
    use core::constants::{
        IT_FLASK, IT_HEALING_POTION, IT_POTION_OF_LIFE, IT_RED_FLOWER, ItemFlags, USE_ACTIVE,
        USE_EMPTY,
    };

    /// Creates a carried use-destroy item for `use_driver` tests.
    fn add_consumable(
        gs: &mut GameState,
        cn: usize,
        item_idx: usize,
        template_id: usize,
        in_citem: bool,
    ) {
        gs.items[item_idx] = core::types::Item::default();
        gs.items[item_idx].used = USE_ACTIVE;
        gs.items[item_idx].temp = template_id as u16;
        gs.items[item_idx].flags = ItemFlags::IF_USE.bits() | ItemFlags::IF_USEDESTROY.bits();
        gs.items[item_idx].carried = cn as u16;
        if in_citem {
            gs.characters[cn].citem = item_idx as u32;
        } else {
            gs.characters[cn].item[0] = item_idx as u32;
        }
    }

    #[test]
    fn consumed_inventory_potion_becomes_flask_in_same_slot() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            gs.item_templates[IT_FLASK].used = USE_ACTIVE;
            add_consumable(gs, cn, item_idx, IT_HEALING_POTION, false);

            use_driver(gs, cn, item_idx, true);

            assert_eq!(gs.characters[cn].item[0], item_idx as u32);
            assert_eq!(gs.items[item_idx].temp as usize, IT_FLASK);
            assert_eq!(gs.items[item_idx].carried as usize, cn);
        });
    }

    #[test]
    fn consumed_cursor_potion_becomes_flask_in_cursor() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            gs.item_templates[IT_FLASK].used = USE_ACTIVE;
            add_consumable(gs, cn, item_idx, IT_POTION_OF_LIFE, true);

            use_driver(gs, cn, item_idx, true);

            assert_eq!(gs.characters[cn].citem, item_idx as u32);
            assert_eq!(gs.items[item_idx].temp as usize, IT_FLASK);
            assert_eq!(gs.items[item_idx].carried as usize, cn);
        });
    }

    #[test]
    fn consumed_non_potion_is_destroyed() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            add_consumable(gs, cn, item_idx, IT_RED_FLOWER, false);

            use_driver(gs, cn, item_idx, true);

            assert_eq!(gs.characters[cn].item[0], 0);
            assert_eq!(gs.items[item_idx].used, USE_EMPTY);
        });
    }

    #[test]
    fn carried_item_activates_for_duration_and_marks_update() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            gs.items[item_idx].used = USE_ACTIVE;
            gs.items[item_idx].flags = ItemFlags::IF_USE.bits() | ItemFlags::IF_USEACTIVATE.bits();
            gs.items[item_idx].duration = 42;
            gs.items[item_idx].carried = cn as u16;
            gs.characters[cn].item[0] = item_idx as u32;

            use_driver(gs, cn, item_idx, true);

            assert_eq!(gs.items[item_idx].active, 42);
            assert_ne!(gs.items[item_idx].flags & ItemFlags::IF_UPDATE.bits(), 0);
        });
    }

    #[test]
    fn carried_item_deactivates_and_marks_update() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            gs.items[item_idx].used = USE_ACTIVE;
            gs.items[item_idx].flags =
                ItemFlags::IF_USE.bits() | ItemFlags::IF_USEDEACTIVATE.bits();
            gs.items[item_idx].active = 7;
            gs.items[item_idx].carried = cn as u16;
            gs.characters[cn].item[0] = item_idx as u32;

            use_driver(gs, cn, item_idx, true);

            assert_eq!(gs.items[item_idx].active, 0);
            assert_ne!(gs.items[item_idx].flags & ItemFlags::IF_UPDATE.bits(), 0);
        });
    }

    #[test]
    fn use_destroy_item_survives_when_rank_is_too_low() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            add_consumable(gs, cn, item_idx, IT_RED_FLOWER, false);
            gs.items[item_idx].min_rank = 1;

            use_driver(gs, cn, item_idx, true);

            assert_eq!(gs.characters[cn].item[0], item_idx as u32);
            assert_eq!(gs.items[item_idx].used, USE_ACTIVE);
        });
    }
}
