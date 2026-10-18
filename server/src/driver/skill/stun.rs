use super::common::{
    add_exhaust, add_spell, chance_base, effective_resist, is_exhausted, spell_immunity,
    spell_race_mod, spellcost,
};
use super::resolve_offensive_target;
use crate::chlog;
use crate::effect::EffectManager;
use crate::game_state::GameState;
use crate::god::God;
use crate::helpers;
use core::constants::{CharacterFlags, ItemFlags};
use core::skills::{SK_IMMUN, SK_RESIST, SK_SENSE, SK_STUN};
use core::string_operations::c_string_to_str;
use core::types::FontColor;

/// Applies the Stun spell to a target character.
///
/// # Arguments
///
/// * `gs` - Active game state used to create the stun item and emit combat feedback.
/// * `cn` - Caster character index.
/// * `co` - Target character index.
/// * `power` - Base stun power before immunity and race modifiers.
///
/// # Returns
///
/// * `true` when Stun was applied, or `false` when the target is immune, item creation fails, or attachment is neutralized.
///
/// # Panics
///
/// * Panics if `cn`, `co`, or the created spell item index is invalid.
pub fn spell_stun(gs: &mut GameState, cn: usize, co: usize, power: i32) -> bool {
    if (gs.characters[co].flags & CharacterFlags::Immortal.bits()) != 0 {
        return false;
    }

    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        return false;
    }
    let in_idx = in_opt.unwrap();

    let mut power = spell_immunity(gs, power, i32::from(gs.characters[co].skill[SK_IMMUN][5]));
    power = spell_race_mod(gs, power, gs.characters[cn].kindred);

    {
        let mut name_bytes = [0u8; 40];
        let name = b"Stun";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        gs.items[in_idx].name = name_bytes;
        gs.items[in_idx].flags |= ItemFlags::IF_SPELL.bits();
        gs.items[in_idx].sprite[1] = 91;
        gs.items[in_idx].duration = (power + core::constants::TICKS) as u32;
        gs.items[in_idx].active = gs.items[in_idx].duration;
        gs.items[in_idx].temp = SK_STUN as u16;
        gs.items[in_idx].power = power as u32;
    }

    if gs.characters[co].skill[SK_SENSE][5] + 10 > power as u16 {
        gs.do_character_log(
            co,
            FontColor::Green,
            &format!(
                "{} cast stun on you.\n",
                c_string_to_str(&gs.characters[cn].reference)
            ),
        );
    } else {
        gs.do_character_log(co, FontColor::Green, "You have been stunned.\n");
    }

    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!(
            "{} was stunned.\n",
            c_string_to_str(&gs.characters[co].reference)
        ),
    );

    if gs.characters[co].flags & CharacterFlags::SpellIgnore.bits() == 0 {
        gs.do_notify_character(
            co as u32,
            i32::from(core::constants::NT_GOTHIT),
            cn as i32,
            0,
            0,
            0,
        );
    }
    gs.do_notify_character(
        cn as u32,
        i32::from(core::constants::NT_DIDHIT),
        co as i32,
        0,
        0,
        0,
    );

    GameState::char_play_sound(gs, co, i32::from(gs.characters[cn].sound) + 7, -150, 0);
    GameState::char_play_sound(gs, cn, i32::from(gs.characters[cn].sound) + 1, -150, 0);
    chlog!(
        cn,
        "Cast Stun on {} for {} power",
        gs.characters[co].get_name().to_owned(),
        power
    );

    if add_spell(gs, co, in_idx) == 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!(
                "Magical interference neutralised the {}'s effect.\n",
                "stun"
            ),
        );
        return false;
    }

    EffectManager::fx_add_effect(
        gs,
        5,
        0,
        i32::from(gs.characters[co].x),
        i32::from(gs.characters[co].y),
        0,
    );

    true
}

/// Handles direct player/NPC use of the Stun skill, including adjacent attackers.
///
/// # Arguments
///
/// * `gs` - Active game state used for target validation, combat checks, and spell application.
/// * `cn` - Caster character index.
///
/// # Panics
///
/// * Panics if `cn`, the selected target index, or an adjacent target index is invalid.
pub fn skill_stun(gs: &mut GameState, cn: usize) {
    let co = resolve_offensive_target(gs, cn);

    if cn == co {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "You cannot stun yourself!\n",
        );
        return;
    }

    if gs.do_char_can_see(cn, co) == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "You cannot see your target.\n",
        );
        return;
    }

    gs.remember_pvp(cn, co);
    if is_exhausted(gs, cn) {
        return;
    }

    if !gs.may_attack_msg(cn, co, true) {
        chlog!(
            cn,
            "Prevented from attacking {}",
            gs.characters[co].get_name().to_owned()
        );
        return;
    }

    if spellcost(gs, cn, 20) != 0 {
        return;
    }

    if chance_base(
        gs,
        cn,
        i32::from(gs.characters[cn].skill[SK_STUN][5]),
        12,
        effective_resist(gs, cn, i32::from(gs.characters[co].skill[SK_RESIST][5])),
    ) != 0
    {
        if cn != co
            && gs.characters[co].skill[SK_SENSE][5] > gs.characters[cn].skill[SK_STUN][5] + 5
        {
            gs.do_character_log(
                co,
                core::types::FontColor::Green,
                &format!(
                    "{} tried to cast stun on you but failed.\n",
                    c_string_to_str(&gs.characters[cn].reference)
                ),
            );
            if gs.characters[co].flags & CharacterFlags::SpellIgnore.bits() == 0 {
                gs.do_notify_character(
                    co as u32,
                    i32::from(core::constants::NT_GOTMISS),
                    cn as i32,
                    0,
                    0,
                    0,
                );
            }
        }
        return;
    }

    if (gs.characters[co].flags & CharacterFlags::Immortal.bits()) != 0 {
        gs.do_character_log(cn, core::types::FontColor::Red, "You lost your focus.\n");
        return;
    }

    let power = i32::from(gs.characters[cn].skill[SK_STUN][5]);
    spell_stun(gs, cn, co, power);

    let co_orig = co;
    let m: usize = gs.characters[cn].x as usize
        + gs.characters[cn].y as usize * core::constants::SERVER_MAPX as usize;

    let adj = [
        1isize,
        -1isize,
        core::constants::SERVER_MAPX as isize,
        -(core::constants::SERVER_MAPX as isize),
    ];
    for delta in adj.iter() {
        let idx = (m as isize + *delta) as usize;
        let maybe_co = gs.map.get(idx).map(|m| m.ch).unwrap_or(0) as usize;
        if maybe_co != 0 && gs.characters[maybe_co].attack_cn == cn as u16 && maybe_co != co_orig {
            let s_rand = helpers::random_mod_i32(20);
            let o_rand = helpers::random_mod_i32(20);
            if i32::from(gs.characters[cn].skill[SK_STUN][5]) + s_rand
                > effective_resist(
                    gs,
                    cn,
                    i32::from(gs.characters[maybe_co].skill[SK_RESIST][5]),
                ) + o_rand
            {
                spell_stun(
                    gs,
                    cn,
                    maybe_co,
                    i32::from(gs.characters[cn].skill[SK_STUN][5]),
                );
            }
        }
    }

    EffectManager::fx_add_effect(
        gs,
        7,
        0,
        i32::from(gs.characters[cn].x),
        i32::from(gs.characters[cn].y),
        0,
    );
    add_exhaust(gs, cn, core::constants::TICKS * 3);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::skill::tests::{active_spell_item, caster_and_target};
    use crate::test_helpers::with_test_gs;
    use core::constants::TICKS;

    #[test]
    fn spell_stun_lasts_power_plus_one_second() {
        with_test_gs(|gs| {
            let (cn, co) = caster_and_target(gs);

            assert!(spell_stun(gs, cn, co, 30));

            let in_ = active_spell_item(gs, co, SK_STUN).expect("stun");
            assert_eq!(gs.items[in_].power, 30);
            assert_eq!(gs.items[in_].active, (30 + TICKS) as u32);
        });
    }

    #[test]
    fn spell_stun_duration_is_reduced_by_target_immunity() {
        with_test_gs(|gs| {
            let (cn, co) = caster_and_target(gs);
            gs.characters[co].skill[SK_IMMUN][5] = 20;

            assert!(spell_stun(gs, cn, co, 30));

            let in_ = active_spell_item(gs, co, SK_STUN).expect("stun");
            assert_eq!(gs.items[in_].active, (20 + TICKS) as u32);
        });
    }

    #[test]
    fn spell_stun_skips_immortal_targets() {
        with_test_gs(|gs| {
            let (cn, co) = caster_and_target(gs);
            gs.characters[co].flags |= CharacterFlags::Immortal.bits();

            assert!(!spell_stun(gs, cn, co, 30));
            assert!(active_spell_item(gs, co, SK_STUN).is_none());
        });
    }

    #[test]
    fn skill_stun_refuses_to_target_the_caster() {
        with_test_gs(|gs| {
            let (cn, _co) = caster_and_target(gs);
            gs.characters[cn].skill_target1 = cn as u16;
            gs.characters[cn].skill[SK_STUN][5] = 50;
            gs.characters[cn].a_mana = 100_000;

            skill_stun(gs, cn);

            assert_eq!(gs.characters[cn].a_mana, 100_000);
            assert!(active_spell_item(gs, cn, SK_STUN).is_none());
        });
    }

    #[test]
    fn skill_stun_spends_mana_on_a_lost_focus_without_exhausting() {
        with_test_gs(|gs| {
            let (cn, co) = caster_and_target(gs);
            gs.characters[cn].skill[SK_STUN][5] = 10;
            gs.characters[co].skill[SK_RESIST][5] = 100;
            gs.characters[cn].a_mana = 100_000;

            skill_stun(gs, cn);

            assert_eq!(gs.characters[cn].a_mana, 80_000);
            assert!(active_spell_item(gs, co, SK_STUN).is_none());
            assert!(!is_exhausted(gs, cn));
        });
    }
}
