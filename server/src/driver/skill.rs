use core::{
    constants::{
        AT_AGIL, AT_STREN, CHD_COMPANION, CHD_TALKATIVE, CNTSAY, COMPANION_TIMEOUT, CT_COMPANION,
        CharacterFlags, DX_DOWN, DX_LEFT, DX_RIGHT, DX_UP, ItemFlags, MAXSAY, NT_DIDHIT, NT_GOTHIT,
        NT_GOTMISS, TICKS, USE_EMPTY,
    },
    skills::{
        SK_AXE, SK_BLADE_DANCE, SK_BLAST, SK_BLESS, SK_CONCEN, SK_CONTAGION, SK_CURSE, SK_DAGGER,
        SK_DELIVER_DEATH, SK_DISARM, SK_DISPEL, SK_DISTRACT, SK_ENHANCE, SK_GASH, SK_GHOST,
        SK_HEAL, SK_IDENT, SK_IMMUN, SK_INNER_STRENGTH, SK_LIGHT, SK_LOCK, SK_MEDIT, SK_MSHIELD,
        SK_PARASITE, SK_PROTECT, SK_RAINS_OF_RENEWAL, SK_RECALL, SK_REGEN, SK_REPAIR, SK_RESIST,
        SK_REST, SK_SEEING_RED, SK_SENSE, SK_STAFF, SK_STUN, SK_SUNS_BLESSING, SK_SUNS_BLESSING2,
        SK_SURROUND, SK_SWORD, SK_THUNDEROUS_FURY, SK_TWOHAND, SK_WARCRY, SK_WARCRY2, SK_WEAPON,
        SK_WIMPY, attribute_name, get_skill_name,
    },
    string_operations::c_string_to_str,
    traits::{
        KIN_ARCHHARAKIM, KIN_ARCHTEMPLAR, KIN_HARAKIM, KIN_MERCENARY, KIN_MONSTER, KIN_SEYAN_DU,
        KIN_SORCERER, KIN_TEMPLAR, KIN_WARRIOR,
    },
    types::FontColor,
};

use crate::{
    chlog, driver, effect::EffectManager, game_state::GameState, god::God, helpers, points,
    populate,
};
use core::types::Character;

use core::constants::LEGACY_TICKS;

/// Returns whether `co` is a player or the ghost companion owned by `cn`.
///
/// # Arguments
///
/// * `cn` - Candidate owner character.
/// * `cn_idx` - Character index for `cn`.
/// * `co` - Candidate target character.
///
/// # Returns
///
/// * `true` when `cn` is a player and `co` is either a player or `cn`'s ghost companion.
pub fn player_or_ghost(cn: &Character, cn_idx: usize, co: &Character) -> bool {
    if (cn.flags & CharacterFlags::Player.bits()) == 0 {
        return false;
    }
    if (co.flags & CharacterFlags::Player.bits()) != 0 {
        return true;
    }
    if co.data[63] as usize == cn_idx {
        return true;
    }
    false
}

/// Consumes mana for a spell after applying Concentration cost reduction.
///
/// # Arguments
///
/// * `gs` - Active game state containing character mana and skills.
/// * `cn` - Caster character index.
/// * `cost` - Base spell cost in whole mana units.
///
/// # Returns
///
/// * `0` when mana was consumed successfully, or `-1` when the caster lacks mana.
///
/// # Panics
///
/// * Panics if `cn` is not a valid character index.
pub fn spellcost(gs: &mut GameState, cn: usize, cost: i32) -> i32 {
    // Ported from C++ spellcost(int cn, int cost)
    // concentrate:
    let mut cost = cost;
    let concen_skill = gs.characters[cn].skill[SK_CONCEN][0];
    if concen_skill != 0 {
        let concen_val = gs.characters[cn].skill[SK_CONCEN][5];
        let t = cost * i32::from(concen_val) / 300;
        if t > cost {
            cost = 1;
        } else {
            cost -= t;
        }
    }
    let a_mana = gs.characters[cn].a_mana;
    if cost * 1000 > a_mana {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "You don't have enough mana.\n",
        );
        return -1;
    }
    gs.characters[cn].a_mana = a_mana - cost * 1000;
    0
}

/// Performs a focus check against an opposing skill or resistance power.
///
/// # Arguments
///
/// * `gs` - Active game state used for caster state and failure messages.
/// * `cn` - Caster character index.
/// * `skill` - Caster skill value used for the check.
/// * `d20` - Base d20 threshold before luck adjustment.
/// * `power` - Opposing power or resistance value.
///
/// # Returns
///
/// * `0` when focus succeeds, or `-1` when focus is lost.
///
/// # Panics
///
/// * Panics if `cn` is not a valid character index.
pub fn chance_base(gs: &mut GameState, cn: usize, skill: i32, d20: i32, power: i32) -> i32 {
    // Ported from C++ chance_base(int cn, int skill, int d20, int power)
    let mut chance = d20 * skill / std::cmp::max(1, power);
    let (flags, luck) = (gs.characters[cn].flags, gs.characters[cn].luck);
    if (flags & CharacterFlags::Player.bits()) != 0 && luck < 0 {
        chance += luck / 500 - 1;
    }

    chance = chance.clamp(0, 18);

    let roll = crate::helpers::random_mod(20);
    if roll as i32 > chance || power > skill + (skill / 2) {
        gs.do_character_log(cn, core::types::FontColor::Red, "You lost your focus!\n");
        return -1;
    }
    0
}

/// Performs a simple spell focus check for `cn`.
///
/// # Arguments
///
/// * `gs` - Active game state used for caster state and failure messages.
/// * `cn` - Caster character index.
/// * `d20` - Base d20 threshold before luck adjustment.
///
/// # Returns
///
/// * `0` when focus succeeds, or `-1` when focus is lost.
///
/// # Panics
///
/// * Panics if `cn` is not a valid character index.
pub fn chance(gs: &mut GameState, cn: usize, d20: i32) -> i32 {
    // Ported from C++ chance(int cn, int d20)
    let mut d20 = d20;
    let (flags, luck) = (gs.characters[cn].flags, gs.characters[cn].luck);
    if (flags & CharacterFlags::Player.bits()) != 0 && luck < 0 {
        d20 += luck / 500 - 1;
    }

    d20 = d20.clamp(0, 18);

    let roll = crate::helpers::random_mod(20);
    if roll as i32 > d20 {
        gs.do_character_log(cn, core::types::FontColor::Red, "You lost your focus!\n");
        return -1;
    }
    0
}

/// Reduces spell power by a target immunity value.
///
/// # Arguments
///
/// * `_gs` - Game state reserved for compatibility with the legacy signature.
/// * `power` - Incoming spell power.
/// * `immun` - Target immunity skill value.
///
/// # Returns
///
/// * Effective spell power after immunity mitigation, with a minimum of `1`.
pub fn spell_immunity(_gs: &GameState, power: i32, immun: i32) -> i32 {
    // Ported from C++ spell_immunity(int power, int immun)
    let immun = immun / 2;
    if power <= immun { 1 } else { power - immun }
}

/// Applies caster kindred and moon-phase modifiers to spell power.
///
/// # Arguments
///
/// * `gs` - Active game state containing global moon-phase flags.
/// * `power` - Base spell power.
/// * `kindred` - Caster kindred bitfield.
///
/// # Returns
///
/// * Modified spell power after kindred and moon-phase adjustments.
pub fn spell_race_mod(gs: &GameState, power: i32, kindred: i32) -> i32 {
    // Ported from C++ spell_race_mod(int power, int kindred)

    let mut modf;
    if (kindred & KIN_ARCHHARAKIM as i32) != 0 {
        modf = 1.05;
    } else if (kindred & KIN_ARCHTEMPLAR as i32) != 0 {
        modf = 0.95;
    } else if (kindred & KIN_SORCERER as i32) != 0 || (kindred & KIN_WARRIOR as i32) != 0 {
        modf = 1.10;
    } else if (kindred & KIN_SEYAN_DU as i32) != 0 {
        modf = 0.95;
    } else if (kindred & KIN_HARAKIM as i32) != 0 {
        modf = 1.00;
    } else if (kindred & KIN_MERCENARY as i32) != 0 {
        modf = 1.05;
    } else if (kindred & KIN_TEMPLAR as i32) != 0 {
        modf = 0.90;
    } else {
        modf = 1.00;
    }

    if gs.globals.newmoon != 0 {
        modf -= 0.15;
    }
    if gs.globals.fullmoon != 0 {
        modf += 0.15;
    }

    (f64::from(power) * modf) as i32
}

/// Adds a spell item to a character's active spell slots.
///
/// Replaces weaker duplicate spells, rejects weaker overwrites, and evicts the
/// weakest active spell when all slots are full and the new spell is stronger.
///
/// # Arguments
///
/// * `gs` - Active game state containing characters, items, and map flags.
/// * `cn` - Target character index receiving the spell.
/// * `in_` - Spell item index to attach.
///
/// # Returns
///
/// * `1` when the spell was attached, or `0` when it was rejected or neutralized.
///
/// # Panics
///
/// * Panics if `cn`, `in_`, an active spell item index, or the character's map position is invalid.
pub fn add_spell(gs: &mut GameState, cn: usize, in_: usize) -> i32 {
    // Ported from C++ add_spell(int cn, int in)
    let mut n = 0;
    let in2: usize;
    let mut weak = 999;
    let mut weakest = 99;
    let mut rejected = false;
    let m = gs.characters[cn].x as usize
        + gs.characters[cn].y as usize * core::constants::SERVER_MAPX as usize;
    let nomagic = gs.map[m].flags & CharacterFlags::NoMagic.bits() != 0;
    if nomagic {
        return 0;
    }
    // Seeing Red grants immunity to new stun/slow/curse/disarm style debuffs
    // while active. Cooldown / passive temps fall outside this list, so the
    // recipient's own Seeing Red marker is never rejected here.
    if is_seeing_red_blocked_temp(gs.items[in_].temp) && has_active_seeing_red(gs, cn) {
        gs.items[in_].used = core::constants::USE_EMPTY;
        return 0;
    }
    // Overwrite spells if same spell is cast twice and the new spell is more powerful
    let mut found = false;

    for i in 0..20 {
        if gs.characters[cn].spell[i] != 0 {
            let it_in2 = gs.characters[cn].spell[i] as usize;
            let temp_in2 = gs.items[it_in2].temp;
            let temp_in = gs.items[in_].temp;
            if temp_in2 == temp_in {
                let power_in = gs.items[in_].power;
                let power_in2 = gs.items[it_in2].power;
                let active_in2 = gs.items[it_in2].active;
                if power_in < power_in2 && active_in2 > core::constants::TICKS as u32 * 60 {
                    gs.items[in_].used = core::constants::USE_EMPTY;
                    rejected = true;
                    found = true;
                    break;
                }
                gs.items[it_in2].used = core::constants::USE_EMPTY;
                n = i;
                found = true;
                break;
            }
        }
    }
    if rejected {
        return 0;
    }
    if found {
        // n is set by the loop above
    } else {
        // Find empty slot or weakest spell
        let mut empty_found = false;
        for i in 0..20 {
            if gs.characters[cn].spell[i] == 0 {
                n = i;
                empty_found = true;
                break;
            }
            let it_in2 = gs.characters[cn].spell[i] as usize;
            let power_in2 = gs.items[it_in2].power;
            if power_in2 < weak {
                weak = power_in2;
                weakest = i;
            }
        }
        if !empty_found {
            let power_in = gs.items[in_].power;
            if weak < 999 && weak < power_in {
                n = weakest;
                in2 = gs.characters[cn].spell[n] as usize;
                gs.items[in2].used = core::constants::USE_EMPTY;
            } else {
                gs.items[in_].used = core::constants::USE_EMPTY;
                return 0;
            }
        }
    }
    // Assign spell
    gs.characters[cn].spell[n] = in_ as u32;
    gs.items[in_].carried = cn as u16;
    gs.do_update_char(cn);
    1
}

/// Returns whether a character currently has the spell-exhaustion marker.
///
/// # Arguments
///
/// * `gs` - Active game state containing character spell slots and items.
/// * `cn` - Character index to inspect.
///
/// # Returns
///
/// * `true` when a spell-exhaustion marker is active, otherwise `false`.
///
/// # Panics
///
/// * Panics if `cn` or an active spell item index is invalid.
pub fn is_exhausted(gs: &mut GameState, cn: usize) -> bool {
    for n in 0..20 {
        let in_ = gs.characters[cn].spell[n] as usize;
        if in_ != 0 {
            let temp = gs.items[in_].temp;
            if temp == SK_BLAST as u16 {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    "You are still exhausted from your last spell!\n",
                );
                return true;
            }
        }
    }
    false
}

/// Adds a temporary spell-exhaustion marker to a character.
///
/// # Arguments
///
/// * `gs` - Active game state used to create and attach the exhaustion item.
/// * `cn` - Character index receiving exhaustion.
/// * `exhaust_length` - Exhaustion duration in ticks.
///
/// # Panics
///
/// * Panics if `cn` is invalid or the created item index becomes invalid.
pub fn add_exhaust(gs: &mut GameState, cn: usize, exhaust_length: i32) {
    // Ported from C++ add_exhaust(int cn, int len)
    let in_ = God::create_item(gs, 1);
    if in_.is_none() {
        log::error!("god_create_item failed in add_exhaust");
        return;
    }
    {
        let item = &mut gs.items[in_.unwrap()];
        let mut name_bytes = [0u8; 40];
        let name = b"Spell Exhaustion";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        item.name = name_bytes;
        item.flags |= ItemFlags::IF_SPELL.bits();
        item.sprite[1] = 97;
        item.duration = exhaust_length as u32;
        item.active = exhaust_length as u32;
        item.temp = SK_BLAST as u16;
        item.power = 255;
    }
    add_spell(gs, cn, in_.unwrap());
}

/// Converts an item template effect into an active spell on a character.
///
/// # Arguments
///
/// * `gs` - Active game state used to copy item bonuses and attach the spell.
/// * `cn` - Character index receiving the copied spell effect.
/// * `in2` - Source item index providing spell metadata and bonuses.
///
/// # Panics
///
/// * Panics if `cn`, `in2`, or the created spell item index is invalid.
pub fn spell_from_item(gs: &mut GameState, cn: usize, in2: usize) {
    // Ported from C++ spell_from_item(int cn, int in2)
    let flags = gs.characters[cn].flags;
    if (flags & CharacterFlags::NoMagic.bits()) != 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "The magic didn't work! Must be external influences.\n",
        );
        return;
    }
    let in_ = God::create_item(gs, 1);
    if in_.is_none() {
        log::error!("god_create_item failed in skill_from_item");
        return;
    }

    let in_ = in_.unwrap();
    {
        gs.items[in_].name = gs.items[in2].name;
        gs.items[in_].flags |= ItemFlags::IF_SPELL.bits();
        gs.items[in_].armor[1] = gs.items[in2].armor[1];
        gs.items[in_].weapon[1] = gs.items[in2].weapon[1];
        gs.items[in_].hp[1] = gs.items[in2].hp[1];
        gs.items[in_].end[1] = gs.items[in2].end[1];
        gs.items[in_].mana[1] = gs.items[in2].mana[1];
        gs.items[in_].sprite_override = gs.items[in2].sprite_override;
        for n in 0..5 {
            gs.items[in_].attrib[n][1] = gs.items[in2].attrib[n][1];
        }
        for n in 0..core::skills::MAX_SKILLS {
            gs.items[in_].skill[n][1] = gs.items[in2].skill[n][1];
        }
        let data0 = gs.items[in2].data[0];
        if data0 != 0 {
            gs.items[in_].sprite[1] = data0 as i16;
        } else {
            gs.items[in_].sprite[1] = 93;
        }
        let duration = gs.items[in2].duration;
        gs.items[in_].duration = duration;
        gs.items[in_].active = duration;
        let data1 = gs.items[in2].data[1];
        if data1 != 0 {
            gs.items[in_].temp = data1 as u16;
        } else {
            gs.items[in_].temp = 101;
        }
        gs.items[in_].power = gs.items[in2].power;
    }
    if add_spell(gs, cn, in_) == 0 {
        let name = gs.items[in_].get_name().to_owned();
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            &format!("Magical interference neutralised the {}'s effect.\n", name,),
        );
        return;
    }
    gs.do_character_log(cn, core::types::FontColor::Green, "You feel changed.\n");
    let sound = gs.characters[cn].sound;
    GameState::char_play_sound(gs, cn, i32::from(sound) + 1, -150, 0);
}

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
        if sense + 10 > power as u8 {
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
        gs.characters[cn].data[71] += CNTSAY;
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

    if chance(gs, cn, 18) != 0 {
        if cn != co {
            let sense = gs.characters[co].skill[SK_SENSE][5];
            let light_skill = gs.characters[cn].skill[SK_LIGHT][5];
            if sense > (light_skill + 5) {
                let reference = gs.characters[cn].reference;
                gs.do_character_log(
                    co,
                    FontColor::Green,
                    &format!(
                        "{} tried to cast light on you but failed.\n",
                        c_string_to_str(&reference)
                    ),
                );
            }
        }
        return;
    }

    let light_skill = gs.characters[cn].skill[SK_LIGHT][5];
    spell_light(gs, cn, co, i32::from(light_skill));

    add_exhaust(gs, cn, TICKS / 4);
}

/// Computes the aggregate spellpower cap for a character.
///
/// # Arguments
///
/// * `cn` - Character whose primary attributes define the cap.
///
/// # Returns
///
/// * Sum of agility, strength, intelligence, willpower, and bravery base attributes.
pub fn spellpower(cn: &Character) -> i32 {
    let a = i32::from(cn.attrib[core::constants::AT_AGIL as usize][0]);
    let b = i32::from(cn.attrib[core::constants::AT_STREN as usize][0]);
    let c = i32::from(cn.attrib[core::constants::AT_INT as usize][0]);
    let d = i32::from(cn.attrib[core::constants::AT_WILL as usize][0]);
    let e = i32::from(cn.attrib[core::constants::AT_BRAVE as usize][0]);
    a + b + c + d + e
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
    if chance(gs, cn, 18) != 0 {
        if cn != co {
            let sense = gs.characters[co].skill[SK_SENSE][5];
            let prot_skill = gs.characters[cn].skill[SK_PROTECT][5];
            if sense > (prot_skill + 5) {
                let reference = gs.characters[cn].reference;
                gs.do_character_log(
                    co,
                    FontColor::Green,
                    &format!(
                        "{} tried to cast protect on you but failed.\n",
                        c_string_to_str(&reference)
                    ),
                );
            }
        }
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
        if chance(gs, cn, 18) != 0 {
            if cn != co {
                let sense = gs.characters[co].skill[SK_SENSE][5];
                let enh_skill = gs.characters[cn].skill[SK_ENHANCE][5];
                if sense > (enh_skill + 5) {
                    let reference = gs.characters[cn].reference;
                    gs.do_character_log(
                        co,
                        FontColor::Yellow,
                        &format!(
                            "{} tried to cast enhance weapon on you but failed.\n",
                            c_string_to_str(&reference)
                        ),
                    );
                }
            }
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
    if chance(gs, cn, 18) != 0 {
        if cn != co {
            let sense = gs.characters[co].skill[SK_SENSE][5];
            let enh_skill = gs.characters[cn].skill[SK_ENHANCE][5];
            if sense > (enh_skill + 5) {
                let reference = gs.characters[cn].reference;
                gs.do_character_log(
                    co,
                    FontColor::Yellow,
                    &format!(
                        "{} tried to cast enhance weapon on you but failed.\n",
                        c_string_to_str(&reference)
                    ),
                );
            }
        }
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
            gs.items[in_].attrib[n][1] = (power / 5 + 3) as i8;
        }
        gs.items[in_].sprite[1] = 88;
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
        if chance(gs, cn, 18) != 0 {
            if cn != co {
                let sense = gs.characters[co].skill[SK_SENSE][5];
                let bless_skill = gs.characters[cn].skill[SK_BLESS][5];
                if sense > (bless_skill + 5) {
                    let reference = gs.characters[cn].reference;
                    gs.do_character_log(
                        co,
                        FontColor::Yellow,
                        &format!(
                            "{} tried to cast bless on you but failed.\n",
                            c_string_to_str(&reference)
                        ),
                    );
                }
            }
            return;
        }
        spell_bless(gs, cn, co, i32::from(gs.characters[cn].skill[SK_BLESS][5]));
        add_exhaust(gs, cn, TICKS);
        return;
    }

    if spellcost(gs, cn, 35) != 0 {
        return;
    }
    if chance(gs, cn, 18) != 0 {
        if cn != co {
            let sense = gs.characters[co].skill[SK_SENSE][5];
            let bless_skill = gs.characters[cn].skill[SK_BLESS][5];
            if sense > (bless_skill + 5) {
                let reference = gs.characters[cn].reference;
                gs.do_character_log(
                    co,
                    FontColor::Yellow,
                    &format!(
                        "{} tried to cast bless on you but failed.\n",
                        c_string_to_str(&reference)
                    ),
                );
            }
        }
        return;
    }

    spell_bless(gs, cn, co, i32::from(gs.characters[cn].skill[SK_BLESS][5]));
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
    if chance(gs, cn, 18) != 0 {
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
        if chance(gs, cn, 18) != 0 {
            if cn != co {
                let sense = gs.characters[co].skill[SK_SENSE][5];
                let heal_skill = gs.characters[cn].skill[SK_HEAL][5];
                if sense > (heal_skill + 5) {
                    let reference = gs.characters[cn].reference;
                    gs.do_character_log(
                        co,
                        FontColor::Green,
                        &format!(
                            "{} tried to cast heal on you but failed.\n",
                            c_string_to_str(&reference)
                        ),
                    );
                }
            }
            return;
        }
        spell_heal(gs, cn, co, i32::from(gs.characters[cn].skill[SK_HEAL][5]));
        add_exhaust(gs, cn, TICKS * 2);
        return;
    }

    if spellcost(gs, cn, 25) != 0 {
        return;
    }
    if chance(gs, cn, 18) != 0 {
        if cn != co {
            let sense = gs.characters[co].skill[SK_SENSE][5];
            let heal_skill = gs.characters[cn].skill[SK_HEAL][5];
            if sense > (heal_skill + 5) {
                let reference = gs.characters[cn].reference;
                gs.do_character_log(
                    co,
                    FontColor::Green,
                    &format!(
                        "{} tried to cast heal on you but failed.\n",
                        c_string_to_str(&reference)
                    ),
                );
            }
        }
        return;
    }

    spell_heal(gs, cn, co, i32::from(gs.characters[cn].skill[SK_HEAL][5]));

    add_exhaust(gs, cn, TICKS * 2);
}

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
            gs.items[in_idx].attrib[n][1] = -((power / 3) as i8);
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
    let co = if gs.characters[cn].skill_target1 != 0 {
        gs.characters[cn].skill_target1 as usize
    } else if gs.characters[cn].attack_cn != 0 {
        gs.characters[cn].attack_cn as usize
    } else {
        cn
    };

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

    if chance_base(
        gs,
        cn,
        i32::from(gs.characters[cn].skill[SK_CURSE][5]),
        10,
        i32::from(gs.characters[co].skill[SK_RESIST][5]),
    ) != 0
    {
        if cn != co
            && gs.characters[co].skill[SK_SENSE][5] > (gs.characters[cn].skill[SK_CURSE][5] + 5)
        {
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

    if (gs.characters[co].flags & CharacterFlags::Immortal.bits()) != 0 {
        gs.do_character_log(cn, core::types::FontColor::Red, "You lost your focus.\n");
        return;
    }

    spell_curse(gs, cn, co, i32::from(gs.characters[cn].skill[SK_CURSE][5]));

    let co_orig = co;
    let curse_base = i32::from(gs.characters[cn].skill[SK_CURSE][0]);
    let curse_power = i32::from(gs.characters[cn].skill[SK_CURSE][5]);
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
        if !gs.may_attack_msg(cn, maybe_co, false) {
            continue;
        }
        if curse_power + helpers::random_mod_i32(20)
            > i32::from(gs.characters[maybe_co].skill[SK_RESIST][5]) + helpers::random_mod_i32(20)
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

    add_exhaust(gs, cn, core::constants::TICKS * 4);
}

/// Attempts to apply Warcry effects to one target.
///
/// # Arguments
///
/// * `gs` - Active game state used for attack validation, item creation, and effects.
/// * `cn` - Caster character index.
/// * `co` - Target character index.
/// * `power` - Warcry power checked against target resistance.
///
/// # Returns
///
/// * `true` when Warcry effects were applied, otherwise `false`.
///
/// # Panics
///
/// * Panics if `cn`, `co`, or a created spell item index is invalid.
pub fn warcry(gs: &mut GameState, cn: usize, co: usize, power: i32) -> bool {
    if gs.characters[cn].attack_cn as usize != co && gs.characters[co].alignment == 10000 {
        return false;
    }

    if !gs.may_attack_msg(cn, co, false) {
        return false;
    }

    if power < i32::from(gs.characters[co].skill[SK_RESIST][5]) {
        return false;
    }

    for n in 1..10 {
        if gs.characters[cn].data[n] as usize == co {
            return false;
        }
    }

    if (gs.characters[co].flags & CharacterFlags::Immortal.bits()) != 0 {
        return false;
    }

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

    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_warcry");
        return false;
    }
    let in_idx = in_opt.unwrap();

    {
        let mut name_bytes = [0u8; 40];
        let name = b"War-Stun";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        gs.items[in_idx].name = name_bytes;
        gs.items[in_idx].flags |= ItemFlags::IF_SPELL.bits();
        gs.items[in_idx].sprite[1] = 91;
        gs.items[in_idx].duration = core::constants::TICKS as u32 * 3;
        gs.items[in_idx].active = core::constants::TICKS as u32 * 3;
        gs.items[in_idx].temp = SK_WARCRY2 as u16;
        gs.items[in_idx].power = power as u32;
    }

    add_spell(gs, co, in_idx);

    let in2_opt = God::create_item(gs, 1);
    if in2_opt.is_none() {
        log::error!("god_create_item failed in skill_warcry");
        return false;
    }
    let in2 = in2_opt.unwrap();
    {
        let mut name_bytes = [0u8; 40];
        let name = b"Warcry";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        gs.items[in2].name = name_bytes;
        gs.items[in2].flags |= ItemFlags::IF_SPELL.bits();
        for n in 0..5 {
            gs.items[in2].attrib[n][1] = -15;
        }
        gs.items[in2].sprite[1] = 89;
        gs.items[in2].duration = (TICKS * 60) as u32;
        gs.items[in2].active = (TICKS * 60) as u32;
        gs.items[in2].temp = SK_WARCRY as u16;
        gs.items[in2].power = (power / 2) as u32;
    }

    add_spell(gs, co, in2);

    let co_name = gs.characters[co].get_name().to_owned();
    log::info!("Character {} cast Warcry on {}", cn, co_name);

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

/// Handles direct player/NPC use of the Warcry skill over nearby targets.
///
/// # Arguments
///
/// * `gs` - Active game state used for endurance costs, nearby target lookup, and spell application.
/// * `cn` - Caster character index.
///
/// # Panics
///
/// * Panics if `cn` is invalid or a scanned map index is invalid.
pub fn skill_warcry(gs: &mut GameState, cn: usize) {
    if gs.characters[cn].a_end < 150 * 1000 {
        gs.do_character_log(cn, core::types::FontColor::Red, "You're too exhausted!\n");
        return;
    }

    gs.characters[cn].a_end -= 150 * 1000;

    let power = i32::from(gs.characters[cn].skill[SK_WARCRY][5]);

    let xf = std::cmp::max(1, i32::from(gs.characters[cn].x) - 10);
    let yf = std::cmp::max(1, i32::from(gs.characters[cn].y) - 10);
    let xt = std::cmp::min(
        core::constants::SERVER_MAPX - 1,
        i32::from(gs.characters[cn].x) + 10,
    );
    let yt = std::cmp::min(
        core::constants::SERVER_MAPY - 1,
        i32::from(gs.characters[cn].y) + 10,
    );

    let mut hit = 0;
    let mut miss = 0;
    for x in xf..xt {
        for y in yf..yt {
            let m = (x + y * core::constants::SERVER_MAPX) as usize;
            let co = gs.map[m].ch as usize;
            if co != 0 {
                if warcry(gs, cn, co, power) {
                    gs.remember_pvp(cn, co);
                    let name = gs.characters[cn].get_name().to_owned();
                    gs.do_character_log(
                        co,
                        core::types::FontColor::Green,
                        &format!(
                            "You hear {}'s warcry. You feel frightened and immobilized.\n",
                            name
                        ),
                    );
                    hit += 1;
                } else {
                    let name = gs.characters[cn].get_name().to_owned();
                    gs.do_character_log(
                        co,
                        core::types::FontColor::Green,
                        &format!("You hear {}'s warcry.\n", name),
                    );
                    miss += 1;
                }
            }
        }
    }
    gs.do_character_log(
        cn,
        core::types::FontColor::Green,
        &format!(
            "You cry out loud and clear. You affected {} of {} creatures in range.\n",
            hit,
            hit + miss
        ),
    );
}

/// Writes detailed item statistics to a character's log.
///
/// # Arguments
///
/// * `gs` - Active game state containing the target item and output character.
/// * `cn` - Character index receiving the item information.
/// * `in_` - Item index to inspect.
/// * `_look` - Legacy look-mode argument retained for signature compatibility.
///
/// # Panics
///
/// * Panics if `cn` or `in_` is not a valid index.
pub fn item_info(gs: &mut GameState, cn: usize, in_: usize, _look: i32) {
    // Name
    let name = gs.items[in_].name;
    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!("{}:\n", c_string_to_str(&name)),
    );

    gs.do_character_log(cn, FontColor::Green, "Stat         Mod0 Mod1 Min\n");

    // Attributes
    for n in 0..5 {
        let (a0, a1, a2) = (
            gs.items[in_].attrib[n][0],
            gs.items[in_].attrib[n][1],
            gs.items[in_].attrib[n][2],
        );
        if a0 == 0 && a1 == 0 && a2 == 0 {
            continue;
        }
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!(
                "{:<12.12} {:+4} {:+4} {:3}\n",
                attribute_name(n),
                a0,
                a1,
                a2
            ),
        );
    }

    // HP/End/Mana
    let (hp0, hp1, hp2) = (
        gs.items[in_].hp[0],
        gs.items[in_].hp[1],
        gs.items[in_].hp[2],
    );
    if hp0 != 0 || hp1 != 0 || hp2 != 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("{:<12.12} {:+4} {:+4} {:3}\n", "Hitpoints", hp0, hp1, hp2),
        );
    }
    let (end0, end1, end2) = (
        gs.items[in_].end[0],
        gs.items[in_].end[1],
        gs.items[in_].end[2],
    );
    if end0 != 0 || end1 != 0 || end2 != 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!(
                "{:<12.12} {:+4} {:+4} {:3}\n",
                "Endurance", end0, end1, end2
            ),
        );
    }
    let (mana0, mana1, mana2) = (
        gs.items[in_].mana[0],
        gs.items[in_].mana[1],
        gs.items[in_].mana[2],
    );
    if mana0 != 0 || mana1 != 0 || mana2 != 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("{:<12.12} {:+4} {:+4} {:3}\n", "Mana", mana0, mana1, mana2),
        );
    }

    for n in 0..core::skills::MAX_SKILLS {
        let (s0, s1, s2) = (
            gs.items[in_].skill[n][0],
            gs.items[in_].skill[n][1],
            gs.items[in_].skill[n][2],
        );
        if s0 == 0 && s1 == 0 && s2 == 0 {
            continue;
        }
        let skill_label = get_skill_name(n);
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("{:<12.12} {:+4} {:+4} {:3}\n", skill_label, s0, s1, s2),
        );
    }

    let (w0, w1) = (gs.items[in_].weapon[0], gs.items[in_].weapon[1]);
    if w0 != 0 || w1 != 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("{:<12.12} {:+4} {:+4}\n", "Weapon", w0, w1),
        );
    }
    let (ar0, ar1) = (gs.items[in_].armor[0], gs.items[in_].armor[1]);
    if ar0 != 0 || ar1 != 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("{:<12.12} {:+4} {:+4}\n", "Armor", ar0, ar1),
        );
    }
    let (l0, l1) = (gs.items[in_].light[0], gs.items[in_].light[1]);
    if l0 != 0 || l1 != 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("{:<12.12} {:+4} {:+4}\n", "Light", l0, l1),
        );
    }

    let power = gs.items[in_].power;
    if power != 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("{:<12.12} {:+4}\n", "Power", power),
        );
    }

    let min_rank = gs.items[in_].min_rank;
    if min_rank != 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("{:<12.12} {:+4}\n", "Min. Rank", min_rank),
        );
    }
}

/// Writes detailed character spell, skill, and attribute information to a character's log.
///
/// # Arguments
///
/// * `gs` - Active game state containing both characters and active spell items.
/// * `cn` - Character index receiving the information.
/// * `co` - Character index being inspected.
///
/// # Panics
///
/// * Panics if `cn`, `co`, or an active spell item index is invalid.
pub fn char_info(gs: &mut GameState, cn: usize, co: usize) {
    // Header
    let name_bytes = gs.characters[co].name;
    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!("{}:\n", c_string_to_str(&name_bytes)),
    );
    gs.do_character_log(cn, FontColor::Green, " \n");

    // Active spells (0..19)
    let mut flag = false;
    for n in 0..20 {
        let in_idx = gs.characters[co].spell[n] as usize;
        if in_idx != 0 {
            let item_name = gs.items[in_idx].get_name().to_owned();
            let active = gs.items[in_idx].active;
            let minutes = active / (TICKS as u32 * 60);
            let seconds = (active / TICKS as u32) % 60;
            let power = gs.items[in_idx].power;
            gs.do_character_log(
                cn,
                FontColor::Green,
                &format!(
                    "{} for {}m {}s power of {}\n",
                    item_name, minutes, seconds, power
                ),
            );
            flag = true;
        }
    }
    if !flag {
        gs.do_character_log(cn, FontColor::Green, "No spells active.\n");
    }
    gs.do_character_log(cn, FontColor::Green, " \n");

    // Skills two-column using static SKILL_NAMES
    let mut n1: i32 = -1;
    let mut n2: i32 = -1;
    for n in 0..core::skills::MAX_SKILLS {
        let s0 = gs.characters[co].skill[n][0];
        if s0 != 0 && n1 == -1 {
            n1 = n as i32;
        } else if s0 != 0 && n2 == -1 {
            n2 = n as i32;
        }

        if n1 != -1 && n2 != -1 {
            let s1_0 = gs.characters[co].skill[n1 as usize][0];
            let s1_5 = gs.characters[co].skill[n1 as usize][5];
            let s2_0 = gs.characters[co].skill[n2 as usize][0];
            let s2_5 = gs.characters[co].skill[n2 as usize][5];
            let name1 = get_skill_name(n1 as usize);
            let name2 = get_skill_name(n2 as usize);
            gs.do_character_log(
                cn,
                FontColor::Green,
                &format!(
                    "{:<12.12} {:3}/{:3}  !  {:<12.12} {:3}/{:3}\n",
                    name1, s1_0, s1_5, name2, s2_0, s2_5
                ),
            );
            n1 = -1;
            n2 = -1;
        }
    }

    if n1 != -1 {
        let s1_0 = gs.characters[co].skill[n1 as usize][0];
        let s1_5 = gs.characters[co].skill[n1 as usize][5];
        let name1 = get_skill_name(n1 as usize);
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("{:<12.12} {:3}/{:3}\n", name1, s1_0, s1_5),
        );
    }

    // Attributes
    let a0_0 = gs.characters[co].attrib[0][0];
    let a0_5 = gs.characters[co].attrib[0][5];
    let a1_0 = gs.characters[co].attrib[1][0];
    let a1_5 = gs.characters[co].attrib[1][5];
    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!(
            "{:<12.12} {:3}/{:3}  !  {:<12.12} {:3}/{:3}\n",
            attribute_name(0),
            a0_0,
            a0_5,
            attribute_name(1),
            a1_0,
            a1_5
        ),
    );
    let a2_0 = gs.characters[co].attrib[2][0];
    let a2_5 = gs.characters[co].attrib[2][5];
    let a3_0 = gs.characters[co].attrib[3][0];
    let a3_5 = gs.characters[co].attrib[3][5];
    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!(
            "{:<12.12} {:3}/{:3}  !  {:<12.12} {:3}/{:3}\n",
            attribute_name(2),
            a2_0,
            a2_5,
            attribute_name(3),
            a3_0,
            a3_5
        ),
    );
    let a4_0 = gs.characters[co].attrib[4][0];
    let a4_5 = gs.characters[co].attrib[4][5];
    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!("{:<12.12} {:3}/{:3}\n", attribute_name(4), a4_0, a4_5),
    );

    gs.do_character_log(cn, FontColor::Green, " \n");
}

/// Handles direct player/NPC use of the Identify skill.
///
/// Identifies or hides identify data for a carried item, or reports character
/// information when no valid carried item is selected.
///
/// # Arguments
///
/// * `gs` - Active game state used for costs, focus checks, item flags, and feedback.
/// * `cn` - Caster character index.
///
/// # Panics
///
/// * Panics if `cn`, the carried item index, or the selected character target is invalid.
pub fn skill_identify(gs: &mut GameState, cn: usize) {
    if is_exhausted(gs, cn) {
        return;
    }

    if spellcost(gs, cn, 25) != 0 {
        return;
    }

    let citem = gs.characters[cn].citem as usize;
    let in_idx: usize;
    let mut co = 0usize;
    let power: i32;

    let sane_item = if citem != 0 {
        citem < gs.items.len() && gs.items[citem].used != core::constants::USE_EMPTY
    } else {
        false
    };

    if citem != 0 && sane_item {
        in_idx = citem;
        power = gs.items[in_idx].power as i32;
    } else {
        let target = gs.characters[cn].skill_target1 as usize;
        if target != 0 {
            co = target;
            power = i32::from(gs.characters[co].skill[SK_RESIST][5]);
        } else {
            co = cn;
            power = 10;
        }
        in_idx = 0;
    }

    if chance_base(
        gs,
        cn,
        i32::from(gs.characters[cn].skill[SK_IDENT][5]),
        18,
        power,
    ) != 0
    {
        return;
    }

    let sound = gs.characters[cn].sound;
    GameState::char_play_sound(gs, cn, i32::from(sound) + 1, -150, 0);
    chlog!(
        cn,
        "Cast Identify on {}",
        if in_idx != 0 {
            gs.items[in_idx].get_name().to_owned()
        } else {
            gs.characters[co].get_name().to_owned()
        }
    );

    if in_idx != 0 {
        item_info(gs, cn, in_idx, 0);
        gs.items[in_idx].flags ^= ItemFlags::IF_IDENTIFIED.bits();
        let identified = (gs.items[in_idx].flags & ItemFlags::IF_IDENTIFIED.bits()) != 0;
        if !identified {
            gs.do_character_log(
                cn,
                core::types::FontColor::Green,
                "Identify data removed from item.\n",
            );
        }
    } else {
        char_info(gs, cn, co);
        EffectManager::fx_add_effect(
            gs,
            6,
            0,
            i32::from(gs.characters[co].x),
            i32::from(gs.characters[co].y),
            0,
        );
    }

    add_exhaust(gs, cn, TICKS * 2);
    EffectManager::fx_add_effect(
        gs,
        7,
        0,
        i32::from(gs.characters[cn].x),
        i32::from(gs.characters[cn].y),
        0,
    );
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
    let co = if gs.characters[cn].skill_target1 != 0 {
        gs.characters[cn].skill_target1 as usize
    } else if gs.characters[cn].attack_cn != 0 {
        gs.characters[cn].attack_cn as usize
    } else {
        cn
    };

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

    let mut power = i32::from(gs.characters[cn].skill[SK_BLAST][5]);
    power = spell_immunity(gs, power, i32::from(gs.characters[co].skill[SK_IMMUN][5]));
    power = spell_race_mod(gs, power, gs.characters[cn].kindred);

    let mut dam = power * 2;

    let mut cost = dam / 8 + 5;
    if (gs.characters[cn].flags & CharacterFlags::Player.bits()) != 0
        && ((gs.characters[cn].kindred as u32) & (KIN_HARAKIM | KIN_ARCHHARAKIM) != 0)
    {
        cost /= 3;
    }

    if spellcost(gs, cn, cost) != 0 {
        return;
    }

    if chance(gs, cn, 18) != 0 {
        if cn != co
            && gs.characters[co].skill[SK_SENSE][5] > gs.characters[cn].skill[SK_BLAST][5] + 5
        {
            gs.do_character_log(
                co,
                FontColor::Green,
                &format!(
                    "{} tried to cast blast on you but failed.\n",
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
        if !gs.may_attack_msg(cn, maybe_co, false) {
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

    add_exhaust(gs, cn, core::constants::TICKS * 6);
    EffectManager::fx_add_effect(
        gs,
        7,
        0,
        i32::from(gs.characters[cn].x),
        i32::from(gs.characters[cn].y),
        0,
    );
}

/// Attempts to repair the item currently carried under a character's cursor.
///
/// # Arguments
///
/// * `gs` - Active game state used for item validation, endurance costs, and replacement item creation.
/// * `cn` - Character index attempting the repair.
///
/// # Panics
///
/// * Panics if `cn`, the carried item index, or the created replacement item index is invalid.
pub fn skill_repair(gs: &mut GameState, cn: usize) {
    let in_idx = gs.characters[cn].citem as usize;
    if in_idx == 0 {
        gs.do_character_log(cn, core::types::FontColor::Green, "Repair. Repair what?\n");
        return;
    }

    if gs.items[in_idx].damage_state == 0 {
        gs.do_character_log(cn, core::types::FontColor::Green, "That isn't damaged.\n");
        return;
    }

    if gs.items[in_idx].power as i32 > i32::from(gs.characters[cn].skill[SK_REPAIR][5])
        || (gs.items[in_idx].flags & ItemFlags::IF_NOREPAIR.bits()) != 0
    {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "That's too difficult for you.\n",
        );
        return;
    }

    if gs.characters[cn].a_end < gs.items[in_idx].power as i32 * 1000 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "You're too exhausted to repair that.\n",
        );
        return;
    }

    let cost = gs.items[in_idx].power as i32;
    gs.characters[cn].a_end -= cost * 1000;

    let mut chan: i32 = if gs.items[in_idx].power != 0 {
        let skill = i32::from(gs.characters[cn].skill[SK_REPAIR][5]);
        let power = gs.items[in_idx].power as i32;
        skill * 15 / power
    } else {
        18
    };

    if chan > 18 {
        chan = 18;
    }

    let die = helpers::random_mod_i32(20);

    if die <= chan {
        let in2_opt = God::create_item(gs, gs.items[in_idx].temp as usize);
        if in2_opt.is_none() {
            gs.do_character_log(cn, core::types::FontColor::Green, "You failed.\n");
            return;
        }
        let in2 = in2_opt.unwrap();
        gs.items[in_idx].used = core::constants::USE_EMPTY;
        gs.characters[cn].citem = in2 as u32;
        gs.items[in2].carried = cn as u16;
        gs.do_character_log(cn, core::types::FontColor::Green, "Success!\n");
    } else {
        gs.do_character_log(cn, core::types::FontColor::Green, "You failed.\n");
        driver::item_damage_citem(gs, cn, 1000000);
        if die - chan > 3 {
            driver::item_damage_citem(gs, cn, 1000000);
        }
        if die - chan > 6 {
            driver::item_damage_citem(gs, cn, 1000000);
        }
    }
    chlog!(
        cn,
        "Cast Repair on {}",
        gs.items[in_idx].get_name().to_owned()
    );
}

/// Handles direct player/NPC use of the Recall skill.
///
/// Creates a temporary Recall spell tied to the caster's temple coordinates.
///
/// # Arguments
///
/// * `gs` - Active game state used for costs, item creation, and spell attachment.
/// * `cn` - Caster character index.
///
/// # Panics
///
/// * Panics if `cn` or the created spell item index is invalid.
pub fn skill_recall(gs: &mut GameState, cn: usize) {
    if is_exhausted(gs, cn) {
        return;
    }

    if spellcost(gs, cn, 15) != 0 {
        return;
    }

    if chance(gs, cn, 18) != 0 {
        return;
    }

    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        gs.do_character_log(cn, core::types::FontColor::Green, "You failed.\n");
        return;
    }
    let in_idx = in_opt.unwrap();

    {
        let mut name_bytes = [0u8; 40];
        let name = b"Recall";
        let len = name.len().min(40);
        name_bytes[..len].copy_from_slice(&name[..len]);
        gs.items[in_idx].name = name_bytes;
        gs.items[in_idx].flags |= ItemFlags::IF_SPELL.bits();
        gs.items[in_idx].sprite[1] = 90;
        let base_dur = 60 - i32::from(gs.characters[cn].skill[SK_RECALL][5] / 4);
        let dur = std::cmp::max(TICKS / 2, base_dur * TICKS / LEGACY_TICKS);
        gs.items[in_idx].duration = dur as u32;
        gs.items[in_idx].active = gs.items[in_idx].duration;
        gs.items[in_idx].temp = SK_RECALL as u16;
        gs.items[in_idx].power = u32::from(gs.characters[cn].skill[SK_RECALL][5]);
        gs.items[in_idx].data[0] = u32::from(gs.characters[cn].temple_x);
        gs.items[in_idx].data[1] = u32::from(gs.characters[cn].temple_y);
    }

    if add_spell(gs, cn, in_idx) == 0 {
        gs.do_character_log(cn, core::types::FontColor::Green, "You failed.\n");
        return;
    }

    chlog!(cn, "Cast Recall");
    add_exhaust(gs, cn, TICKS);
    EffectManager::fx_add_effect(
        gs,
        7,
        0,
        i32::from(gs.characters[cn].x),
        i32::from(gs.characters[cn].y),
        0,
    );
}

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

    if gs.characters[co].skill[SK_SENSE][5] + 10 > power as u8 {
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
    let co = if gs.characters[cn].skill_target1 != 0 {
        gs.characters[cn].skill_target1 as usize
    } else if gs.characters[cn].attack_cn != 0 {
        gs.characters[cn].attack_cn as usize
    } else {
        cn
    };

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
        i32::from(gs.characters[co].skill[SK_RESIST][5]),
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
                > i32::from(gs.characters[maybe_co].skill[SK_RESIST][5]) + o_rand
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

/// Removes all active spell items from a character.
///
/// # Arguments
///
/// * `gs` - Active game state containing character spell slots and spell items.
/// * `cn` - Character index whose spells should be removed.
///
/// # Panics
///
/// * Panics if `cn` or an active spell item index is invalid.
pub fn remove_spells(gs: &mut GameState, cn: usize) {
    for n in 0..20usize {
        let in_idx = gs.characters[cn].spell[n] as usize;
        if in_idx == 0 {
            continue;
        }
        gs.items[in_idx].used = core::constants::USE_EMPTY;
        gs.characters[cn].spell[n] = 0;
    }
    gs.do_update_char(cn);
}

/// Handles direct player/NPC use of the Dispel Magic skill.
///
/// Chooses a curse first, then the first non-Guardian-Angel spell, and treats
/// hostile dispels as attacks where appropriate.
///
/// # Arguments
///
/// * `gs` - Active game state used for target validation, spell removal, and feedback.
/// * `cn` - Caster character index.
///
/// # Panics
///
/// * Panics if `cn`, the selected target index, or an active spell item index is invalid.
pub fn skill_dispel(gs: &mut GameState, cn: usize) {
    // Port of C `skill_dispel(int cn)`.
    let target = gs.characters[cn].skill_target1 as usize;
    let co = if target != 0 { target } else { cn };

    if gs.do_char_can_see(cn, co) == 0 {
        gs.do_character_log(cn, FontColor::Red, "You cannot see your target.\n");
        return;
    }

    if is_exhausted(gs, cn) {
        return;
    }

    // Select which spell slot to remove.
    let mut slot: Option<usize> = None;

    // 1) Prefer removing curse from target.
    for n in 0..20usize {
        let in_idx = gs.characters[co].spell[n] as usize;
        if in_idx == 0 {
            continue;
        }
        if gs.items[in_idx].temp == SK_CURSE as u16 {
            slot = Some(n);
            break;
        }
    }

    // 2) If no curse found, remove first non-wimpy spell.
    if slot.is_none() {
        for n in 0..20usize {
            let in_idx = gs.characters[co].spell[n] as usize;
            if in_idx == 0 {
                continue;
            }
            let temp = gs.items[in_idx].temp;
            if temp == SK_WIMPY as u16 {
                continue;
            }
            slot = Some(n);
            break;
        }

        // No target spell found.
        if slot.is_none() {
            if co == cn {
                gs.do_character_log(cn, FontColor::Red, "But you aren't spelled!\n");
            } else {
                let name = gs.characters[co].get_name().to_owned();
                gs.do_character_log(cn, FontColor::Red, &format!("{} isn't spelled!\n", name));
            }
            return;
        }

        // Dispelling someone else's non-curse spell is treated like an attack.
        if target != 0 && !gs.may_attack_msg(cn, co, true) {
            chlog!(
                cn,
                "Prevented from dispelling {}",
                gs.characters[co].get_name().to_owned()
            );
            return;
        }
    }

    let slot = slot.expect("slot must be set");
    let in_idx = gs.characters[co].spell[slot] as usize;
    if in_idx == 0 {
        return;
    }

    let pwr = gs.items[in_idx].power as i32;

    if spellcost(gs, cn, 25) != 0 {
        return;
    }

    let dispel_skill = i32::from(gs.characters[cn].skill[SK_DISPEL][5]);
    let kindred = gs.characters[cn].kindred;
    if chance_base(gs, cn, spell_race_mod(gs, dispel_skill, kindred), 12, pwr) != 0 {
        if cn != co {
            let sense = i32::from(gs.characters[co].skill[SK_SENSE][5]);
            if sense > dispel_skill + 5 {
                let reference = gs.characters[cn].reference;
                gs.do_character_log(
                    co,
                    FontColor::Green,
                    &format!(
                        "{} tried to cast dispel magic on you but failed.\n",
                        c_string_to_str(&reference)
                    ),
                );
            }
        }
        return;
    }

    let removed_temp = gs.items[in_idx].temp;
    let removed_name = gs.items[in_idx].get_name().to_owned();

    // Remove the spell item and unlink it from the target.
    gs.items[in_idx].used = USE_EMPTY;
    gs.characters[co].spell[slot] = 0;
    gs.do_update_char(co);

    // Remember PvP attacks when dispelling non-curse from someone else.
    if target != 0 && removed_temp != SK_CURSE as u16 {
        gs.remember_pvp(cn, co);
    }

    let sound = i32::from(gs.characters[cn].sound);

    if target != 0 {
        let sense = i32::from(gs.characters[co].skill[SK_SENSE][5]);
        if sense + 10 > dispel_skill {
            let reference = gs.characters[cn].reference;
            gs.do_character_log(
                co,
                FontColor::Green,
                &format!(
                    "{} cast dispel magic on you.\n",
                    c_string_to_str(&reference)
                ),
            );
        } else {
            gs.do_character_log(
                co,
                FontColor::Green,
                &format!("{} has been removed.\n", removed_name),
            );
        }

        let target_name = gs.characters[co].get_name().to_owned();
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("Removed {} from {}.\n", removed_name, target_name),
        );

        let target_is_player = (gs.characters[co].flags & CharacterFlags::Player.bits()) != 0;
        if removed_temp != SK_CURSE as u16 && !target_is_player {
            if (gs.characters[co].flags & CharacterFlags::SpellIgnore.bits()) == 0 {
                gs.do_notify_character(co as u32, i32::from(NT_GOTHIT), cn as i32, 0, 0, 0);
            }
            gs.do_notify_character(cn as u32, i32::from(NT_DIDHIT), co as i32, 0, 0, 0);
        }

        GameState::char_play_sound(gs, co, sound + 1, -150, 0);
        GameState::char_play_sound(gs, cn, sound + 1, -150, 0);
        chlog!(
            cn,
            "Cast Dispel on {}",
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
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("{} has been removed.\n", removed_name),
        );
        GameState::char_play_sound(gs, cn, sound + 1, -150, 0);
        chlog!(cn, "Cast Dispel");
        EffectManager::fx_add_effect(
            gs,
            6,
            0,
            i32::from(gs.characters[cn].x),
            i32::from(gs.characters[cn].y),
            0,
        );
    }

    add_exhaust(gs, cn, TICKS * 2);
    EffectManager::fx_add_effect(
        gs,
        7,
        0,
        i32::from(gs.characters[cn].x),
        i32::from(gs.characters[cn].y),
        0,
    );
}

/// Handles direct player/NPC use of the Ghost Companion skill.
///
/// Creates and initializes a companion NPC for the caster, optionally setting
/// it to attack the selected target.
///
/// # Arguments
///
/// * `gs` - Active game state used for target validation, companion creation, and initialization.
/// * `cn` - Caster character index.
///
/// # Panics
///
/// * Panics if `cn`, the selected target index, or the created companion index is invalid.
pub fn skill_ghost(gs: &mut GameState, cn: usize) {
    // Check if in build mode
    if (gs.characters[cn].flags & CharacterFlags::BuildMode.bits()) != 0 {
        gs.do_character_log(cn, FontColor::Red, "Not in build mode.\n");
        return;
    }

    let existing_companion = {
        if (gs.characters[cn].flags & CharacterFlags::Player.bits()) != 0 {
            let co = gs.characters[cn].data[CHD_COMPANION] as usize;
            if co != 0 {
                if Character::is_sane_character(co)
                    && gs.characters[co].data[63] == cn as i32
                    && (gs.characters[co].flags & CharacterFlags::Body.bits()) == 0
                    && gs.characters[co].used != USE_EMPTY
                {
                    Some(co)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    };

    if let Some(co) = existing_companion {
        gs.do_character_log(
            cn,
            FontColor::Red,
            &format!("You may not have more than one Ghost Companion ({}).\n", co),
        );
        return;
    }

    // Get target
    let mut co = gs.characters[cn].skill_target1 as usize;
    if co == cn {
        co = 0;
    }

    // Check visibility
    if co != 0 && gs.do_char_can_see(cn, co) == 0 {
        gs.do_character_log(cn, FontColor::Red, "You cannot see your target.\n");
        return;
    }

    if is_exhausted(gs, cn) {
        return;
    }

    // Check if can attack target
    if co != 0 && !gs.may_attack_msg(cn, co, true) {
        chlog!(
            cn,
            "Prevented from attacking {} ({})",
            gs.characters[co].get_name().to_owned(),
            co
        );
        return;
    }

    if spellcost(gs, cn, 45) != 0 {
        return;
    }

    // No GC in Gatekeeper's room
    let (cx, cy) = (gs.characters[cn].x, gs.characters[cn].y);
    if (39..=47).contains(&cx) && (594..=601).contains(&cy) {
        gs.do_character_log(cn, FontColor::Red, "You must fight this battle alone.\n");
        return;
    }

    // Chance check
    if chance(gs, cn, 15) != 0 {
        if co != 0 && cn != co {
            let sense = i32::from(gs.characters[co].skill[SK_SENSE][5]);
            let ghost_skill = i32::from(gs.characters[cn].skill[SK_GHOST][5]);
            if sense > ghost_skill + 5 {
                let cn_ref = gs.characters[cn].reference;
                gs.do_character_log(
                    co,
                    FontColor::Green,
                    &format!(
                        "{} tried to cast ghost companion on you but failed.\n",
                        c_string_to_str(&cn_ref)
                    ),
                );
                if (gs.characters[co].flags & CharacterFlags::SpellIgnore.bits()) == 0 {
                    gs.do_notify_character(co as u32, i32::from(NT_GOTMISS), cn as i32, 0, 0, 0);
                }
            }
        }
        return;
    }

    // Create companion
    let cc_opt = populate::pop_create_char(gs, CT_COMPANION as usize, true);
    if cc_opt.is_none() {
        gs.do_character_log(
            cn,
            FontColor::Red,
            "The ghost companion could not materialize.\n",
        );
        return;
    }
    let cc = cc_opt.unwrap();

    let (cc_x, cc_y) = (gs.characters[cn].x as usize, gs.characters[cn].y as usize);
    if !God::drop_char_fuzzy(gs, cc, cc_x, cc_y) {
        gs.characters[cc].used = USE_EMPTY;
        gs.do_character_log(
            cn,
            FontColor::Red,
            "The ghost companion could not materialize.\n",
        );
        return;
    }

    // Assign a randomized name to the companion
    let random_name = {
        let mut candidate = None;
        for _ in 0..100 {
            let name = core::names::randomly_generate_name();
            let name_exists = gs.characters.iter().enumerate().any(|(idx, other)| {
                idx != cc && other.used != USE_EMPTY && other.get_name().eq_ignore_ascii_case(&name)
            });
            if !name_exists {
                candidate = Some(name);
                break;
            }
        }
        candidate.unwrap_or_else(core::names::randomly_generate_name)
    };

    {
        let companion = &mut gs.characters[cc];

        let mut name_bytes = [0u8; 40];
        let name_src = random_name.as_bytes();
        let name_len = name_src.len().min(name_bytes.len());
        name_bytes[..name_len].copy_from_slice(&name_src[..name_len]);
        companion.name = name_bytes;

        let mut reference_bytes = [0u8; 40];
        reference_bytes[..name_len].copy_from_slice(&name_src[..name_len]);
        companion.reference = reference_bytes;

        let desc = companion.get_default_description();
        let desc_src = desc.as_bytes();
        let mut desc_bytes = [0u8; 200];
        let desc_len = desc_src.len().min(desc_bytes.len());
        desc_bytes[..desc_len].copy_from_slice(&desc_src[..desc_len]);
        companion.description = desc_bytes;
    }

    if co != 0 {
        if (gs.characters[co].flags & CharacterFlags::SpellIgnore.bits()) == 0 {
            gs.do_notify_character(co as u32, i32::from(NT_GOTHIT), cn as i32, 0, 0, 0);
        }
        gs.do_notify_character(cn as u32, i32::from(NT_DIDHIT), co as i32, 0, 0, 0);
    }

    if (gs.characters[cn].flags & CharacterFlags::Player.bits()) != 0 {
        gs.characters[cn].data[CHD_COMPANION] = cc as i32;
    }

    let mut base = (i32::from(gs.characters[cn].skill[SK_GHOST][5]) * 4) / 11;
    let kindred = gs.characters[cn].kindred;
    base = spell_race_mod(gs, base, kindred);

    let ticker = gs.globals.ticker;

    let co_id = if co != 0 {
        helpers::char_id(&gs.characters[co])
    } else {
        0
    };

    gs.characters[cc].data[29] = 0;
    gs.characters[cc].data[42] = 65536 + cn as i32;
    gs.characters[cc].kindred &= !(KIN_MONSTER as i32);
    gs.characters[cc].flags &= !CharacterFlags::Player.bits();

    if co != 0 {
        gs.characters[cc].attack_cn = co as u16;
        let idx = co as i32 | (co_id << 16);
        gs.characters[cc].data[80] = idx;
    }

    gs.characters[cc].data[63] = cn as i32;
    gs.characters[cc].data[69] = cn as i32;

    if (gs.characters[cn].flags & CharacterFlags::Player.bits()) != 0 {
        gs.characters[cc].data[CHD_COMPANION] = 0;
    } else {
        gs.characters[cc].data[CHD_COMPANION] = ticker + TICKS * 60 * 5;
    }
    gs.characters[cc].data[98] = ticker + COMPANION_TIMEOUT;

    let text0 = b"#14#Yes! %s buys the farm!";
    gs.characters[cc].text[0][..text0.len()].copy_from_slice(text0);
    let text1 = b"#13#Yahoo! An enemy! Prepare to die, %s!";
    gs.characters[cc].text[1][..text1.len()].copy_from_slice(text1);
    let text3 = b"My successor will avenge me, %s!";
    gs.characters[cc].text[3][..text3.len()].copy_from_slice(text3);

    gs.characters[cc].data[48] = 33;

    gs.characters[cc].data[CHD_TALKATIVE] =
        gs.character_templates[CT_COMPANION as usize].data[CHD_TALKATIVE];

    for n in 0..5 {
        let mut tmp = base;
        tmp = tmp * 3 / std::cmp::max(1, i32::from(gs.characters[cc].attrib[n][3]));
        gs.characters[cc].attrib[n][0] = std::cmp::max(
            10,
            std::cmp::min(i32::from(gs.characters[cc].attrib[n][2]), tmp) as u8,
        );
    }

    for n in 0..core::skills::MAX_SKILLS {
        let mut tmp = base;
        tmp = tmp * 3 / std::cmp::max(1, i32::from(gs.characters[cc].skill[n][3]));
        if gs.characters[cc].skill[n][2] != 0 {
            gs.characters[cc].skill[n][0] = std::cmp::min(gs.characters[cc].skill[n][2], tmp as u8);
        }
    }

    gs.characters[cc].hp[0] = std::cmp::max(
        50,
        std::cmp::min(i32::from(gs.characters[cc].hp[2]), base * 5),
    ) as u16;
    gs.characters[cc].end[0] = std::cmp::max(
        50,
        std::cmp::min(i32::from(gs.characters[cc].end[2]), base * 5),
    ) as u16;
    gs.characters[cc].mana[0] = 0;

    let mut pts = 0i32;

    let attribs = gs.characters[cc].attrib;
    let hp0 = gs.characters[cc].hp[0];
    let end0 = gs.characters[cc].end[0];
    let mana0 = gs.characters[cc].mana[0];
    let skills = gs.characters[cc].skill;

    for attrib in &attribs[..5] {
        for m in 10..i32::from(attrib[0]) {
            pts += points::attrib_needed(m, 3);
        }
    }

    for m in 50..i32::from(hp0) {
        pts += points::hp_needed(m, 3);
    }

    for m in 50..i32::from(end0) {
        pts += points::end_needed(m, 2);
    }

    for m in 50..i32::from(mana0) {
        pts += points::mana_needed(m, 3);
    }

    for skill in &skills[..50] {
        for m in 1..i32::from(skill[0]) {
            pts += points::skill_needed(m, 2);
        }
    }

    gs.characters[cc].points_tot = pts;
    gs.characters[cc].gold = 0;
    gs.characters[cc].a_hp = 999999;
    gs.characters[cc].a_end = 999999;
    gs.characters[cc].a_mana = 999999;
    gs.characters[cc].alignment = gs.characters[cn].alignment / 2;

    let agil = gs.characters[cc].attrib[AT_AGIL as usize][0];
    let stren = gs.characters[cc].attrib[AT_STREN as usize][0];

    if agil >= 90 && stren >= 90 {
        gs.characters[cc].armor_bonus = 48 + 32;
        gs.characters[cc].weapon_bonus = 40 + 32;
    } else if agil >= 72 && stren >= 72 {
        gs.characters[cc].armor_bonus = 36 + 28;
        gs.characters[cc].weapon_bonus = 32 + 28;
    } else if agil >= 40 && stren >= 40 {
        gs.characters[cc].armor_bonus = 30 + 24;
        gs.characters[cc].weapon_bonus = 24 + 24;
    } else if agil >= 24 && stren >= 24 {
        gs.characters[cc].armor_bonus = 24 + 20;
        gs.characters[cc].weapon_bonus = 16 + 20;
    } else if agil >= 16 && stren >= 16 {
        gs.characters[cc].armor_bonus = 18 + 16;
        gs.characters[cc].weapon_bonus = 8 + 16;
    } else if agil >= 12 && stren >= 12 {
        gs.characters[cc].armor_bonus = 12 + 12;
        gs.characters[cc].weapon_bonus = 8 + 12;
    } else if agil >= 10 && stren >= 10 {
        gs.characters[cc].armor_bonus = 6 + 8;
        gs.characters[cc].weapon_bonus = 8 + 8;
    }

    let (cc_name, cn_ref) = (gs.characters[cc].name, gs.characters[cn].reference);
    log::info!(
        "Created {} ({}) with base {} as Ghost Companion for {}",
        c_string_to_str(&cc_name),
        cc,
        base,
        c_string_to_str(&cn_ref)
    );

    // Make companion speak
    if co != 0 {
        let co_name = gs.characters[co].get_name().to_owned();
        gs.do_sayx(
            cc,
            &format!("#13#Yahoo! An enemy! Prepare to die, {}!", co_name),
        );
    } else {
        let rank = core::ranks::points2rank(pts as u32);
        let cn_name = gs.characters[cn].get_name().to_owned();
        if rank < 6 {
            // GC not yet Master Sergeant
            gs.do_sayx(cc, &format!("I shall defend you and obey your commands, {}. I will WAIT, FOLLOW , be QUIET or ATTACK for you and tell you WHAT TIME. You may also command me to TRANSFER my experience to you, though I'd rather you didn't.\n", cn_name));
        } else {
            gs.do_sayx(cc, &format!("Thank you for creating me, {}!\n", cn_name));
        }
    }

    gs.do_update_char(cc);

    add_exhaust(gs, cn, TICKS * 4);

    let (cc_x, cc_y) = (
        i32::from(gs.characters[cc].x),
        i32::from(gs.characters[cc].y),
    );
    EffectManager::fx_add_effect(gs, 6, 0, cc_x, cc_y, 0);
    let (cn_x, cn_y) = (
        i32::from(gs.characters[cn].x),
        i32::from(gs.characters[cn].y),
    );
    EffectManager::fx_add_effect(gs, 7, 0, cn_x, cn_y, 0);
}

/// Returns whether one character is facing another adjacent character.
///
/// # Arguments
///
/// * `cn` - Character whose facing direction is tested.
/// * `co` - Character whose position is compared against `cn`.
///
/// # Returns
///
/// * `true` when `co` is directly in front of `cn`, otherwise `false`.
pub fn is_facing(cn: &Character, co: &Character) -> bool {
    let dir = cn.dir;
    let cx = cn.x;
    let cy = cn.y;
    let ox = co.x;
    let oy = co.y;

    match dir {
        DX_RIGHT => cx + 1 == ox && cy == oy,
        DX_LEFT => cx - 1 == ox && cy == oy,
        DX_UP => cx == ox && cy - 1 == oy,
        DX_DOWN => cx == ox && cy + 1 == oy,
        _ => false,
    }
}

/// Returns whether one character has another adjacent character behind them.
///
/// # Arguments
///
/// * `cn` - Character whose facing direction is tested.
/// * `co` - Character whose position is compared against `cn`.
///
/// # Returns
///
/// * `true` when `co` is directly behind `cn`, otherwise `false`.
pub fn is_back(cn: &Character, co: &Character) -> bool {
    let dir = cn.dir;
    let cx = cn.x;
    let cy = cn.y;
    let ox = co.x;
    let oy = co.y;

    match dir {
        DX_LEFT => cx + 1 == ox && cy == oy,
        DX_RIGHT => cx - 1 == ox && cy == oy,
        DX_DOWN => cx == ox && cy - 1 == oy,
        DX_UP => cx == ox && cy + 1 == oy,
        _ => false,
    }
}

/// Logs the standard no-magic failure message for a character.
///
/// # Arguments
///
/// * `gs` - Active game state used for character logging.
/// * `cn` - Character index receiving the failure message.
///
/// # Panics
///
/// * Panics if `cn` is not a valid character index.
pub fn nomagic(gs: &mut GameState, cn: usize) {
    gs.do_character_log(
        cn,
        FontColor::Green,
        "Your magic fails. You seem to be unable to cast spells.\n",
    );
}

/// Returns true if the character currently has an active spell-item with the
/// given skill `temp`, i.e. that skill is still on cooldown.
///
/// # Arguments
///
/// * `gs` - Mutable reference to the game state.
/// * `cn` - Character index to inspect.
/// * `skill_temp` - The `temp` value of the cooldown spell-item to look for
///   (typically the originating skill constant such as `SK_DELIVER_DEATH`).
///
/// # Returns
///
/// * `true` if a matching cooldown spell-item is still present.
fn skill_on_cooldown(gs: &mut GameState, cn: usize, skill_temp: u16) -> bool {
    for n in 0..20 {
        let in_ = gs.characters[cn].spell[n] as usize;
        if in_ != 0 && gs.items[in_].temp == skill_temp {
            gs.do_character_log(cn, FontColor::Red, "That ability is still recharging.\n");
            return true;
        }
    }
    false
}

/// Adds a per-skill cooldown spell-item to the caster.
///
/// The created item has no stat effect; its sole purpose is to remain in
/// `character.spell[]` for `len` ticks so that `skill_on_cooldown` can detect
/// it. Mirrors the pattern of `add_exhaust` but uses a caller-supplied skill
/// `temp` so each ability tracks its own recharge timer independently.
///
/// # Arguments
///
/// * `gs` - Mutable reference to the game state.
/// * `cn` - Character index to receive the cooldown marker.
/// * `len` - Cooldown duration in ticks.
/// * `skill_temp` - Originating skill constant stored in `item.temp`.
/// * `name` - Display name used when the cooldown is reported (must fit 40 bytes).
fn add_skill_cooldown(gs: &mut GameState, cn: usize, len: i32, skill_temp: u16, name: &[u8]) {
    let in_ = match God::create_item(gs, 1) {
        Some(i) => i,
        None => {
            log::error!("god_create_item failed in add_skill_cooldown");
            return;
        }
    };
    {
        let item = &mut gs.items[in_];
        let mut name_bytes = [0u8; 40];
        let nlen = name.len().min(40);
        name_bytes[..nlen].copy_from_slice(&name[..nlen]);
        item.name = name_bytes;
        item.flags |= ItemFlags::IF_SPELL.bits();
        item.sprite[1] = 97;
        item.duration = len as u32;
        item.active = len as u32;
        item.temp = skill_temp;
        item.power = 255;
    }
    add_spell(gs, cn, in_);
}

/// Resolves the active offensive target for a skill cast.
///
/// Mirrors the target-selection pattern shared by `skill_blast`, `skill_curse`
/// and `skill_stun`: prefer `skill_target1`, fall back to `attack_cn`, then
/// the caster themselves.
fn resolve_offensive_target(gs: &GameState, cn: usize) -> usize {
    if gs.characters[cn].skill_target1 != 0 {
        gs.characters[cn].skill_target1 as usize
    } else if gs.characters[cn].attack_cn != 0 {
        gs.characters[cn].attack_cn as usize
    } else {
        cn
    }
}

/// Common preflight checks for hostile single-target casts.
///
/// Validates visibility, self-target rejection, stoned target, attack
/// permission, and remembers the PvP exchange. Returns true if the cast may
/// proceed.
fn hostile_cast_preflight(gs: &mut GameState, cn: usize, co: usize, self_msg: &str) -> bool {
    if cn == co {
        gs.do_character_log(cn, FontColor::Red, self_msg);
        return false;
    }
    if gs.do_char_can_see(cn, co) == 0 {
        gs.do_character_log(cn, FontColor::Red, "You cannot see your target.\n");
        return false;
    }
    if (gs.characters[co].flags & CharacterFlags::Stoned.bits()) != 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            "Your target is lagging. Try again later.\n",
        );
        return false;
    }
    if !gs.may_attack_msg(cn, co, true) {
        chlog!(
            cn,
            "Prevented from attacking {}",
            gs.characters[co].get_name().to_owned()
        );
        return false;
    }
    gs.remember_pvp(cn, co);
    true
}

/// Internal helper: attach a Parasite-family DoT spell-item to `co`.
///
/// Used both for the direct Parasite/Contagion cast and for on-death spread.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `caster` - Caster character index (recorded in `item.data[0]` for lifesteal).
/// * `co` - Target character index.
/// * `power` - Caster skill power; drives damage and resistance roll.
/// * `temp` - Either `SK_PARASITE` or `SK_CONTAGION`.
/// * `duration_ticks` - Total duration in ticks.
/// * `name` - Display name shown to players.
///
/// # Returns
///
/// * `true` if the DoT was successfully attached.
pub(crate) fn apply_parasitic_dot(
    gs: &mut GameState,
    caster: usize,
    co: usize,
    power: i32,
    temp: u16,
    duration_ticks: i32,
    name: &[u8],
) -> bool {
    if (gs.characters[co].flags & CharacterFlags::Immortal.bits()) != 0 {
        return false;
    }
    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in apply_parasitic_dot");
        return false;
    }
    let in_idx = in_opt.unwrap();

    let mut power = spell_immunity(gs, power, i32::from(gs.characters[co].skill[SK_IMMUN][5]));
    power = spell_race_mod(gs, power, gs.characters[caster].kindred);
    if power < 1 {
        gs.items[in_idx].used = USE_EMPTY;
        return false;
    }

    {
        let item = &mut gs.items[in_idx];
        let mut name_bytes = [0u8; 40];
        let nlen = name.len().min(40);
        name_bytes[..nlen].copy_from_slice(&name[..nlen]);
        item.name = name_bytes;
        item.flags |= ItemFlags::IF_SPELL.bits();
        item.sprite[1] = 89;
        item.duration = duration_ticks as u32;
        item.active = duration_ticks as u32;
        item.temp = temp;
        item.power = power as u32;
        item.data[0] = caster as u32;
        // data[1] is reserved for Contagion to track the last tick at which it
        // spread, preventing repeat spreads within the same combat round.
        item.data[1] = 0;
    }

    if add_spell(gs, co, in_idx) == 0 {
        return false;
    }
    true
}

/// Active spell: infest the target with parasites that drain HP over time and
/// heal the caster for a fraction of the damage dealt.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index.
pub fn skill_parasite(gs: &mut GameState, cn: usize) {
    let co = resolve_offensive_target(gs, cn);
    if !hostile_cast_preflight(gs, cn, co, "You cannot infect yourself.\n") {
        return;
    }
    if spellcost(gs, cn, 20) != 0 {
        return;
    }
    if chance(gs, cn, 18) != 0 {
        return;
    }

    let power = i32::from(gs.characters[cn].skill[SK_PARASITE][5]);
    if !apply_parasitic_dot(
        gs,
        cn,
        co,
        power,
        SK_PARASITE as u16,
        TICKS * 8,
        b"Parasite",
    ) {
        gs.do_character_log(
            cn,
            FontColor::Green,
            "Magical interference neutralised your parasite.\n",
        );
        return;
    }

    let name = gs.characters[co].get_name().to_owned();
    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!("{} was infested with parasites.\n", name),
    );
    gs.do_character_log(co, FontColor::Green, "Parasites burrow into your flesh!\n");
    gs.do_notify_character(co as u32, i32::from(NT_GOTHIT), cn as i32, 0, 0, 0);
    gs.do_notify_character(cn as u32, i32::from(NT_DIDHIT), co as i32, 0, 0, 0);
    chlog!(cn, "Cast Parasite on {}", name);
    EffectManager::fx_add_effect(
        gs,
        5,
        0,
        i32::from(gs.characters[co].x),
        i32::from(gs.characters[co].y),
        0,
    );
}

/// Active spell: distract the target, reducing their Agility for a short time.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index.
pub fn skill_distract(gs: &mut GameState, cn: usize) {
    let co = resolve_offensive_target(gs, cn);
    if !hostile_cast_preflight(gs, cn, co, "You cannot distract yourself.\n") {
        return;
    }
    if spellcost(gs, cn, 15) != 0 {
        return;
    }
    if chance(gs, cn, 18) != 0 {
        return;
    }

    let power = i32::from(gs.characters[cn].skill[SK_DISTRACT][5]);
    let power = spell_immunity(gs, power, i32::from(gs.characters[co].skill[SK_IMMUN][5]));
    let power = spell_race_mod(gs, power, gs.characters[cn].kindred);
    if power < 1 {
        return;
    }
    if (gs.characters[co].flags & CharacterFlags::Immortal.bits()) != 0 {
        gs.do_character_log(cn, FontColor::Red, "You lost your focus.\n");
        return;
    }

    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_distract");
        return;
    }
    let in_idx = in_opt.unwrap();
    let agility_penalty = -((power / 3).clamp(1, 30)) as i8;
    {
        let item = &mut gs.items[in_idx];
        let mut name_bytes = [0u8; 40];
        let name = b"Distract";
        let nlen = name.len().min(40);
        name_bytes[..nlen].copy_from_slice(&name[..nlen]);
        item.name = name_bytes;
        item.flags |= ItemFlags::IF_SPELL.bits();
        item.sprite[1] = 89;
        item.duration = (TICKS * 10) as u32;
        item.active = (TICKS * 10) as u32;
        item.temp = SK_DISTRACT as u16;
        item.power = power as u32;
        item.attrib[AT_AGIL as usize][1] = agility_penalty;
    }
    if add_spell(gs, co, in_idx) == 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            "Magical interference neutralised your distraction.\n",
        );
        return;
    }

    let name = gs.characters[co].get_name().to_owned();
    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!("{} is now distracted.\n", name),
    );
    gs.do_character_log(co, FontColor::Green, "You feel distracted!\n");
    gs.do_notify_character(co as u32, i32::from(NT_GOTHIT), cn as i32, 0, 0, 0);
    gs.do_notify_character(cn as u32, i32::from(NT_DIDHIT), co as i32, 0, 0, 0);
    chlog!(cn, "Cast Distract on {}", name);
    EffectManager::fx_add_effect(
        gs,
        5,
        0,
        i32::from(gs.characters[co].x),
        i32::from(gs.characters[co].y),
        0,
    );
}

/// Active melee finisher: a devastating blow against a low-health adjacent
/// enemy. Deals massively amplified weapon damage when the target is below
/// 25% HP, otherwise deals modestly increased damage. Long cooldown.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index.
pub fn skill_deliver_death(gs: &mut GameState, cn: usize) {
    let co = resolve_offensive_target(gs, cn);
    if !hostile_cast_preflight(gs, cn, co, "You cannot strike yourself down.\n") {
        return;
    }
    if skill_on_cooldown(gs, cn, SK_DELIVER_DEATH as u16) {
        return;
    }
    // Adjacency check
    let dx = (i32::from(gs.characters[cn].x) - i32::from(gs.characters[co].x)).abs();
    let dy = (i32::from(gs.characters[cn].y) - i32::from(gs.characters[co].y)).abs();
    if dx > 1 || dy > 1 {
        gs.do_character_log(cn, FontColor::Red, "Your target is too far away.\n");
        return;
    }
    if gs.characters[cn].a_end < 150 * 1000 {
        gs.do_character_log(cn, FontColor::Red, "You're too exhausted!\n");
        return;
    }
    gs.characters[cn].a_end -= 150 * 1000;

    let weapon = i32::from(gs.characters[cn].weapon).max(1);
    let max_hp = i32::from(gs.characters[co].hp[5]).max(1);
    let cur_hp_pct = gs.characters[co].a_hp / max_hp; // 0..1000
    let dam = if cur_hp_pct < 250 {
        weapon * 5 + helpers::random_mod_i32(weapon * 2)
    } else {
        weapon * 2 + helpers::random_mod_i32(weapon)
    };

    let applied = gs.do_hurt(cn, co, dam, 0);
    if applied < 1 {
        gs.do_character_log(cn, FontColor::Green, "Your blow glances off harmlessly.\n");
    } else {
        let name = gs.characters[co].get_name().to_owned();
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("You deliver death to {} for {} HP.\n", name, applied),
        );
        chlog!(cn, "Cast Deliver Death on {} for {} HP", name, applied);
    }

    let tx = i32::from(gs.characters[co].x);
    let ty = i32::from(gs.characters[co].y);
    gs.do_area_sound(co, 0, tx, ty, i32::from(gs.characters[cn].sound) + 4);
    EffectManager::fx_add_effect(gs, 5, 0, tx, ty, 0);

    add_skill_cooldown(
        gs,
        cn,
        TICKS * 45,
        SK_DELIVER_DEATH as u16,
        b"Deliver Death Cooldown",
    );
}

/// Active spell: weakens the target's effective weapon skill for a short time,
/// reducing their hit chance in melee.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index.
pub fn skill_disarm(gs: &mut GameState, cn: usize) {
    let co = resolve_offensive_target(gs, cn);
    if !hostile_cast_preflight(gs, cn, co, "You cannot disarm yourself.\n") {
        return;
    }
    if spellcost(gs, cn, 25) != 0 {
        return;
    }
    if chance(gs, cn, 18) != 0 {
        return;
    }

    let power = i32::from(gs.characters[cn].skill[SK_DISARM][5]);
    let power = spell_immunity(gs, power, i32::from(gs.characters[co].skill[SK_IMMUN][5]));
    let power = spell_race_mod(gs, power, gs.characters[cn].kindred);
    if power < 1 {
        return;
    }
    if (gs.characters[co].flags & CharacterFlags::Immortal.bits()) != 0 {
        gs.do_character_log(cn, FontColor::Red, "You lost your focus.\n");
        return;
    }

    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_disarm");
        return;
    }
    let in_idx = in_opt.unwrap();
    let weapon_penalty = -((power / 2).clamp(1, 50)) as i8;
    {
        let item = &mut gs.items[in_idx];
        let mut name_bytes = [0u8; 40];
        let name = b"Disarm";
        let nlen = name.len().min(40);
        name_bytes[..nlen].copy_from_slice(&name[..nlen]);
        item.name = name_bytes;
        item.flags |= ItemFlags::IF_SPELL.bits();
        item.sprite[1] = 89;
        item.duration = (TICKS * 15) as u32;
        item.active = (TICKS * 15) as u32;
        item.temp = SK_DISARM as u16;
        item.power = power as u32;
        item.skill[SK_WEAPON][1] = weapon_penalty;
    }
    if add_spell(gs, co, in_idx) == 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            "Magical interference neutralised your disarm.\n",
        );
        return;
    }

    let name = gs.characters[co].get_name().to_owned();
    gs.do_character_log(cn, FontColor::Green, &format!("{} was disarmed.\n", name));
    gs.do_character_log(
        co,
        FontColor::Green,
        "Your weapon feels heavy and unfamiliar!\n",
    );
    gs.do_notify_character(co as u32, i32::from(NT_GOTHIT), cn as i32, 0, 0, 0);
    gs.do_notify_character(cn as u32, i32::from(NT_DIDHIT), co as i32, 0, 0, 0);
    chlog!(cn, "Cast Disarm on {}", name);
    EffectManager::fx_add_effect(
        gs,
        5,
        0,
        i32::from(gs.characters[co].x),
        i32::from(gs.characters[co].y),
        0,
    );
}

/// Active spell: a virulent parasitic infection that lasts much longer than
/// Parasite and spreads to adjacent enemies when the host dies.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index.
pub fn skill_contagion(gs: &mut GameState, cn: usize) {
    let co = resolve_offensive_target(gs, cn);
    if !hostile_cast_preflight(gs, cn, co, "You cannot infect yourself.\n") {
        return;
    }
    if spellcost(gs, cn, 40) != 0 {
        return;
    }
    if chance(gs, cn, 18) != 0 {
        return;
    }

    let power = i32::from(gs.characters[cn].skill[SK_CONTAGION][5]);
    if !apply_parasitic_dot(
        gs,
        cn,
        co,
        power,
        SK_CONTAGION as u16,
        TICKS * 60 * 8,
        b"Contagion",
    ) {
        gs.do_character_log(
            cn,
            FontColor::Green,
            "Magical interference neutralised your contagion.\n",
        );
        return;
    }

    let name = gs.characters[co].get_name().to_owned();
    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!("{} was struck by a virulent contagion.\n", name),
    );
    gs.do_character_log(co, FontColor::Green, "A virulent contagion takes hold!\n");
    gs.do_notify_character(co as u32, i32::from(NT_GOTHIT), cn as i32, 0, 0, 0);
    gs.do_notify_character(cn as u32, i32::from(NT_DIDHIT), co as i32, 0, 0, 0);
    chlog!(cn, "Cast Contagion on {}", name);
    EffectManager::fx_add_effect(
        gs,
        5,
        0,
        i32::from(gs.characters[co].x),
        i32::from(gs.characters[co].y),
        0,
    );
}

/// Active melee skill: a sweeping flurry of strikes that hits every adjacent
/// hostile target for double the standard Surround Hit damage. Costs
/// endurance and goes on a short cooldown afterwards.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index.
pub fn skill_blade_dance(gs: &mut GameState, cn: usize) {
    if skill_on_cooldown(gs, cn, SK_BLADE_DANCE as u16) {
        return;
    }
    if gs.characters[cn].a_end < 200 * 1000 {
        gs.do_character_log(cn, FontColor::Red, "You're too exhausted!\n");
        return;
    }
    gs.characters[cn].a_end -= 200 * 1000;

    let weapon = i32::from(gs.characters[cn].weapon).max(1);
    let caster_x = i32::from(gs.characters[cn].x);
    let caster_y = i32::from(gs.characters[cn].y);
    let attacker_is_player = (gs.characters[cn].flags & CharacterFlags::Player.bits()) != 0;
    let aoe_base = if attacker_is_player { 4 } else { 1 };

    let mut hits = 0;
    for co in helpers::skill_aoe_targets(gs, Some(cn), caster_x, caster_y, aoe_base) {
        if co == cn {
            continue;
        }
        if !gs.may_attack_msg(cn, co, false) {
            continue;
        }
        let base_dam = weapon + helpers::random_mod_i32(weapon.max(1));
        // Mirror Surround Hit's reduction (3/4 of base) then double it for Blade Dance.
        let sdam = (base_dam - base_dam / 4) * 2;
        gs.remember_pvp(cn, co);
        let applied = gs.do_hurt(cn, co, sdam, 0);
        if applied > 0 {
            hits += 1;
            let name = gs.characters[co].get_name().to_owned();
            gs.do_character_log(
                cn,
                FontColor::Green,
                &format!("Your blade strikes {} for {} HP.\n", name, applied),
            );
        }
        let tx = i32::from(gs.characters[co].x);
        let ty = i32::from(gs.characters[co].y);
        EffectManager::fx_add_effect(gs, 5, 0, tx, ty, 0);
    }

    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!("Your blade dance lands {} strike(s).\n", hits),
    );
    chlog!(cn, "Performed Blade Dance ({} hits)", hits);

    add_skill_cooldown(
        gs,
        cn,
        TICKS * 30,
        SK_BLADE_DANCE as u16,
        b"Blade Dance Cooldown",
    );
}

// =====================================================================
// Templar talent-granted skills (SK_RAINS_OF_RENEWAL .. SK_INNER_STRENGTH)
// =====================================================================

/// Returns whether `temp` corresponds to a debuff blocked by Seeing Red.
///
/// Cooldown markers and passive buffs are intentionally excluded so that the
/// caster's own Seeing Red item still attaches.
///
/// # Arguments
///
/// * `temp` - The `item.temp` of an incoming spell attachment.
///
/// # Returns
///
/// * `true` if Seeing Red should refuse this attachment.
pub(crate) fn is_seeing_red_blocked_temp(temp: u16) -> bool {
    matches!(
        temp as usize,
        SK_STUN | SK_WARCRY2 | SK_CURSE | SK_CONTAGION | SK_DISTRACT | SK_DISARM
    )
}

/// Returns whether `cn` currently has an active Seeing Red spell item.
///
/// # Arguments
///
/// * `gs` - Game state holding spell slots and item table.
/// * `cn` - Character index to inspect.
///
/// # Returns
///
/// * `true` if a Seeing Red marker is present in `character.spell[]`.
pub(crate) fn has_active_seeing_red(gs: &GameState, cn: usize) -> bool {
    for n in 0..20 {
        let in_ = gs.characters[cn].spell[n] as usize;
        if in_ != 0 && gs.items[in_].temp == SK_SEEING_RED as u16 {
            return true;
        }
    }
    false
}

/// Active self-cast: Rains of Renewal. Spends endurance up front and attaches
/// a heal-over-time spell item that ticks via the spell processor in
/// `state/stats.rs`.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index.
pub fn skill_rains_of_renewal(gs: &mut GameState, cn: usize) {
    if skill_on_cooldown(gs, cn, SK_RAINS_OF_RENEWAL as u16) {
        return;
    }
    if gs.characters[cn].a_end < 100 * 1000 {
        gs.do_character_log(cn, FontColor::Red, "You're too exhausted!\n");
        return;
    }
    gs.characters[cn].a_end -= 100 * 1000;

    let power = i32::from(gs.characters[cn].skill[SK_RAINS_OF_RENEWAL][5]);
    let duration = TICKS * 20;

    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_rains_of_renewal");
        return;
    }
    let in_idx = in_opt.unwrap();
    {
        let item = &mut gs.items[in_idx];
        let mut name_bytes = [0u8; 40];
        let name = b"Rains of Renewal";
        let nlen = name.len().min(40);
        name_bytes[..nlen].copy_from_slice(&name[..nlen]);
        item.name = name_bytes;
        item.flags |= ItemFlags::IF_SPELL.bits();
        item.sprite[1] = 88;
        item.duration = duration as u32;
        item.active = duration as u32;
        item.temp = SK_RAINS_OF_RENEWAL as u16;
        item.power = power.max(1) as u32;
    }
    if add_spell(gs, cn, in_idx) == 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            "Magical interference neutralised your blessing.\n",
        );
        return;
    }

    gs.do_character_log(cn, FontColor::Green, "Renewing rains wash over you.\n");
    chlog!(cn, "Cast Rains of Renewal");
    EffectManager::fx_add_effect(
        gs,
        7,
        0,
        i32::from(gs.characters[cn].x),
        i32::from(gs.characters[cn].y),
        0,
    );
}

/// Active melee strike: Gash. Pays 5% of current HP and deals weapon damage
/// scaled by the Gash skill power, then enters a short cooldown.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index.
pub fn skill_gash(gs: &mut GameState, cn: usize) {
    let co = resolve_offensive_target(gs, cn);
    if !hostile_cast_preflight(gs, cn, co, "You cannot gash yourself.\n") {
        return;
    }
    if skill_on_cooldown(gs, cn, SK_GASH as u16) {
        return;
    }
    let dx = (i32::from(gs.characters[cn].x) - i32::from(gs.characters[co].x)).abs();
    let dy = (i32::from(gs.characters[cn].y) - i32::from(gs.characters[co].y)).abs();
    if dx > 1 || dy > 1 {
        gs.do_character_log(cn, FontColor::Red, "Your target is too far away.\n");
        return;
    }

    // Self damage: 5% of current HP (a_hp is stored in 1/1000ths).
    let self_dam_units = (gs.characters[cn].a_hp / 20).max(1);
    if gs.characters[cn].a_hp <= self_dam_units {
        gs.do_character_log(
            cn,
            FontColor::Red,
            "You are too wounded to gash yourself.\n",
        );
        return;
    }
    gs.characters[cn].a_hp -= self_dam_units;

    let weapon = i32::from(gs.characters[cn].weapon).max(1);
    let power = i32::from(gs.characters[cn].skill[SK_GASH][5]);
    // Amplification scales with skill power: +50% at power=50, +150% at power=150.
    let bonus_pct = power.clamp(10, 200);
    let dam = weapon * (100 + bonus_pct) / 100 + helpers::random_mod_i32(weapon.max(1));

    let applied = gs.do_hurt(cn, co, dam, 0);
    if applied < 1 {
        gs.do_character_log(cn, FontColor::Green, "Your blow glances off harmlessly.\n");
    } else {
        let name = gs.characters[co].get_name().to_owned();
        gs.do_character_log(
            cn,
            FontColor::Green,
            &format!("You gash {} for {} HP.\n", name, applied),
        );
        chlog!(cn, "Cast Gash on {} for {} HP", name, applied);
    }

    let tx = i32::from(gs.characters[co].x);
    let ty = i32::from(gs.characters[co].y);
    gs.do_area_sound(co, 0, tx, ty, i32::from(gs.characters[cn].sound) + 4);
    EffectManager::fx_add_effect(gs, 5, 0, tx, ty, 0);

    add_skill_cooldown(gs, cn, TICKS * 10, SK_GASH as u16, b"Gash Cooldown");
}

/// Active self-buff: Sun's Blessing. No resource cost. Long cooldown set
/// slightly shorter than the buff duration so the player can recast as the
/// effect is expiring without stacking.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index.
pub fn skill_suns_blessing(gs: &mut GameState, cn: usize) {
    if skill_on_cooldown(gs, cn, SK_SUNS_BLESSING as u16) {
        return;
    }
    let power = i32::from(gs.characters[cn].skill[SK_SUNS_BLESSING][5]);
    let bonus = (power / 10 + 2).clamp(1, 30) as i8;
    let buff_duration = TICKS * 60;
    let cooldown_len = TICKS * 55;

    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_suns_blessing");
        return;
    }
    let in_idx = in_opt.unwrap();
    {
        let item = &mut gs.items[in_idx];
        let mut name_bytes = [0u8; 40];
        let name = b"Sun's Blessing";
        let nlen = name.len().min(40);
        name_bytes[..nlen].copy_from_slice(&name[..nlen]);
        item.name = name_bytes;
        item.flags |= ItemFlags::IF_SPELL.bits();
        item.sprite[1] = 88;
        item.duration = buff_duration as u32;
        item.active = buff_duration as u32;
        item.temp = SK_SUNS_BLESSING2 as u16;
        item.power = power.max(1) as u32;
        for n in 0..5 {
            item.attrib[n][1] = bonus;
        }
        item.armor[1] = bonus;
        item.weapon[1] = bonus;
    }
    if add_spell(gs, cn, in_idx) == 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            "Magical interference neutralised your blessing.\n",
        );
        return;
    }

    add_skill_cooldown(
        gs,
        cn,
        cooldown_len,
        SK_SUNS_BLESSING as u16,
        b"Sun's Blessing Cooldown",
    );

    gs.do_character_log(
        cn,
        FontColor::Green,
        "The sun's blessing strengthens you.\n",
    );
    chlog!(cn, "Cast Sun's Blessing");
    EffectManager::fx_add_effect(
        gs,
        6,
        0,
        i32::from(gs.characters[cn].x),
        i32::from(gs.characters[cn].y),
        0,
    );
}

/// Active self-buff: Seeing Red. Spends endurance; while active the caster
/// gains a large temporary weapon-skill bonus and refuses to receive new
/// stun/curse/distract/disarm style debuff attachments.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index.
pub fn skill_seeing_red(gs: &mut GameState, cn: usize) {
    if skill_on_cooldown(gs, cn, SK_SEEING_RED as u16) {
        return;
    }
    if gs.characters[cn].a_end < 150 * 1000 {
        gs.do_character_log(cn, FontColor::Red, "You're too exhausted!\n");
        return;
    }
    gs.characters[cn].a_end -= 150 * 1000;

    let power = i32::from(gs.characters[cn].skill[SK_SEEING_RED][5]);
    // Roughly double outgoing damage by mirroring the caster's current
    // weapon value as a flat weapon[1] bonus, capped to i8 range.
    let weapon = i32::from(gs.characters[cn].weapon).clamp(1, 120) as i8;
    let duration = TICKS * (5 + (power / 5).clamp(0, 25));
    let cooldown_len = TICKS * 60;

    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_seeing_red");
        return;
    }
    let in_idx = in_opt.unwrap();
    {
        let item = &mut gs.items[in_idx];
        let mut name_bytes = [0u8; 40];
        let name = b"Seeing Red";
        let nlen = name.len().min(40);
        name_bytes[..nlen].copy_from_slice(&name[..nlen]);
        item.name = name_bytes;
        item.flags |= ItemFlags::IF_SPELL.bits();
        item.sprite[1] = 88;
        item.duration = duration as u32;
        item.active = duration as u32;
        item.temp = SK_SEEING_RED as u16;
        item.power = power.max(1) as u32;
        item.weapon[1] = weapon;
    }
    if add_spell(gs, cn, in_idx) == 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            "Magical interference neutralised your fury.\n",
        );
        return;
    }

    add_skill_cooldown(
        gs,
        cn,
        cooldown_len,
        SK_SEEING_RED as u16,
        b"Seeing Red Cooldown",
    );

    gs.do_character_log(cn, FontColor::Green, "You are seeing red!\n");
    chlog!(cn, "Cast Seeing Red");
    EffectManager::fx_add_effect(
        gs,
        5,
        0,
        i32::from(gs.characters[cn].x),
        i32::from(gs.characters[cn].y),
        0,
    );
}

/// Active AoE: Thunderous Fury. Upgraded Warcry that stuns every nearby
/// hostile and deals a weakened blast of damage to each. Long cooldown.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index.
pub fn skill_thunderous_fury(gs: &mut GameState, cn: usize) {
    if skill_on_cooldown(gs, cn, SK_THUNDEROUS_FURY as u16) {
        return;
    }
    if gs.characters[cn].a_end < 250 * 1000 {
        gs.do_character_log(cn, FontColor::Red, "You're too exhausted!\n");
        return;
    }
    gs.characters[cn].a_end -= 250 * 1000;

    let power = i32::from(gs.characters[cn].skill[SK_THUNDEROUS_FURY][5]);
    let blast_base = (power / 4).max(1);

    let xf = std::cmp::max(1, i32::from(gs.characters[cn].x) - 10);
    let yf = std::cmp::max(1, i32::from(gs.characters[cn].y) - 10);
    let xt = std::cmp::min(
        core::constants::SERVER_MAPX - 1,
        i32::from(gs.characters[cn].x) + 10,
    );
    let yt = std::cmp::min(
        core::constants::SERVER_MAPY - 1,
        i32::from(gs.characters[cn].y) + 10,
    );

    let mut hit = 0;
    let mut miss = 0;
    for x in xf..xt {
        for y in yf..yt {
            let m = (x + y * core::constants::SERVER_MAPX) as usize;
            let co = gs.map[m].ch as usize;
            if co == 0 || co == cn {
                continue;
            }
            if warcry(gs, cn, co, power) {
                gs.remember_pvp(cn, co);
                let name = gs.characters[cn].get_name().to_owned();
                gs.do_character_log(
                    co,
                    FontColor::Green,
                    &format!("{}'s thunderous fury crashes over you!\n", name),
                );
                let dam = blast_base + helpers::random_mod_i32(blast_base);
                let _ = gs.do_hurt(cn, co, dam, 0);
                let tx = i32::from(gs.characters[co].x);
                let ty = i32::from(gs.characters[co].y);
                EffectManager::fx_add_effect(gs, 5, 0, tx, ty, 0);
                hit += 1;
            } else {
                miss += 1;
            }
        }
    }

    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!(
            "Your thunderous fury strikes {} of {} foes.\n",
            hit,
            hit + miss
        ),
    );
    chlog!(cn, "Cast Thunderous Fury ({} hits)", hit);

    add_skill_cooldown(
        gs,
        cn,
        TICKS * 30,
        SK_THUNDEROUS_FURY as u16,
        b"Thunderous Fury Cooldown",
    );
}

/// Active AoE + self buff: Inner Strength. Upgraded Warcry that stuns nearby
/// hostiles and grants the caster a temporary weapon-skill bonus.
///
/// # Arguments
///
/// * `gs` - Game state.
/// * `cn` - Caster character index.
pub fn skill_inner_strength(gs: &mut GameState, cn: usize) {
    if skill_on_cooldown(gs, cn, SK_INNER_STRENGTH as u16) {
        return;
    }
    if gs.characters[cn].a_end < 200 * 1000 {
        gs.do_character_log(cn, FontColor::Red, "You're too exhausted!\n");
        return;
    }
    gs.characters[cn].a_end -= 200 * 1000;

    let power = i32::from(gs.characters[cn].skill[SK_INNER_STRENGTH][5]);
    let buff_amount = ((power / 5) + 2).clamp(1, 50) as i8;
    let buff_duration = TICKS * 30;

    // Self weapon-skill buff item.
    let in_opt = God::create_item(gs, 1);
    if in_opt.is_none() {
        log::error!("god_create_item failed in skill_inner_strength");
        return;
    }
    let in_idx = in_opt.unwrap();
    {
        let item = &mut gs.items[in_idx];
        let mut name_bytes = [0u8; 40];
        let name = b"Inner Strength";
        let nlen = name.len().min(40);
        name_bytes[..nlen].copy_from_slice(&name[..nlen]);
        item.name = name_bytes;
        item.flags |= ItemFlags::IF_SPELL.bits();
        item.sprite[1] = 88;
        item.duration = buff_duration as u32;
        item.active = buff_duration as u32;
        item.temp = SK_INNER_STRENGTH as u16;
        item.power = power.max(1) as u32;
        item.skill[SK_WEAPON][1] = buff_amount;
    }
    if add_spell(gs, cn, in_idx) == 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            "Magical interference neutralised your shout.\n",
        );
        return;
    }

    // AoE stun (Warcry behaviour) over the surrounding tiles.
    let xf = std::cmp::max(1, i32::from(gs.characters[cn].x) - 10);
    let yf = std::cmp::max(1, i32::from(gs.characters[cn].y) - 10);
    let xt = std::cmp::min(
        core::constants::SERVER_MAPX - 1,
        i32::from(gs.characters[cn].x) + 10,
    );
    let yt = std::cmp::min(
        core::constants::SERVER_MAPY - 1,
        i32::from(gs.characters[cn].y) + 10,
    );

    let mut hit = 0;
    let mut miss = 0;
    for x in xf..xt {
        for y in yf..yt {
            let m = (x + y * core::constants::SERVER_MAPX) as usize;
            let co = gs.map[m].ch as usize;
            if co == 0 || co == cn {
                continue;
            }
            if warcry(gs, cn, co, power) {
                gs.remember_pvp(cn, co);
                hit += 1;
            } else {
                miss += 1;
            }
        }
    }

    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!(
            "Inner strength steadies you; {} of {} foes are shaken.\n",
            hit,
            hit + miss
        ),
    );
    chlog!(cn, "Cast Inner Strength ({} hits)", hit);

    add_skill_cooldown(
        gs,
        cn,
        TICKS * 30,
        SK_INNER_STRENGTH as u16,
        b"Inner Strength Cooldown",
    );
}

/// Dispatches direct skill use to the matching skill handler.
///
/// # Arguments
///
/// * `gs` - Active game state used for skill lookup and handler execution.
/// * `cn` - Character index using the skill.
/// * `nr` - Skill number to dispatch.
///
/// # Panics
///
/// * Panics if `cn` is invalid or `nr` cannot be used as a skill-table index.
pub fn skill_driver(gs: &mut GameState, cn: usize, nr: i32) {
    // Check whether the character can use this skill/spell
    if gs.characters[cn].skill[nr as usize][0] == 0 {
        gs.do_character_log(cn, FontColor::Green, "You cannot use this skill/spell.\n");
        return;
    }

    match nr {
        x if x == SK_LIGHT as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_light(gs, cn);
            }
        }
        x if x == SK_PROTECT as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_protect(gs, cn);
            }
        }
        x if x == SK_ENHANCE as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_enhance(gs, cn);
            }
        }
        x if x == SK_BLESS as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_bless(gs, cn);
            }
        }
        x if x == SK_CURSE as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_curse(gs, cn);
            }
        }
        x if x == SK_IDENT as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_identify(gs, cn);
            }
        }
        x if x == SK_BLAST as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_blast(gs, cn);
            }
        }
        x if x == SK_REPAIR as i32 => skill_repair(gs, cn),
        x if x == SK_LOCK as i32 => gs.do_character_log(
            cn,
            FontColor::Green,
            "You cannot use this skill directly. Hold a lock-pick under your mouse cursor and click on the door.\n",
        ),
        x if x == SK_RECALL as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_recall(gs, cn);
            }
        }
        x if x == SK_STUN as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_stun(gs, cn);
            }
        }
        x if x == SK_DISPEL as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_dispel(gs, cn);
            }
        }
        x if x == SK_WIMPY as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_wimp(gs, cn);
            }
        }
        x if x == SK_HEAL as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_heal(gs, cn);
            }
        }
        x if x == SK_GHOST as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_ghost(gs, cn);
            }
        }
        x if x == SK_MSHIELD as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_mshield(gs, cn);
            }
        }
        x if x == SK_IMMUN as i32 => gs.do_character_log(
            cn,
            FontColor::Green,
            "You use this skill automatically when someone casts evil spells on you.\n",
        ),
        x if x == SK_REGEN as i32 || x == SK_REST as i32 || x == SK_MEDIT as i32 => {
            gs.do_character_log(
                cn,
                FontColor::Green,
                "You use this skill automatically when you stand still.\n",
            );
        }
        x if x == SK_WEAPON as i32
            || x == SK_DAGGER as i32
            || x == SK_SWORD as i32
            || x == SK_AXE as i32
            || x == SK_STAFF as i32
            || x == SK_TWOHAND as i32
            || x == SK_SURROUND as i32 =>
        {
            gs.do_character_log(
                cn,
                FontColor::Green,
                "You use this skill automatically when you fight.\n",
            );
        }
        x if x == SK_CONCEN as i32 => gs.do_character_log(
            cn,
            FontColor::Green,
            "You use this skill automatically when you cast spells.\n",
        ),
        x if x == SK_WARCRY as i32 => skill_warcry(gs, cn),
         x if x == SK_PARASITE as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_parasite(gs, cn);
            }
        }
        x if x == SK_DISTRACT as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_distract(gs, cn);
            }
        }
        x if x == SK_DELIVER_DEATH as i32 => skill_deliver_death(gs, cn),
        x if x == SK_DISARM as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_disarm(gs, cn);
            }
        }
        x if x == SK_CONTAGION as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_contagion(gs, cn);
            }
        }
        x if x == SK_BLADE_DANCE as i32 => skill_blade_dance(gs, cn),
        x if x == SK_RAINS_OF_RENEWAL as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_rains_of_renewal(gs, cn);
            }
        }
        x if x == SK_GASH as i32 => skill_gash(gs, cn),
        x if x == SK_SUNS_BLESSING as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_suns_blessing(gs, cn);
            }
        }
        x if x == SK_SEEING_RED as i32 => skill_seeing_red(gs, cn),
        x if x == SK_THUNDEROUS_FURY as i32 => skill_thunderous_fury(gs, cn),
        x if x == SK_INNER_STRENGTH as i32 => skill_inner_strength(gs, cn),
        _ => {
            gs.do_character_log(cn, FontColor::Green, "You cannot use this skill/spell.\n");
        }
    }
}
