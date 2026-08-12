//! Aura pulse logic and state management.
//!
//! Auras are processed once per game tick. Each active source that is due to
//! pulse scans nearby map tiles, validates potential targets, and either
//! creates a new spell item or refreshes/replaces an existing one of the same
//! aura type.

use super::templates::aura_template;
use super::{AuraId, AuraState, AuraTemplate};
use crate::driver;
use crate::game_state::GameState;
use core::constants::USE_ACTIVE;
use core::types::FontColor;

/// Adds an aura to a character, replacing any existing aura on that character.
///
/// # Arguments
///
/// * `gs` - Active game state.
/// * `cn` - Character index that will emit the aura.
/// * `id` - Aura template to activate.
pub fn add_aura(gs: &mut GameState, cn: usize, id: AuraId) {
    if cn == 0 || cn >= core::constants::MAXCHARS {
        return;
    }
    let template = aura_template(id);
    let next_pulse_tick = gs
        .globals
        .ticker
        .wrapping_add(template.pulse_interval_ticks);
    gs.aura_states.insert(
        cn,
        AuraState {
            id,
            next_pulse_tick,
        },
    );
}

/// Removes an aura from a character.
///
/// # Arguments
///
/// * `gs` - Active game state.
/// * `cn` - Character index to stop emitting.
pub fn remove_aura(gs: &mut GameState, cn: usize) {
    if cn == 0 || cn >= core::constants::MAXCHARS {
        return;
    }
    gs.aura_states.remove(&cn);
}

/// Returns true if the character currently has the specified aura active.
///
/// # Arguments
///
/// * `gs` - Active game state.
/// * `cn` - Character index to inspect.
/// * `id` - Aura type to look for.
pub fn has_aura(gs: &GameState, cn: usize, id: AuraId) -> bool {
    if cn == 0 || cn >= core::constants::MAXCHARS {
        return false;
    }
    gs.aura_states.get(&cn).is_some_and(|state| state.id == id)
}

/// Toggles an aura on or off for a character.
///
/// # Arguments
///
/// * `gs` - Active game state.
/// * `cn` - Character index toggling the aura.
/// * `id` - Aura type to toggle.
/// * `on_msg` - Message logged when the aura is activated.
/// * `off_msg` - Message logged when the aura is dismissed.
pub fn toggle_aura(gs: &mut GameState, cn: usize, id: AuraId, on_msg: &str, off_msg: &str) {
    if cn == 0 || cn >= core::constants::MAXCHARS {
        return;
    }
    if has_aura(gs, cn, id) {
        remove_aura(gs, cn);
        gs.do_character_log(cn, FontColor::Green, off_msg);
    } else {
        add_aura(gs, cn, id);
        gs.do_character_log(cn, FontColor::Green, on_msg);
    }
}

/// Processes all active aura pulses for the current tick.
///
/// # Arguments
///
/// * `gs` - Active game state.
/// * `current_tick` - Current server tick.
pub fn tick_auras(gs: &mut GameState, current_tick: i32) {
    core::measure!("aura.tick", {
        let mut pending: Vec<(usize, AuraId)> = Vec::new();
        for (&cn, state) in &gs.aura_states {
            if cn == 0 || cn >= core::constants::MAXCHARS {
                continue;
            }
            if gs.characters[cn].used != USE_ACTIVE {
                continue;
            }
            if current_tick >= state.next_pulse_tick {
                pending.push((cn, state.id));
            }
        }

        for (cn, id) in pending {
            let template = aura_template(id);
            pulse_aura(gs, cn, &template);
            if let Some(state) = gs.aura_states.get_mut(&cn) {
                state.next_pulse_tick = state
                    .next_pulse_tick
                    .wrapping_add(template.pulse_interval_ticks);
            }
        }
    });
}

/// Performs a single aura pulse.
///
/// # Arguments
///
/// * `gs` - Active game state.
/// * `source_cn` - Character index emitting the pulse.
/// * `template` - Aura template being pulsed.
fn pulse_aura(gs: &mut GameState, source_cn: usize, template: &AuraTemplate) {
    let (sx, sy) = {
        let ch = &gs.characters[source_cn];
        (i32::from(ch.x), i32::from(ch.y))
    };

    let radius = template.radius_tiles;
    let xf = std::cmp::max(1, sx - radius);
    let yf = std::cmp::max(1, sy - radius);
    let xt = std::cmp::min(core::constants::SERVER_MAPX - 2, sx + radius);
    let yt = std::cmp::min(core::constants::SERVER_MAPY - 2, sy + radius);

    core::measure!("aura.tile_scan", {
        for y in yf..=yt {
            let row_base = y * core::constants::SERVER_MAPX;
            for x in xf..=xt {
                let target_cn = gs.map[(x + row_base) as usize].ch as usize;
                if target_cn == 0 || target_cn == source_cn {
                    continue;
                }
                if template.is_valid_target(gs, source_cn, target_cn) {
                    apply_or_refresh_aura(gs, source_cn, target_cn, template);
                }
            }
        }
    });

    // Beneficial auras also affect their caster, who is not found by scanning
    // the surrounding map tiles.
    if matches!(template.kind, super::AuraKind::Buff) {
        apply_or_refresh_aura(gs, source_cn, source_cn, template);
    }
}

/// Applies or refreshes an aura on a single target.
///
/// Overlapping auras of the same type maintain a single instance on the
/// target. A pulse from the same source refreshes duration; a strictly
/// stronger source replaces the existing spell; weaker or equal sources are
/// ignored.
///
/// # Arguments
///
/// * `gs` - Active game state.
/// * `source_cn` - Character index emitting the aura.
/// * `target_cn` - Character index receiving the aura.
/// * `template` - Aura template being applied.
fn apply_or_refresh_aura(
    gs: &mut GameState,
    source_cn: usize,
    target_cn: usize,
    template: &AuraTemplate,
) {
    let new_power = template.aura_power(gs, source_cn);
    match find_aura_spell(gs, target_cn, template.temp) {
        Some((slot, spell_idx)) => {
            let existing_source = gs.items[spell_idx].data[0] as usize;
            let existing_power = gs.items[spell_idx].power;

            if existing_source == source_cn {
                // Same source: refresh duration so the buff never flickers.
                gs.items[spell_idx].active = template.spell_duration_ticks as u32;
                gs.items[spell_idx].duration = template.spell_duration_ticks as u32;
                gs.characters[target_cn].set_do_update_flags();
            } else if new_power > existing_power {
                // Strictly stronger source: remove old and apply new.
                gs.items[spell_idx].used = core::constants::USE_EMPTY;
                gs.characters[target_cn].spell[slot] = 0;
                if let Some(new_idx) = template.create_spell_item(gs, source_cn) {
                    driver::skill::add_spell(gs, target_cn, new_idx);
                }
            }
            // Weaker or equal: do nothing to prevent flip-flopping.
        }
        None => {
            if let Some(new_idx) = template.create_spell_item(gs, source_cn) {
                driver::skill::add_spell(gs, target_cn, new_idx);
            }
        }
    }
}

/// Finds an existing aura spell item of the given temp on a character.
///
/// # Arguments
///
/// * `gs` - Active game state.
/// * `target_cn` - Character index to inspect.
/// * `temp` - Spell item temp marker to search for.
///
/// # Returns
///
/// * `Some((slot, item_idx))` if found, otherwise `None`.
fn find_aura_spell(gs: &GameState, target_cn: usize, temp: u16) -> Option<(usize, usize)> {
    for slot in 0..20 {
        let spell_idx = gs.characters[target_cn].spell[slot] as usize;
        if spell_idx != 0 && gs.items[spell_idx].temp == temp {
            return Some((slot, spell_idx));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_remove_aura() {
        crate::test_helpers::with_test_gs(|gs| {
            gs.globals.ticker = 100;
            add_aura(gs, 1, AuraId::CurseAura);
            assert!(has_aura(gs, 1, AuraId::CurseAura));
            remove_aura(gs, 1);
            assert!(!has_aura(gs, 1, AuraId::CurseAura));
        });
    }

    #[test]
    fn tick_advances_next_pulse() {
        crate::test_helpers::with_test_gs(|gs| {
            gs.globals.ticker = 100;
            gs.characters[1].used = core::constants::USE_ACTIVE;
            add_aura(gs, 1, AuraId::CurseAura);
            let before = gs.aura_states[&1].next_pulse_tick;
            tick_auras(gs, before);
            let after = gs.aura_states[&1].next_pulse_tick;
            assert!(after > before);
        });
    }

    #[test]
    fn inactive_source_is_skipped() {
        crate::test_helpers::with_test_gs(|gs| {
            gs.globals.ticker = 100;
            add_aura(gs, 1, AuraId::CurseAura);
            gs.characters[1].used = core::constants::USE_EMPTY;
            // Should not panic and should not advance the pulse.
            let before = gs.aura_states[&1].next_pulse_tick;
            tick_auras(gs, before);
            assert_eq!(gs.aura_states[&1].next_pulse_tick, before);
        });
    }

    #[test]
    fn same_source_refreshes_existing_aura() {
        crate::test_helpers::with_test_gs(|gs| {
            gs.globals.ticker = 100;

            // Item template 1 must be active for God::create_item to succeed in tests.
            gs.item_templates[1].used = core::constants::USE_ACTIVE;

            // Source stands at (100, 100) in group 1.
            gs.characters[1].used = core::constants::USE_ACTIVE;
            gs.characters[1].x = 100;
            gs.characters[1].y = 100;
            gs.characters[1].data[core::constants::CHD_GROUP] = 1;
            let source_idx = 100 + 100 * core::constants::SERVER_MAPX as usize;
            gs.map[source_idx].ch = 1;

            // Target stands adjacent in group 2 (enemy).
            gs.characters[2].used = core::constants::USE_ACTIVE;
            gs.characters[2].x = 101;
            gs.characters[2].y = 100;
            gs.characters[2].data[core::constants::CHD_GROUP] = 2;
            let target_idx = 101 + 100 * core::constants::SERVER_MAPX as usize;
            gs.map[target_idx].ch = 2;

            add_aura(gs, 1, AuraId::CurseAura);
            let first_pulse = gs.aura_states[&1].next_pulse_tick;
            tick_auras(gs, first_pulse);

            // Target should have the curse aura spell.
            let template = aura_template(AuraId::CurseAura);
            let (slot, spell_idx) =
                find_aura_spell(gs, 2, template.temp).expect("target should have curse spell");
            assert_eq!(gs.items[spell_idx].data[0], 1);

            // Simulate the spell ticking down, then pulse again from the same source.
            gs.items[spell_idx].active = 10;
            let second_pulse = gs.aura_states[&1].next_pulse_tick;
            tick_auras(gs, second_pulse);

            // Same slot should still hold the spell, now refreshed.
            let (refreshed_slot, refreshed_idx) =
                find_aura_spell(gs, 2, template.temp).expect("spell should still exist");
            assert_eq!(slot, refreshed_slot);
            assert_eq!(refreshed_idx, spell_idx);
            assert_eq!(
                gs.items[spell_idx].active,
                template.spell_duration_ticks as u32
            );
        });
    }

    #[test]
    fn curse_aura_power_scales_with_skill_total() {
        crate::test_helpers::with_test_gs(|gs| {
            gs.item_templates[1].used = core::constants::USE_ACTIVE;
            gs.characters[1].used = core::constants::USE_ACTIVE;

            let template = aura_template(AuraId::CurseAura);
            let skill_idx = core::skills::SK_AURA_CURSE;

            // Baseline at low skill total.
            gs.characters[1].skill[skill_idx][core::skills::SkillIndex::TotalValue as usize] = 1;
            let baseline_power = template.aura_power(gs, 1);
            let baseline_idx = template.create_spell_item(gs, 1).unwrap();
            let baseline_penalty = gs.items[baseline_idx].attrib[0][1];

            // Higher skill total should produce a stronger aura.
            gs.characters[1].skill[skill_idx][core::skills::SkillIndex::TotalValue as usize] = 100;
            let scaled_power = template.aura_power(gs, 1);
            let scaled_idx = template.create_spell_item(gs, 1).unwrap();
            let scaled_penalty = gs.items[scaled_idx].attrib[0][1];

            assert!(scaled_power > baseline_power);
            assert!(scaled_penalty.abs() > baseline_penalty.abs());
        });
    }

    #[test]
    fn war_banner_aura_power_scales_with_skill_total() {
        crate::test_helpers::with_test_gs(|gs| {
            gs.item_templates[1].used = core::constants::USE_ACTIVE;
            gs.characters[1].used = core::constants::USE_ACTIVE;

            let template = aura_template(AuraId::WarBannerAura);
            let skill_idx = core::skills::SK_AURA_WAR_BANNER;

            gs.characters[1].skill[skill_idx][core::skills::SkillIndex::TotalValue as usize] = 1;
            let baseline_idx = template.create_spell_item(gs, 1).unwrap();
            let baseline_armor = gs.items[baseline_idx].armor[1];

            gs.characters[1].skill[skill_idx][core::skills::SkillIndex::TotalValue as usize] = 100;
            let scaled_idx = template.create_spell_item(gs, 1).unwrap();
            let scaled_armor = gs.items[scaled_idx].armor[1];

            assert!(scaled_armor > baseline_armor);
        });
    }

    #[test]
    fn war_banner_applies_buff_to_caster() {
        crate::test_helpers::with_test_gs(|gs| {
            gs.globals.ticker = 100;
            gs.item_templates[1].used = core::constants::USE_ACTIVE;

            // Source stands at (100, 100) in group 1 with no nearby allies.
            gs.characters[1].used = core::constants::USE_ACTIVE;
            gs.characters[1].x = 100;
            gs.characters[1].y = 100;
            gs.characters[1].data[core::constants::CHD_GROUP] = 1;
            let source_idx = 100 + 100 * core::constants::SERVER_MAPX as usize;
            gs.map[source_idx].ch = 1;

            add_aura(gs, 1, AuraId::WarBannerAura);
            let first_pulse = gs.aura_states[&1].next_pulse_tick;
            tick_auras(gs, first_pulse);

            let template = aura_template(AuraId::WarBannerAura);
            let (slot, spell_idx) =
                find_aura_spell(gs, 1, template.temp).expect("caster should have war banner spell");
            assert_eq!(gs.items[spell_idx].data[0], 1);

            // Simulate the spell ticking down, then pulse again from the same source.
            gs.items[spell_idx].active = 10;
            let second_pulse = gs.aura_states[&1].next_pulse_tick;
            tick_auras(gs, second_pulse);

            let (refreshed_slot, refreshed_idx) =
                find_aura_spell(gs, 1, template.temp).expect("spell should still exist");
            assert_eq!(slot, refreshed_slot);
            assert_eq!(refreshed_idx, spell_idx);
            assert_eq!(
                gs.items[spell_idx].active,
                template.spell_duration_ticks as u32
            );
        });
    }

    #[test]
    fn curse_aura_does_not_apply_to_caster() {
        crate::test_helpers::with_test_gs(|gs| {
            gs.globals.ticker = 100;
            gs.item_templates[1].used = core::constants::USE_ACTIVE;

            gs.characters[1].used = core::constants::USE_ACTIVE;
            gs.characters[1].x = 100;
            gs.characters[1].y = 100;
            gs.characters[1].data[core::constants::CHD_GROUP] = 1;
            let source_idx = 100 + 100 * core::constants::SERVER_MAPX as usize;
            gs.map[source_idx].ch = 1;

            add_aura(gs, 1, AuraId::CurseAura);
            let first_pulse = gs.aura_states[&1].next_pulse_tick;
            tick_auras(gs, first_pulse);

            let template = aura_template(AuraId::CurseAura);
            assert!(
                find_aura_spell(gs, 1, template.temp).is_none(),
                "caster should not be cursed by their own debuff aura"
            );
        });
    }
}
