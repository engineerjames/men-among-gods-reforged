use super::item_damage_weapon;
use crate::effect::EffectManager;
use crate::game_state::GameState;
use crate::god::God;
use crate::helpers::{self};
use crate::{player, points, populate};
use core::constants::{CharacterFlags, ItemFlags, SERVER_MAPX, TICKS, USE_EMPTY};
use core::skills::{self};

/// Handles the legacy `use_crystal_sub` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `_cn` - Character index reserved for this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_crystal_sub` hook.
///
/// # Returns
///
/// * Value produced by the legacy `use_crystal_sub` implementation.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_crystal_sub` is outside the corresponding game-state collection.
pub fn use_crystal_sub(gs: &mut GameState, _cn: usize, item_idx: usize) -> i32 {
    // Get group id
    let group = gs.items[item_idx].data[0];

    // Count existing NPCs in the same group and bucket their base values
    let mut baseg = [0i32; 20];
    let mut cnt = 0i32;
    {
        for n in 1..core::constants::MAXCHARS {
            let ch = &gs.characters[n];
            if ch.used == core::constants::USE_ACTIVE
                && (ch.flags & CharacterFlags::Body.bits()) == 0
                && ch.data[42] == group as i32
            {
                let mut base = ch.data[0];
                if base > 99 {
                    base = 99;
                }
                let idx = (base / 5) as usize;
                if idx < baseg.len() {
                    baseg[idx] += 1;
                }
                cnt += 1;
            }
        }
    };

    // compute how many are still missing
    let need = gs.items[item_idx].data[1] as i32 - cnt;
    if need <= 0 {
        return 0;
    }

    // find smallest bucket
    let mut sbase = 0usize;
    let mut tmpmin = i32::MAX;
    for (i, &val) in baseg.iter().enumerate() {
        if val < tmpmin {
            tmpmin = val;
            sbase = i;
        }
    }

    log::info!(
        "Randoms: smallest base is {} with {} (miss={})",
        sbase,
        tmpmin,
        need
    );

    // choose a random template from temps
    let temps: [usize; 6] = [2, 4, 76, 78, 150, 151];
    let tmp = temps[helpers::random_mod_usize(temps.len())];

    // create char
    let cc_i32 = match God::create_char(gs, tmp, false) {
        Some(id) => id,
        None => return 0,
    };
    let cc = cc_i32 as usize;

    // attempt to drop at crystal position
    let (item_x, item_y) = (gs.items[item_idx].x, gs.items[item_idx].y);
    if !God::drop_char_fuzzy(gs, cc, item_x as usize, item_y as usize) {
        God::destroy_items(gs, cc);
        {
            gs.characters[cc].used = core::constants::USE_EMPTY;
        };
        return 0;
    }

    // set group
    {
        gs.characters[cc].data[42] = group as i32;
    };

    // pick spawn tile until valid
    let m = loop {
        let m_try = (helpers::random_mod_usize(64) + 128)
            + (helpers::random_mod_usize(64) + 64) * core::constants::SERVER_MAPX as usize;
        if player::commands::plr_check_target(gs, m_try) {
            break m_try;
        }
    };

    // configure the character
    {
        let ch = &mut gs.characters[cc];
        ch.goto_x = (m % core::constants::SERVER_MAPX as usize) as u16;
        ch.goto_y = (m / core::constants::SERVER_MAPX as usize) as u16;
        ch.data[60] = TICKS * 20;
        ch.data[62] = 1;

        // texts (format strings)
        let t0 = b"Yes! Die, %s!";
        ch.text[0][..t0.len()].copy_from_slice(t0);
        ch.text[0][t0.len()..].fill(0);
        let t1 = b"Yahoo! An enemy! Prepare to die, %s!";
        ch.text[1][..t1.len()].copy_from_slice(t1);
        ch.text[1][t1.len()..].fill(0);
        let t3 = b"Thank you %s! Everything is better than being here.";
        ch.text[3][..t3.len()].copy_from_slice(t3);
        ch.text[3][t3.len()..].fill(0);
        ch.data[48] = 33;

        // base and attributes
        let base = (sbase * 5) as i32 + helpers::random_mod_i32(5);
        ch.data[0] = base;

        for n in 0..5 {
            let mut t = base + helpers::random_mod_i32(15);
            let diff = std::cmp::max(1, i32::from(ch.attrib[n][3]));
            t = t * 3 / diff;
            let maxv = i32::from(ch.attrib[n][2]);
            let v = std::cmp::max(10, std::cmp::min(maxv, t));
            ch.attrib[n][0] = v as u16;
        }

        for n in 0..core::skills::MAX_SKILLS {
            let mut t = base + helpers::random_mod_i32(15);
            let diff = std::cmp::max(1, i32::from(ch.skill[n][3]));
            t = t * 3 / diff;
            if ch.skill[n][2] != 0 {
                let maxv = i32::from(ch.skill[n][2]);
                ch.skill[n][0] = std::cmp::min(maxv, t) as u16;
            }
        }

        ch.hp[0] = std::cmp::max(
            50,
            std::cmp::min(i32::from(ch.hp[2]), base * 5 + helpers::random_mod_i32(50)),
        ) as u16;
        ch.end[0] = std::cmp::max(
            50,
            std::cmp::min(i32::from(ch.end[2]), base * 5 + helpers::random_mod_i32(50)),
        ) as u16;
        ch.mana[0] = std::cmp::max(
            50,
            std::cmp::min(
                i32::from(ch.mana[2]),
                base * 5 + helpers::random_mod_i32(50),
            ),
        ) as u16;

        // calculate experience points
        let mut pts = 0i32;
        for z in 0..5 {
            for m in 10..i32::from(ch.attrib[z][0]) {
                pts += points::attrib_needed(m, 3);
            }
        }
        for m in 50..i32::from(ch.hp[0]) {
            pts += points::hp_needed(m, 3);
        }
        for m in 50..i32::from(ch.end[0]) {
            pts += points::end_needed(m, 2);
        }
        for m in 50..i32::from(ch.mana[0]) {
            pts += points::mana_needed(m, 3);
        }
        for z in 0..core::skills::MAX_SKILLS {
            for m in 1..i32::from(ch.skill[z][0]) {
                pts += points::skill_needed(m, 2);
            }
        }

        ch.points_tot = pts;
        ch.gold = base * base + 1;
        ch.a_hp = 999999;
        ch.a_end = 999999;
        if ch.skill[skills::SK_MEDIT][0] > 0 {
            ch.a_mana = 1000000;
        } else {
            let mut v = 1i32;
            for _ in 0..6 {
                v *= helpers::random_mod_i32(4);
            }
            ch.a_mana = v * 100;
        }

        ch.alignment = -(helpers::random_mod_i32(7500) as i16);
    };

    log::info!("Created random dungeon NPC from crystal (template {})", tmp);

    // Equip character based on attributes/skills
    {
        {
            let _ch = gs.characters[cc];
        };

        {
            if i32::from(gs.characters[cc].attrib[core::constants::AT_AGIL as usize][0]) >= 90
                && i32::from(gs.characters[cc].attrib[core::constants::AT_STREN as usize][0]) >= 90
            {
                let tmp = populate::pop_create_item(gs, 94, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_HEAD] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 95, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_BODY] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 98, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_ARMS] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 97, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_LEGS] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 99, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_FEET] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 96, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_CLOAK] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
            } else if i32::from(gs.characters[cc].attrib[core::constants::AT_AGIL as usize][0])
                >= 72
                && i32::from(gs.characters[cc].attrib[core::constants::AT_STREN as usize][0]) >= 72
            {
                let tmp = populate::pop_create_item(gs, 75, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_HEAD] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 76, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_BODY] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 79, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_ARMS] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 78, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_LEGS] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 80, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_FEET] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 77, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_CLOAK] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
            } else if i32::from(gs.characters[cc].attrib[core::constants::AT_AGIL as usize][0])
                >= 40
                && i32::from(gs.characters[cc].attrib[core::constants::AT_STREN as usize][0]) >= 40
            {
                let tmp = populate::pop_create_item(gs, 69, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_HEAD] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 71, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_BODY] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 73, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_ARMS] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 72, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_LEGS] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 74, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_FEET] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 70, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_CLOAK] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
            } else if i32::from(gs.characters[cc].attrib[core::constants::AT_AGIL as usize][0])
                >= 24
                && i32::from(gs.characters[cc].attrib[core::constants::AT_STREN as usize][0]) >= 24
            {
                let tmp = populate::pop_create_item(gs, 63, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_HEAD] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 65, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_BODY] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 67, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_ARMS] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 66, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_LEGS] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 68, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_FEET] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 64, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_CLOAK] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
            } else if i32::from(gs.characters[cc].attrib[core::constants::AT_AGIL as usize][0])
                >= 16
                && i32::from(gs.characters[cc].attrib[core::constants::AT_STREN as usize][0]) >= 16
            {
                let tmp = populate::pop_create_item(gs, 57, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_HEAD] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 59, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_BODY] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 61, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_ARMS] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 60, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_LEGS] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 62, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_FEET] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 58, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_CLOAK] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
            } else if i32::from(gs.characters[cc].attrib[core::constants::AT_AGIL as usize][0])
                >= 12
                && i32::from(gs.characters[cc].attrib[core::constants::AT_STREN as usize][0]) >= 12
            {
                let tmp = populate::pop_create_item(gs, 51, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_HEAD] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 53, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_BODY] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 55, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_ARMS] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 54, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_LEGS] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 56, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_FEET] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 52, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_CLOAK] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
            } else if i32::from(gs.characters[cc].attrib[core::constants::AT_AGIL as usize][0])
                >= 10
                && i32::from(gs.characters[cc].attrib[core::constants::AT_STREN as usize][0]) >= 10
            {
                let tmp = populate::pop_create_item(gs, 39, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_HEAD] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 42, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_BODY] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 44, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_ARMS] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 43, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_LEGS] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 41, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_FEET] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
                let tmp = populate::pop_create_item(gs, 40, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_CLOAK] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
            }

            // choose weapon based on skills/attribs (partial port of original logic)
            helpers::sync_weapon_skill(&mut gs.characters[cc].skill);
            if i32::from(gs.characters[cc].skill[skills::SK_WEAPON][0]) >= 60
                && i32::from(gs.characters[cc].attrib[core::constants::AT_AGIL as usize][0]) >= 50
                && i32::from(gs.characters[cc].attrib[core::constants::AT_STREN as usize][0]) >= 75
            {
                let tmp = populate::pop_create_item(gs, 125, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_RHAND] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
            } else if i32::from(gs.characters[cc].skill[skills::SK_WEAPON][0]) >= 45
                && i32::from(gs.characters[cc].attrib[core::constants::AT_AGIL as usize][0]) >= 40
                && i32::from(gs.characters[cc].attrib[core::constants::AT_STREN as usize][0]) >= 60
            {
                let tmp = populate::pop_create_item(gs, 38, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_RHAND] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
            } else if i32::from(gs.characters[cc].skill[skills::SK_WEAPON][0]) >= 30
                && i32::from(gs.characters[cc].attrib[core::constants::AT_AGIL as usize][0]) >= 30
                && i32::from(gs.characters[cc].attrib[core::constants::AT_STREN as usize][0]) >= 40
            {
                let tmp = populate::pop_create_item(gs, 37, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_RHAND] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
            } else if i32::from(gs.characters[cc].skill[skills::SK_WEAPON][0]) >= 15
                && i32::from(gs.characters[cc].attrib[core::constants::AT_AGIL as usize][0]) >= 20
                && i32::from(gs.characters[cc].attrib[core::constants::AT_STREN as usize][0]) >= 24
            {
                let tmp = populate::pop_create_item(gs, 36, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_RHAND] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
            } else if i32::from(gs.characters[cc].skill[skills::SK_WEAPON][0]) >= 1
                && i32::from(gs.characters[cc].attrib[core::constants::AT_AGIL as usize][0]) >= 10
                && i32::from(gs.characters[cc].attrib[core::constants::AT_STREN as usize][0]) >= 12
            {
                let tmp = populate::pop_create_item(gs, 35, cc);
                if tmp != 0 {
                    gs.characters[cc].worn[core::constants::WN_RHAND] = tmp as u32;
                    gs.items[tmp].carried = cc as u16;
                }
            }
        };
    }

    // occasional extra items (partial port)
    if helpers::random_mod_i32(30) == 0
        && gs.characters[cc].data[0] > 5
        && let Some(it) = God::create_item(gs, helpers::random_mod_usize(2) + 273)
    {
        God::give_character_item(gs, cc, it);
    }
    if helpers::random_mod_i32(60) == 0
        && gs.characters[cc].data[0] > 15
        && let Some(it) = God::create_item(gs, helpers::random_mod_usize(2) + 192)
    {
        God::give_character_item(gs, cc, it);
    }
    if helpers::random_mod_i32(150) == 0
        && gs.characters[cc].data[0] > 20
        && let Some(it) = God::create_item(gs, helpers::random_mod_usize(9) + 181)
    {
        God::give_character_item(gs, cc, it);
    }

    // update character state
    gs.do_update_char(cc);

    need
}

/// Handles the legacy `use_crystal` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_crystal` hook.
///
/// # Returns
///
/// * `true` when `use_crystal` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_crystal` is outside the corresponding game-state collection.
pub fn use_crystal(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    let mut cnt = 0;

    // Call use_crystal_sub until it returns <= 4, up to 4 times
    while use_crystal_sub(gs, cn, item_idx) > 4 && cnt < 4 {
        cnt += 1;
    }

    cnt == 0
}

/// Handles the legacy `use_mine_respawn` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `_cn` - Character index reserved for this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_mine_respawn` hook.
///
/// # Returns
///
/// * `true` when `use_mine_respawn` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_mine_respawn` is outside the corresponding game-state collection.
pub fn use_mine_respawn(gs: &mut GameState, _cn: usize, item_idx: usize) -> bool {
    // Get group, template, and max count from item data
    let (group, template, max_cnt) = {
        let item = &gs.items[item_idx];
        (
            item.data[0] as i32,
            item.data[1] as usize,
            item.data[2] as i32,
        )
    };

    // Check if mine wall items exist (data[3-9])
    for n in 3..10 {
        let map_idx = gs.items[item_idx].data[n] as usize;
        if map_idx == 0 {
            break;
        }

        // Check if there's a mine wall item at that location
        let in2 = gs.map[map_idx].it as usize;
        if in2 == 0 {
            return false;
        }

        let driver = gs.items[in2].driver;
        if driver != 26 {
            return false;
        }
    }

    // Count active NPCs in this group
    let cnt = {
        let mut count = 0;
        for n in 1..core::constants::MAXCHARS {
            if gs.characters[n].used == core::constants::USE_ACTIVE
                && (gs.characters[n].flags & CharacterFlags::Body.bits()) == 0
                && gs.characters[n].data[42] == group
            {
                count += 1;
            }
        }
        count
    };

    // Don't spawn if too many NPCs already
    if cnt > max_cnt {
        return false;
    }

    // create the NPC from template
    let cc = match populate::pop_create_char(gs, template, false) {
        Some(cc) => cc,
        None => return false,
    };

    // drop the character near the mine item
    let (item_x, item_y) = (gs.items[item_idx].x, gs.items[item_idx].y);
    if !God::drop_char_fuzzy(gs, cc, item_x as usize, item_y as usize) {
        log::warn!("use_mine_respawn ({},{}): drop_char failed", item_x, item_y);
        // cleanup on failure
        God::destroy_items(gs, cc);
        {
            gs.characters[cc].used = core::constants::USE_EMPTY;
        };
        return false;
    }

    // ensure the new character is visible/updated
    gs.do_update_char(cc);

    true
}

/// Handles the legacy `mine_wall` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `mine_wall` hook.
///
/// # Returns
///
/// * Value produced by the legacy `mine_wall` implementation.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `mine_wall` is outside the corresponding game-state collection.
pub fn mine_wall(gs: &mut GameState, cn: usize, item_idx: usize) -> i32 {
    // If no item provided, get it from the map
    let in_idx = if item_idx == 0 {
        let (x, y) = {
            if cn == 0 {
                (0, 0)
            } else {
                (gs.characters[cn].x as usize, gs.characters[cn].y as usize)
            }
        };
        let m = x + y * core::constants::SERVER_MAPX as usize;
        let map_item = gs.map[m].it;
        if map_item == 0 {
            return 0;
        }
        map_item as usize
    } else {
        item_idx
    };

    // Get original template, position, and carried status
    let (temp, item_x, item_y, carried, should_rebuild) = {
        (
            gs.items[in_idx].data[0] as usize,
            i32::from(gs.items[in_idx].x),
            i32::from(gs.items[in_idx].y),
            gs.items[in_idx].carried,
            gs.items[in_idx].data[3] != 0,
        )
    };

    // Add rebuild wall effect if data[3] is set
    if should_rebuild {
        // Use the template id as the effect parameter (matches original server behavior)
        let temp_id = gs.items[in_idx].temp;
        EffectManager::fx_add_effect(
            gs,
            10,
            core::constants::TICKS * 60 * 15,
            item_x,
            item_y,
            i32::from(temp_id),
        );
        log::info!("mine_wall: added rebuild effect (temp={})", temp_id);
    }

    // Replace the item with a copy of the item template (it_temp[temp]) and
    // restore position/carried/temp fields (this mirrors the original C++ behavior).
    let template_copy = gs.item_templates[temp];
    {
        gs.items[in_idx] = template_copy;
        gs.items[in_idx].x = item_x as u16;
        gs.items[in_idx].y = item_y as u16;
        gs.items[in_idx].carried = carried;
        gs.items[in_idx].temp = temp as u16;
        if carried != 0 {
            gs.items[in_idx].flags |= ItemFlags::IF_UPDATE.bits();
        }
    };

    gs.items[in_idx].data[2] as i32
}

/// Handles the legacy `use_mine` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_mine` hook.
///
/// # Returns
///
/// * `true` when `use_mine` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_mine` is outside the corresponding game-state collection.
pub fn use_mine(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    use core::constants::{AT_STREN, WN_RHAND};

    // Get character strength
    let mut str = { i32::from(gs.characters[cn].attrib[AT_STREN as usize][5]) };

    // Check and subtract endurance
    let insufficient_endurance = {
        if gs.characters[cn].a_end < 1500 {
            true
        } else {
            gs.characters[cn].a_end -= 1000;
            false
        }
    };

    if insufficient_endurance {
        gs.do_character_log(
            cn,
            core::types::FontColor::Green,
            "You're too exhausted to continue digging.\n",
        );
        {
            gs.characters[cn].misc_action = 0; // DR_IDLE
        };
        return false;
    }

    // Check for proper tools in right hand
    let (has_pickaxe, has_weapon) = {
        let in2 = gs.characters[cn].worn[WN_RHAND] as usize;
        if in2 != 0 {
            {
                let temp = gs.items[in2].temp;
                (temp == 458, true) // 458 is pickaxe
            }
        } else {
            (false, false)
        }
    };

    if has_weapon {
        if has_pickaxe {
            item_damage_weapon(gs, cn, str / 10);
            str *= 2;
        } else {
            item_damage_weapon(gs, cn, str * 10);
            str /= 4;
        }
        GameState::char_play_sound(gs, cn, 11, -150, 0);
        gs.do_area_sound(
            cn,
            0,
            i32::from(gs.characters[cn].x),
            i32::from(gs.characters[cn].y),
            11,
        );
    } else {
        str /= 10;
        let low_health = {
            if gs.characters[cn].a_hp < 10000 {
                true
            } else {
                gs.characters[cn].a_hp -= 500;
                false
            }
        };

        if low_health {
            gs.do_character_log(
                cn,
                core::types::FontColor::Green,
                "You don't want to kill yourself beating at this wall with your bare hands, so you stop.\n",
            );
            {
                gs.characters[cn].misc_action = 0; // DR_IDLE
            };
            return false;
        }
    }

    // Apply damage to mine wall
    let tmp = {
        let current = gs.items[item_idx].data[1] as i32;
        let new_val = current - str;
        if new_val > 0 {
            gs.items[item_idx].data[1] = new_val as u32;
        }
        new_val
    };

    if tmp <= 0 {
        // Wall destroyed
        let (x, y) = (gs.items[item_idx].x, gs.items[item_idx].y);
        gs.reset_go(i32::from(x), i32::from(y));
        gs.remove_lights(i32::from(x), i32::from(y));

        let _result = mine_wall(gs, cn, item_idx);

        gs.reset_go(i32::from(x), i32::from(y));
        gs.add_lights(i32::from(x), i32::from(y));
    }

    false
}

/// Handles the legacy `use_mine_fast` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_mine_fast` hook.
///
/// # Returns
///
/// * `true` when `use_mine_fast` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_mine_fast` is outside the corresponding game-state collection.
pub fn use_mine_fast(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    if cn == 0 {
        return false;
    }

    let carried = gs.items[item_idx].carried;
    if carried != 0 {
        return false;
    }

    // Get item position and template
    let (x, y, temp) = {
        (
            gs.items[item_idx].x,
            gs.items[item_idx].y,
            gs.items[item_idx].temp,
        )
    };

    EffectManager::fx_add_effect(
        gs,
        10,
        core::constants::TICKS * 60 * 15,
        i32::from(x),
        i32::from(y),
        i32::from(temp),
    );

    gs.reset_go(i32::from(x), i32::from(y));
    gs.remove_lights(i32::from(x), i32::from(y));

    // Remove item from map
    {
        gs.map[(x + y * SERVER_MAPX as u16) as usize].it = 0;
    };

    {
        gs.items[item_idx].used = USE_EMPTY;
    };

    gs.reset_go(i32::from(x), i32::from(y));
    gs.add_lights(i32::from(x), i32::from(y));

    true
}
