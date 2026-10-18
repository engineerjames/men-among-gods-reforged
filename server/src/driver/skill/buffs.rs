use super::common::{
    add_exhaust, add_spell, is_exhausted, player_or_ghost, spell_race_mod, spellcost, spellpower,
    unbuffed_skill_power,
};
use crate::chlog;
use crate::effect::EffectManager;
use crate::game_state::GameState;
use crate::god::God;
use core::constants::{CNTSAY, CharacterFlags, ItemFlags, MAXSAY, TICKS};
use core::skills::{
    SK_BLESS, SK_ENHANCE, SK_HEAL, SK_LIGHT, SK_MSHIELD, SK_PROTECT, SK_SENSE, SK_WIMPY,
};
use core::string_operations::c_string_to_str;
use core::types::FontColor;

/// Applies the Light spell to a target character.
///
/// # Arguments
///
/// * `gs` - Active game state used to create the spell item and emit feedback.
/// * `cn` - Caster character index.
/// * `co` - Target character index.
/// * `power` - Base spell power before race and moon modifiers.
///
/// # Returns
///
/// * `true` when Light was applied, or `false` when item creation or attachment failed.
///
/// # Panics
///
/// * Panics if `cn`, `co`, or the created spell item index is invalid.
pub fn spell_light(gs: &mut GameState, cn: usize, co: usize, power: i32) -> bool {
    // Ported from C++ spell_light(int cn, int co, int power)
    let in_ = God::create_item(gs, 1);
    if in_.is_none() {
        log::error!("god_create_item failed in spell_light");
        return false;
    }
    let power = spell_race_mod(gs, power, gs.characters[cn].kindred);
    {
        let in_idx = in_.unwrap();
        let mut name_bytes = [0u8; 40];
        let name = b"Light";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        gs.items[in_idx].name = name_bytes;
        gs.items[in_idx].flags |= ItemFlags::IF_SPELL.bits();
        gs.items[in_idx].light[1] = std::cmp::min(250, power * 4) as i16;
        gs.items[in_idx].sprite[1] = 85;
        gs.items[in_idx].duration = (TICKS * 60 * 30) as u32;
        gs.items[in_idx].active = (TICKS * 60 * 30) as u32;
        gs.items[in_idx].temp = SK_LIGHT as u16;
        gs.items[in_idx].power = power as u32;
    }
    if cn != co {
        if add_spell(gs, co, in_.unwrap()) == 0 {
            let name = gs.items[in_.unwrap()].get_name().to_owned();
            gs.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!("Magical interference neutralised the {}'s effect.\n", name),
            );
            return false;
        }
        let sense = gs.characters[co].skill[SK_SENSE][5];
        if sense + 10 > power as u16 {
            let reference = gs.characters[cn].reference;
            gs.do_character_log(
                co,
                core::types::FontColor::Green,
                &format!("{} cast light on you.\n", c_string_to_str(&reference)),
            );
        } else {
            gs.do_character_log(
                co,
                core::types::FontColor::Green,
                "You start to emit light.\n",
            );
        }
        let name = gs.characters[co].name;
        let (x, y) = (gs.characters[co].x, gs.characters[co].y);
        gs.do_area_log(
            co,
            0,
            i32::from(x),
            i32::from(y),
            core::types::FontColor::Green,
            &format!("{} starts to emit light.\n", c_string_to_str(&name)),
        );
        let sound = gs.characters[cn].sound;
        GameState::char_play_sound(gs, co, i32::from(sound) + 1, -150, 0);
        GameState::char_play_sound(gs, cn, i32::from(sound) + 1, -150, 0);
        let (x, y) = (gs.characters[co].x, gs.characters[co].y);
        EffectManager::fx_add_effect(gs, 7, 0, i32::from(x), i32::from(y), 0);
    } else {
        if add_spell(gs, cn, in_.unwrap()) == 0 {
            let name = gs.items[in_.unwrap()].get_name().to_owned();
            gs.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!("Magical interference neutralised the {}'s effect.\n", name),
            );
            return false;
        }
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "You start to emit light.\n",
        );
        let sound = gs.characters[cn].sound;
        GameState::char_play_sound(gs, cn, i32::from(sound) + 1, -150, 0);
        let flags = gs.characters[cn].flags;
        if (flags & CharacterFlags::Player.bits()) != 0 {
            chlog!(cn, "Cast Light");
        }
        let (x, y) = (gs.characters[cn].x, gs.characters[cn].y);
        EffectManager::fx_add_effect(gs, 7, 0, i32::from(x), i32::from(y), 0);
    }
    let (x, y) = (gs.characters[cn].x, gs.characters[cn].y);
    EffectManager::fx_add_effect(gs, 7, 0, i32::from(x), i32::from(y), 0);
    true
}

/// Handles direct player/NPC use of the Light skill.
///
/// # Arguments
///
/// * `gs` - Active game state used for target validation, costs, and spell application.
/// * `cn` - Caster character index.
///
/// # Panics
///
/// * Panics if `cn` or the selected target index is invalid.
pub fn skill_light(gs: &mut GameState, cn: usize) {
    // rate limit for player
    let is_player = (gs.characters[cn].flags & CharacterFlags::Player.bits()) != 0;
    if is_player {
        gs.characters[cn].data[71] = (gs.characters[cn].data[71] + CNTSAY).min(MAXSAY + CNTSAY);
        let over = gs.characters[cn].data[71] > MAXSAY;
        if over {
            gs.do_character_log(cn, FontColor::Red, "Oops, you're a bit too fast for me!\n");
            return;
        }
    }

    let co = if gs.characters[cn].skill_target1 != 0 {
        gs.characters[cn].skill_target1 as usize
    } else {
        cn
    };

    if gs.do_char_can_see(cn, co) == 0 {
        gs.do_character_log(cn, FontColor::Red, "You cannot see your target.\n");
        return;
    }

    if is_exhausted(gs, cn) {
        return;
    }

    if spellcost(gs, cn, 5) != 0 {
        return;
    }

    let light_skill = gs.characters[cn].skill[SK_LIGHT][5];
    spell_light(gs, cn, co, i32::from(light_skill));

    add_exhaust(gs, cn, TICKS / 4);
}

/// Applies the Protection spell to a target character.
///
/// # Arguments
///
/// * `gs` - Active game state used to create the spell item and emit feedback.
/// * `cn` - Caster character index.
/// * `co` - Target character index.
/// * `power` - Base spell power before target cap and race modifiers.
///
/// # Returns
///
/// * `true` when Protection was applied, or `false` when item creation or attachment failed.
///
/// # Panics
///
/// * Panics if `cn`, `co`, or the created spell item index is invalid.
pub fn spell_protect(gs: &mut GameState, cn: usize, co: usize, power: i32) -> bool {
    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_protect");
        return false;
    }
    let in_ = in_opt.unwrap();

    // cap power to target's spellpower
    let mut power = power;
    let target_spellpower = spellpower(&gs.characters[co]);
    if power > target_spellpower {
        if cn != co {
            let reference = gs.characters[co].reference;
            gs.do_character_log(
                cn,
                FontColor::Yellow,
                &format!(
                    "Seeing that {} is not powerful enough for your spell, you reduced its strength.\n",
                    c_string_to_str(&reference)
                ),
            );
        } else {
            gs.do_character_log(
                cn,
                FontColor::Green,
                "You are not powerful enough to use the full strength of this spell.\n",
            );
        }
        power = target_spellpower;
    }

    let power = spell_race_mod(gs, power, gs.characters[cn].kindred);

    {
        let mut name_bytes = [0u8; 40];
        let name = b"Protection";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        gs.items[in_].name = name_bytes;
        gs.items[in_].flags |= ItemFlags::IF_SPELL.bits();
        gs.items[in_].armor[1] = (power / 4 + 4) as i8;
        gs.items[in_].sprite[1] = 86;
        gs.items[in_].duration = (TICKS * 60 * 10) as u32;
        gs.items[in_].active = (TICKS * 60 * 10) as u32;
        gs.items[in_].temp = SK_PROTECT as u16;
        gs.items[in_].power = power as u32;
    }

    if cn != co {
        if add_spell(gs, co, in_) == 0 {
            let name = gs.items[in_].get_name().to_owned();
            gs.do_character_log(
                cn,
                FontColor::Green,
                &format!("Magical interference neutralised the {}'s effect.\n", name),
            );
            return false;
        }

        let sense = gs.characters[co].skill[SK_SENSE][5];
        if i32::from(sense) + 10 > power {
            let reference = gs.characters[cn].reference;
            gs.do_character_log(
                co,
                FontColor::Green,
                &format!("{} cast protect on you.\n", c_string_to_str(&reference)),
            );
        } else {
            gs.do_character_log(co, FontColor::Red, "You feel protected.\n");
        }

        let name = gs.characters[co].get_name().to_owned();
        gs.do_character_log(
            cn,
            FontColor::Yellow,
            &format!("{} is now protected.\n", name),
        );
        let sound = gs.characters[cn].sound;
        GameState::char_play_sound(gs, co, i32::from(sound) + 1, -150, 0);
        GameState::char_play_sound(gs, cn, i32::from(sound) + 1, -150, 0);
        let target_name = gs.characters[co].get_name().to_owned();
        chlog!(cn, "Cast Protect on {}", target_name);
        EffectManager::fx_add_effect(
            gs,
            6,
            0,
            i32::from(gs.characters[co].x),
            i32::from(gs.characters[co].y),
            0,
        );
    } else {
        if add_spell(gs, cn, in_) == 0 {
            let name = gs.items[in_].get_name().to_owned();
            gs.do_character_log(
                cn,
                FontColor::Green,
                &format!("Magical interference neutralised the {}'s effect.\n", name),
            );
            return false;
        }
        gs.do_character_log(cn, FontColor::Green, "You feel protected.\n");
        let sound = gs.characters[cn].sound;
        GameState::char_play_sound(gs, cn, i32::from(sound) + 1, -150, 0);
        let flags = gs.characters[cn].flags;
        if (flags & CharacterFlags::Player.bits()) != 0 {
            chlog!(cn, "Cast Protect");
        }
        let (x, y) = (gs.characters[cn].x, gs.characters[cn].y);
        EffectManager::fx_add_effect(gs, 6, 0, i32::from(x), i32::from(y), 0);
    }

    EffectManager::fx_add_effect(
        gs,
        7,
        0,
        i32::from(gs.characters[cn].x),
        i32::from(gs.characters[cn].y),
        0,
    );

    true
}

/// Handles direct player/NPC use of the Protection skill.
///
/// # Arguments
///
/// * `gs` - Active game state used for target validation, costs, and spell application.
/// * `cn` - Caster character index.
///
/// # Panics
///
/// * Panics if `cn` or the selected target index is invalid.
pub fn skill_protect(gs: &mut GameState, cn: usize) {
    let has_skill = gs.characters[cn].skill[SK_PROTECT][5] != 0;
    if !has_skill {
        return;
    }

    let mut co = if gs.characters[cn].skill_target1 != 0 {
        gs.characters[cn].skill_target1 as usize
    } else {
        cn
    };

    if gs.do_char_can_see(cn, co) == 0 {
        gs.do_character_log(cn, FontColor::Red, "You cannot see your target.\n");
        return;
    }

    if is_exhausted(gs, cn) {
        return;
    }

    if !player_or_ghost(&gs.characters[cn], cn, &gs.characters[co]) {
        let name_from = gs.characters[co].get_name().to_owned();
        let name_to = gs.characters[cn].get_name().to_owned();
        gs.do_character_log(
            cn,
            FontColor::Red,
            &format!(
                "Changed target of spell from {} to {}.\n",
                name_from, name_to
            ),
        );
        co = cn;
    }

    if spellcost(gs, cn, 15) != 0 {
        return;
    }

    let power = i32::from(gs.characters[cn].skill[SK_PROTECT][5]);
    spell_protect(gs, cn, co, power);

    add_exhaust(gs, cn, TICKS / 2);
}

/// Applies the Enhance Weapon spell to a target character.
///
/// # Arguments
///
/// * `gs` - Active game state used to create the spell item and emit feedback.
/// * `cn` - Caster character index.
/// * `co` - Target character index.
/// * `power` - Base spell power before target cap and race modifiers.
///
/// # Returns
///
/// * `true` when Enhance Weapon was applied, or `false` when item creation or attachment failed.
///
/// # Panics
///
/// * Panics if `cn`, `co`, or the created spell item index is invalid.
pub fn spell_enhance(gs: &mut GameState, cn: usize, co: usize, power: i32) -> bool {
    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_enhance");
        return false;
    }
    let in_ = in_opt.unwrap();

    // cap power to target's spellpower
    let mut power = power;
    let target_spellpower = spellpower(&gs.characters[co]);
    if power > target_spellpower {
        if cn != co {
            let reference = gs.characters[co].reference;
            gs.do_character_log(
                cn,
                FontColor::Yellow,
                &format!(
                    "Seeing that {} is not powerful enough for your spell, you reduced its strength.\n",
                    c_string_to_str(&reference)
                ),
            );
        } else {
            gs.do_character_log(
                cn,
                FontColor::Yellow,
                "You are not powerful enough to use the full strength of this spell.\n",
            );
        }
        power = target_spellpower;
    }

    let power = spell_race_mod(gs, power, gs.characters[cn].kindred);

    {
        let mut name_bytes = [0u8; 40];
        let name = b"Enhance Weapon";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        gs.items[in_].name = name_bytes;
        gs.items[in_].flags |= ItemFlags::IF_SPELL.bits();
        gs.items[in_].weapon[1] = (power / 4 + 4) as i8;
        gs.items[in_].sprite[1] = 87;
        gs.items[in_].duration = (TICKS * 60 * 10) as u32;
        gs.items[in_].active = (TICKS * 60 * 10) as u32;
        gs.items[in_].temp = SK_ENHANCE as u16;
        gs.items[in_].power = power as u32;
    }

    if cn != co {
        if add_spell(gs, co, in_) == 0 {
            let name = gs.items[in_].get_name().to_owned();
            gs.do_character_log(
                cn,
                FontColor::Yellow,
                &format!("Magical interference neutralised the {}'s effect.\n", name),
            );
            return false;
        }
        let sense = gs.characters[co].skill[SK_SENSE][5];
        if i32::from(sense) + 10 > power {
            let reference = gs.characters[cn].reference;
            gs.do_character_log(
                co,
                FontColor::Yellow,
                &format!(
                    "{} cast enhance weapon on you.\n",
                    c_string_to_str(&reference)
                ),
            );
        } else {
            gs.do_character_log(co, FontColor::Red, "Your weapon feels stronger.\n");
        }
        gs.do_character_log(
            cn,
            FontColor::Yellow,
            &format!(
                "{}'s weapon is now stronger.\n",
                gs.characters[co].get_name().to_owned()
            ),
        );
        let sound = gs.characters[cn].sound;
        GameState::char_play_sound(gs, co, i32::from(sound) + 1, -150, 0);
        GameState::char_play_sound(gs, cn, i32::from(sound) + 1, -150, 0);
        let target_name = gs.characters[co].get_name().to_owned();
        chlog!(cn, "Cast Enhance on {}", target_name);

        EffectManager::fx_add_effect(
            gs,
            6,
            0,
            i32::from(gs.characters[co].x),
            i32::from(gs.characters[co].y),
            0,
        );
    } else {
        if add_spell(gs, cn, in_) == 0 {
            let name = gs.items[in_].get_name().to_owned();
            gs.do_character_log(
                cn,
                FontColor::Yellow,
                &format!("Magical interference neutralised the {}'s effect.\n", name),
            );
            return false;
        }
        gs.do_character_log(cn, FontColor::Green, "Your weapon feels stronger.\n");
        let sound = gs.characters[cn].sound;
        GameState::char_play_sound(gs, cn, i32::from(sound) + 1, -150, 0);
        let flags = gs.characters[cn].flags;
        if (flags & CharacterFlags::Player.bits()) != 0 {
            chlog!(cn, "Cast Enhance");
        }
        EffectManager::fx_add_effect(
            gs,
            6,
            0,
            i32::from(gs.characters[cn].x),
            i32::from(gs.characters[cn].y),
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

    true
}

/// Handles direct player/NPC use of the Enhance Weapon skill.
///
/// # Arguments
///
/// * `gs` - Active game state used for target validation, costs, and spell application.
/// * `cn` - Caster character index.
///
/// # Panics
///
/// * Panics if `cn` or the selected target index is invalid.
pub fn skill_enhance(gs: &mut GameState, cn: usize) {
    let co = if gs.characters[cn].skill_target1 != 0 {
        gs.characters[cn].skill_target1 as usize
    } else {
        cn
    };

    if gs.do_char_can_see(cn, co) == 0 {
        gs.do_character_log(cn, FontColor::Red, "You cannot see your target.\n");
        return;
    }

    if is_exhausted(gs, cn) {
        return;
    }

    if !player_or_ghost(&gs.characters[cn], cn, &gs.characters[co]) {
        let name_from = gs.characters[co].get_name().to_owned();
        let name_to = gs.characters[cn].get_name().to_owned();
        gs.do_character_log(
            cn,
            FontColor::Red,
            &format!(
                "Changed target of spell from {} to {}.\n",
                name_from, name_to
            ),
        );
        // change target to self
        let co = cn;
        // continue with self
        if spellcost(gs, cn, 15) != 0 {
            return;
        }
        let power = i32::from(gs.characters[cn].skill[SK_ENHANCE][5]);
        spell_enhance(gs, cn, co, power);
        add_exhaust(gs, cn, TICKS / 2);
        return;
    }

    if spellcost(gs, cn, 15) != 0 {
        return;
    }

    let power = i32::from(gs.characters[cn].skill[SK_ENHANCE][5]);
    spell_enhance(gs, cn, co, power);
    add_exhaust(gs, cn, TICKS / 2);
}

/// Applies the Bless spell to a target character.
///
/// # Arguments
///
/// * `gs` - Active game state used to create the spell item and emit feedback.
/// * `cn` - Caster character index.
/// * `co` - Target character index.
/// * `power` - Base spell power before target cap and race modifiers.
///
/// # Returns
///
/// * `true` when Bless was applied, or `false` when item creation or attachment failed.
///
/// # Panics
///
/// * Panics if `cn`, `co`, or the created spell item index is invalid.
pub fn spell_bless(gs: &mut GameState, cn: usize, co: usize, power: i32) -> bool {
    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_bless");
        return false;
    }
    let in_ = in_opt.unwrap();

    let mut power = power;
    let tmp = spellpower(&gs.characters[co]);
    if power > tmp {
        if cn != co {
            let reference = gs.characters[co].reference;
            gs.do_character_log(
                cn,
                FontColor::Yellow,
                &format!(
                    "Seeing that {} is not powerful enough for your spell, you reduced its strength.\n",
                    c_string_to_str(&reference)
                ),
            );
        } else {
            gs.do_character_log(
                cn,
                FontColor::Yellow,
                "You are not powerful enough to use the full strength of this spell.\n",
            );
        }
        power = tmp;
    }

    let power = spell_race_mod(gs, power, gs.characters[cn].kindred);

    {
        let mut name_bytes = [0u8; 40];
        let name = b"Bless";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        gs.items[in_].name = name_bytes;
        gs.items[in_].flags |= ItemFlags::IF_SPELL.bits();
        for n in 0..5 {
            gs.items[in_].attrib[n][1] = (power / 5 + 3) as i16;
        }
        gs.items[in_].sprite[1] = 88; // TODO: assign unique spell-effect sprite ID once buff sprite assets are finalized
        gs.items[in_].duration = (TICKS * 60 * 10) as u32;
        gs.items[in_].active = (TICKS * 60 * 10) as u32;
        gs.items[in_].temp = SK_BLESS as u16;
        gs.items[in_].power = power as u32;
    }

    if cn != co {
        if add_spell(gs, co, in_) == 0 {
            let name = gs.items[in_].get_name().to_owned();
            gs.do_character_log(
                cn,
                FontColor::Yellow,
                &format!("Magical interference neutralised the {}'s effect.\n", name),
            );
            return false;
        }
        let sense = gs.characters[co].skill[SK_SENSE][5];
        if i32::from(sense) + 10 > power {
            let reference = gs.characters[cn].reference;
            gs.do_character_log(
                co,
                FontColor::Yellow,
                &format!("{} cast bless on you.\n", c_string_to_str(&reference)),
            );
        } else {
            gs.do_character_log(co, FontColor::Red, "You have been blessed.\n");
        }
        gs.do_character_log(
            cn,
            FontColor::Yellow,
            &format!("{} was blessed.\n", gs.characters[co].get_name().to_owned()),
        );
        let sound = gs.characters[cn].sound;
        GameState::char_play_sound(gs, co, i32::from(sound) + 1, -150, 0);
        GameState::char_play_sound(gs, cn, i32::from(sound) + 1, -150, 0);
        chlog!(
            cn,
            "Cast Bless on {}",
            gs.characters[co].get_name().to_owned()
        );
        EffectManager::fx_add_effect(
            gs,
            6,
            0,
            i32::from(gs.characters[co].x),
            i32::from(gs.characters[co].y),
            0,
        );
    } else {
        if add_spell(gs, cn, in_) == 0 {
            let name = gs.items[in_].get_name().to_owned();
            gs.do_character_log(
                cn,
                FontColor::Yellow,
                &format!("Magical interference neutralised the {}'s effect.\n", name),
            );
            return false;
        }
        gs.do_character_log(cn, FontColor::Green, "You have been blessed.\n");
        let sound = gs.characters[cn].sound;
        GameState::char_play_sound(gs, cn, i32::from(sound) + 1, -150, 0);
        let flags = gs.characters[cn].flags;
        if (flags & CharacterFlags::Player.bits()) != 0 {
            chlog!(cn, "Cast Bless");
        }
        EffectManager::fx_add_effect(
            gs,
            6,
            0,
            i32::from(gs.characters[cn].x),
            i32::from(gs.characters[cn].y),
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

    true
}

/// Handles direct player/NPC use of the Bless skill.
///
/// # Arguments
///
/// * `gs` - Active game state used for target validation, costs, and spell application.
/// * `cn` - Caster character index.
///
/// # Panics
///
/// * Panics if `cn` or the selected target index is invalid.
pub fn skill_bless(gs: &mut GameState, cn: usize) {
    let co = if gs.characters[cn].skill_target1 != 0 {
        gs.characters[cn].skill_target1 as usize
    } else {
        cn
    };

    if gs.do_char_can_see(cn, co) == 0 {
        gs.do_character_log(cn, FontColor::Red, "You cannot see your target.\n");
        return;
    }

    if is_exhausted(gs, cn) {
        return;
    }

    if !player_or_ghost(&gs.characters[cn], cn, &gs.characters[co]) {
        let name_from = gs.characters[co].get_name().to_owned();
        let name_to = gs.characters[cn].get_name().to_owned();
        gs.do_character_log(
            cn,
            FontColor::Red,
            &format!(
                "Changed target of spell from {} to {}.\n",
                name_from, name_to
            ),
        );
        // change target to self
        let co = cn;
        if spellcost(gs, cn, 35) != 0 {
            return;
        }
        spell_bless(gs, cn, co, unbuffed_skill_power(gs, cn, SK_BLESS));
        add_exhaust(gs, cn, TICKS);
        return;
    }

    if spellcost(gs, cn, 35) != 0 {
        return;
    }

    spell_bless(gs, cn, co, unbuffed_skill_power(gs, cn, SK_BLESS));
    add_exhaust(gs, cn, TICKS);
}

/// Toggles the Guardian Angel protective spell for a character.
///
/// # Arguments
///
/// * `gs` - Active game state used to inspect spells, create the guardian item, and emit feedback.
/// * `cn` - Character index activating or dismissing Guardian Angel.
///
/// # Panics
///
/// * Panics if `cn`, an active spell item index, or the created spell item index is invalid.
pub fn skill_wimp(gs: &mut GameState, cn: usize) {
    // If Guardian Angel already active, remove it
    for n in 0..20 {
        let in_idx = gs.characters[cn].spell[n];
        if in_idx != 0 {
            let temp = gs.items[in_idx as usize].temp;
            if temp == SK_WIMPY as u16 {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Green,
                    "Guardian Angel no longer active.\n",
                );
                gs.items[in_idx as usize].used = core::constants::USE_EMPTY;
                gs.characters[cn].spell[n] = 0;
                gs.do_update_char(cn);
                chlog!(cn, "Dismissed Guardian Angel");
                return;
            }
        }
    }

    let a_end = gs.characters[cn].a_end;
    if a_end < 20000 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "You're too exhausted to call on your Guardian Angel.\n",
        );
        return;
    }

    gs.characters[cn].a_end -= 20000;

    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_wimp");
        return;
    }
    let in_idx = in_opt.unwrap();

    {
        let mut name_bytes = [0u8; 40];
        let name = b"Guardian Angel";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        gs.items[in_idx].name = name_bytes;
        gs.items[in_idx].flags |= ItemFlags::IF_SPELL.bits() | ItemFlags::IF_PERMSPELL.bits();
        gs.items[in_idx].hp[0] = -1;
        gs.items[in_idx].end[0] = -1;
        gs.items[in_idx].mana[0] = -1;
        gs.items[in_idx].sprite[1] = 94;
        gs.items[in_idx].duration = (TICKS * 60 * 60 * 2) as u32;
        gs.items[in_idx].active = (TICKS * 60 * 60 * 2) as u32;
        gs.items[in_idx].temp = SK_WIMPY as u16;
        gs.items[in_idx].power = u32::from(gs.characters[cn].skill[SK_WIMPY][5]);
    }

    if add_spell(gs, cn, in_idx) == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!(
                "Magical interference neutralised the {}'s effect.\n",
                gs.items[in_idx].get_name().to_owned()
            ),
        );
        return;
    }
    gs.do_character_log(
        cn,
        core::types::FontColor::Green,
        "Guardian Angel active!\n",
    );
    let sound = gs.characters[cn].sound;
    GameState::char_play_sound(gs, cn, i32::from(sound) + 1, -150, 0);
    chlog!(cn, "Cast Guardian Angel");
    EffectManager::fx_add_effect(
        gs,
        7,
        0,
        i32::from(gs.characters[cn].x),
        i32::from(gs.characters[cn].y),
        0,
    );
    EffectManager::fx_add_effect(
        gs,
        6,
        0,
        i32::from(gs.characters[cn].x),
        i32::from(gs.characters[cn].y),
        0,
    );
}

/// Applies the Magic Shield spell to a target character.
///
/// # Arguments
///
/// * `gs` - Active game state used to create the spell item and emit feedback.
/// * `cn` - Caster character index.
/// * `co` - Target character index.
/// * `power` - Base spell power before race and moon modifiers.
///
/// # Returns
///
/// * `true` when Magic Shield was applied, or `false` when item creation or attachment failed.
///
/// # Panics
///
/// * Panics if `cn`, `co`, or the created spell item index is invalid.
pub fn spell_mshield(gs: &mut GameState, cn: usize, co: usize, power: i32) -> bool {
    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_mshield");
        return false;
    }
    let in_ = in_opt.unwrap();

    {
        let mut name_bytes = [0u8; 40];
        let name = b"Magic Shield";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        gs.items[in_].name = name_bytes;
        gs.items[in_].flags |= ItemFlags::IF_SPELL.bits();
        gs.items[in_].sprite[1] = 95;
        let dur = spell_race_mod(gs, power * 256, gs.characters[cn].kindred);
        gs.items[in_].duration = dur as u32;
        gs.items[in_].active = dur as u32;
        gs.items[in_].armor[1] = (gs.items[in_].active / 1024) as i8 + 1;
        gs.items[in_].temp = SK_MSHIELD as u16;
        gs.items[in_].power = gs.items[in_].active / 256;
    }

    if cn != co {
        if add_spell(gs, co, in_) == 0 {
            let name = gs.items[in_].get_name().to_owned();
            gs.do_character_log(
                cn,
                FontColor::Green,
                &format!("Magical interference neutralised the {}'s effect.\n", name),
            );
            return false;
        }
        let sense = gs.characters[co].skill[SK_SENSE][5];
        if i32::from(sense) + 10 > power {
            let reference = gs.characters[cn].reference;
            gs.do_character_log(
                co,
                FontColor::Green,
                &format!(
                    "{} cast magic shield on you.\n",
                    c_string_to_str(&reference)
                ),
            );
        } else {
            gs.do_character_log(co, FontColor::Red, "Magic Shield active!\n");
        }
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!(
                "{}'s Magic Shield activated.\n",
                gs.characters[co].get_name().to_owned()
            ),
        );
        let sound = gs.characters[cn].sound;
        GameState::char_play_sound(gs, co, i32::from(sound) + 1, -150, 0);
        GameState::char_play_sound(gs, cn, i32::from(sound) + 1, -150, 0);
        chlog!(
            cn,
            "Cast Magic Shield on {}",
            gs.characters[co].get_name().to_owned()
        );
        EffectManager::fx_add_effect(
            gs,
            6,
            0,
            i32::from(gs.characters[co].x),
            i32::from(gs.characters[co].y),
            0,
        );
    } else {
        if add_spell(gs, cn, in_) == 0 {
            let name = gs.items[in_].get_name().to_owned();
            gs.do_character_log(
                cn,
                FontColor::Green,
                &format!("Magical interference neutralised the {}'s effect.\n", name),
            );
            return false;
        }
        gs.do_character_log(cn, FontColor::Green, "Magic Shield active!\n");
        let sound = gs.characters[cn].sound;
        GameState::char_play_sound(gs, cn, i32::from(sound) + 1, -150, 0);
        let flags = gs.characters[cn].flags;
        if (flags & CharacterFlags::Player.bits()) != 0 {
            chlog!(cn, "Cast Magic Shield");
        }
        EffectManager::fx_add_effect(
            gs,
            6,
            0,
            i32::from(gs.characters[cn].x),
            i32::from(gs.characters[cn].y),
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

    true
}

/// Handles direct player/NPC use of the Magic Shield skill.
///
/// # Arguments
///
/// * `gs` - Active game state used for costs, focus checks, and spell application.
/// * `cn` - Caster character index.
///
/// # Panics
///
/// * Panics if `cn` is not a valid character index.
pub fn skill_mshield(gs: &mut GameState, cn: usize) {
    if is_exhausted(gs, cn) {
        return;
    }

    if spellcost(gs, cn, 25) != 0 {
        return;
    }

    spell_mshield(
        gs,
        cn,
        cn,
        i32::from(gs.characters[cn].skill[SK_MSHIELD][5]),
    );
    add_exhaust(gs, cn, core::constants::TICKS * 3);
}

/// Heals a target character and emits spell feedback.
///
/// # Arguments
///
/// * `gs` - Active game state used to adjust hit points and emit feedback.
/// * `cn` - Caster character index.
/// * `co` - Target character index.
/// * `power` - Healing power used to restore hit points.
///
/// # Returns
///
/// * Always `true` after applying the heal.
///
/// # Panics
///
/// * Panics if `cn` or `co` is not a valid character index.
pub fn spell_heal(gs: &mut GameState, cn: usize, co: usize, power: i32) -> bool {
    if cn != co {
        gs.characters[co].a_hp += spell_race_mod(gs, power * 2500, gs.characters[cn].kindred);
        if gs.characters[co].a_hp > i32::from(gs.characters[co].hp[5]) * 1000 {
            gs.characters[co].a_hp = i32::from(gs.characters[co].hp[5]) * 1000;
        }
        let sense = gs.characters[co].skill[SK_SENSE][5];
        if i32::from(sense) + 10 > power {
            let reference = gs.characters[cn].reference;
            gs.do_character_log(
                co,
                FontColor::Green,
                &format!("{} cast heal on you.\n", c_string_to_str(&reference)),
            );
        } else {
            gs.do_character_log(co, FontColor::Red, "You have been healed.\n");
        }
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("{} was healed.\n", gs.characters[co].get_name().to_owned()),
        );
        let sound = gs.characters[cn].sound;
        GameState::char_play_sound(gs, co, i32::from(sound) + 1, -150, 0);
        GameState::char_play_sound(gs, cn, i32::from(sound) + 1, -150, 0);
        chlog!(
            cn,
            "Cast Heal on {}",
            gs.characters[co].get_name().to_owned()
        );
        EffectManager::fx_add_effect(
            gs,
            6,
            0,
            i32::from(gs.characters[co].x),
            i32::from(gs.characters[co].y),
            0,
        );
    } else {
        gs.characters[cn].a_hp += power * 2500;
        if gs.characters[cn].a_hp > i32::from(gs.characters[cn].hp[5]) * 1000 {
            gs.characters[cn].a_hp = i32::from(gs.characters[cn].hp[5]) * 1000;
        }
        gs.do_character_log(cn, FontColor::Green, "You have been healed.\n");
        let sound = gs.characters[cn].sound;
        GameState::char_play_sound(gs, cn, i32::from(sound) + 1, -150, 0);
        let flags = gs.characters[cn].flags;
        if (flags & CharacterFlags::Player.bits()) != 0 {
            chlog!(cn, "Cast Heal");
        }
        EffectManager::fx_add_effect(
            gs,
            6,
            0,
            i32::from(gs.characters[cn].x),
            i32::from(gs.characters[cn].y),
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

    true
}

/// Handles direct player/NPC use of the Heal skill.
///
/// # Arguments
///
/// * `gs` - Active game state used for target validation, costs, and healing.
/// * `cn` - Caster character index.
///
/// # Panics
///
/// * Panics if `cn` or the selected target index is invalid.
pub fn skill_heal(gs: &mut GameState, cn: usize) {
    let mut co = if gs.characters[cn].skill_target1 != 0 {
        gs.characters[cn].skill_target1 as usize
    } else {
        cn
    };

    if gs.do_char_can_see(cn, co) == 0 {
        gs.do_character_log(cn, FontColor::Red, "You cannot see your target.\n");
        return;
    }

    if is_exhausted(gs, cn) {
        return;
    }

    if !player_or_ghost(&gs.characters[cn], cn, &gs.characters[co]) {
        let name_from = gs.characters[co].get_name().to_owned();
        let name_to = gs.characters[cn].get_name().to_owned();
        gs.do_character_log(
            cn,
            FontColor::Red,
            &format!(
                "Changed target of spell from {} to {}.\n",
                name_from, name_to
            ),
        );
        co = cn;
        if spellcost(gs, cn, 25) != 0 {
            return;
        }
        spell_heal(gs, cn, co, i32::from(gs.characters[cn].skill[SK_HEAL][5]));
        add_exhaust(gs, cn, TICKS * 2);
        return;
    }

    if spellcost(gs, cn, 25) != 0 {
        return;
    }

    spell_heal(gs, cn, co, i32::from(gs.characters[cn].skill[SK_HEAL][5]));

    add_exhaust(gs, cn, TICKS * 2);
}
