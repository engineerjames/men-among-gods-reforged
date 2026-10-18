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
use core::constants::{CharacterFlags, ItemFlags, NT_DIDHIT, NT_GOTHIT, TICKS};
use core::skills::{SK_CURSE, SK_IMMUN, SK_RESIST, SK_SENSE};
use core::string_operations::c_string_to_str;
use core::types::FontColor;

/// Applies the Curse spell to a target character.
///
/// # Arguments
///
/// * `gs` - Active game state used to create the curse item and emit combat feedback.
/// * `cn` - Caster character index.
/// * `co` - Target character index.
/// * `power` - Base curse power before immunity and race modifiers.
///
/// # Returns
///
/// * `true` when Curse was applied, or `false` when the target is immune, item creation fails, or attachment is neutralized.
///
/// # Panics
///
/// * Panics if `cn`, `co`, or the created spell item index is invalid.
pub fn spell_curse(gs: &mut GameState, cn: usize, co: usize, power: i32) -> bool {
    let flags = gs.characters[co].flags;
    if (flags & CharacterFlags::Immortal.bits()) != 0 {
        return false;
    }

    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in spell_curse");
        return false;
    }
    let in_idx = in_opt.unwrap();

    let mut power = power;
    power = spell_immunity(gs, power, i32::from(gs.characters[co].skill[SK_IMMUN][5]));
    power = spell_race_mod(gs, power, gs.characters[cn].kindred);

    {
        let mut name_bytes = [0u8; 40];
        let name = b"Curse";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        gs.items[in_idx].name = name_bytes;
        gs.items[in_idx].flags |= ItemFlags::IF_SPELL.bits();
        for n in 0..5 {
            gs.items[in_idx].attrib[n][1] = -((power / 3) as i16);
        }
        gs.items[in_idx].sprite[1] = 89;
        gs.items[in_idx].duration = (TICKS * 60 * 2) as u32;
        gs.items[in_idx].active = (TICKS * 60 * 2) as u32;
        gs.items[in_idx].temp = SK_CURSE as u16;
        gs.items[in_idx].power = power as u32;
    }

    if add_spell(gs, co, in_idx) == 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!(
                "Magical interference neutralised the {}'s effect.\n",
                gs.items[in_idx].get_name().to_owned()
            ),
        );
        return false;
    }

    let sense = gs.characters[co].skill[SK_SENSE][5];
    if (i32::from(sense) + 10) > power {
        let reference = gs.characters[cn].reference;
        gs.do_character_log(
            co,
            FontColor::Green,
            &format!("{} cast curse on you.\n", c_string_to_str(&reference)),
        );
    } else {
        gs.do_character_log(co, FontColor::Green, "You have been cursed.\n");
    }

    let name = gs.characters[co].get_name().to_owned();
    gs.do_character_log(cn, FontColor::Green, &format!("{} was cursed.\n", name));

    // Match C: don't generate spell-attack notifications when the target is ignoring spells.
    if (gs.characters[co].flags & CharacterFlags::SpellIgnore.bits()) == 0 {
        gs.do_notify_character(co as u32, i32::from(NT_GOTHIT), cn as i32, 0, 0, 0);
    }
    gs.do_notify_character(cn as u32, i32::from(NT_DIDHIT), co as i32, 0, 0, 0);

    let sound = gs.characters[cn].sound;
    GameState::char_play_sound(gs, co, i32::from(sound) + 7, -150, 0);
    GameState::char_play_sound(gs, cn, i32::from(sound) + 1, -150, 0);
    chlog!(
        cn,
        "Cast Curse on {}",
        gs.characters[co].get_name().to_owned()
    );
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

/// Handles direct player/NPC use of the Curse skill, including area expansion.
///
/// # Arguments
///
/// * `gs` - Active game state used for target validation, combat checks, and spell application.
/// * `cn` - Caster character index.
///
/// # Panics
///
/// * Panics if `cn`, the selected target index, or an area target index is invalid.
pub fn skill_curse(gs: &mut GameState, cn: usize) {
    let co = resolve_offensive_target(gs, cn);

    if cn == co {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "You cannot curse yourself.\n",
        );
        return;
    }

    if gs.do_char_can_see(cn, co) == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "You cannot see your target.\n",
        );
        return;
    }

    gs.remember_pvp(cn, co);
    if is_exhausted(gs, cn) {
        return;
    }

    if spellcost(gs, cn, 35) != 0 {
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

    cast_curse_effect(gs, cn, co, SK_CURSE);

    add_exhaust(gs, cn, core::constants::TICKS * 4);
}

/// Rolls the resist/sense chance and applies Curse (with AoE splash) to `co`
/// using the caster's selected source skill level.
///
/// Shared by the direct Curse cast (`skill_curse`, after its mana/cooldown
/// gate) and the Seyan'Du Free Curse rune proc (no gate). Does not check
/// exhaustion, mana cost, or `may_attack_msg` — callers must do so first.
///
/// # Arguments
///
/// * `gs` - Active game state used for the resist roll, item creation, and effects.
/// * `cn` - Caster character index.
/// * `co` - Primary target character index.
/// * `source_skill` - Skill row that supplies Curse power and area scaling.
pub fn cast_curse_effect(gs: &mut GameState, cn: usize, co: usize, source_skill: usize) {
    let source_power = gs.characters[cn].skill[source_skill][5];
    if chance_base(
        gs,
        cn,
        i32::from(source_power),
        10,
        effective_resist(gs, cn, i32::from(gs.characters[co].skill[SK_RESIST][5])),
    ) != 0
    {
        if cn != co && gs.characters[co].skill[SK_SENSE][5] > (source_power + 5) {
            let reference = gs.characters[cn].reference;
            gs.do_character_log(
                co,
                core::types::FontColor::Green,
                &format!(
                    "{} tried to cast curse on you but failed.\n",
                    c_string_to_str(&reference)
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

    apply_curse_effect_after_focus(gs, cn, co, source_skill);
}

/// Applies Curse to the primary target and eligible area targets after the
/// caster has passed the focus check.
///
/// # Arguments
///
/// * `gs` - Active game state used for spell creation and area targeting.
/// * `cn` - Caster character index.
/// * `co` - Primary target character index.
/// * `source_skill` - Skill row that supplies Curse power and area scaling.
pub(super) fn apply_curse_effect_after_focus(
    gs: &mut GameState,
    cn: usize,
    co: usize,
    source_skill: usize,
) {
    let source_power = gs.characters[cn].skill[source_skill][5];

    if (gs.characters[co].flags & CharacterFlags::Immortal.bits()) != 0 {
        gs.do_character_log(cn, core::types::FontColor::Red, "You lost your focus.\n");
        return;
    }

    spell_curse(gs, cn, co, i32::from(source_power));

    let co_orig = co;
    let curse_base = i32::from(gs.characters[cn].skill[source_skill][0]);
    let curse_power = i32::from(source_power);
    let aoe_base = if (gs.characters[cn].flags & CharacterFlags::Player.bits()) != 0 {
        curse_base
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
        if use_legacy_cross && gs.characters[maybe_co].attack_cn as usize != cn {
            continue;
        }
        if !gs.may_aoe_attack(cn, maybe_co) {
            continue;
        }
        if curse_power + helpers::random_mod_i32(20)
            > effective_resist(
                gs,
                cn,
                i32::from(gs.characters[maybe_co].skill[SK_RESIST][5]),
            ) + helpers::random_mod_i32(20)
            && spell_curse(gs, cn, maybe_co, curse_power)
        {
            gs.remember_pvp(cn, maybe_co);
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
}
