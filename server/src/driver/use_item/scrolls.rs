use super::take_item_from_char;
use crate::game_state::GameState;
use crate::god::God;
use crate::{chlog, driver, points};
use core::constants::{MAXSKILL, USE_EMPTY};
use core::skills::{self, attribute_name};

/// Handles the legacy `use_scroll` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_scroll` hook.
///
/// # Returns
///
/// * `true` when `use_scroll` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_scroll` is outside the corresponding game-state collection.
pub fn use_scroll(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    let skill_nr = skills::canonicalize_weapon_skill(gs.items[item_idx].data[0] as usize);
    let teaches_only = gs.items[item_idx].data[1] != 0;

    if skill_nr >= MAXSKILL {
        return false;
    }

    let current_val = gs.characters[cn].skill[skill_nr][0];
    let max_val = gs.characters[cn].skill[skill_nr][2];
    let difficulty = gs.characters[cn].skill[skill_nr][3];

    if current_val != 0 {
        if teaches_only {
            let name = skills::get_skill_name(skill_nr);
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!("You already know {}.\n", name),
            );
            return false;
        }

        if current_val >= max_val {
            let name = skills::get_skill_name(skill_nr);
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!("You cannot raise skill {} any higher.\n", name),
            );
            return false;
        }

        let name = skills::get_skill_name(skill_nr);
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!("Raised {} by one.\n", name),
        );

        let v = i32::from(current_val);
        let diff = i32::from(difficulty);
        let pts = points::skill_needed(v, diff);

        // We also intentionally leave out rank-based EXP bonuses here as well,
        // since this function just adds exp based on skill-raise specifics.
        gs.characters[cn].points_tot += pts;
        gs.characters[cn].skill[skill_nr][0] += 1;
        gs.do_check_new_level(cn);
        log::info!(
            "Used scroll to raise skill {} for {} (pts={})",
            skill_nr,
            cn,
            pts
        );
    } else if max_val == 0 {
        let name = skills::get_skill_name(skill_nr);
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!("This scroll teaches {}, which you cannot learn.\n", name),
        );
        return false;
    } else {
        gs.characters[cn].skill[skill_nr][0] = 1;
        let name = skills::get_skill_name(skill_nr);
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!("You learned {}!\n", name),
        );
        log::info!("Used scroll to learn {} (cn={})", skill_nr, cn);
    }

    gs.items[item_idx].used = USE_EMPTY;
    God::take_from_char(gs, item_idx, cn);
    gs.characters[cn].set_do_update_flags();

    true
}

/// Handles the legacy `use_scroll2` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_scroll2` hook.
///
/// # Returns
///
/// * `true` when `use_scroll2` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_scroll2` is outside the corresponding game-state collection.
pub fn use_scroll2(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    let attrib_nr = gs.items[item_idx].data[0] as usize;
    let current_val = gs.characters[cn].attrib[attrib_nr][0];
    let max_val = gs.characters[cn].attrib[attrib_nr][2];
    let difficulty = gs.characters[cn].attrib[attrib_nr][3];

    if current_val >= max_val {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!(
                "You cannot raise attribute {} any higher.\n",
                attribute_name(attrib_nr)
            ),
        );
        return false;
    }

    // Calculate points needed: v*v*v*diff/20
    let v = i32::from(current_val);
    let diff = i32::from(difficulty);
    let pts = (v * v * v * diff) / 20;

    gs.do_character_log(
        cn,
        core::types::FontColor::Yellow,
        &format!("Raised attribute {} by one.\n", attribute_name(attrib_nr)),
    );
    chlog!(
        cn,
        "used a scroll to raise attribute {} (pts={})",
        attribute_name(attrib_nr),
        pts
    );

    gs.characters[cn].points_tot += pts;
    gs.characters[cn].attrib[attrib_nr][0] += 1;
    gs.do_check_new_level(cn);
    gs.items[item_idx].used = USE_EMPTY;

    take_item_from_char(gs, item_idx, cn);
    gs.characters[cn].set_do_update_flags();

    true
}

/// Handles the legacy `use_scroll3` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_scroll3` hook.
///
/// # Returns
///
/// * `true` when `use_scroll3` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_scroll3` is outside the corresponding game-state collection.
pub fn use_scroll3(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    let amount = gs.items[item_idx].data[0] as i32;
    let current_hp = gs.characters[cn].hp[0];
    let max_hp = gs.characters[cn].hp[2];
    let difficulty = gs.characters[cn].hp[3];

    if current_hp >= max_hp {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "You cannot raise Hitpoints any higher.\n",
        );
        return false;
    }

    gs.do_character_log(
        cn,
        core::types::FontColor::Yellow,
        &format!("Raised Hitpoints by {}.\n", amount),
    );

    let v = i32::from(current_hp);
    let diff = i32::from(difficulty);
    let mut pts = 0;
    for n in 0..amount {
        pts += (n + v) * diff;
    }

    // We also intentionally leave out rank-based EXP bonuses here as well,
    // since this function just adds exp based on skill-raise specifics.
    gs.characters[cn].points_tot += pts;
    gs.characters[cn].hp[0] += amount as u16;
    gs.do_check_new_level(cn);
    gs.items[item_idx].used = USE_EMPTY;

    take_item_from_char(gs, item_idx, cn);
    gs.characters[cn].set_do_update_flags();

    true
}

/// Handles the legacy `use_scroll4` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_scroll4` hook.
///
/// # Returns
///
/// * `true` when `use_scroll4` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_scroll4` is outside the corresponding game-state collection.
pub fn use_scroll4(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    let amount = gs.items[item_idx].data[0] as i32;
    let current_end = gs.characters[cn].end[0];
    let max_end = gs.characters[cn].end[2];
    let difficulty = gs.characters[cn].end[3];

    if current_end >= max_end {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "You cannot raise Endurance any higher.\n",
        );
        return false;
    }

    gs.do_character_log(
        cn,
        core::types::FontColor::Yellow,
        &format!("Raised Endurance by {}.\n", amount),
    );

    let v = i32::from(current_end);
    let diff = i32::from(difficulty);
    let mut pts = 0;
    for n in 0..amount {
        pts += ((n + v) * diff) / 2;
    }

    // We also intentionally leave out rank-based EXP bonuses here as well,
    // since this function just adds exp based on skill-raise specifics.
    gs.characters[cn].points_tot += pts;
    gs.characters[cn].end[0] += amount as u16;
    gs.do_check_new_level(cn);
    gs.items[item_idx].used = USE_EMPTY;

    take_item_from_char(gs, item_idx, cn);
    gs.characters[cn].set_do_update_flags();

    true
}

/// Handles the legacy `use_scroll5` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_scroll5` hook.
///
/// # Returns
///
/// * `true` when `use_scroll5` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_scroll5` is outside the corresponding game-state collection.
pub fn use_scroll5(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    let amount = gs.items[item_idx].data[0] as i32;
    let current_mana = gs.characters[cn].mana[0];
    let max_mana = gs.characters[cn].mana[2];
    let difficulty = gs.characters[cn].mana[3];

    if current_mana >= max_mana {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "You cannot raise Mana any higher.\n",
        );
        return false;
    }

    gs.do_character_log(
        cn,
        core::types::FontColor::Yellow,
        &format!("Raised Mana by {}.\n", amount),
    );

    let v = i32::from(current_mana);
    let diff = i32::from(difficulty);
    let mut pts = 0;

    for n in 0..amount {
        pts += (n + v) * diff;
    }

    // We also intentionally leave out rank-based EXP bonuses here as well,
    // since this function just adds exp based on skill-raise specifics.
    gs.characters[cn].points_tot += pts;
    gs.characters[cn].mana[0] += amount as u16;
    gs.do_check_new_level(cn);
    gs.items[item_idx].used = USE_EMPTY;

    take_item_from_char(gs, item_idx, cn);
    gs.characters[cn].set_do_update_flags();

    true
}

/// Handles the legacy `spell_scroll` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `spell_scroll` hook.
///
/// # Returns
///
/// * `true` when `spell_scroll` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `spell_scroll` is outside the corresponding game-state collection.
pub fn spell_scroll(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    // Read scroll data
    let (spell, power, charges) = {
        (
            gs.items[item_idx].data[0],
            gs.items[item_idx].data[1],
            gs.items[item_idx].data[2],
        )
    };

    if charges == 0 {
        gs.do_character_log(cn, core::types::FontColor::Yellow, "Nothing happened!\n");
        return false;
    }

    // Get target (skill_target1 or self)
    let mut co = gs.characters[cn].skill_target1 as usize;
    if co == 0 {
        co = cn;
    }

    // Check if can see target
    let can_see = gs.do_char_can_see(cn, co) != 0;
    if !can_see {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "You cannot see your target.\n",
        );
        return false;
    }

    // Check attack spells for may_attack
    if spell as usize == skills::SK_CURSE || spell as usize == skills::SK_STUN {
        if !gs.may_attack_msg(cn, co, true) {
            log::info!("Prevented from attacking target {}", co);
            return false;
        }
    } else {
        if !driver::player_or_ghost(&gs.characters[cn], cn, &gs.characters[co]) {
            // Change target to self
            co = cn;
        }
    }

    // Cast spell
    let ret: bool = match spell as usize {
        skills::SK_LIGHT => {
            driver::spell_light(gs, cn, co, power as i32);
            true
        }
        skills::SK_ENHANCE => {
            driver::spell_enhance(gs, cn, co, power as i32);
            true
        }
        skills::SK_PROTECT => {
            driver::spell_protect(gs, cn, co, power as i32);
            true
        }
        skills::SK_BLESS => {
            driver::spell_bless(gs, cn, co, power as i32);
            true
        }
        skills::SK_MSHIELD => {
            driver::spell_mshield(gs, cn, co, power as i32);
            true
        }
        skills::SK_CURSE => {
            let target_resistance = gs.characters[co].skill[skills::SK_RESIST][5];
            if driver::chance_base(gs, cn, power as i32, 10, i32::from(target_resistance)) != 0 {
                true
            } else {
                driver::spell_curse(gs, cn, co, power as i32)
            }
        }
        skills::SK_STUN => {
            let target_resistance = gs.characters[co].skill[skills::SK_RESIST][5];
            if driver::chance_base(gs, cn, power as i32, 12, i32::from(target_resistance)) != 0 {
                true
            } else {
                driver::spell_stun(gs, cn, co, power as i32)
            }
        }
        _ => false,
    };

    // Decrement charges if spell succeeded
    if ret {
        let new_charges = charges - 1;
        gs.items[item_idx].data[2] = new_charges;
        gs.items[item_idx].value /= 2;

        if new_charges < 1 {
            return true; // Scroll consumed
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::points;
    use crate::test_helpers::{add_test_player, with_test_gs};
    use core::constants::{MAXSKILL, USE_ACTIVE, USE_EMPTY};
    use core::skills::{SK_REPAIR, SK_SWORD, SK_WEAPON};

    /// Creates a used scroll item in the first inventory slot.
    fn add_inventory_scroll(gs: &mut GameState, cn: usize, item_idx: usize) {
        gs.items[item_idx].used = USE_ACTIVE;
        gs.items[item_idx].carried = cn as u16;
        gs.characters[cn].item[0] = item_idx as u32;
    }

    #[test]
    fn use_scroll_learns_unknown_canonical_weapon_skill() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            add_inventory_scroll(gs, cn, item_idx);
            gs.items[item_idx].data[0] = SK_SWORD as u32;
            gs.characters[cn].skill[SK_WEAPON][2] = 10;

            assert!(use_scroll(gs, cn, item_idx));
            assert_eq!(gs.characters[cn].skill[SK_WEAPON][0], 1);
            assert_eq!(gs.characters[cn].item[0], 0);
            assert_eq!(gs.items[item_idx].used, USE_EMPTY);
        });
    }

    #[test]
    fn use_scroll_raises_known_skill_and_awards_points() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            add_inventory_scroll(gs, cn, item_idx);
            gs.items[item_idx].data[0] = SK_REPAIR as u32;
            gs.characters[cn].skill[SK_REPAIR][0] = 2;
            gs.characters[cn].skill[SK_REPAIR][2] = 5;
            gs.characters[cn].skill[SK_REPAIR][3] = 4;

            assert!(use_scroll(gs, cn, item_idx));
            assert_eq!(gs.characters[cn].skill[SK_REPAIR][0], 3);
            assert_eq!(gs.characters[cn].points_tot, points::skill_needed(2, 4));
            assert_eq!(gs.items[item_idx].used, USE_EMPTY);
        });
    }

    #[test]
    fn teach_only_scroll_refuses_already_known_skill() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            add_inventory_scroll(gs, cn, item_idx);
            gs.items[item_idx].data[0] = SK_REPAIR as u32;
            gs.items[item_idx].data[1] = 1;
            gs.characters[cn].skill[SK_REPAIR][0] = 1;
            gs.characters[cn].skill[SK_REPAIR][2] = 5;

            assert!(!use_scroll(gs, cn, item_idx));
            assert_eq!(gs.characters[cn].skill[SK_REPAIR][0], 1);
            assert_eq!(gs.characters[cn].item[0], item_idx as u32);
            assert_eq!(gs.items[item_idx].used, USE_ACTIVE);
        });
    }

    #[test]
    fn invalid_skill_scroll_is_not_consumed() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            add_inventory_scroll(gs, cn, item_idx);
            gs.items[item_idx].data[0] = MAXSKILL as u32;

            assert!(!use_scroll(gs, cn, item_idx));
            assert_eq!(gs.characters[cn].item[0], item_idx as u32);
            assert_eq!(gs.items[item_idx].used, USE_ACTIVE);
        });
    }

    #[test]
    fn attribute_scroll_raises_attribute_and_consumes_item() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            add_inventory_scroll(gs, cn, item_idx);
            gs.items[item_idx].data[0] = 0;
            gs.characters[cn].attrib[0][0] = 5;
            gs.characters[cn].attrib[0][2] = 7;
            gs.characters[cn].attrib[0][3] = 4;

            assert!(use_scroll2(gs, cn, item_idx));
            assert_eq!(gs.characters[cn].attrib[0][0], 6);
            assert_eq!(gs.characters[cn].points_tot, 25);
            assert_eq!(gs.characters[cn].item[0], 0);
            assert_eq!(gs.items[item_idx].used, USE_EMPTY);
        });
    }

    #[test]
    fn attribute_scroll_refuses_when_attribute_is_at_max() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            add_inventory_scroll(gs, cn, item_idx);
            gs.items[item_idx].data[0] = 0;
            gs.characters[cn].attrib[0][0] = 7;
            gs.characters[cn].attrib[0][2] = 7;

            assert!(!use_scroll2(gs, cn, item_idx));
            assert_eq!(gs.characters[cn].item[0], item_idx as u32);
            assert_eq!(gs.items[item_idx].used, USE_ACTIVE);
        });
    }

    #[test]
    fn hitpoint_scroll_adds_amount_and_cost_for_each_point() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            add_inventory_scroll(gs, cn, item_idx);
            gs.items[item_idx].data[0] = 3;
            gs.characters[cn].hp[0] = 5;
            gs.characters[cn].hp[2] = 10;
            gs.characters[cn].hp[3] = 2;

            assert!(use_scroll3(gs, cn, item_idx));
            assert_eq!(gs.characters[cn].hp[0], 8);
            assert_eq!(gs.characters[cn].points_tot, 36);
            assert_eq!(gs.items[item_idx].used, USE_EMPTY);
        });
    }

    #[test]
    fn spell_scroll_without_charges_does_nothing() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let item_idx = 1;
            add_inventory_scroll(gs, cn, item_idx);
            gs.items[item_idx].data[2] = 0;
            gs.items[item_idx].value = 100;

            assert!(!spell_scroll(gs, cn, item_idx));
            assert_eq!(gs.items[item_idx].data[2], 0);
            assert_eq!(gs.items[item_idx].value, 100);
            assert_eq!(gs.items[item_idx].used, USE_ACTIVE);
        });
    }
}
