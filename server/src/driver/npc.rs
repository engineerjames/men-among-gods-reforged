use crate::core;
use crate::driver;
use crate::effect::EffectManager;
use crate::game_state::GameState;
use crate::god::God;
use crate::helpers;
use crate::player;
use crate::populate;
use core::constants::*;
use core::skills;
use core::string_operations::c_string_to_str;
use core::traits;
use core::types::Character;

// Helper functions

/// Returns the X offset for a given frustration value.
///
/// # Arguments
///
/// * `f` - Frustration value
///
/// # Returns
///
/// The X offset as an i32.
pub fn get_frust_x_off(f: i32) -> i32 {
    match f % 5 {
        0 => 0,
        1 => 1,
        2 => -1,
        3 => 2,
        4 => -2,
        _ => 0,
    }
}

/// Returns the Y offset for a given frustration value.
///
/// # Arguments
///
/// * `f` - Frustration value
///
/// # Returns
///
/// The Y offset as an i32.
pub fn get_frust_y_off(f: i32) -> i32 {
    match (f / 5) % 5 {
        0 => 0,
        1 => 1,
        2 => -1,
        3 => 2,
        4 => -2,
        _ => 0,
    }
}

/// Calculates the maximum of the absolute X or Y distance between two characters.
///
/// # Arguments
///
/// * `cn` - First character number (index)
/// * `co` - Second character number (index)
///
/// # Returns
///
/// The maximum of the absolute X or Y distance as i32.
pub fn npc_dist(cn: &Character, co: &Character) -> i32 {
    std::cmp::max((cn.x - co.x).abs(), (cn.y - co.y).abs()) as i32
}

// ****************************************************
// NPC Message Handling and AI Functions
// ****************************************************

/// Adds an enemy to the NPC's enemy list if conditions are met.
///
/// # Arguments
///
/// * `cn` - NPC character number
/// * `co` - Enemy character number
/// * `always` - If true, always add as enemy regardless of some conditions
///
/// # Returns
///
/// `true` if the enemy was added, `false` otherwise.
/// Pure eligibility check: whether an NPC should consider `co` as a potential enemy
/// based on group membership and relative power.
pub fn npc_should_consider_enemy(cn: &Character, co: &Character, always: bool) -> bool {
    // Same group -- never fight
    if cn.data[42] == co.data[42] {
        return false;
    }
    // Group 1 mobs shall not attack ghost companions
    if !always && cn.data[42] == 1 && (co.data[42] & 0x10000) != 0 {
        return false;
    }
    // Too weak relative to the target
    if !always && (cn.points_tot + 500) * 25 < co.points_tot {
        return false;
    }
    true
}

pub fn npc_add_enemy(gs: &mut GameState, cn: usize, co: usize, always: bool) -> bool {
    if !npc_should_consider_enemy(&gs.characters[cn], &gs.characters[co], always) {
        return false;
    }

    let ticker = gs.globals.ticker;
    gs.characters[cn].data[76] =
        gs.characters[co].x as i32 + gs.characters[co].y as i32 * SERVER_MAPX;
    gs.characters[cn].data[77] = ticker;

    let cc = gs.characters[cn].attack_cn;
    let d1 = if cc > 0 && usize::from(cc) < MAXCHARS {
        npc_dist(&gs.characters[cn], &gs.characters[cc as usize])
    } else {
        i32::MAX
    };
    let d2 = npc_dist(&gs.characters[cn], &gs.characters[co]);

    let flags = gs.globals.flags;
    if gs.characters[cn].attack_cn == 0
        || (d1 > d2 && (flags & 0x04) != 0)
        || (d1 == d2
            && (cc == 0 || gs.characters[cc as usize].attack_cn != cn as u16)
            && gs.characters[co].attack_cn == cn as u16)
    {
        gs.characters[cn].attack_cn = co as u16;
        gs.characters[cn].goto_x = 0;
        gs.characters[cn].data[58] = 2;
    }

    let idx = co as i32 | (helpers::char_id(&gs.characters[co]) << 16);

    // Check if already in enemy list
    for n in 80..92 {
        if gs.characters[cn].data[n] == idx {
            return false;
        }
    }

    // Shift enemy list and add new enemy
    for n in (81..92).rev() {
        gs.characters[cn].data[n] = gs.characters[cn].data[n - 1];
    }
    gs.characters[cn].data[80] = idx;

    true
}

pub fn npc_is_enemy(cn: &Character, co: &Character, co_idx: usize) -> bool {
    let idx = co_idx as i32 | (helpers::char_id(co) << 16);

    for n in 80..92 {
        if cn.data[n] == idx {
            return true;
        }
    }
    false
}

pub fn npc_list_enemies(gs: &mut GameState, npc: usize, cn: usize) -> bool {
    let npc_name = c_string_to_str(&gs.characters[npc].name).to_string();
    let mut enemies = Vec::new();

    for n in 80..92 {
        let cv = (gs.characters[npc].data[n] & 0xFFFF) as usize;
        if cv > 0 && cv < MAXCHARS {
            enemies.push(c_string_to_str(&gs.characters[cv].name).to_string());
        }
    }

    gs.do_character_log(
        cn,
        core::types::FontColor::Green,
        &format!("Enemies of {}:", npc_name),
    );

    if enemies.is_empty() {
        gs.do_character_log(cn, core::types::FontColor::Green, "-none-");
        false
    } else {
        for enemy_name in enemies {
            gs.do_character_log(
                cn,
                core::types::FontColor::Green,
                &format!("  {}", enemy_name),
            );
        }
        true
    }
}

pub fn npc_remove_enemy(gs: &mut GameState, npc: usize, enemy: usize) -> bool {
    let mut found = false;

    for n in 80..92 {
        if (gs.characters[npc].data[n] & 0xFFFF) as usize == enemy {
            found = true;
        }
        if found {
            if n < 91 {
                gs.characters[npc].data[n] = gs.characters[npc].data[n + 1];
            } else {
                gs.characters[npc].data[n] = 0;
            }
        }
    }

    found
}

pub fn npc_saytext_n(gs: &mut GameState, npc: usize, n: usize, name: Option<&str>) {
    let ch_npc = &gs.characters[npc];

    if (ch_npc.flags & CharacterFlags::ShutUp.bits()) != 0 {
        return;
    }

    if n >= ch_npc.text.len() {
        return;
    }

    let base_text = c_string_to_str(&ch_npc.text[n]);
    if base_text.is_empty() {
        return;
    }

    let text = if let Some(name_str) = name {
        base_text.replace("%s", name_str)
    } else {
        base_text.to_string()
    };

    let temp = ch_npc.temp;
    let talkative = ch_npc.data[71]; // CHD_TALKATIVE

    if temp == CT_COMPANION as u16 {
        if talkative == -10 {
            gs.do_sayx(npc, &text);
        }
    } else {
        gs.do_sayx(npc, &text);
    }
}

pub fn npc_gotattack(gs: &mut GameState, cn: usize, co: usize, _dam: i32) -> bool {
    gs.characters[cn].data[92] = TICKS * 60;

    let ticker = gs.globals.ticker;

    // Special handling for high alignment NPCs being attacked by players
    if co > 0
        && co < MAXCHARS
        && (gs.characters[co].flags & CharacterFlags::Player.bits()) != 0
        && gs.characters[cn].alignment == 10000
        && (gs.characters[cn].get_name() != "Peacekeeper"
            || gs.characters[cn].a_hp < (gs.characters[cn].hp[5] * 500) as i32)
        && gs.characters[cn].data[70] < ticker
    {
        gs.do_sayx(cn, "Skua! Protect the innocent! Send me a Peacekeeper!");
        EffectManager::fx_add_effect(
            gs,
            6,
            0,
            gs.characters[cn].x as i32,
            gs.characters[cn].y as i32,
            0,
        );
        gs.characters[cn].data[70] = ticker + (TICKS * 60);

        let cc = God::create_char(gs, 80, true);
        if cc.is_some() && cc.unwrap() > 0 && cc.unwrap() < MAXCHARS as i32 {
            let cc = cc.unwrap() as usize;
            gs.characters[cc].temp = CT_COMPANION as u16;
            gs.characters[cc].data[42] = 65536 + cn as i32;
            gs.characters[cc].data[59] = 65536 + cn as i32;
            gs.characters[cc].data[24] = 0;
            gs.characters[cc].data[36] = 0;
            gs.characters[cc].data[43] = 0;
            gs.characters[cc].data[80] = co as i32 | (helpers::char_id(&gs.characters[co]) << 16);
            gs.characters[cc].data[63] = cn as i32;
            gs.characters[cc].data[64] = ticker + 120 * TICKS;
            gs.characters[cc].data[70] = ticker + (TICKS * 60);

            gs.characters[cc].set_name("Shadow of Peace");
            gs.characters[cc].set_reference("Shadow of Peace");
            gs.characters[cc].set_description("You see a Shadow of Peace.");

            if !God::drop_char_fuzzy(
                gs,
                cc,
                gs.characters[co].x as usize,
                gs.characters[co].y as usize,
            ) {
                God::destroy_items(gs, cc);
                gs.characters[cc].used = 0;
            }
        }
    }

    // Help request for good aligned characters
    if gs.characters[cn].alignment > 1000
        && gs.characters[cn].data[70] < ticker
        && gs.characters[cn].a_mana < gs.characters[cn].mana[5] as i32 * 333
    {
        gs.do_sayx(cn, "Skua! Help me!");
        gs.characters[cn].data[70] = ticker + (TICKS * 60 * 2);
        gs.characters[cn].a_mana = gs.characters[cn].mana[5] as i32 * 1000;
        EffectManager::fx_add_effect(
            gs,
            6,
            0,
            gs.characters[cn].x as i32,
            gs.characters[cn].y as i32,
            0,
        );
    }

    // Shout for help
    if gs.characters[cn].data[52] != 0
        && gs.characters[cn].a_hp < gs.characters[cn].hp[5] as i32 * 666
    {
        if gs.characters[cn].data[55] + (TICKS * 60) < ticker {
            gs.characters[cn].data[54] = 0;
            gs.characters[cn].data[55] = ticker;
            if co < MAXCHARS {
                let co_name = gs.characters[co].get_name().to_string();
                npc_saytext_n(gs, cn, 4, Some(&co_name));
            }
            gs.do_npc_shout(
                cn,
                NT_SHOUT as i32,
                cn as i32,
                gs.characters[cn].data[52],
                gs.characters[cn].x as i32,
                gs.characters[cn].y as i32,
            );
        }
    }

    // Can't see attacker - panic mode
    let character_can_see = gs.do_char_can_see(cn, co);
    if co >= MAXCHARS || character_can_see == 0 {
        gs.characters[cn].data[78] = ticker + (TICKS * 30);
        return true;
    }

    // Fight back
    if co < MAXCHARS {
        let co_name = gs.characters[co].get_name().to_string();
        let cn_name = gs.characters[cn].get_name().to_string();
        if npc_add_enemy(gs, cn, co, true) {
            npc_saytext_n(gs, cn, 1, Some(&co_name));
            log::info!(
                "NPC {} ({}) added {} ({}) to enemy list for attacking",
                cn,
                cn_name,
                co,
                co_name
            );
        }
    }

    true
}

pub fn npc_gothit(gs: &mut GameState, cn: usize, co: usize, dam: i32) -> bool {
    npc_gotattack(gs, cn, co, dam)
}

pub fn npc_gotmiss(gs: &mut GameState, cn: usize, co: usize) -> bool {
    npc_gotattack(gs, cn, co, 0)
}

pub fn npc_didhit(_cn: usize, _co: usize, _dam: i32) -> bool {
    false
}

pub fn npc_didmiss(_cn: usize, _co: usize) -> bool {
    false
}

pub fn npc_killed(gs: &mut GameState, cn: usize, cc: usize, co: usize) -> bool {
    if gs.characters[cn].attack_cn == co as u16 {
        gs.characters[cn].attack_cn = 0;
    }
    gs.characters[cn].data[76] = 0;
    gs.characters[cn].data[77] = 0;
    gs.characters[cn].data[78] = 0;

    let idx = co as i32 | (helpers::char_id(&gs.characters[co]) << 16);

    for n in 80..92 {
        if gs.characters[cn].data[n] == idx {
            if cn == cc && co < MAXCHARS {
                let co_name = gs.characters[co].get_name().to_string();
                npc_saytext_n(gs, cn, 0, Some(&co_name));
                gs.characters[cn].data[n] = 0;
            } else {
                gs.characters[cn].data[n] = 0;
            }
            return true;
        }
    }

    false
}

pub fn npc_didkill(gs: &mut GameState, cn: usize, co: usize) -> bool {
    npc_killed(gs, cn, cn, co)
}

pub fn npc_gotexp(_cn: usize, _amount: i32) -> bool {
    false
}

pub fn npc_seekill(gs: &mut GameState, cn: usize, cc: usize, co: usize) -> bool {
    npc_killed(gs, cn, cc, co)
}

pub fn npc_seeattack(gs: &mut GameState, cn: usize, cc: usize, co: usize) -> bool {
    gs.characters[cn].data[92] = TICKS * 60;

    let cn_can_see_co = gs.do_char_can_see(cn, co);
    let cn_can_see_cc = gs.do_char_can_see(cn, cc);

    if cn_can_see_co == 0 || cn_can_see_cc == 0 {
        return true; // Processed - can't see participants
    }

    // Prevent fight mode logic
    if gs.characters[cn].data[24] != 0 {
        let diff = (gs.characters[cc].alignment - 50) - gs.characters[co].alignment;
        let (ret, c2, c3) = if diff <= 0 {
            if gs.characters[cn].data[24] > 0 {
                (npc_add_enemy(gs, cn, cc, true), cc, co)
            } else {
                (npc_add_enemy(gs, cn, co, true), co, cc)
            }
        } else {
            if gs.characters[cn].data[24] > 0 {
                (npc_add_enemy(gs, cn, co, true), co, cc)
            } else {
                (npc_add_enemy(gs, cn, cc, true), cc, co)
            }
        };

        if ret {
            let c2_name = gs.characters[c2].get_name().to_string();
            let c3_name = gs.characters[c3].get_name().to_string();
            npc_saytext_n(gs, cn, 1, Some(&c2_name));
            log::info!(
                "NPC {} added {} to enemy list for attacking {}",
                cn,
                c2_name,
                c3_name
            );
        }
        return true;
    }

    // Protect character by template
    if gs.characters[cn].data[31] != 0 {
        if gs.characters[co].temp == gs.characters[cn].data[31] as u16 {
            if npc_add_enemy(gs, cn, cc, true) {
                let cc_name = gs.characters[cc].get_name().to_string();
                let co_name = gs.characters[co].get_name().to_string();
                npc_saytext_n(gs, cn, 1, Some(&cc_name));
                log::info!(
                    "NPC {} added {} to enemy list for attacking {} (protect char)",
                    cn,
                    cc_name,
                    co_name
                );
            }
            if gs.characters[cn].data[65] == 0 {
                gs.characters[cn].data[65] = co as i32;
            }
        }
    }

    // Protect character by number (CHD_MASTER)
    if gs.characters[cn].data[63] != 0 {
        if co == gs.characters[cn].data[63] as usize {
            if npc_add_enemy(gs, cn, cc, true) {
                let cc_name = gs.characters[cc].get_name().to_string();
                let co_name = gs.characters[co].get_name().to_string();
                npc_saytext_n(gs, cn, 1, Some(&cc_name));
                log::info!(
                    "NPC {} added {} to enemy list for attacking {} (protect char)",
                    cn,
                    cc_name,
                    co_name
                );
            }
            if gs.characters[cn].data[65] == 0 {
                gs.characters[cn].data[65] = co as i32;
            }
        }
        if cc == gs.characters[cn].data[63] as usize {
            if npc_add_enemy(gs, cn, co, true) {
                let co_name = gs.characters[co].get_name().to_string();
                let cc_name = gs.characters[cc].get_name().to_string();
                npc_saytext_n(gs, cn, 1, Some(&co_name));
                log::info!(
                    "NPC {} added {} to enemy list for being attacked by {} (protect char)",
                    cn,
                    co_name,
                    cc_name
                );
            }
            if gs.characters[cn].data[65] == 0 {
                gs.characters[cn].data[65] = cc as i32;
            }
        }
    }

    // Protect by group (CHD_HELPGROUP)
    if gs.characters[cn].data[59] != 0 {
        if gs.characters[cn].data[59] == gs.characters[co].data[42] {
            if npc_add_enemy(gs, cn, cc, true) {
                let cc_name = gs.characters[cc].get_name().to_string();
                let co_name = gs.characters[co].get_name().to_string();
                npc_saytext_n(gs, cn, 1, Some(&cc_name));
                log::info!(
                    "NPC {} added {} to enemy list for attacking {} (protect group)",
                    cn,
                    cc_name,
                    co_name
                );
            }
            if gs.characters[cn].data[65] == 0 {
                gs.characters[cn].data[65] = co as i32;
            }
        }
        if gs.characters[cn].data[59] == gs.characters[cc].data[42] {
            if npc_add_enemy(gs, cn, co, true) {
                let co_name = gs.characters[co].get_name().to_string();
                let cc_name = gs.characters[cc].get_name().to_string();
                npc_saytext_n(gs, cn, 1, Some(&co_name));
                log::info!(
                    "NPC {} added {} to enemy list for being attacked by {} (protect group)",
                    cn,
                    co_name,
                    cc_name
                );
            }
            if gs.characters[cn].data[65] == 0 {
                gs.characters[cn].data[65] = cc as i32;
            }
        }
    }

    // If one of the participants is my companion and its master is me, register the helper index
    if gs.characters[co].temp == core::constants::CT_COMPANION as u16
        && gs.characters[co].data[63] == cn as i32
    {
        if gs.characters[cn].data[65] == 0 {
            gs.characters[cn].data[65] = co as i32;
        }
    }

    if gs.characters[cc].temp == core::constants::CT_COMPANION as u16
        && gs.characters[cc].data[63] == cn as i32
    {
        if gs.characters[cn].data[65] == 0 {
            gs.characters[cn].data[65] = cc as i32;
        }
    }

    false
}

pub fn npc_seehit(gs: &mut GameState, cn: usize, cc: usize, co: usize) -> bool {
    if npc_seeattack(gs, cn, cc, co) {
        return true;
    }
    if npc_see(gs, cn, cc) {
        return true;
    }
    if npc_see(gs, cn, co) {
        return true;
    }
    false
}

pub fn npc_seemiss(gs: &mut GameState, cn: usize, cc: usize, co: usize) -> bool {
    if npc_seeattack(gs, cn, cc, co) {
        return true;
    }
    if npc_see(gs, cn, cc) {
        return true;
    }
    if npc_see(gs, cn, co) {
        return true;
    }
    false
}

pub fn npc_give(gs: &mut GameState, cn: usize, co: usize, in_item: usize, money: i32) -> bool {
    // If giver is a player/usurp, set active timer; otherwise ensure group active
    if (gs.characters[co].flags & (CharacterFlags::Player.bits() | CharacterFlags::Usurp.bits()))
        != 0
    {
        gs.characters[cn].data[92] = TICKS * 60;
    } else if !gs.characters[cn].group_active() {
        return false;
    }

    // Item given and matches what NPC wants
    if in_item != 0 && gs.items[in_item].temp as i32 == gs.characters[cn].data[49] {
        // Black candle special-case
        if gs.characters[cn].data[49] == 740 && gs.characters[cn].temp == 518 {
            gs.characters[co].data[43] += 1;
            // Remove item from NPC and destroy it
            God::take_from_char(gs, in_item, cn);
            gs.items[in_item].used = core::constants::USE_EMPTY;

            gs.do_sayx(
                cn,
                &format!(
                    "Ah, a black candle! Great work, {}! Now we will have peace for a while...",
                    gs.characters[co].get_name()
                ),
            );
            gs.do_area_log(
                cn,
                0,
                gs.characters[cn].x as i32,
                gs.characters[cn].y as i32,
                core::types::FontColor::Yellow,
                &format!(
                    "The Cityguard is impressed by {}'s deed.\n",
                    gs.characters[co].get_name()
                ),
            );
        } else {
            // Thank you message
            let ref_name = c_string_to_str(&gs.items[in_item].reference).to_string();
            gs.do_sayx(
                cn,
                &format!(
                    "Thank you {}. That's the {} I wanted.",
                    gs.characters[co].get_name(),
                    ref_name
                ),
            );
        }

        // Quest-requested items: teach skill / give exp
        let nr = gs.characters[cn].data[50];
        if nr != 0 {
            let mut skill_nr = nr as usize;
            let co_kindred = gs.characters[co].kindred as u32;

            if skill_nr == skills::SK_STUN
                && (co_kindred & (traits::KIN_TEMPLAR | traits::KIN_ARCHTEMPLAR)) != 0
            {
                skill_nr = skills::SK_IMMUN;
            }
            if skill_nr == skills::SK_CURSE
                && (co_kindred & (traits::KIN_TEMPLAR | traits::KIN_ARCHTEMPLAR)) != 0
            {
                skill_nr = skills::SK_SURROUND;
            }
            if skill_nr == skills::SK_STUN
                && (co_kindred & traits::KIN_SEYAN_DU) != 0
                && gs.characters[co].skill[skills::SK_STUN][0] != 0
            {
                skill_nr = skills::SK_IMMUN;
            }
            if skill_nr == skills::SK_CURSE
                && (co_kindred & traits::KIN_SEYAN_DU) != 0
                && gs.characters[co].skill[skills::SK_CURSE][0] != 0
            {
                skill_nr = skills::SK_SURROUND;
            }

            if skill_nr == skills::SK_STUN && (co_kindred & traits::KIN_SEYAN_DU) != 0 {
                gs.do_sayx(
                    cn,
                    &format!(
                        "Bring me the item again to learn Immunity, {}!",
                        gs.characters[co].get_name()
                    ),
                );
            }
            if skill_nr == skills::SK_CURSE && (co_kindred & traits::KIN_SEYAN_DU) != 0 {
                gs.do_sayx(
                    cn,
                    &format!(
                        "Bring me the item again to learn Surround Hit, {}!",
                        gs.characters[co].get_name()
                    ),
                );
            }

            let skill_name = skills::get_skill_name(skill_nr);
            gs.do_sayx(cn, &format!("Now I'll teach you {}.", skill_name));

            if gs.characters[co].skill[skill_nr][0] != 0 {
                gs.do_sayx(
                    cn,
                    &format!(
                        "But you already know {}, {}!",
                        skill_name,
                        gs.characters[co].get_name()
                    ),
                );
                // give item back to player
                God::take_from_char(gs, in_item, cn);
                God::give_character_item(gs, co, in_item);
                gs.do_character_log(
                    co,
                    core::types::FontColor::Green,
                    &format!(
                        "{} did not accept the {}.\n",
                        gs.characters[cn].get_name(),
                        gs.items[in_item].get_name().to_string()
                    ),
                );
            } else {
                // teach skill
                gs.characters[co].skill[skill_nr][0] = 1;
                gs.do_character_log(
                    co,
                    core::types::FontColor::Green,
                    &format!("You learned {}!\n", skill_name),
                );
                gs.characters[co].set_do_update_flags();

                let give_exp = gs.characters[cn].data[51];
                if give_exp != 0 {
                    gs.do_sayx(
                        cn,
                        &format!(
                            "Now I'll teach you a bit about life, the world and everything, {}.",
                            gs.characters[co].get_name()
                        ),
                    );
                    gs.do_give_exp(co, give_exp, 0, -1);
                }

                // take and destroy the offered item
                God::take_from_char(gs, in_item, cn);
                gs.items[in_item].used = core::constants::USE_EMPTY;
            }
        }

        // Return-gift
        let give_temp = gs.characters[cn].data[66];
        if give_temp != 0 {
            gs.do_sayx(
                cn,
                &format!(
                    "Here is your {} in exchange.",
                    c_string_to_str(&gs.item_templates[give_temp as usize].reference).to_string()
                ),
            );
            God::take_from_char(gs, in_item, cn);
            gs.items[in_item].used = core::constants::USE_EMPTY;
            if let Some(new_item) = God::create_item(gs, give_temp as usize) {
                God::give_character_item(gs, co, new_item);
            }
        }

        // Riddle-giver special
        let ar = gs.characters[cn].data[72];
        if gs.characters[co].is_player()
            && (core::constants::RIDDLE_MIN_AREA..=core::constants::RIDDLE_MAX_AREA).contains(&ar)
        {
            let idx = (ar - core::constants::RIDDLE_MIN_AREA) as usize;
            // check Lab9 guesser
            let still = crate::lab9::lab9_get_guesser(gs, idx);
            if still != 0 && still as usize != co {
                gs.do_sayx(
                    cn,
                    &format!(
                        "I'm still riddling {}; please come back later!\n",
                        gs.characters[still as usize].get_name().to_string()
                    ),
                );
                God::take_from_char(gs, in_item, cn);
                God::give_character_item(gs, co, in_item);
                return false;
            }

            // Destroy gift from player and pose riddle
            God::take_from_char(gs, in_item, co);
            gs.items[in_item].used = core::constants::USE_EMPTY;
            crate::lab9::lab9_pose_riddle(gs, cn, co);
        }

        return false;
    } else if in_item == 0 && money != 0 {
        // NPC doesn't take money
        gs.do_sayx(cn, "I don't take money from you!");
        gs.characters[co].gold += money;
        gs.characters[cn].gold -= money;
    } else {
        // Not accepted - return item to giver
        God::take_from_char(gs, in_item, cn);
        God::give_character_item(gs, co, in_item);
        gs.do_character_log(
            co,
            core::types::FontColor::Green,
            &format!(
                "{} did not accept the {}.\n",
                gs.characters[cn].get_name(),
                gs.items[in_item].get_name().to_string()
            ),
        );
    }

    false
}

pub fn npc_died(gs: &mut GameState, cn: usize, co: usize) -> bool {
    // Mirror C++ behavior: chance = characters[cn].data[48]
    let chance = gs.characters[cn].data[48];
    if chance != 0 && co > 0 {
        // random 0..99 < chance
        let roll = helpers::random_mod_i32(100);
        if roll < chance {
            let co_name = if co < MAXCHARS {
                gs.characters[co].get_name().to_string()
            } else {
                String::new()
            };
            npc_saytext_n(
                gs,
                cn,
                3,
                if co_name.is_empty() {
                    None
                } else {
                    Some(&co_name)
                },
            );
        }
        return true;
    }
    false
}

pub fn npc_shout(gs: &mut GameState, cn: usize, co: usize, code: i32, x: i32, y: i32) -> bool {
    if gs.characters[cn].data[53] != 0 && gs.characters[cn].data[53] == code {
        gs.characters[cn].data[92] = TICKS * 60;
        gs.characters[cn].data[54] = x + y * SERVER_MAPX;
        gs.characters[cn].data[55] = gs.globals.ticker;

        let co_name = if co < MAXCHARS {
            gs.characters[co].get_name().to_string()
        } else {
            String::new()
        };

        npc_saytext_n(
            gs,
            cn,
            5,
            if co_name.is_empty() {
                None
            } else {
                Some(&co_name)
            },
        );

        // Cancel current actions
        gs.characters[cn].goto_x = 0;
        gs.characters[cn].misc_action = 0;

        return true;
    }
    false
}

pub fn npc_hitme(gs: &mut GameState, cn: usize, co: usize) -> bool {
    let cn_can_see_co = gs.do_char_can_see(cn, co);

    if cn_can_see_co == 0 {
        return true;
    }

    let _data_26 = gs.characters[cn].data[26];

    // TODO: Implement trap logic
    false
}

pub fn npc_msg(
    gs: &mut GameState,
    cn: usize,
    msg_type: i32,
    dat1: i32,
    dat2: i32,
    dat3: i32,
    dat4: i32,
) -> bool {
    // Check for special driver
    let special_driver = gs.characters[cn].data[25];

    if special_driver != 0 {
        return match special_driver {
            1 => driver::npc_stunrun_msg(gs, cn, msg_type as u8, dat1, dat2, dat3, dat4),
            2 => driver::npc_cityattack_msg(gs, cn, msg_type, dat1, dat2, dat3, dat4),
            3 => driver::npc_malte_msg(gs, cn, msg_type, dat1, dat2, dat3, dat4),
            _ => {
                log::error!("Unknown special driver {} for {}", special_driver, cn);
                false
            }
        };
    }

    match msg_type {
        x if x == NT_GOTHIT as i32 => npc_gothit(gs, cn, dat1 as usize, dat2),
        x if x == NT_GOTMISS as i32 => npc_gotmiss(gs, cn, dat1 as usize),
        x if x == NT_DIDHIT as i32 => npc_didhit(cn, dat1 as usize, dat2),
        x if x == NT_DIDMISS as i32 => npc_didmiss(cn, dat1 as usize),
        x if x == NT_DIDKILL as i32 => npc_didkill(gs, cn, dat1 as usize),
        x if x == NT_GOTEXP as i32 => npc_gotexp(cn, dat1),
        x if x == NT_SEEKILL as i32 => npc_seekill(gs, cn, dat1 as usize, dat2 as usize),
        x if x == NT_SEEHIT as i32 => npc_seehit(gs, cn, dat1 as usize, dat2 as usize),
        x if x == NT_SEEMISS as i32 => npc_seemiss(gs, cn, dat1 as usize, dat2 as usize),
        x if x == NT_GIVE as i32 => npc_give(gs, cn, dat1 as usize, dat2 as usize, dat3),
        x if x == NT_SEE as i32 => npc_see(gs, cn, dat1 as usize),
        x if x == NT_DIED as i32 => npc_died(gs, cn, dat1 as usize),
        x if x == NT_SHOUT as i32 => npc_shout(gs, cn, dat1 as usize, dat2, dat3, dat4),
        x if x == NT_HITME as i32 => npc_hitme(gs, cn, dat1 as usize),
        _ => {
            log::error!("Unknown NPC message for {}: {}", cn, msg_type);
            false
        }
    }
}

// ****************************************************
// Spell and Combat Functions
// ****************************************************

pub fn get_spellcost(cn: &Character, spell: usize) -> i32 {
    (match spell {
        skills::SK_BLAST => cn.skill[skills::SK_BLAST][5] / 5,
        skills::SK_IDENT => 50,
        skills::SK_CURSE => 35,
        skills::SK_BLESS => 35,
        skills::SK_ENHANCE => 15,
        skills::SK_PROTECT => 15,
        skills::SK_LIGHT => 5,
        skills::SK_STUN => 20,
        skills::SK_HEAL => 25,
        skills::SK_GHOST => 45,
        skills::SK_MSHIELD => 25,
        skills::SK_RECALL => 15,
        _ => 255, // Originally was 9999 which is invalid for a u8
    }) as i32
}

pub fn spellflag(spell: usize) -> u32 {
    match spell {
        skills::SK_LIGHT => SP_LIGHT,
        skills::SK_PROTECT => SP_PROTECT,
        skills::SK_ENHANCE => SP_ENHANCE,
        skills::SK_BLESS => SP_BLESS,
        skills::SK_HEAL => SP_HEAL,
        skills::SK_CURSE => SP_CURSE,
        skills::SK_STUN => SP_STUN,
        skills::SK_DISPEL => SP_DISPEL,
        _ => 0,
    }
}

pub fn npc_check_target(gs: &GameState, x: usize, y: usize) -> bool {
    if x < 1 || x >= SERVER_MAPX as usize || y < 1 || y >= SERVER_MAPY as usize {
        return false;
    }

    let m = x + y * SERVER_MAPX as usize;

    let map_item = if gs.map[m].it == 0 {
        None
    } else {
        Some(gs.items[gs.map[m].it as usize])
    };

    if map_item.is_none() {
        return false;
    }

    let map_item = map_item.unwrap();
    if gs.map[m].flags & (core::constants::MF_MOVEBLOCK as u64 | core::constants::MF_NOMONST as u64)
        != 0
        || gs.map[m].ch != 0
        || gs.map[m].to_ch != 0
        || (map_item.flags & ItemFlags::IF_MOVEBLOCK.bits() != 0 && map_item.driver != 2)
    {
        return false;
    }

    true
}

pub fn npc_is_stunned(cn: &Character, items: &[core::types::Item]) -> bool {
    for n in 0..20 {
        let active_spell = cn.spell[n];
        if active_spell != 0 && items[active_spell as usize].temp == skills::SK_STUN as u16 {
            return true;
        }
    }

    false
}

pub fn npc_is_blessed(cn: &Character, items: &[core::types::Item]) -> bool {
    for n in 0..20 {
        let active_spell = cn.spell[n];
        if active_spell != 0 && items[active_spell as usize].temp == skills::SK_BLESS as u16 {
            return true;
        }
    }

    false
}

/// Pure pre-condition check: whether an NPC can consider casting `spell` on `co`
/// based on character flags, skill availability, and spell-specific feasibility.
pub fn npc_spell_preconditions_met(cn: &Character, co: &Character, spell: usize) -> bool {
    if cn.flags & CharacterFlags::NoMagic.bits() != 0 {
        return false;
    }
    if co.used != core::constants::USE_ACTIVE {
        return false;
    }
    if co.flags & CharacterFlags::Body.bits() != 0 {
        return false;
    }
    if cn.skill[spell][0] == 0 {
        return false;
    }
    if co.flags & CharacterFlags::Stoned.bits() != 0 {
        return false;
    }
    if spell == skills::SK_BLAST && (cn.skill[skills::SK_BLAST][5] as i16 - co.armor) < 10 {
        return false;
    }
    if spell == skills::SK_CURSE
        && 10 * cn.skill[skills::SK_CURSE][5] as i32
            / (std::cmp::max(1, co.skill[skills::SK_RESIST][5]) as i32)
            < 7
    {
        return false;
    }
    if spell == skills::SK_STUN
        && 10 * cn.skill[skills::SK_STUN][5] as i32
            / (std::cmp::max(1, co.skill[skills::SK_RESIST][5]) as i32)
            < 5
    {
        return false;
    }
    true
}

pub fn npc_try_spell(gs: &mut GameState, cn: usize, co: usize, spell: usize) -> bool {
    if !npc_spell_preconditions_met(&gs.characters[cn], &gs.characters[co], spell) {
        return false;
    }

    let mut should_return_false_early = false;
    for n in 0..20 {
        let item_index = gs.characters[cn].spell[n];
        if item_index == 0 {
            continue;
        }
        if gs.items[item_index as usize].temp as usize == skills::SK_BLAST {
            should_return_false_early = true;
            break;
        }
    }
    if should_return_false_early {
        return false;
    }

    let mana = gs.characters[cn].a_mana / 1000;

    // C++ logic: scan target's active spells; if we find the same spell with
    // sufficient power and still > 50% duration remaining, we do NOT cast it again.
    let mut found = false;
    for n in 0..20 {
        let item_index = gs.characters[co].spell[n];
        if item_index == 0 {
            continue;
        }

        let should_break = gs.items[item_index as usize].temp as usize == spell
            && gs.items[item_index as usize].power + 10
                >= spell_immunity(
                    gs.characters[cn].skill[spell][5] as i32,
                    gs.characters[co].skill[skills::SK_IMMUN][5] as i32,
                ) as u32
            && gs.items[item_index as usize].active > gs.items[item_index as usize].duration / 2;

        if should_break {
            found = true;
            break;
        }
    }

    // Match C++: only cast if such a spell was NOT found on the target.
    if !found {
        let tmp = spellflag(spell);

        if mana >= get_spellcost(&gs.characters[cn], spell)
            && gs.characters[co].data[96] as u32 & tmp == 0
        {
            gs.characters[cn].skill_nr = spell as u16;
            gs.characters[cn].skill_target1 = co as u16;
            gs.characters[co].data[96] |= tmp as i32;
            // Match C++ parameter semantics: effect[11].data[0]=target character id,
            // effect[11].data[1]=spellflag bitmask to clear later.
            EffectManager::fx_add_effect(gs, 11, 8, co as i32, tmp as i32, 0);
            return true;
        }
    }

    false
}

pub fn spell_immunity(power: i32, immunity: i32) -> i32 {
    let half_immunity = immunity / 2;
    if power <= half_immunity {
        return 1;
    }

    power - half_immunity
}

pub fn npc_can_spell(cn: &Character, co: &Character, spell: usize) -> bool {
    if cn.a_mana / 1000 < get_spellcost(cn, spell) {
        return false;
    }
    if cn.skill[spell][0] == 0 {
        return false;
    }
    if co.skill[spell][5] > cn.skill[spell][5] {
        return false;
    }
    true
}

pub fn npc_quaff_potion(gs: &mut GameState, cn: usize, itemp: i32, stemp: i32) -> bool {
    for n in 0..20 {
        let item_index = gs.characters[cn].spell[n];

        if item_index == 0 {
            continue;
        }

        if gs.items[item_index as usize].temp as i32 == stemp {
            return false;
        }
    }

    // Find potion and quaff it
    let mut should_quaff = false;
    let mut name = String::new();
    let mut item_index = 0usize;
    for n in 0..40 {
        let idx = gs.characters[cn].item[n];

        if idx == 0 {
            continue;
        }

        if gs.items[idx as usize].temp == itemp as u16 {
            should_quaff = true;
            name = gs.items[idx as usize].get_name().to_string();
            item_index = idx as usize;
            break;
        }
    }

    if !should_quaff {
        return false;
    }

    gs.do_area_log(
        cn,
        0,
        gs.characters[cn].x as i32,
        gs.characters[cn].y as i32,
        core::types::FontColor::Yellow,
        &format!("The {} uses a {}.\n", gs.characters[cn].get_name(), name),
    );

    driver::use_driver(gs, cn, item_index, true);

    true
}

pub fn die_companion(gs: &mut GameState, cn: usize) {
    EffectManager::fx_add_effect(
        gs,
        7,
        0,
        gs.characters[cn].x as i32,
        gs.characters[cn].y as i32,
        0,
    );
    God::destroy_items(gs, cn);
    gs.characters[cn].gold = 0;

    gs.do_character_killed(cn, 0, false);
}

// ****************************************************
// High Priority NPC Driver
// ****************************************************

pub fn npc_driver_high(gs: &mut GameState, cn: usize) -> bool {
    // Check for special driver
    let special_driver = gs.characters[cn].data[25];
    if special_driver != 0 {
        return match special_driver {
            1 => driver::npc_stunrun_high(gs, cn),
            2 => driver::npc_cityattack_high(gs, cn),
            3 => driver::npc_malte_high(gs, cn),
            _ => {
                log::error!("Unknown special driver {} for {}", special_driver, cn);
                false
            }
        };
    }

    let ticker = gs.globals.ticker;
    let _flags = gs.globals.flags;

    // reset panic mode if expired
    if gs.characters[cn].data[78] < ticker {
        gs.characters[cn].data[78] = 0;
    }

    // self destruct
    {
        let mut do_die = false;
        let d64 = gs.characters[cn].data[64];
        if d64 != 0 {
            if d64 < (TICKS * 60 * 15) {
                gs.characters[cn].data[64] = d64 + ticker;
            }
            if gs.characters[cn].data[64] < ticker {
                // NPC should self-destruct
                do_die = true;
            }
        }
        if do_die {
            gs.do_sayx(cn, "Free!");
            God::destroy_items(gs, cn);
            player::plr_map_remove(gs, cn);
            gs.characters[cn].used = USE_EMPTY;
            npc_remove_enemy(gs, cn, 0);
            return true;
        }
    }

    // Count down master-no-see timer for player ghost companions
    {
        let temp = gs.characters[cn].temp;
        let data64 = gs.characters[cn].data[64];
        if temp == CT_COMPANION as u16 && data64 == 0 {
            let co = gs.characters[cn].data[CHD_MASTER];
            let master_ok = {
                let co_usize = co as usize;
                if co_usize >= gs.characters.len() {
                    false
                } else {
                    gs.characters[co_usize].used != USE_EMPTY
                        && gs.characters[co_usize].data[64] == cn as i32
                }
            };
            if !master_ok {
                log::warn!("{} killed for bad master({})", cn, co);
                die_companion(gs, cn);
                return true;
            }

            let should_self_destruct = gs.globals.ticker > gs.characters[cn].data[98];
            if should_self_destruct {
                let co = gs.characters[cn].data[CHD_MASTER] as usize;
                if co < gs.characters.len() {
                    gs.characters[co].luck -= 1;
                }
                log::info!("{} Self-destructed because of neglect by master", cn);
                die_companion(gs, cn);
                return true;
            }
        }
    }

    // Count down riddle timeout for riddle givers
    {
        let area_of_knowledge = gs.characters[cn].data[72];
        if (core::constants::RIDDLE_MIN_AREA..=core::constants::RIDDLE_MAX_AREA)
            .contains(&area_of_knowledge)
        {
            crate::lab9::lab9_tick_riddle_timeout(gs, cn);
        }
    }

    // heal us if we're hurt
    {
        let a_hp = gs.characters[cn].a_hp;
        let hp5 = gs.characters[cn].hp[5];
        if a_hp < hp5 as i32 * 600 {
            if npc_try_spell(gs, cn, cn, skills::SK_HEAL) {
                return true;
            }
        }
    }

    // donate/destroy citem if that's our job
    {
        let citem = gs.characters[cn].citem as usize;
        let donate_dest = gs.characters[cn].data[47];
        if citem != 0 && donate_dest != 0 {
            let it = &gs.items[citem];
            let take_action = it.damage_state != 0
                || (it.flags & ItemFlags::IF_SHOPDESTROY.bits() != 0)
                || (it.flags & ItemFlags::IF_DONATE.bits() == 0);
            if take_action {
                gs.items[citem].used = USE_EMPTY;
                gs.characters[cn].citem = 0;
            } else {
                // reset ages/damage
                gs.items[citem].current_age[0] = 0;
                gs.items[citem].current_age[1] = 0;
                gs.items[citem].current_damage = 0;
                God::donate_item(gs, citem, donate_dest);
                gs.characters[cn].citem = 0;
            }
        }
    }

    // donate item[39]
    {
        let it39 = gs.characters[cn].item[39] as usize;
        let donate_dest = gs.characters[cn].data[47];
        if it39 != 0 && donate_dest != 0 {
            let it = &gs.items[it39];
            let take_action = it.damage_state != 0
                || (it.flags & ItemFlags::IF_SHOPDESTROY.bits() != 0)
                || (it.flags & ItemFlags::IF_DONATE.bits() == 0);
            if take_action {
                gs.items[it39].used = USE_EMPTY;
                gs.characters[cn].citem = 0;
            } else {
                gs.items[it39].current_age[0] = 0;
                gs.items[it39].current_age[1] = 0;
                gs.items[it39].current_damage = 0;
                God::donate_item(gs, it39, donate_dest);
                gs.characters[cn].item[39] = 0;
            }
        }
    }

    // generic spell management
    {
        let a_mana = gs.characters[cn].a_mana;
        let med_skill = gs.characters[cn].skill[skills::SK_MEDIT][0];
        if a_mana > (gs.characters[cn].mana[5] as i32) * 850 && med_skill != 0 {
            if a_mana > 75000 && npc_try_spell(gs, cn, cn, skills::SK_BLESS) {
                return true;
            }
            if npc_try_spell(gs, cn, cn, skills::SK_PROTECT) {
                return true;
            }
            if npc_try_spell(gs, cn, cn, skills::SK_MSHIELD) {
                return true;
            }
            if npc_try_spell(gs, cn, cn, skills::SK_ENHANCE) {
                return true;
            }
            if npc_try_spell(gs, cn, cn, skills::SK_BLESS) {
                return true;
            }
        }
    }

    // generic endurance management (mode switching)
    {
        let data58 = gs.characters[cn].data[58];
        let a_end = gs.characters[cn].a_end;
        if data58 > 1 && a_end > 10000 {
            if gs.characters[cn].mode != 2 {
                gs.characters[cn].mode = 2;
                gs.do_update_char(cn);
            }
        } else if data58 == 1 && a_end > 10000 {
            if gs.characters[cn].mode != 1 {
                gs.characters[cn].mode = 1;
                gs.do_update_char(cn);
            }
        } else {
            if gs.characters[cn].mode != 0 {
                gs.characters[cn].mode = 0;
                gs.do_update_char(cn);
            }
        }
    }

    // create light
    {
        let data62 = gs.characters[cn].data[62];
        let _data58 = gs.characters[cn].data[58];
        if data62 > _data58 {
            let cx = gs.characters[cn].x as usize;
            let cy = gs.characters[cn].y as usize;
            let light = gs.check_dlight(cx, cy);
            let idx = cx + cy * SERVER_MAPX as usize;
            let map_light = gs.map[idx].light;
            if light < 20 && map_light < 20 {
                if npc_try_spell(gs, cn, cn, skills::SK_LIGHT) {
                    return true;
                }
            }
        }
    }

    // make sure protected character survives
    {
        let co = gs.characters[cn].data[63] as usize;
        if co != 0 {
            let a_hp = gs.characters[co].a_hp;
            let hp5 = gs.characters[co].hp[5];
            if a_hp < hp5 as i32 * 600 {
                if npc_try_spell(gs, cn, co, skills::SK_HEAL) {
                    return true;
                }
            }
        }
    }

    // help friend
    {
        let co = gs.characters[cn].data[65] as usize;
        if co != 0 {
            let cc = gs.characters[co].attack_cn as usize;

            if gs.characters[cn].a_mana
                > (get_spellcost(&gs.characters[cn], skills::SK_BLESS) * 2
                    + get_spellcost(&gs.characters[cn], skills::SK_PROTECT)
                    + get_spellcost(&gs.characters[cn], skills::SK_ENHANCE))
            {
                if npc_try_spell(gs, cn, cn, skills::SK_BLESS) {
                    return true;
                }
            }

            if gs.characters[co].a_hp < gs.characters[co].hp[5] as i32 * 600 {
                if npc_try_spell(gs, cn, co, skills::SK_HEAL) {
                    return true;
                }
            }

            if !npc_can_spell(&gs.characters[co], &gs.characters[cn], skills::SK_PROTECT)
                && npc_try_spell(gs, cn, co, skills::SK_PROTECT)
            {
                return true;
            }
            if !npc_can_spell(&gs.characters[co], &gs.characters[cn], skills::SK_ENHANCE)
                && npc_try_spell(gs, cn, co, skills::SK_ENHANCE)
            {
                return true;
            }
            if !npc_can_spell(&gs.characters[co], &gs.characters[cn], skills::SK_BLESS)
                && npc_try_spell(gs, cn, co, skills::SK_BLESS)
            {
                return true;
            }

            if cc != 0
                && gs.characters[co].a_hp < gs.characters[co].hp[5] as i32 * 650
                && npc_is_enemy(&gs.characters[cn], &gs.characters[cc], cc)
            {
                if npc_try_spell(gs, cn, cc, skills::SK_BLAST) {
                    return true;
                }
            }
            gs.characters[cn].data[65] = 0;
        }
    }

    // generic fight-magic management
    {
        let co = gs.characters[cn].attack_cn as usize;
        let in_fight = co != 0 || gs.characters[cn].data[78] != 0;
        if in_fight {
            if npc_quaff_potion(gs, cn, 833, 254) {
                return true;
            }
            if npc_quaff_potion(gs, cn, 267, 254) {
                return true;
            }

            if co != 0
                && (gs.characters[cn].a_hp < gs.characters[cn].hp[5] as i32 * 600
                    || helpers::random_mod_i32(10) == 0)
            {
                if npc_try_spell(gs, cn, co, skills::SK_BLAST) {
                    return true;
                }
            }

            if co != 0 && gs.globals.ticker > gs.characters[cn].data[75] {
                if npc_try_spell(gs, cn, co, skills::SK_STUN) {
                    gs.characters[cn].data[75] = gs.globals.ticker
                        + gs.characters[cn].skill[skills::SK_STUN][5] as i32
                        + TICKS * 8;
                    return true;
                }
            }

            if gs.characters[cn].a_mana > 75000 && npc_try_spell(gs, cn, cn, skills::SK_BLESS) {
                return true;
            }
            if npc_try_spell(gs, cn, cn, skills::SK_PROTECT) {
                return true;
            }
            if npc_try_spell(gs, cn, cn, skills::SK_MSHIELD) {
                return true;
            }
            if npc_try_spell(gs, cn, cn, skills::SK_ENHANCE) {
                return true;
            }
            if npc_try_spell(gs, cn, cn, skills::SK_BLESS) {
                return true;
            }
            if co != 0 && npc_try_spell(gs, cn, co, skills::SK_CURSE) {
                return true;
            }
            if co != 0
                && gs.globals.ticker > gs.characters[cn].data[74] + (TICKS * 10)
                && npc_try_spell(gs, cn, co, skills::SK_GHOST)
            {
                gs.characters[cn].data[74] = gs.globals.ticker;
                return true;
            }

            if co != 0 && gs.characters[co].armor + 5 > gs.characters[cn].weapon {
                if npc_try_spell(gs, cn, co, skills::SK_BLAST) {
                    return true;
                }
            }
        }
    }

    // did we panic?
    if gs.characters[cn].data[78] != 0
        && gs.characters[cn].attack_cn == 0
        && gs.characters[cn].goto_x == 0
    {
        let (x, y) = (gs.characters[cn].x, gs.characters[cn].y);
        let rx = helpers::random_mod_i32(10);
        let ry = helpers::random_mod_i32(10);
        gs.characters[cn].goto_x = (x as i32 + 5 - rx) as u16;
        gs.characters[cn].goto_y = (y as i32 + 5 - ry) as u16;
        return true;
    }

    // are we on protect and want to follow our master?
    {
        let co = gs.characters[cn].data[69] as usize;
        if gs.characters[cn].attack_cn == 0 && co != 0 {
            if driver::follow_driver(gs, cn, co) {
                let (cn_x, cn_y, co_y) = (
                    gs.characters[cn].x,
                    gs.characters[cn].y,
                    gs.characters[co].y,
                );
                let dist = (cn_x - co_y).abs() + (cn_y - co_y).abs();
                gs.characters[cn].data[58] = if dist > 6 { 2 } else { 1 };
                return true;
            }
        }
    }

    // don't scan if we don't use the information
    if gs.characters[cn].data[41] == 0 && gs.characters[cn].data[47] == 0 {
        return false;
    }

    // save some work
    if gs.characters[cn].data[41] != 0 && gs.characters[cn].misc_action == DR_USE as u16 {
        return false;
    }
    if gs.characters[cn].data[47] != 0 && gs.characters[cn].misc_action == DR_PICKUP as u16 {
        return false;
    }
    if gs.characters[cn].data[47] != 0 && gs.characters[cn].misc_action == DR_USE as u16 {
        return false;
    }

    // scan nearby map for items of interest
    let ch_pos = (gs.characters[cn].x, gs.characters[cn].y);
    // indoor detection
    let indoor1 = {
        let idx = ch_pos.0 as usize + ch_pos.1 as usize * SERVER_MAPX as usize;
        (gs.map[idx].flags & MF_INDOORS as u64) != 0
    };
    let min_y = std::cmp::max(ch_pos.1 as i32 - 8, 1) as usize;
    let max_y = std::cmp::min(ch_pos.1 as i32 + 8, SERVER_MAPY - 1) as usize;
    let min_x = std::cmp::max(ch_pos.0 as i32 - 8, 1) as usize;
    let max_x = std::cmp::min(ch_pos.0 as i32 + 8, SERVER_MAPX - 1) as usize;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let m = x + y * SERVER_MAPX as usize;
            let map_it = gs.map[m].it as usize;
            if map_it == 0 {
                continue;
            }

            let indoor2 = (gs.map[m].flags & MF_INDOORS as u64) != 0;
            let it_temp = gs.items[map_it].temp as i32;

            if it_temp == gs.characters[cn].data[41] {
                // check active and light conditions - TODO: check actual map light/dlight
                let active = gs.items[map_it].active;
                if active == 0 {
                    gs.characters[cn].misc_action = DR_USE as u16;
                    gs.characters[cn].misc_target1 = x as u16;
                    gs.characters[cn].misc_target2 = y as u16;
                    gs.characters[cn].goto_x = 0u16;
                    gs.characters[cn].data[58] = 1;
                    return true;
                }
                // TODO: handle case when active and dlight > 200 and !indoor2
            }

            if gs.characters[cn].data[47] != 0 && indoor1 == indoor2 {
                let flags = gs.items[map_it].flags;
                if flags & ItemFlags::IF_TAKE.bits() != 0 {
                    let (ch_x, ch_y) = (gs.characters[cn].x as i32, gs.characters[cn].y as i32);
                    let can_reach = gs.can_go(ch_x, ch_y, x as i32, y as i32) != 0;
                    let can_see = gs.do_char_can_see_item(cn, map_it) != 0;

                    if can_reach && can_see && it_temp != 18 {
                        gs.characters[cn].misc_action = DR_PICKUP as u16;
                        gs.characters[cn].misc_target1 = x as u16;
                        gs.characters[cn].misc_target2 = y as u16;
                        gs.characters[cn].goto_x = 0u16;
                        gs.characters[cn].data[58] = 1;
                        return true;
                    }
                }
                if gs.items[map_it].driver == 7 {
                    let (ch_x, ch_y) = (gs.characters[cn].x as i32, gs.characters[cn].y as i32);
                    let can_reach = gs.can_go(ch_x, ch_y, x as i32, y as i32) != 0;
                    let can_see = gs.do_char_can_see_item(cn, map_it) != 0;

                    if can_reach && can_see && x + 1 < SERVER_MAPX as usize {
                        let map_idx = x + 1 + y * SERVER_MAPX as usize;
                        let is_empty = gs.map[map_idx].it == 0;

                        if is_empty && player::plr_check_target(gs, map_idx) {
                            if let Some(in2) = God::create_item(gs, 18) {
                                gs.items[in2].carried = cn as u16;
                                gs.characters[cn].citem = in2 as u32;
                                gs.characters[cn].misc_action = DR_DROP as u16;
                                gs.characters[cn].misc_target1 = (x + 1) as u16;
                                gs.characters[cn].misc_target2 = y as u16;
                                gs.characters[cn].goto_x = 0u16;
                                gs.characters[cn].data[58] = 1;
                                return true;
                            }
                        }
                    }
                }
            }
        }
    }

    false
}

// ****************************************************
// Low Priority NPC Driver
// ****************************************************

pub fn npc_driver_low(gs: &mut GameState, cn: usize) {
    // Check for special driver
    let special_driver = gs.characters[cn].data[25];

    if special_driver != 0 {
        match special_driver {
            1 => {
                driver::npc_stunrun_low(gs, cn);
            }
            2 => {
                driver::npc_cityattack_low(gs, cn);
            }
            3 => {
                driver::npc_malte_low(gs, cn);
            }
            _ => {
                log::error!("Unknown special driver {} for {}", special_driver, cn);
            }
        }
        return;
    }

    let ticker = gs.globals.ticker;
    let flags = gs.globals.flags;

    // Handle action results
    if gs.characters[cn].last_action == ERR_SUCCESS as i8 {
        gs.characters[cn].data[36] = 0;
    } else if gs.characters[cn].last_action == ERR_FAILED as i8 {
        gs.characters[cn].data[36] += 1;
    }

    // Are we supposed to loot graves?
    let alignment = gs.characters[cn].alignment;
    let temp = gs.characters[cn].temp;
    let character_flags = gs.characters[cn].flags;

    if alignment < 0
        && (flags & GF_LOOTING) != 0
        && ((cn & 15) == (ticker as usize & 15)
            || (character_flags & CharacterFlags::IsLooting.bits()) != 0)
        && temp != CT_COMPANION as u16
    {
        if npc_grave_logic(gs, cn) {
            return;
        }
    }

    // Did someone call help? - high prio
    let (data_55, data_54) = (gs.characters[cn].data[55], gs.characters[cn].data[54]);

    if data_55 != 0 && data_55 + (TICKS * 120) > ticker && data_54 != 0 {
        let m = data_54;
        gs.characters[cn].goto_x = ((m % SERVER_MAPX) + get_frust_x_off(ticker)) as u16;
        gs.characters[cn].goto_y = ((m / SERVER_MAPX) + get_frust_y_off(ticker)) as u16;
        gs.characters[cn].data[58] = 2;
        return;
    }

    // Go to last known enemy position and stay there for up to 30 seconds
    let (data_77, data_76, data_36) = (
        gs.characters[cn].data[77],
        gs.characters[cn].data[76],
        gs.characters[cn].data[36],
    );

    if data_77 != 0 && data_77 + (TICKS * 30) > ticker {
        let m = data_76;
        gs.characters[cn].goto_x = ((m % SERVER_MAPX) + get_frust_x_off(data_36)) as u16;
        gs.characters[cn].goto_y = ((m / SERVER_MAPX) + get_frust_y_off(data_36)) as u16;
        return;
    }

    // We're hurt: rest
    let (a_hp, hp_5) = (gs.characters[cn].a_hp, gs.characters[cn].hp[5]);

    if a_hp < (hp_5 as i32 * 750) {
        return;
    }

    // Close door, medium prio
    for n in 20..24 {
        let m = gs.characters[cn].data[n];

        if m != 0 {
            let m = m as usize;
            // Check if the door is free
            let is_free = gs.map[m].ch == 0
                && gs.map[m].to_ch == 0
                && gs.map[m + 1].ch == 0
                && gs.map[m + 1].to_ch == 0
                && gs.map[m - 1].ch == 0
                && gs.map[m - 1].to_ch == 0
                && gs.map[m + SERVER_MAPX as usize].ch == 0
                && gs.map[m + SERVER_MAPX as usize].to_ch == 0
                && gs.map[m - SERVER_MAPX as usize].ch == 0
                && gs.map[m - SERVER_MAPX as usize].to_ch == 0;

            if is_free {
                let it_idx = gs.map[m].it;
                let is_active = if it_idx != 0 {
                    gs.items[it_idx as usize].active
                } else {
                    0
                };

                if it_idx != 0 && is_active != 0 {
                    gs.characters[cn].misc_action = core::constants::DR_USE as u16;
                    gs.characters[cn].misc_target1 = (m % SERVER_MAPX as usize) as u16;
                    gs.characters[cn].misc_target2 = (m / SERVER_MAPX as usize) as u16;
                    gs.characters[cn].data[58] = 1;
                    return;
                }
            }
        }
    }

    // Activate light, medium prio
    for n in 32..36 {
        let m = gs.characters[cn].data[n];

        if m != 0 && m < (SERVER_MAPX * SERVER_MAPY) {
            let m = m as usize;
            let it_idx = gs.map[m].it;
            let is_active = if it_idx != 0 {
                gs.items[it_idx as usize].active
            } else {
                1
            };

            if it_idx != 0 && is_active == 0 {
                gs.characters[cn].misc_action = core::constants::DR_USE as u16;
                gs.characters[cn].misc_target1 = (m % SERVER_MAPX as usize) as u16;
                gs.characters[cn].misc_target2 = (m / SERVER_MAPX as usize) as u16;
                gs.characters[cn].data[58] = 1;
                return;
            }
        }
    }

    // Patrol, low
    let data_10 = gs.characters[cn].data[10];
    if data_10 != 0 {
        let mut n = gs.characters[cn].data[19];

        if !(10..=18).contains(&n) {
            n = 10;
            gs.characters[cn].data[19] = n;
        }

        let data_57 = gs.characters[cn].data[57];
        if data_57 > ticker {
            return;
        }

        let m = gs.characters[cn].data[n as usize];
        let data_36 = gs.characters[cn].data[36];
        let ch_x = gs.characters[cn].x;
        let ch_y = gs.characters[cn].y;
        let data_79 = gs.characters[cn].data[79];

        let x = (m % SERVER_MAPX) + get_frust_x_off(data_36);
        let y = (m / SERVER_MAPX) + get_frust_y_off(data_36);

        if data_36 > 20 || ((ch_x as i32 - x).abs() + (ch_y as i32 - y).abs()) < 4 {
            if data_36 <= 20 && data_79 != 0 {
                gs.characters[cn].data[57] = ticker + data_79;
            }

            n += 1;
            if n > 18 {
                n = 10;
            }

            let data_n = gs.characters[cn].data[n as usize];
            if data_n == 0 {
                n = 10;
            }

            gs.characters[cn].data[19] = n;
            gs.characters[cn].data[36] = 0;

            return;
        }

        gs.characters[cn].goto_x = x as u16;
        gs.characters[cn].goto_y = y as u16;
        gs.characters[cn].data[58] = 0;
        return;
    }

    // Random walk, low
    let data_60 = gs.characters[cn].data[60];
    if data_60 != 0 {
        gs.characters[cn].data[58] = 0;

        let data_61 = gs.characters[cn].data[61];
        if data_61 < 1 {
            gs.characters[cn].data[61] = data_60;

            let (ch_x, ch_y, data_73, data_29) = (
                gs.characters[cn].x,
                gs.characters[cn].y,
                gs.characters[cn].data[73],
                gs.characters[cn].data[29],
            );

            let mut panic = 0;
            let mut x = 0;
            let mut y = 0;

            for attempt in 0..5 {
                // TODO: Call RANDOM function (doesn't exist yet, use placeholder)
                x = ch_x as i32 - 5 + (ticker % 11); // RANDOM(11)
                y = ch_y as i32 - 5 + ((ticker / 11) % 11); // RANDOM(11)

                if !(1..SERVER_MAPX).contains(&x) || !(1..=SERVER_MAPX).contains(&y) {
                    panic = attempt + 1;
                    continue;
                }

                if data_73 != 0 {
                    // Too far away from origin?
                    let xo = data_29 % SERVER_MAPX;
                    let yo = data_29 / SERVER_MAPX;

                    if (x - xo).abs() + (y - yo).abs() > data_73 {
                        // Try to return to origin
                        let plr_check_target = |tx: i32, ty: i32| -> bool {
                            npc_check_target(gs, tx as usize, ty as usize)
                        };

                        if plr_check_target(xo, yo) {
                            gs.characters[cn].goto_x = xo as u16;
                            gs.characters[cn].goto_y = yo as u16;
                            return;
                        } else if plr_check_target(xo + 1, yo) {
                            gs.characters[cn].goto_x = (xo + 1) as u16;
                            gs.characters[cn].goto_y = yo as u16;
                            return;
                        } else if plr_check_target(xo - 1, yo) {
                            gs.characters[cn].goto_x = (xo - 1) as u16;
                            gs.characters[cn].goto_y = yo as u16;
                            return;
                        } else if plr_check_target(xo, yo + 1) {
                            gs.characters[cn].goto_x = xo as u16;
                            gs.characters[cn].goto_y = (yo + 1) as u16;
                            return;
                        } else if plr_check_target(xo, yo - 1) {
                            gs.characters[cn].goto_x = xo as u16;
                            gs.characters[cn].goto_y = (yo - 1) as u16;
                            return;
                        } else {
                            panic = attempt + 1;
                            continue;
                        }
                    }
                }

                if !npc_check_target(gs, x as usize, y as usize) {
                    panic = attempt + 1;
                    continue;
                }

                if gs.can_go(ch_x as i32, ch_y as i32, x, y) == 0 {
                    panic = attempt + 1;
                    continue;
                }

                panic = attempt;
                break;
            }

            if panic == 5 {
                return;
            }

            gs.characters[cn].goto_x = x as u16;
            gs.characters[cn].goto_y = y as u16;
            return;
        } else {
            gs.characters[cn].data[61] -= 1;
            return;
        }
    }

    // Resting position, lowest prio
    let data_29 = gs.characters[cn].data[29];
    if data_29 != 0 {
        let data_36 = gs.characters[cn].data[36];
        let m = data_29;
        let x = (m % SERVER_MAPX) + get_frust_x_off(data_36);
        let y = (m / SERVER_MAPX) + get_frust_y_off(data_36);

        gs.characters[cn].data[58] = 0;

        let (ch_x, ch_y, ch_dir, data_30) = (
            gs.characters[cn].x,
            gs.characters[cn].y,
            gs.characters[cn].dir,
            gs.characters[cn].data[30],
        );

        if ch_x != x as i16 || ch_y != y as i16 {
            gs.characters[cn].goto_x = x as u16;
            gs.characters[cn].goto_y = y as u16;
            return;
        }

        if ch_dir as i32 != data_30 {
            {
                gs.characters[cn].misc_action = core::constants::DR_TURN as u16;

                // Turn toward an adjacent tile based on desired direction.
                // (misc_target1/misc_target2 are coordinates, not the direction value.)
                let mut target_x = x;
                let mut target_y = y;

                match data_30 {
                    d if d == DX_UP as i32 => target_y -= 1,
                    d if d == DX_DOWN as i32 => target_y += 1,
                    d if d == DX_LEFT as i32 => target_x -= 1,
                    d if d == DX_RIGHT as i32 => target_x += 1,
                    d if d == DX_LEFTUP as i32 => {
                        target_x -= 1;
                        target_y -= 1;
                    }
                    d if d == DX_LEFTDOWN as i32 => {
                        target_x -= 1;
                        target_y += 1;
                    }
                    d if d == DX_RIGHTUP as i32 => {
                        target_x += 1;
                        target_y -= 1;
                    }
                    d if d == DX_RIGHTDOWN as i32 => {
                        target_x += 1;
                        target_y += 1;
                    }
                    _ => {
                        gs.characters[cn].misc_action = DR_IDLE as u16;
                        return;
                    }
                }

                if !(0..SERVER_MAPX).contains(&target_x) || !(0..SERVER_MAPY).contains(&target_y) {
                    gs.characters[cn].misc_action = core::constants::DR_IDLE as u16;
                    return;
                }

                gs.characters[cn].misc_target1 = target_x as u16;
                gs.characters[cn].misc_target2 = target_y as u16;
            }
            return;
        }
    }

    // Reset talked-to list
    let data_67 = gs.characters[cn].data[67];
    if data_67 + (TICKS * 60 * 5) < ticker {
        let data_37 = gs.characters[cn].data[37];
        if data_37 != 0 {
            for n in 37..41 {
                gs.characters[cn].data[n] = 1; // Hope we never have a character nr 1!
            }
        }
        gs.characters[cn].data[67] = ticker;
    }

    // Special sub-proc for Shiva (black stronghold mage)
    let (data_26, a_mana, mana_5) = (
        gs.characters[cn].data[26],
        gs.characters[cn].a_mana,
        gs.characters[cn].mana[5],
    );

    if data_26 == 2 && a_mana > (mana_5 as i32 * 900) {
        // Count active monsters of type 27
        let mut m = 0;
        for n in 1..MAXCHARS {
            let (used, flags, data_42) = if n >= gs.characters.len() {
                (0, 0, 0)
            } else {
                (
                    gs.characters[n].used,
                    gs.characters[n].flags,
                    gs.characters[n].data[42],
                )
            };

            if used != USE_ACTIVE {
                continue;
            }
            if (flags & (CharacterFlags::Body.bits() | CharacterFlags::Respawn.bits())) != 0 {
                continue;
            }
            if data_42 == 27 {
                m += 1;
            }
        }

        if m < 15 {
            let mut n = 0;

            // Check candles
            let candle_positions = [(446, 347), (450, 351), (457, 348), (457, 340), (449, 340)];

            for (cx, cy) in &candle_positions {
                let map_idx = cx + cy * SERVER_MAPX as usize;
                let it_idx = gs.map[map_idx].it;
                let is_active = if it_idx != 0 {
                    gs.items[it_idx as usize].active
                } else {
                    0
                };

                if it_idx != 0 {
                    if is_active == 0 {
                        n += 1;
                    } else {
                        if shiva_activate_candle(gs, cn, it_idx as usize) {
                            return;
                        }
                    }
                }
            }

            if n > 0 {
                for m_idx in 0..n {
                    let co = match populate::pop_create_char(gs, 503 + m_idx, false) {
                        Some(co) => co,
                        None => {
                            gs.do_sayx(cn, &format!("create char ({})", m_idx));
                            break;
                        }
                    };

                    if !God::drop_char_fuzzy(gs, co, 452, 345) {
                        gs.do_sayx(cn, &format!("drop char ({})", m_idx));
                        God::destroy_items(gs, co);
                        gs.characters[co].used = 0;
                        break;
                    }

                    EffectManager::fx_add_effect(
                        gs,
                        6,
                        0,
                        gs.characters[co].x as i32,
                        gs.characters[co].y as i32,
                        0,
                    );
                }

                EffectManager::fx_add_effect(
                    gs,
                    7,
                    0,
                    gs.characters[cn].x as i32,
                    gs.characters[cn].y as i32,
                    0,
                );
                gs.do_sayx(cn, "Khuzak gurawin duskar!");

                gs.characters[cn].a_mana -= (n * 100 * 1000) as i32;

                log::info!("created {} new monsters", n);
            }
        }

        gs.characters[cn].a_mana -= 100 * 1000;
    }
}

// ****************************************************
// Grave Looting and Equipment Functions
// ****************************************************

pub fn npc_check_placement(gs: &GameState, in_idx: usize, n: usize) -> bool {
    let placement = gs.items[in_idx].placement;

    match n {
        WN_HEAD => (placement & PL_HEAD) != 0,
        WN_NECK => (placement & PL_NECK) != 0,
        WN_BODY => (placement & PL_BODY) != 0,
        WN_ARMS => (placement & PL_ARMS) != 0,
        WN_BELT => (placement & PL_BELT) != 0,
        WN_LEGS => (placement & PL_LEGS) != 0,
        WN_FEET => (placement & PL_FEET) != 0,
        WN_LHAND => (placement & PL_SHIELD) != 0,
        WN_RHAND => (placement & PL_WEAPON) != 0,
        WN_CLOAK => (placement & PL_CLOAK) != 0,
        WN_LRING | WN_RRING => (placement & PL_RING) != 0,
        _ => false,
    }
}

pub fn npc_can_wear_item(ch: &Character, it: &core::types::Item) -> bool {
    // Check attribute requirements
    for m in 0..5 {
        if it.attrib[m][2] > ch.attrib[m][0] as i8 {
            return false;
        }
    }

    // Check skill requirements
    for m in 0..50 {
        if it.skill[m][2] > ch.skill[m][0] as i8 {
            return false;
        }
    }

    // Check other requirements
    if it.hp[2] > ch.hp[0] as i16 {
        return false;
    }

    if it.end[2] > ch.end[0] as i16 {
        return false;
    }

    if it.mana[2] > ch.mana[0] as i16 {
        return false;
    }

    true
}

pub fn npc_item_value(it: &core::types::Item) -> i32 {
    let mut score = 0;

    for n in 0..50 {
        // TODO: Do a deeper dive into what this is doing -- originally
        // the C code has it.attrib here which is clearly wrong since attrib
        // only has 5 entries.
        score += it.skill[n][0] as i32 * 5;
    }

    score += (it.value / 10) as i32;
    score += it.weapon[0] as i32 * 50;
    score += it.armor[0] as i32 * 50;
    score -= it.damage_state as i32;

    score
}

pub fn npc_want_item(gs: &mut GameState, cn: usize, in_idx: usize) -> bool {
    let item_38 = gs.characters[cn].item[38];

    if item_38 != 0 {
        return false; // hack: don't take more stuff if inventory is almost full
    }

    let citem = gs.characters[cn].citem;

    if citem != 0 {
        log::info!("have {} in citem", gs.items[in_idx].get_name());

        let do_store_item = gs.do_store_item(cn);
        if do_store_item == -1 {
            gs.items[citem as usize].used = USE_EMPTY;
            gs.characters[cn].citem = 0;
        }
    }

    let temp = gs.items[in_idx].temp;

    if temp == 833 || temp == 267 {
        gs.characters[cn].citem = in_idx as u32;
        gs.items[in_idx].carried = cn as u16;
        gs.do_store_item(cn);
        return true;
    }

    false
}

pub fn npc_equip_item(gs: &mut GameState, cn: usize, in_idx: usize) -> bool {
    let citem = gs.characters[cn].citem;

    if citem != 0 {
        log::info!("have {} in citem", gs.items[in_idx].get_name());

        let do_store_item = gs.do_store_item(cn);
        if do_store_item == -1 {
            gs.items[citem as usize].used = USE_EMPTY;
            gs.characters[cn].citem = 0;
        }
    }

    for n in 0..20 {
        let worn_n = gs.characters[cn].worn[n];

        if worn_n == 0
            || npc_item_value(&gs.items[in_idx]) > npc_item_value(&gs.items[worn_n as usize])
        {
            if npc_check_placement(gs, in_idx, n) {
                if npc_can_wear_item(&gs.characters[cn], &gs.items[in_idx]) {
                    log::info!("now wearing {}", gs.items[in_idx].get_name());

                    // Remove old item if any
                    if worn_n != 0 {
                        log::info!("storing item");
                        gs.characters[cn].citem = worn_n;

                        let do_store_item = gs.do_store_item(cn);
                        if do_store_item == -1 {
                            return false; // Stop looting if our backpack is full
                        }
                    }

                    gs.characters[cn].worn[n] = in_idx as u32;
                    gs.characters[cn].set_do_update_flags();
                    gs.items[in_idx].carried = cn as u16;

                    return true;
                }
            }
        }
    }

    false
}

pub fn npc_loot_grave(gs: &mut GameState, cn: usize, in_idx: usize) -> bool {
    let ch_x = gs.characters[cn].x;
    let ch_y = gs.characters[cn].y;
    let ch_dir = gs.characters[cn].dir;
    let frust = gs.characters[cn].data[36];

    let (it_x, it_y) = (gs.items[in_idx].x, gs.items[in_idx].y);

    // Check if we're adjacent and facing the grave
    if ((ch_x as i32 - it_x as i32).abs() + (ch_y as i32 - it_y as i32).abs()) > 1
        || helpers::drv_dcoor2dir(it_x as i32 - ch_x as i32, it_y as i32 - ch_y as i32)
            != ch_dir as i32
    {
        if frust > 20 {
            log::debug!(
                "NPC {} giving up on grave {} due to high frustration ({})",
                cn,
                in_idx,
                frust
            );
            return false;
        }

        gs.characters[cn].misc_action = DR_USE as u16;
        gs.characters[cn].misc_target1 = it_x;
        gs.characters[cn].misc_target2 = it_y;
        return true;
    }

    let co = gs.items[in_idx].data[0] as usize;

    if !Character::is_sane_character(co) {
        log::warn!(
            "NPC {} tried to loot grave {} but corpse character {} is invalid",
            cn,
            in_idx,
            co
        );
        return false;
    }

    let is_body = (gs.characters[co].flags & CharacterFlags::Body.bits()) != 0;

    if !is_body {
        log::warn!(
            "NPC {} tried to loot grave {} but corpse character {} is not a body (slot reused?)",
            cn,
            in_idx,
            co
        );
        return false;
    }

    // Try to loot worn items
    for n in 0..20 {
        let worn_item = gs.characters[co].worn[n];

        if worn_item != 0 {
            let in_item = worn_item as usize;
            if npc_equip_item(gs, cn, in_item) {
                let item_name = gs.items[in_item].get_name().to_string();
                let co_name = gs.characters[co].get_name().to_string();
                log::info!("got {} from {}'s grave", item_name, co_name);
                gs.characters[co].worn[n] = 0;
                return true;
            }
        }
    }

    // Try to loot inventory items
    for n in 0..40 {
        let inv_item = gs.characters[co].item[n];

        if inv_item != 0 {
            let in_item = inv_item as usize;

            if npc_equip_item(gs, cn, in_item) {
                let item_name = gs.items[in_item].get_name().to_string();
                let co_name = gs.characters[co].get_name().to_string();
                log::info!("got {} from {}'s grave", item_name, co_name);
                gs.characters[co].item[n] = 0;
                return true;
            }

            if npc_want_item(gs, cn, in_item) {
                let item_name = gs.items[in_item].get_name().to_string();
                let co_name = gs.characters[co].get_name().to_string();
                log::info!("got {} from {}'s grave", item_name, co_name);
                gs.characters[co].item[n] = 0;
                return true;
            }
        }
    }

    // Try to loot gold
    let co_gold = gs.characters[co].gold;
    if co_gold != 0 {
        let co_name = gs.characters[co].get_name().to_string();
        log::info!(
            "got {:.2}G from {}'s grave",
            co_gold as f32 / 100.0,
            co_name
        );
        gs.characters[cn].gold += co_gold;
        gs.characters[co].gold = 0;
        return true;
    }

    false
}

pub fn npc_already_searched_grave(cn: &Character, in_idx: usize) -> bool {
    let text_9 = &cn.text[9];

    let mut n = 0;
    while n < 160 {
        if n + 4 <= text_9.len() {
            let value =
                i32::from_le_bytes([text_9[n], text_9[n + 1], text_9[n + 2], text_9[n + 3]]);

            if value == in_idx as i32 {
                return true;
            }
        }
        n += std::mem::size_of::<i32>();
    }

    false
}

pub fn npc_add_searched_grave(gs: &mut GameState, cn: usize, in_idx: usize) {
    let int_size = std::mem::size_of::<i32>();
    let text_9_len = gs.characters[cn].text[9].len();

    if text_9_len > int_size {
        gs.characters[cn].text[9].copy_within(0..(text_9_len - int_size), int_size);
    }

    let bytes = (in_idx as i32).to_le_bytes();
    if text_9_len >= int_size {
        gs.characters[cn].text[9][0..int_size].copy_from_slice(&bytes);
    }
}

pub fn npc_grave_logic(gs: &mut GameState, cn: usize) -> bool {
    let (ch_x, ch_y) = (gs.characters[cn].x, gs.characters[cn].y);

    let min_y = std::cmp::max(ch_y as i32 - 8, 1);
    let max_y = std::cmp::min(ch_y as i32 + 8, SERVER_MAPY - 1);
    let min_x = std::cmp::max(ch_x as i32 - 8, 1);
    let max_x = std::cmp::min(ch_x as i32 + 8, SERVER_MAPX - 1);

    for y in min_y..max_y {
        for x in min_x..max_x {
            let map_idx = (x + y * SERVER_MAPX) as usize;
            let in_idx = gs.map[map_idx].it;

            if in_idx != 0 {
                let in_idx = in_idx as usize;

                let is_grave = gs.items[in_idx].temp == 170;

                if is_grave {
                    let (it_x, it_y) = (gs.items[in_idx].x, gs.items[in_idx].y);

                    let can_reach =
                        gs.can_go(ch_x as i32, ch_y as i32, it_x as i32, it_y as i32) != 0;

                    let can_see = gs.do_char_can_see_item(cn, in_idx) != 0;

                    if can_reach
                        && can_see
                        && !npc_already_searched_grave(&gs.characters[cn], in_idx)
                    {
                        if !npc_loot_grave(gs, cn, in_idx) {
                            npc_add_searched_grave(gs, cn, in_idx);
                            gs.characters[cn].flags &= !CharacterFlags::IsLooting.bits();
                        } else {
                            gs.characters[cn].flags |= CharacterFlags::IsLooting.bits();
                        }
                        return true;
                    }
                }
            }
        }
    }

    false
}

// ****************************************************
// Shop Functions
// ****************************************************

pub fn update_shop(gs: &mut GameState, cn: usize) {
    let mut sale = [0i32; 10];

    let data_copy = gs.characters[cn].data;
    sale.copy_from_slice(&data_copy[0..10]);

    gs.do_sort(cn, "v");

    let mut m = 0;
    let mut x = 0;

    for n in 0..40 {
        let in_idx = gs.characters[cn].item[n];

        if in_idx == 0 {
            m += 1;
        } else {
            let temp = gs.items[in_idx as usize].temp;

            let mut found = false;
            for z in 0..10 {
                if temp == sale[z] as u16 {
                    sale[z] = 0;
                    found = true;
                    break;
                }
            }

            if !found {
                x = n;
            }
        }
    }

    if m < 2 {
        let in_idx = gs.characters[cn].item[x];

        if in_idx != 0 {
            let flags = gs.items[in_idx as usize].flags;

            if (flags & ItemFlags::IF_DONATE.bits()) != 0 {
                God::donate_item(gs, in_idx as usize, 0);
                gs.items[in_idx as usize].used = USE_EMPTY;
            } else {
                gs.items[in_idx as usize].used = USE_EMPTY;
            }

            gs.characters[cn].item[x] = 0;
        }
    }

    // Check if our store is complete - create missing items
    for n in 0..10 {
        let temp = sale[n];
        if temp == 0 {
            continue;
        }

        let in_idx = God::create_item(gs, temp as usize);

        if in_idx.is_some() {
            if !God::give_character_item(gs, cn, in_idx.unwrap()) {
                gs.items[in_idx.unwrap()].used = USE_EMPTY;
            }
        }
    }

    // Small-repair all items (reset damage and age)
    // Junk all items needing serious repair
    for n in 0..40 {
        let in_idx = gs.characters[cn].item[n];

        if in_idx != 0 {
            let damage_state = gs.items[in_idx as usize].damage_state;
            let flags = gs.items[in_idx as usize].flags;

            if damage_state != 0 || (flags & ItemFlags::IF_SHOPDESTROY.bits()) != 0 {
                gs.items[in_idx as usize].used = USE_EMPTY;
                gs.characters[cn].item[n] = 0;
            } else {
                gs.items[in_idx as usize].current_damage = 0;
                gs.items[in_idx as usize].current_age[0] = 0;
                gs.items[in_idx as usize].current_age[1] = 0;
            }
        }
    }

    gs.do_sort(cn, "v");
}

// ****************************************************
// Special Functions
// ****************************************************

pub fn shiva_activate_candle(gs: &mut GameState, cn: usize, in_idx: usize) -> bool {
    let (mdtime, mdday) = (gs.globals.mdtime, gs.globals.mdday);

    if mdtime > 2000 {
        return false;
    }

    let data_0 = gs.characters[cn].data[0];
    if data_0 >= mdday {
        return false;
    }

    log::info!(
        "Created new candle, time={}, day={}, last day={}",
        mdtime,
        mdday,
        data_0
    );

    gs.characters[cn].data[0] = mdday + 9;

    gs.items[in_idx].active = 0;

    let light_0 = gs.items[in_idx].light[0];
    let light_1 = gs.items[in_idx].light[1];
    let it_x = gs.items[in_idx].x;
    let it_y = gs.items[in_idx].y;

    if light_0 != light_1 && it_x > 0 {
        gs.do_add_light(it_x as i32, it_y as i32, light_0 as i32 - light_1 as i32);
    }

    EffectManager::fx_add_effect(gs, 6, 0, it_x as i32, it_y as i32, 0);
    EffectManager::fx_add_effect(
        gs,
        7,
        0,
        gs.characters[cn].x as i32,
        gs.characters[cn].y as i32,
        0,
    );

    gs.do_sayx(cn, "Shirak ishagur gorweran dulak!");

    gs.characters[cn].a_mana -= 800 * 1000;

    true
}

// ****************************************************
// Helper Functions for npc_see
// ****************************************************

pub fn is_unique_item(it: &core::types::Item) -> bool {
    const UNIQUE_TEMPS: [u16; 60] = [
        280, 281, 282, 283, 284, 285, 286, 287, 288, 289, 290, 291, 292, 525, 526, 527, 528, 529,
        530, 531, 532, 533, 534, 535, 536, 537, 538, 539, 540, 541, 542, 543, 544, 545, 546, 547,
        548, 549, 550, 551, 552, 553, 554, 555, 556, 572, 573, 574, 575, 576, 577, 578, 579, 580,
        581, 582, 583, 584, 585, 586,
    ];

    UNIQUE_TEMPS.contains(&it.temp)
}

pub fn count_uniques(cn: &Character, items: &[core::types::Item]) -> i32 {
    let mut cnt = 0;

    let citem = cn.citem;
    if citem != 0 && (citem & 0x80000000) == 0 && is_unique_item(&items[citem as usize]) {
        cnt += 1;
    }

    for n in 0..40 {
        let in_idx = cn.item[n];
        if in_idx != 0 && is_unique_item(&items[in_idx as usize]) {
            cnt += 1;
        }
    }

    for n in 0..20 {
        let in_idx = cn.worn[n];
        if in_idx != 0 && is_unique_item(&items[in_idx as usize]) {
            cnt += 1;
        }
    }

    for n in 0..62 {
        let in_idx = cn.depot[n];
        if in_idx != 0 && is_unique_item(&items[in_idx as usize]) {
            cnt += 1;
        }
    }

    cnt
}

pub fn npc_cityguard_see(gs: &mut GameState, cn: usize, co: usize, flag: i32) -> bool {
    let co_group = gs.characters[co].data[42];

    if co_group == 27 {
        let ticker = gs.globals.ticker;
        let data_55 = gs.characters[cn].data[55];
        let data_52 = gs.characters[cn].data[52];
        let ch_x = gs.characters[cn].x;
        let ch_y = gs.characters[cn].y;

        if data_55 + (TICKS * 180) < ticker {
            gs.characters[cn].data[54] = 0;
            gs.characters[cn].data[55] = ticker;

            let co_name = gs.characters[co].get_name().to_string();

            npc_saytext_n(gs, cn, 4, Some(&co_name));
            gs.do_npc_shout(
                cn,
                NT_SHOUT as i32,
                cn as i32,
                data_52,
                ch_x as i32,
                ch_y as i32,
            );

            for n in 1..MAXCHARS {
                if n >= gs.characters.len() {
                    break;
                }
                let is_player = (gs.characters[n].flags
                    & (CharacterFlags::Player.bits() | CharacterFlags::Usurp.bits()))
                    != 0;
                let used = gs.characters[n].used;
                let no_shout = (gs.characters[n].flags & CharacterFlags::NoShout.bits()) != 0;

                if is_player && used == USE_ACTIVE && !no_shout {
                    let message = if flag != 0 {
                        "Cityguard: \"The monsters are approaching the city! Alert!\""
                    } else {
                        "Cityguard: \"The monsters are approaching the outpost! Alert!\""
                    };
                    log::info!("[char {}] {}", n, message);
                }
            }
        }
    }

    false
}

// ****************************************************
// NPC See Function
// ****************************************************

pub fn npc_see(gs: &mut GameState, cn: usize, co: usize) -> bool {
    let ticker = gs.globals.ticker;

    let co_flags = gs.characters[co].flags;
    if (co_flags & (CharacterFlags::Player.bits() | CharacterFlags::Usurp.bits())) != 0 {
        gs.characters[cn].data[92] = TICKS * 60;
    } else {
        if gs.characters[cn].group_active() {
            gs.characters[cn].data[92] = TICKS * 60;
        }
    }

    let can_see = gs.do_char_can_see(cn, co);
    if can_see == 0 {
        return true;
    }

    let temp = gs.characters[cn].temp;
    let data_63 = gs.characters[cn].data[63];

    if temp == CT_COMPANION as u16 && co == data_63 as usize {
        gs.characters[cn].data[98] = ticker + COMPANION_TIMEOUT;
    }

    let data_26 = gs.characters[cn].data[26];
    if data_26 != 0 {
        let ret = match data_26 {
            1 => npc_cityguard_see(gs, cn, co, 0),
            3 => npc_cityguard_see(gs, cn, co, 1),
            _ => false,
        };
        if ret {
            return true;
        }
    }

    let cn_x = gs.characters[cn].x;
    let cn_y = gs.characters[cn].y;
    let co_x = gs.characters[co].x;
    let co_y = gs.characters[co].y;

    let indoor1 = {
        let idx = cn_x as usize + cn_y as usize * SERVER_MAPX as usize;
        (gs.map[idx].flags & MF_INDOORS as u64) != 0
    };

    let indoor2 = {
        let idx = co_x as usize + co_y as usize * SERVER_MAPX as usize;
        (gs.map[idx].flags & MF_INDOORS as u64) != 0
    };

    let attack_cn = gs.characters[cn].attack_cn;
    if attack_cn == 0 {
        let co_id = helpers::char_id(&gs.characters[co]) as u32;
        let idx = co as i32 | ((co_id as i32) << 16);

        let mut found = false;
        for n in 80..92 {
            if gs.characters[cn].data[n] == idx {
                found = true;
                break;
            }
        }

        if found {
            gs.characters[cn].attack_cn = co as u16;
            gs.characters[cn].goto_x = 0;
            gs.characters[cn].data[58] = 2;
            return true;
        }
    }

    let data_43 = gs.characters[cn].data[43];
    if data_43 != 0 {
        let co_group = gs.characters[co].data[42];
        let co_temp = gs.characters[co].temp;

        let mut found = false;
        for n in 43..47 {
            let data_n = gs.characters[cn].data[n];
            if data_n != 0 && co_group == data_n {
                found = true;
                break;
            }
            if data_n == 65536
                && ((co_flags & CharacterFlags::Player.bits()) != 0
                    || co_temp == CT_COMPANION as u16)
            {
                found = true;
                break;
            }
        }

        if !found {
            let mut should_attack = true;

            let data_95 = gs.characters[cn].data[95];
            let data_93 = gs.characters[cn].data[93];
            let data_29 = gs.characters[cn].data[29];

            if data_95 == 2 && data_93 != 0 {
                let rest_x = (data_29 % SERVER_MAPX) as i16;
                let rest_y = (data_29 / SERVER_MAPX) as i16;
                let dist =
                    std::cmp::max((rest_x - co_x).abs() as i32, (rest_y - co_y).abs() as i32);

                if dist > data_93 {
                    should_attack = false;
                }
            }

            if should_attack && npc_add_enemy(gs, cn, co, false) {
                let co_name = gs.characters[co].get_name().to_string();
                npc_saytext_n(gs, cn, 1, Some(&co_name));
                log::info!(
                    "Added {} to kill list because he's not in my group",
                    co_name
                );
                return true;
            }
        }
    }

    let data_95 = gs.characters[cn].data[95];
    let data_93 = gs.characters[cn].data[93];
    let data_27 = gs.characters[cn].data[27];
    let data_29 = gs.characters[cn].data[29];
    let data_94 = gs.characters[cn].data[94];

    if data_95 == 1
        && (co_flags & CharacterFlags::Player.bits()) != 0
        && ticker > data_27 + (TICKS * 120)
    {
        let x1 = co_x as i32;
        let x2 = data_29 % SERVER_MAPX;
        let y1 = co_y as i32;
        let y2 = data_29 / SERVER_MAPX;
        let dist = (x1 - x2).abs() + (y1 - y2).abs();

        if dist <= data_93 {
            if npc_add_enemy(gs, cn, co, false) {
                let co_name = gs.characters[co].get_name().to_string();
                npc_saytext_n(gs, cn, 1, Some(&co_name));
                log::info!(
                    "Added {} to kill list because he didn't say the password",
                    co_name
                );
                return true;
            }
        } else if dist <= data_93 * 2 && data_94 + (TICKS * 15) < ticker {
            npc_saytext_n(gs, cn, 8, None);
            gs.characters[cn].data[94] = ticker;
            return true;
        }
    }

    let attack_cn = gs.characters[cn].attack_cn;
    let data_37 = gs.characters[cn].data[37];
    let data_56 = gs.characters[cn].data[56];

    if attack_cn == 0
        && (co_flags & CharacterFlags::Player.bits()) != 0
        && data_37 != 0
        && indoor1 == indoor2
        && data_56 < ticker
    {
        let mut already_talked = false;
        for n in 37..41 {
            if gs.characters[cn].data[n] == co as i32 {
                already_talked = true;
                break;
            }
        }

        if !already_talked {
            let text_2 = gs.characters[cn].text[2];
            let text_2_str = c_string_to_str(&text_2).to_string();
            let co_name = gs.characters[co].get_name().to_string();

            let co_kindred = gs.characters[co].kindred as u32;
            let co_skill_19 = gs.characters[co].skill[19][0];

            if text_2_str == "#stunspec\0" || text_2_str.starts_with("#stunspec") {
                let message = if (co_kindred & (traits::KIN_TEMPLAR | traits::KIN_ARCHTEMPLAR)) != 0
                    || ((co_kindred & traits::KIN_SEYAN_DU) != 0 && co_skill_19 != 0)
                {
                    format!("Hello, {}. I'll teach you Immunity, if you bring me the potion from the Skeleton Lord.", co_name)
                } else {
                    format!(
                        "Hello, {}. I'll teach you Stun, if you bring me the potion from the Skeleton Lord.",
                        co_name
                    )
                };
                gs.do_sayx(cn, &message);
            } else if text_2_str == "#cursespec\0" || text_2_str.starts_with("#cursespec") {
                let message = if (co_kindred & (traits::KIN_TEMPLAR | traits::KIN_ARCHTEMPLAR)) != 0
                    || ((co_kindred & traits::KIN_SEYAN_DU) != 0 && co_skill_19 != 0)
                {
                    format!(
                        "Hi, {}. Bring me a Potion of Life and I'll teach you Surround Hit.",
                        co_name
                    )
                } else {
                    format!(
                        "Hi, {}. Bring me a Potion of Life and I'll teach you Curse.",
                        co_name
                    )
                };
                gs.do_sayx(cn, &message);
            } else {
                let cn_temp = gs.characters[cn].temp;
                if cn_temp == 180 && (co_kindred & traits::KIN_PURPLE) != 0 {
                    gs.do_sayx(cn, &format!("Greetings, {}!", co_name));
                } else {
                    npc_saytext_n(gs, cn, 2, Some(&co_name));
                }
            }

            gs.characters[cn].data[40] = gs.characters[cn].data[39];
            gs.characters[cn].data[39] = gs.characters[cn].data[38];
            gs.characters[cn].data[38] = gs.characters[cn].data[37];
            gs.characters[cn].data[37] = co as i32;
            gs.characters[cn].data[56] = ticker + (TICKS * 30);

            let data_26 = gs.characters[cn].data[26];
            if data_26 == 5 {
                let cnt = count_uniques(&gs.characters[co], &gs.items);

                if cnt == 1 {
                    gs.do_sayx(
                        cn,
                        &format!(
                            "I see you have a sword dedicated to the gods. Make good use of it, {}.\n",
                            co_name
                        ),
                    );
                } else if cnt > 1 {
                    gs.do_sayx(
                        cn,
                        &format!(
                            "I see you have several swords dedicated to the gods. They will get angry if you keep more than one, {}.\n",
                            co_name
                        ),
                    );
                }
            }
        }
    }

    false
}
