use super::common::{
    add_exhaust, is_exhausted, spell_immunity, spell_race_mod, spellcost, spellcost_endurance,
};
use super::resolve_offensive_target;
use crate::chlog;
use crate::effect::EffectManager;
use crate::game_state::GameState;
use crate::helpers;
use core::constants::{CT_COMPANION, CharacterFlags};
use core::skills::{SK_ANGUISH_LAVA, SK_BLAST, SK_IMMUN, SK_SPELLCASTER_KINDRED_SPIRIT};
use core::traits::{KIN_ARCHHARAKIM, KIN_HARAKIM};
use core::types::{Character, FontColor};

/// Returns whether a Blast cast should consume endurance instead of mana.
///
/// # Arguments
///
/// * `gs` - Active game state containing owner and companion state.
/// * `cn` - Caster character index.
///
/// # Returns
///
/// * `true` for ghost companions whose owner has Spellcaster Kindred Spirit.
fn blast_uses_endurance(gs: &GameState, cn: usize) -> bool {
    if gs.characters[cn].temp != CT_COMPANION as u16 {
        return false;
    }
    let owner = gs.characters[cn].data[63] as usize;
    Character::is_sane_character(owner)
        && gs.characters[owner].skill[SK_SPELLCASTER_KINDRED_SPIRIT][0] != 0
}

/// Consumes the appropriate resource for a Blast cast.
///
/// # Arguments
///
/// * `gs` - Active game state containing caster resources.
/// * `cn` - Caster character index.
/// * `cost` - Base spell cost in whole resource units.
///
/// # Returns
///
/// * `0` when the resource was consumed successfully, otherwise `-1`.
pub(super) fn spellcost_blast(gs: &mut GameState, cn: usize, cost: i32) -> i32 {
    if blast_uses_endurance(gs, cn) {
        spellcost_endurance(gs, cn, cost)
    } else {
        spellcost(gs, cn, cost)
    }
}

/// Handles direct player/NPC use of the Blast skill, including area expansion.
///
/// # Arguments
///
/// * `gs` - Active game state used for target validation, mana costs, damage, and effects.
/// * `cn` - Caster character index.
///
/// # Panics
///
/// * Panics if `cn`, the selected target index, or an area target index is invalid.
pub fn skill_blast(gs: &mut GameState, cn: usize) {
    let co = resolve_offensive_target(gs, cn);

    if gs.do_char_can_see(cn, co) == 0 {
        gs.do_character_log(cn, FontColor::Green, "You cannot see your target.\n");
        return;
    }

    if cn == co {
        gs.do_character_log(cn, FontColor::Green, "You cannot blast yourself!\n");
        return;
    }

    if (gs.characters[co].flags & CharacterFlags::Stoned.bits()) != 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            "Your target is lagging. Try again later.\n",
        );
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

    gs.remember_pvp(cn, co);

    if is_exhausted(gs, cn) {
        return;
    }

    let (power, dam) = compute_blast_damage(gs, cn, co);

    let mut cost = dam / 8 + 5;
    if (gs.characters[cn].flags & CharacterFlags::Player.bits()) != 0
        && ((gs.characters[cn].kindred as u32) & (KIN_HARAKIM | KIN_ARCHHARAKIM) != 0)
    {
        cost /= 3;
    }

    if spellcost_blast(gs, cn, cost) != 0 {
        return;
    }

    cast_blast_effect(gs, cn, co, power, dam);

    add_exhaust(gs, cn, core::constants::TICKS * 6);
}

/// Computes Blast damage against `co`, applying immunity/race modifiers and
/// consuming an Anguish (Lava) marker on the target if present.
///
/// Shared by [`skill_blast`]'s mana-cost calculation and the Seyan'Du Free
/// Blast rune proc.
///
/// # Arguments
///
/// * `gs` - Active game state used to read skills and the target's items.
/// * `cn` - Caster character index.
/// * `co` - Target character index.
///
/// # Returns
///
/// * `(power, damage)` to pass to [`cast_blast_effect`].
pub(crate) fn compute_blast_damage(gs: &mut GameState, cn: usize, co: usize) -> (i32, i32) {
    let mut power = i32::from(gs.characters[cn].skill[SK_BLAST][5]);
    power = spell_immunity(gs, power, i32::from(gs.characters[co].skill[SK_IMMUN][5]));
    power = spell_race_mod(gs, power, gs.characters[cn].kindred);

    let mut dam = power * 2;

    // Anguish (Lava): if the target carries the marker, amplify and consume.
    let mut anguish_lava_item: Option<usize> = None;
    for n in 0..20 {
        let in_ = gs.characters[co].spell[n] as usize;
        if in_ != 0 && gs.items[in_].temp == SK_ANGUISH_LAVA as u16 {
            anguish_lava_item = Some(in_);
            break;
        }
    }
    if let Some(in_) = anguish_lava_item {
        let anguish_power = gs.items[in_].power as i32;
        let mult_pct = (150 + anguish_power / 2).clamp(150, 200);
        dam = dam * mult_pct / 100;
        gs.items[in_].active = 0;
    }

    (power, dam)
}

/// Applies Blast damage (with AoE splash) to `co` for a precomputed
/// `power`/`dam` pair. There is no resist roll; the cast always lands.
///
/// Shared by the direct Blast cast (`skill_blast`, after its mana/cooldown
/// gate) and the Seyan'Du Free Blast rune proc (no gate). Does not check
/// exhaustion, mana cost, or `may_attack_msg` — callers must do so first.
///
/// # Arguments
///
/// * `gs` - Active game state used for damage and effects.
/// * `cn` - Caster character index.
/// * `co` - Primary target character index.
/// * `power` - Effective Blast power from [`compute_blast_damage`], used only for the cast log line.
/// * `dam` - Damage amount from [`compute_blast_damage`].
pub fn cast_blast_effect(gs: &mut GameState, cn: usize, co: usize, power: i32, mut dam: i32) {
    gs.do_area_sound(
        co,
        0,
        i32::from(gs.characters[co].x),
        i32::from(gs.characters[co].y),
        i32::from(gs.characters[cn].sound) + 6,
    );
    GameState::char_play_sound(gs, co, i32::from(gs.characters[cn].sound) + 6, -150, 0);

    chlog!(
        cn,
        "Cast Blast on {} for {} power",
        gs.characters[co].get_name().to_owned(),
        power
    );
    let tmp = gs.do_hurt(cn, co, dam, 1);

    if tmp < 1 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            "You cannot penetrate your target's armor.\n",
        );
    } else {
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("You blast your target for {} HP.\n", tmp),
        );
    }

    EffectManager::fx_add_effect(
        gs,
        5,
        0,
        i32::from(gs.characters[co].x),
        i32::from(gs.characters[co].y),
        0,
    );

    let co_orig = co;
    dam = dam / 2 + dam / 4;

    let blast_base = i32::from(gs.characters[cn].skill[SK_BLAST][0]);
    let aoe_base = if (gs.characters[cn].flags & CharacterFlags::Player.bits()) != 0 {
        blast_base
    } else {
        1
    };
    let use_legacy_cross = helpers::skill_aoe_uses_legacy_cross(aoe_base);
    let caster_x = i32::from(gs.characters[cn].x);
    let caster_y = i32::from(gs.characters[cn].y);

    for maybe_co in helpers::skill_aoe_targets(gs, Some(cn), caster_x, caster_y, aoe_base) {
        if maybe_co == cn || maybe_co == co_orig {
            continue;
        }
        if use_legacy_cross && gs.characters[maybe_co].attack_cn != cn as u16 {
            continue;
        }
        if !gs.may_aoe_attack(cn, maybe_co) {
            continue;
        }

        gs.remember_pvp(cn, maybe_co);
        let tmp2 = gs.do_hurt(cn, maybe_co, dam, 1);
        if tmp2 < 1 {
            gs.do_character_log(
                cn,
                FontColor::Green,
                "You cannot penetrate your target's armor.\n",
            );
        } else {
            gs.do_character_log(
                cn,
                FontColor::Green,
                &format!("You blast your target for {} HP.\n", tmp2),
            );
        }
        EffectManager::fx_add_effect(
            gs,
            5,
            0,
            i32::from(gs.characters[maybe_co].x),
            i32::from(gs.characters[maybe_co].y),
            0,
        );
    }

    EffectManager::fx_add_effect(
        gs,
        7,
        0,
        i32::from(gs.characters[cn].x),
        i32::from(gs.characters[cn].y),
        0,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::skill::tests::{attach_marker, caster_and_target};
    use crate::test_helpers::with_test_gs;

    #[test]
    fn compute_blast_damage_is_double_power_after_immunity() {
        with_test_gs(|gs| {
            let (cn, co) = caster_and_target(gs);
            gs.characters[cn].skill[SK_BLAST][5] = 50;

            assert_eq!(compute_blast_damage(gs, cn, co), (50, 100));

            gs.characters[co].skill[SK_IMMUN][5] = 40;
            assert_eq!(compute_blast_damage(gs, cn, co), (30, 60));
        });
    }

    #[test]
    fn compute_blast_damage_consumes_anguish_lava_for_bonus_damage() {
        with_test_gs(|gs| {
            let (cn, co) = caster_and_target(gs);
            gs.characters[cn].skill[SK_BLAST][5] = 50;
            attach_marker(gs, co, 0, 500, SK_ANGUISH_LAVA as u16);

            // Marker power 25 -> 150% + 12% bonus.
            assert_eq!(compute_blast_damage(gs, cn, co), (50, 162));
            assert_eq!(gs.items[500].active, 0);
        });
    }

    #[test]
    fn compute_blast_damage_caps_anguish_lava_bonus_at_double() {
        with_test_gs(|gs| {
            let (cn, co) = caster_and_target(gs);
            gs.characters[cn].skill[SK_BLAST][5] = 50;
            attach_marker(gs, co, 0, 500, SK_ANGUISH_LAVA as u16);
            gs.items[500].power = 500;

            assert_eq!(compute_blast_damage(gs, cn, co), (50, 200));
        });
    }

    #[test]
    fn spellcost_blast_spends_mana_for_regular_casters() {
        with_test_gs(|gs| {
            let (cn, _co) = caster_and_target(gs);
            gs.characters[cn].a_mana = 10_000;
            gs.characters[cn].a_end = 10_000;

            assert_eq!(spellcost_blast(gs, cn, 5), 0);

            assert_eq!(gs.characters[cn].a_mana, 5_000);
            assert_eq!(gs.characters[cn].a_end, 10_000);
        });
    }

    #[test]
    fn skill_blast_rejects_self_and_stoned_targets() {
        with_test_gs(|gs| {
            let (cn, co) = caster_and_target(gs);
            gs.characters[cn].skill[SK_BLAST][5] = 50;
            gs.characters[cn].a_mana = 100_000;

            gs.characters[cn].skill_target1 = cn as u16;
            skill_blast(gs, cn);
            assert_eq!(gs.characters[cn].a_mana, 100_000);
            assert!(!is_exhausted(gs, cn));

            gs.characters[cn].skill_target1 = co as u16;
            gs.characters[co].flags |= CharacterFlags::Stoned.bits();
            skill_blast(gs, cn);
            assert_eq!(gs.characters[cn].a_mana, 100_000);
            assert!(!is_exhausted(gs, cn));
        });
    }

    #[test]
    fn skill_blast_is_refused_while_exhausted() {
        with_test_gs(|gs| {
            let (cn, _co) = caster_and_target(gs);
            gs.characters[cn].skill[SK_BLAST][5] = 50;
            gs.characters[cn].a_mana = 100_000;
            add_exhaust(gs, cn, 10);

            skill_blast(gs, cn);

            assert_eq!(gs.characters[cn].a_mana, 100_000);
        });
    }
}
