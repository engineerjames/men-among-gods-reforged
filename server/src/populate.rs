use core::{
    constants::{
        AT_AGIL, AT_BRAVE, AT_INT, AT_STREN, AT_WILL, DX_DOWN, MAXCHARS, MAXEFFECT, MAXITEM,
        MAXTCHARS, MAXTITEM, MF_INDOORS, MF_MOVEBLOCK, MF_SIGHTBLOCK, SERVER_MAPX, SERVER_MAPY,
        TICKS, USE_ACTIVE, USE_EMPTY,
    },
    skills,
};

use {core::constants::CharacterFlags, core::constants::ItemFlags};

use crate::{
    driver::{self, use_item},
    effect::EffectManager,
    game_state::GameState,
    god::God,
    helpers, player,
};

/// Port of `init_lights` from `populate.cpp`
/// Initialize lighting on the map
pub fn init_lights(gs: &mut GameState) {
    let mut cnt1 = 0;
    let mut cnt2 = 0;

    // First pass: clear all light and dlight values
    for y in 0..SERVER_MAPY as usize {
        for x in 0..SERVER_MAPX as usize {
            let m = x + y * SERVER_MAPX as usize;
            gs.map[m].light = 0;
            gs.map[m].dlight = 0;
        }
    }

    // Second pass: compute dlight for indoor tiles, then add lights from items
    for y in 0..SERVER_MAPY as usize {
        for x in 0..SERVER_MAPX as usize {
            let m = x + y * SERVER_MAPX as usize;

            // Compute daylight for indoor tiles first
            let is_indoors = gs.map[m].flags & MF_INDOORS as u64 != 0;

            if is_indoors {
                gs.compute_dlight(x as i32, y as i32);
                cnt2 += 1;
            }

            // Then add light from items
            let in_id = gs.map[m].it;

            if in_id == 0 {
                continue;
            }

            let active = gs.items[in_id as usize].active;
            let light_active = gs.items[in_id as usize].light[1];
            let light_inactive = gs.items[in_id as usize].light[0];

            if active != 0 {
                if light_active != 0 {
                    gs.do_add_light(x as i32, y as i32, light_active as i32);
                    cnt1 += 1;
                }
            } else {
                if light_inactive != 0 {
                    gs.do_add_light(x as i32, y as i32, light_inactive as i32);
                    cnt1 += 1;
                }
            }
        }
    }

    log::info!("Initialized lights: {} items, {} indoor tiles", cnt1, cnt2);
}

/// Create an item for a character using an explicit game-state borrow.
///
/// # Arguments
/// * `gs` - Mutable game state.
/// * `temp` - Item template id.
/// * `cn` - Character receiving or influencing the item roll.
pub fn pop_create_item(gs: &mut GameState, temp: usize, cn: usize) -> usize {
    let mut in_id = 0;
    let alignment = gs.characters[cn].alignment;

    // First check: Gorn uniques (1/150 chance)
    if in_id == 0 && alignment < 0 && helpers::random_mod(150) == 0 {
        in_id = match temp {
            27 => God::create_item(gs, 542),  // bronze dagger
            28 => God::create_item(gs, 543),  // steel dagger
            29 => God::create_item(gs, 544),  // gold dagger
            30 => God::create_item(gs, 545),  // crystal dagger
            523 => God::create_item(gs, 546), // titan dagger
            31 => God::create_item(gs, 547),  // bronze sword
            32 => God::create_item(gs, 548),  // steel sword
            33 => God::create_item(gs, 549),  // gold sword
            34 => God::create_item(gs, 550),  // crystal sword
            524 => God::create_item(gs, 551), // titan sword
            35 => God::create_item(gs, 552),  // bronze two
            36 => God::create_item(gs, 553),  // steel two
            37 => God::create_item(gs, 554),  // gold two
            38 => God::create_item(gs, 555),  // crystal two
            125 => God::create_item(gs, 556), // titan two
            _ => None,
        }
        .unwrap_or(0);
    }

    // Second check: Kwai uniques (1/150 chance)
    if in_id == 0 && alignment < 0 && helpers::random_mod(150) == 0 {
        in_id = match temp {
            27 => God::create_item(gs, 527),  // bronze dagger
            28 => God::create_item(gs, 528),  // steel dagger
            29 => God::create_item(gs, 529),  // gold dagger
            30 => God::create_item(gs, 530),  // crystal dagger
            523 => God::create_item(gs, 531), // titan dagger
            31 => God::create_item(gs, 532),  // bronze sword
            32 => God::create_item(gs, 533),  // steel sword
            33 => God::create_item(gs, 534),  // gold sword
            34 => God::create_item(gs, 535),  // crystal sword
            524 => God::create_item(gs, 536), // titan sword
            35 => God::create_item(gs, 537),  // bronze two
            36 => God::create_item(gs, 538),  // steel two
            37 => God::create_item(gs, 539),  // gold two
            38 => God::create_item(gs, 540),  // crystal two
            125 => God::create_item(gs, 541), // titan two
            _ => None,
        }
        .unwrap_or(0);
    }

    // Third check: Purple One uniques (1/150 chance)
    if in_id == 0 && alignment < 0 && helpers::random_mod(150) == 0 {
        in_id = match temp {
            27 => God::create_item(gs, 572),  // bronze dagger
            28 => God::create_item(gs, 573),  // steel dagger
            29 => God::create_item(gs, 574),  // gold dagger
            30 => God::create_item(gs, 575),  // crystal dagger
            523 => God::create_item(gs, 576), // titan dagger
            31 => God::create_item(gs, 577),  // bronze sword
            32 => God::create_item(gs, 578),  // steel sword
            33 => God::create_item(gs, 579),  // gold sword
            34 => God::create_item(gs, 580),  // crystal sword
            524 => God::create_item(gs, 581), // titan sword
            35 => God::create_item(gs, 582),  // bronze two
            36 => God::create_item(gs, 583),  // steel two
            37 => God::create_item(gs, 584),  // gold two
            38 => God::create_item(gs, 585),  // crystal two
            125 => God::create_item(gs, 586), // titan two
            _ => None,
        }
        .unwrap_or(0);
    }

    // Fourth check: Skua uniques (1/150 chance)
    if in_id == 0 && alignment < 0 && helpers::random_mod(150) == 0 {
        in_id = match temp {
            27 => God::create_item(gs, 280),  // bronze dagger
            28 => God::create_item(gs, 281),  // steel dagger
            29 => God::create_item(gs, 282),  // gold dagger
            30 => God::create_item(gs, 283),  // crystal dagger
            523 => God::create_item(gs, 525), // titan dagger
            31 => God::create_item(gs, 284),  // bronze sword
            32 => God::create_item(gs, 285),  // steel sword
            33 => God::create_item(gs, 286),  // gold sword
            34 => God::create_item(gs, 287),  // crystal sword
            524 => God::create_item(gs, 526), // titan sword
            35 => God::create_item(gs, 288),  // bronze two
            36 => God::create_item(gs, 289),  // steel two
            37 => God::create_item(gs, 290),  // gold two
            38 => God::create_item(gs, 291),  // crystal two
            125 => God::create_item(gs, 292), // titan two
            _ => None,
        }
        .unwrap_or(0);
    }

    // Default: create item from template
    if in_id == 0 {
        in_id = God::create_item(gs, temp).unwrap_or(0);

        // Apply item damage for regular items
        if in_id != 0 {
            let max_damage = gs.items[in_id].max_damage;
            if max_damage > 0 {
                // 50% chance to age the item first
                if helpers::random_mod(2) == 0 {
                    gs.items[in_id].current_damage = max_damage + 1;
                    use_item::item_age(gs, in_id);
                }
                // Set random damage
                gs.items[in_id].current_damage = helpers::random_mod(max_damage);
            }
        }
    } else {
        let char_name = gs.characters[cn].get_name().to_string();
        let item_name = gs.items[in_id].get_name().to_string();
        log::info!("{} got unique item {}.", char_name, item_name);
    }

    in_id
}

/// Port of `pop_create_bonus` from `populate.c`
/// Creates bonus items based on character rank (points_tot)
/// Create a bonus item using an explicit game-state borrow.
///
/// # Arguments
/// * `gs` - Mutable game state.
/// * `cn` - Character receiving the bonus.
/// * `_chance` - Legacy chance parameter retained for parity.
pub fn pop_create_bonus(gs: &mut GameState, cn: usize, _chance: i32) -> i32 {
    let points_tot = gs.characters[cn].points_tot;

    let template = if points_tot > 20000000 {
        // Very high rank items
        const GOOD_ITEMS: [usize; 2] = [273, 274];
        const GREAT_ITEMS: [usize; 32] = [
            273, 274, 693, 273, 274, 694, 273, 274, 695, 273, 274, 696, 273, 274, 697, 273, 274,
            698, 361, 360, 487, 361, 360, 488, 361, 360, 489, 337, 361, 292, 525, 526,
        ];

        if helpers::random_mod(5) != 0 {
            GOOD_ITEMS[helpers::random_mod_usize(GOOD_ITEMS.len())]
        } else {
            GREAT_ITEMS[helpers::random_mod_usize(GREAT_ITEMS.len())]
        }
    } else if points_tot > 1500000 {
        // High rank items
        const GOOD_ITEMS: [usize; 2] = [273, 274];
        const GREAT_ITEMS: [usize; 31] = [
            273, 274, 699, 273, 274, 700, 273, 274, 701, 273, 274, 702, 273, 274, 703, 273, 274,
            704, 361, 360, 347, 361, 360, 344, 361, 360, 341, 337, 283, 287, 291,
        ];

        if helpers::random_mod(5) != 0 {
            GOOD_ITEMS[helpers::random_mod_usize(GOOD_ITEMS.len())]
        } else {
            GREAT_ITEMS[helpers::random_mod_usize(GREAT_ITEMS.len())]
        }
    } else if points_tot > 125000 {
        // Medium rank items
        const GOOD_ITEMS: [usize; 2] = [101, 102];
        const GREAT_ITEMS: [usize; 29] = [
            101, 102, 705, 101, 102, 706, 101, 102, 707, 101, 102, 708, 101, 102, 709, 101, 102,
            710, 360, 338, 361, 340, 361, 343, 361, 346, 282, 286, 290,
        ];

        if helpers::random_mod(5) != 0 {
            GOOD_ITEMS[helpers::random_mod_usize(GOOD_ITEMS.len())]
        } else {
            GREAT_ITEMS[helpers::random_mod_usize(GREAT_ITEMS.len())]
        }
    } else if points_tot > 11250 {
        // Low rank items
        const GOOD_ITEMS: [usize; 3] = [18, 46, 100];
        const GREAT_ITEMS: [usize; 29] = [
            18, 46, 711, 18, 46, 712, 18, 46, 713, 18, 46, 714, 18, 46, 715, 18, 46, 716, 360, 338,
            361, 339, 361, 342, 361, 345, 281, 285, 289,
        ];

        if helpers::random_mod(5) != 0 {
            GOOD_ITEMS[helpers::random_mod_usize(GOOD_ITEMS.len())]
        } else {
            GREAT_ITEMS[helpers::random_mod_usize(GREAT_ITEMS.len())]
        }
    } else {
        // Lowest rank items
        const GOOD_ITEMS: [usize; 3] = [18, 46, 100];
        const GREAT_ITEMS: [usize; 15] = [
            18, 46, 361, 348, 18, 46, 351, 18, 46, 354, 361, 338, 280, 284, 288,
        ];

        if helpers::random_mod(5) != 0 {
            GOOD_ITEMS[helpers::random_mod_usize(GOOD_ITEMS.len())]
        } else {
            GREAT_ITEMS[helpers::random_mod_usize(GREAT_ITEMS.len())]
        }
    };

    let in_id = God::create_item(gs, template);

    if let Some(in_id) = in_id {
        let char_name = gs.characters[cn].get_name().to_string();
        let item_name = gs.items[in_id].get_name().to_string();
        log::info!("{} got {} (template={})", char_name, item_name, template);
        in_id as i32
    } else {
        0
    }
}

/// Port of `pop_create_bonus_belt` from `populate.cpp`
/// Creates special rainbow belts with random skills
/// Create a randomized rainbow belt using an explicit game-state borrow.
///
/// # Arguments
/// * `gs` - Mutable game state.
/// * `cn` - Character receiving the belt.
pub fn pop_create_bonus_belt(gs: &mut GameState, cn: usize) -> i32 {
    let points_tot = gs.characters[cn].points_tot;

    // Calculate rank (from points2rank - needs to be implemented elsewhere)
    let rank = if points_tot < 1000 {
        0
    } else {
        ((points_tot as f64).ln() / 10.0) as i32
    };

    if rank == 0 {
        return 0;
    }

    let in_id = God::create_item(gs, 1106); // Rainbow belt template
    if in_id.is_none() {
        return 0;
    }
    let in_id = in_id.unwrap();

    // Customize the belt item (clear template and set sprite/name/description)
    {
        let item = &mut gs.items[in_id];
        item.temp = 0; // Clear template
        item.sprite[0] = 16964;
        let name_bytes = b"Rainbow Belt";
        item.name[..name_bytes.len()].copy_from_slice(name_bytes);
        item.name[name_bytes.len()..].fill(0);
        let desc_bytes = b"An ancient belt. It seems to be highly magical";
        item.description[..desc_bytes.len()].copy_from_slice(desc_bytes);
        item.description[desc_bytes.len()..].fill(0);
        let ref_bytes = b"rainbow belt";
        item.reference[..ref_bytes.len()].copy_from_slice(ref_bytes);
        item.reference[ref_bytes.len()..].fill(0);
    }

    log::info!(
        "Character {} with rank {} got Rainbow Belt (t={})",
        cn,
        rank,
        0
    );

    let mut num_skills = helpers::random_mod(rank as u32);
    if num_skills == 0 {
        num_skills = 1; // Ensure at least 1 skill
    }

    // Update item properties
    {
        let item = &mut gs.items[in_id];
        item.power += 5 * num_skills;
        item.value += 10000 * num_skills;
    }

    // Add random skills to belt
    for _ in 0..num_skills {
        let skill_number = helpers::random_mod(40); // 0-39
        let mut skill_value = helpers::random_mod(rank as u32);
        skill_value >>= 1; // Divide by 2, max is rank/2 (max 12)
        if skill_value == 0 {
            skill_value = 1; // Ensure at least 1
        }

        {
            let item = &mut gs.items[in_id];
            match skill_number {
                // Attributes
                0 => {
                    // Bravery (AT_BRAVE)
                    item.attrib[AT_BRAVE as usize][0] += skill_value as i8;
                    if item.attrib[AT_BRAVE as usize][0] > 12 {
                        item.attrib[AT_BRAVE as usize][0] = 12;
                    }
                    item.attrib[AT_BRAVE as usize][2] = (10
                        + (item.attrib[AT_BRAVE as usize][0] as u32 * helpers::random_mod(7)))
                        as i8;
                }
                1 => {
                    // Willpower (AT_WILL)
                    item.attrib[AT_WILL as usize][0] += skill_value as i8;
                    if item.attrib[AT_WILL as usize][0] > 12 {
                        item.attrib[AT_WILL as usize][0] = 12;
                    }
                    item.attrib[AT_WILL as usize][2] = (10
                        + (item.attrib[AT_WILL as usize][0] as u32 * helpers::random_mod(7)))
                        as i8;
                }
                2 => {
                    // Intuition (AT_INT)
                    item.attrib[AT_INT as usize][0] += skill_value as i8;
                    if item.attrib[AT_INT as usize][0] > 12 {
                        item.attrib[AT_INT as usize][0] = 12;
                    }
                    item.attrib[AT_INT as usize][2] = (10
                        + (item.attrib[AT_INT as usize][0] as u32 * helpers::random_mod(7)))
                        as i8;
                }
                3 => {
                    // Agility (AT_AGIL)
                    item.attrib[AT_AGIL as usize][0] += skill_value as i8;
                    if item.attrib[AT_AGIL as usize][0] > 12 {
                        item.attrib[AT_AGIL as usize][0] = 12;
                    }
                    item.attrib[AT_AGIL as usize][2] = (10
                        + (item.attrib[AT_AGIL as usize][0] as u32 * helpers::random_mod(7)))
                        as i8;
                }
                4 => {
                    // Strength (AT_STREN)
                    item.attrib[AT_STREN as usize][0] += skill_value as i8;
                    if item.attrib[AT_STREN as usize][0] > 12 {
                        item.attrib[AT_STREN as usize][0] = 12;
                    }
                    item.attrib[AT_STREN as usize][2] = (10
                        + (item.attrib[AT_STREN as usize][0] as u32 * helpers::random_mod(7)))
                        as i8;
                }
                // HP
                5 => {
                    item.hp[0] += (skill_value * 5) as i16;
                    if item.hp[0] > 60 {
                        item.hp[0] = 60;
                    }
                    item.hp[2] = (50 + (item.hp[0] as u32 * helpers::random_mod(7))) as i16;
                }
                // Endurance
                6 => {
                    item.end[0] += (skill_value * 5) as i16;
                    if item.end[0] > 60 {
                        item.end[0] = 60;
                    }
                    item.end[2] = (50 + (item.end[0] as u32 * helpers::random_mod(7))) as i16;
                }
                // Mana
                7 => {
                    item.mana[0] += (skill_value * 5) as i16;
                    if item.mana[0] > 60 {
                        item.mana[0] = 60;
                    }
                    item.mana[2] = (50 + (item.mana[0] as u32 * helpers::random_mod(7))) as i16;
                }
                // Armor
                8 => {
                    item.armor[0] += skill_value as i8;
                    if item.armor[0] > 12 {
                        item.armor[0] = 12;
                    }
                }
                // Warcry
                9 => {
                    item.skill[skills::SK_WARCRY][0] += skill_value as i8;
                    if item.skill[skills::SK_WARCRY][0] > 12 {
                        item.skill[skills::SK_WARCRY][0] = 12;
                    }
                }
                // Hand to Hand
                10 => {
                    item.skill[skills::SK_HAND][0] += skill_value as i8;
                    if item.skill[skills::SK_HAND][0] > 12 {
                        item.skill[skills::SK_HAND][0] = 12;
                    }
                    item.skill[skills::SK_HAND][2] =
                        (item.skill[skills::SK_HAND][0] as u32 * helpers::random_mod(7)) as i8;
                }
                // Sword
                11 => {
                    item.skill[skills::SK_SWORD][0] += skill_value as i8;
                    if item.skill[skills::SK_SWORD][0] > 12 {
                        item.skill[skills::SK_SWORD][0] = 12;
                    }
                }
                // Dagger
                12 => {
                    item.skill[skills::SK_DAGGER][0] += skill_value as i8;
                    if item.skill[skills::SK_DAGGER][0] > 12 {
                        item.skill[skills::SK_DAGGER][0] = 12;
                    }
                }
                // Two-Handed
                13 => {
                    item.skill[skills::SK_TWOHAND][0] += skill_value as i8;
                    if item.skill[skills::SK_TWOHAND][0] > 12 {
                        item.skill[skills::SK_TWOHAND][0] = 12;
                    }
                }
                // Lockpick
                14 => {
                    item.skill[skills::SK_LOCK][0] += skill_value as i8;
                    if item.skill[skills::SK_LOCK][0] > 12 {
                        item.skill[skills::SK_LOCK][0] = 12;
                    }
                    item.skill[skills::SK_LOCK][2] =
                        (item.skill[skills::SK_LOCK][0] as u32 * helpers::random_mod(7)) as i8;
                }
                // Stealth
                15 => {
                    item.skill[skills::SK_STEALTH][0] += skill_value as i8;
                    if item.skill[skills::SK_STEALTH][0] > 12 {
                        item.skill[skills::SK_STEALTH][0] = 12;
                    }
                }
                // Perception
                16 => {
                    item.skill[skills::SK_PERCEPT][0] += skill_value as i8;
                    if item.skill[skills::SK_PERCEPT][0] > 12 {
                        item.skill[skills::SK_PERCEPT][0] = 12;
                    }
                    item.skill[skills::SK_PERCEPT][2] =
                        (item.skill[skills::SK_PERCEPT][0] as u32 * helpers::random_mod(7)) as i8;
                }
                // Magic Shield
                17 => {
                    item.skill[skills::SK_MSHIELD][0] += skill_value as i8;
                    if item.skill[skills::SK_MSHIELD][0] > 12 {
                        item.skill[skills::SK_MSHIELD][0] = 12;
                    }
                }
                // Barter
                18 => {
                    item.skill[skills::SK_BARTER][0] += skill_value as i8;
                    if item.skill[skills::SK_BARTER][0] > 12 {
                        item.skill[skills::SK_BARTER][0] = 12;
                    }
                    item.skill[skills::SK_BARTER][2] =
                        (item.skill[skills::SK_BARTER][0] as u32 * helpers::random_mod(7)) as i8;
                }
                // Repair
                19 => {
                    item.skill[skills::SK_REPAIR][0] += skill_value as i8;
                    if item.skill[skills::SK_REPAIR][0] > 12 {
                        item.skill[skills::SK_REPAIR][0] = 12;
                    }
                    item.skill[skills::SK_REPAIR][2] =
                        (item.skill[skills::SK_REPAIR][0] as u32 * helpers::random_mod(7)) as i8;
                }
                // Light
                20 => {
                    item.skill[skills::SK_LIGHT][0] += skill_value as i8;
                    if item.skill[skills::SK_LIGHT][0] > 12 {
                        item.skill[skills::SK_LIGHT][0] = 12;
                    }
                    item.skill[skills::SK_LIGHT][2] =
                        (item.skill[skills::SK_LIGHT][0] as u32 * helpers::random_mod(7)) as i8;
                }
                // Recall
                21 => {
                    item.skill[skills::SK_RECALL][0] += skill_value as i8;
                    if item.skill[skills::SK_RECALL][0] > 12 {
                        item.skill[skills::SK_RECALL][0] = 12;
                    }
                    item.skill[skills::SK_RECALL][2] =
                        (item.skill[skills::SK_RECALL][0] as u32 * helpers::random_mod(7)) as i8;
                }
                // Protect
                22 => {
                    item.skill[skills::SK_PROTECT][0] += skill_value as i8;
                    if item.skill[skills::SK_PROTECT][0] > 12 {
                        item.skill[skills::SK_PROTECT][0] = 12;
                    }
                    item.skill[skills::SK_PROTECT][2] =
                        (item.skill[skills::SK_PROTECT][0] as u32 * helpers::random_mod(7)) as i8;
                }
                // Enhance
                23 => {
                    item.skill[skills::SK_ENHANCE][0] += skill_value as i8;
                    if item.skill[skills::SK_ENHANCE][0] > 12 {
                        item.skill[skills::SK_ENHANCE][0] = 12;
                    }
                    item.skill[skills::SK_ENHANCE][2] =
                        (item.skill[skills::SK_ENHANCE][0] as u32 * helpers::random_mod(7)) as i8;
                }
                // Stun
                24 => {
                    item.skill[skills::SK_STUN][0] += skill_value as i8;
                    if item.skill[skills::SK_STUN][0] > 12 {
                        item.skill[skills::SK_STUN][0] = 12;
                    }
                }
                // Curse
                25 => {
                    item.skill[skills::SK_CURSE][0] += skill_value as i8;
                    if item.skill[skills::SK_CURSE][0] > 12 {
                        item.skill[skills::SK_CURSE][0] = 12;
                    }
                }
                // Bless
                26 => {
                    item.skill[skills::SK_BLESS][0] += skill_value as i8;
                    if item.skill[skills::SK_BLESS][0] > 12 {
                        item.skill[skills::SK_BLESS][0] = 12;
                    }
                    item.skill[skills::SK_BLESS][2] =
                        (item.skill[skills::SK_BLESS][0] as u32 * helpers::random_mod(7)) as i8;
                }
                // Identify
                27 => {
                    item.skill[skills::SK_IDENT][0] += skill_value as i8;
                    if item.skill[skills::SK_IDENT][0] > 12 {
                        item.skill[skills::SK_IDENT][0] = 12;
                    }
                    item.skill[skills::SK_IDENT][2] =
                        (item.skill[skills::SK_IDENT][0] as u32 * helpers::random_mod(7)) as i8;
                }
                // Resist
                28 => {
                    item.skill[skills::SK_RESIST][0] += skill_value as i8;
                    if item.skill[skills::SK_RESIST][0] > 12 {
                        item.skill[skills::SK_RESIST][0] = 12;
                    }
                    item.skill[skills::SK_RESIST][2] =
                        (item.skill[skills::SK_RESIST][0] as u32 * helpers::random_mod(7)) as i8;
                }
                // Blast
                29 => {
                    item.skill[skills::SK_BLAST][0] += skill_value as i8;
                    if item.skill[skills::SK_BLAST][0] > 12 {
                        item.skill[skills::SK_BLAST][0] = 12;
                    }
                }
                // Dispel
                30 => {
                    item.skill[skills::SK_DISPEL][0] += skill_value as i8;
                    if item.skill[skills::SK_DISPEL][0] > 12 {
                        item.skill[skills::SK_DISPEL][0] = 12;
                    }
                }
                // Heal
                31 => {
                    item.skill[skills::SK_HEAL][0] += skill_value as i8;
                    if item.skill[skills::SK_HEAL][0] > 12 {
                        item.skill[skills::SK_HEAL][0] = 12;
                    }
                    item.skill[skills::SK_HEAL][2] =
                        (item.skill[skills::SK_HEAL][0] as u32 * helpers::random_mod(7)) as i8;
                }
                // Ghost
                32 => {
                    item.skill[skills::SK_GHOST][0] += skill_value as i8;
                    if item.skill[skills::SK_GHOST][0] > 12 {
                        item.skill[skills::SK_GHOST][0] = 12;
                    }
                }
                // Regeneration
                33 => {
                    item.skill[skills::SK_REGEN][0] += skill_value as i8;
                    if item.skill[skills::SK_REGEN][0] > 12 {
                        item.skill[skills::SK_REGEN][0] = 12;
                    }
                }
                // Rest
                34 => {
                    item.skill[skills::SK_REST][0] += skill_value as i8;
                    if item.skill[skills::SK_REST][0] > 12 {
                        item.skill[skills::SK_REST][0] = 12;
                    }
                    item.skill[skills::SK_REST][2] =
                        (item.skill[skills::SK_REST][0] as u32 * helpers::random_mod(7)) as i8;
                }
                // Meditation
                35 => {
                    item.skill[skills::SK_MEDIT][0] += skill_value as i8;
                    if item.skill[skills::SK_MEDIT][0] > 12 {
                        item.skill[skills::SK_MEDIT][0] = 12;
                    }
                }
                // Sense
                36 => {
                    item.skill[skills::SK_SENSE][0] += skill_value as i8;
                    if item.skill[skills::SK_SENSE][0] > 12 {
                        item.skill[skills::SK_SENSE][0] = 12;
                    }
                    item.skill[skills::SK_SENSE][2] =
                        (item.skill[skills::SK_SENSE][0] as u32 * helpers::random_mod(7)) as i8;
                }
                // Immunity
                37 => {
                    item.skill[skills::SK_IMMUN][0] += skill_value as i8;
                    if item.skill[skills::SK_IMMUN][0] > 12 {
                        item.skill[skills::SK_IMMUN][0] = 12;
                    }
                }
                // Surround Hit
                38 => {
                    item.skill[skills::SK_SURROUND][0] += skill_value as i8;
                    if item.skill[skills::SK_SURROUND][0] > 12 {
                        item.skill[skills::SK_SURROUND][0] = 12;
                    }
                }
                // Concentration
                39 => {
                    item.skill[skills::SK_CONCEN][0] += skill_value as i8;
                    if item.skill[skills::SK_CONCEN][0] > 12 {
                        item.skill[skills::SK_CONCEN][0] = 12;
                    }
                }
                _ => {}
            }
        }
    }

    in_id as i32
}

/// Port of `pop_create_char` from `populate.cpp`
/// Creates a character from a template
/// Create a character from a template using an explicit game-state borrow.
///
/// # Arguments
/// * `gs` - Mutable game state.
/// * `template_id` - Character template id.
/// * `drop` - Whether to place the character on the map immediately.
pub fn pop_create_char(gs: &mut GameState, template_id: usize, drop: bool) -> Option<usize> {
    // Find a free character slot.
    let cn = match (1..MAXCHARS).find(|&i| gs.characters[i].used == USE_EMPTY) {
        Some(index) => index,
        None => {
            log::error!("MAXCHARS reached!");
            return None;
        }
    };

    // Copy template and set initial fields (matches C++: ch[cn] = ch_temp[n]).
    {
        gs.characters[cn] = gs.character_templates[template_id];
        gs.characters[cn].pass1 = helpers::random_mod(0x3fffffff);
        gs.characters[cn].pass2 = helpers::random_mod(0x3fffffff);
        gs.characters[cn].temp = template_id as u16;
    }

    let mut flag = false;
    let mut hasitems = false;

    // Create inventory items from template.
    for m in 0..40usize {
        let tmp_template = gs.characters[cn].item[m];
        if tmp_template == 0 {
            continue;
        }

        let tmp_instance = God::create_item(gs, tmp_template as usize).unwrap_or(0);
        if tmp_instance == 0 {
            flag = true;
            gs.characters[cn].item[m] = 0;
        } else {
            gs.items[tmp_instance].carried = cn as u16;
            gs.characters[cn].item[m] = tmp_instance as u32;
            hasitems = true;
        }
    }

    // Create worn items from template (uses pop_create_item to preserve unique logic).
    for m in 0..20usize {
        let tmp_template = gs.characters[cn].worn[m];
        if tmp_template == 0 {
            continue;
        }

        let tmp_instance = pop_create_item(gs, tmp_template as usize, cn);
        if tmp_instance == 0 {
            flag = true;
            gs.characters[cn].worn[m] = 0;
        } else {
            gs.items[tmp_instance].carried = cn as u16;
            gs.characters[cn].worn[m] = tmp_instance as u32;
            hasitems = true;
        }
    }

    // Clear spells from template.
    for m in 0..20usize {
        gs.characters[cn].spell[m] = 0;
    }

    // Create carried item (citem) from template.
    let tmp_template = gs.characters[cn].citem;
    if tmp_template != 0 {
        let tmp_instance = God::create_item(gs, tmp_template as usize).unwrap_or(0);
        if tmp_instance == 0 {
            flag = true;
            gs.characters[cn].citem = 0;
        } else {
            gs.items[tmp_instance].carried = cn as u16;
            gs.characters[cn].citem = tmp_instance as u32;
            hasitems = true;
        }
    }

    // Roll back if any item creation failed.
    if flag {
        God::destroy_items(gs, cn);
        gs.characters[cn].used = USE_EMPTY;
        return None;
    }

    // Finalize stats (mana logic matches C++).
    {
        let characters = &mut gs.characters;
        characters[cn].a_end = 1000000;
        characters[cn].a_hp = 1000000;

        if characters[cn].skill[skills::SK_MEDIT][0] != 0 {
            characters[cn].a_mana = 1000000;
        } else {
            let r1 = helpers::random_mod(8) as i32;
            let r2 = helpers::random_mod(8) as i32;
            let r3 = helpers::random_mod(8) as i32;
            let r4 = helpers::random_mod(8) as i32;
            characters[cn].a_mana = r1 * r2 * r3 * r4 * 100;
        }

        characters[cn].dir = DX_DOWN;
        characters[cn].data[92] = TICKS * 60;
    }

    // Bonus item / belt logic (matches C++: only if evil and hasitems; only first free slot).
    let has_meditation = gs.characters[cn].skill[skills::SK_MEDIT][0] != 0;
    let a_mana = gs.characters[cn].a_mana;
    let alignment = gs.characters[cn].alignment;

    let mut chance: i32 = 25;
    if !has_meditation && a_mana > 15 * 100 {
        chance -= 6;
    }
    if !has_meditation && a_mana > 30 * 100 {
        chance -= 6;
    }
    if !has_meditation && a_mana > 65 * 100 {
        chance -= 6;
    }

    if alignment < 0 && hasitems {
        // Bonus item: at most one, attempt on first empty slot.
        let first_empty_slot = {
            let items = gs.characters[cn].item;
            items.iter().position(|&it| it == 0)
        };
        if let Some(slot) = first_empty_slot {
            if chance > 0 && helpers::random_mod(chance as u32) == 0 {
                let tmp = pop_create_bonus(gs, cn, chance);
                if tmp != 0 {
                    let tmp = tmp as usize;
                    gs.items[tmp].carried = cn as u16;
                    gs.characters[cn].item[slot] = tmp as u32;
                }
            }
        }

        // Rainbow belt: at most one, attempt on (new) first empty slot.
        let first_empty_slot = {
            let items = gs.characters[cn].item;
            items.iter().position(|&it| it == 0)
        };
        if let Some(slot) = first_empty_slot {
            if helpers::random_mod(10000) == 0 {
                let tmp = pop_create_bonus_belt(gs, cn);
                if tmp != 0 {
                    let tmp = tmp as usize;
                    gs.items[tmp].carried = cn as u16;
                    gs.characters[cn].item[slot] = tmp as u32;
                }
            }
        }
    }

    // Drop character on map if requested (matches C++: exact coords, cleanup on failure).
    if drop {
        let (x, y) = {
            let ch = gs.characters[cn];
            (ch.x, ch.y)
        };

        if x < 0 || y < 0 || !God::drop_char(gs, cn, x as usize, y as usize) {
            log::error!("Could not drop char template {}", template_id);
            God::destroy_items(gs, cn);
            gs.characters[cn].used = USE_EMPTY;
            return None;
        }
    }

    gs.do_update_char(cn);
    gs.globals.npcs_created += 1;

    Some(cn)
}

/// Port of `reset_char` from `populate.cpp`
/// Resets a character template and all instances
pub fn reset_char(gs: &mut GameState, n: usize) {
    if !(1..MAXTCHARS).contains(&n) {
        log::error!("reset_char: invalid template {}", n);
        return;
    }

    let used = gs.character_templates[n].used;
    let has_respawn = (gs.character_templates[n].flags & CharacterFlags::Respawn.bits()) != 0;

    if used == USE_EMPTY || !has_respawn {
        log::error!(
            "reset_char: template {} is not in use or does not have respawn flag",
            n
        );
        return;
    }

    let name = gs.character_templates[n].get_name().to_string();
    log::info!("Resetting char {} ({})", n, name);

    // Recalculate character template points
    let mut cnt = 0;

    // Destroy all instances of this template (they will be respawned)
    for cn in 1..MAXCHARS {
        let temp = gs.characters[cn].temp;
        let used = gs.characters[cn].used;
        let char_name = gs.characters[cn].get_name().to_string();
        let x = gs.characters[cn].x;
        let y = gs.characters[cn].y;

        if temp as usize == n && used == USE_ACTIVE {
            log::info!(" --> {} ({}) ({},{})", char_name, cn, x, y);

            // Destroy items and remove from map
            God::destroy_items(gs, cn);
            player::plr_map_remove(gs, cn);

            // Mark character as unused
            gs.characters[cn].used = USE_EMPTY;

            cnt += 1;
        }
    }

    // Clean up effects referencing this template (type 2 = respawn timer)
    for m in 0..MAXEFFECT {
        let effect_used = gs.effects[m].used;
        let effect_type = gs.effects[m].effect_type;
        let data2 = gs.effects[m].data[2];

        if effect_used == USE_ACTIVE && effect_type == 2 && data2 == n as u32 {
            log::info!(" --> effect {}", m);
            gs.effects[m].used = USE_EMPTY;
        }
    }

    // Clean up items carried by this template
    for m in 0..MAXITEM {
        let item_used = gs.items[m].used;
        let carried = gs.items[m].carried;

        if item_used == USE_ACTIVE && carried as usize == n {
            let temp = gs.items[m].temp;
            let item_template = gs.item_templates[temp as usize];
            gs.items[m] = item_template;
            gs.items[m].temp = temp;
        }
    }

    if cnt != 1 {
        log::warn!("AUTO-RESPAWN: Found {} instances of {} ({})", cnt, name, n);
    }

    // Schedule respawn if template is still active
    let template_used = gs.character_templates[n].used;
    if template_used == USE_ACTIVE {
        let template_x = gs.character_templates[n].x;
        let template_y = gs.character_templates[n].y;

        EffectManager::fx_add_effect(
            gs,
            2,          // Effect type 2 = respawn timer
            TICKS * 10, // 10 seconds delay
            template_x as i32,
            template_y as i32,
            n as i32,
        );
        log::info!("Scheduled respawn for template {}", n);
    }
}

/// Port of `skillcost` from `populate.cpp`
/// Calculates the cost of raising a skill
pub fn skillcost(val: i32, dif: i32, start: i32) -> i32 {
    let mut p = 0;
    for n in start..val {
        p += n / 10 + 1 + dif;
    }
    p
}

/// Port of `pop_skill` from `populate.cpp`
/// Updates skills for all characters
pub fn pop_skill(gs: &mut GameState) {
    for cn in 1..MAXCHARS {
        let is_player = (gs.characters[cn].flags & CharacterFlags::Player.bits()) != 0
            && gs.characters[cn].used == USE_ACTIVE;
        if !is_player {
            continue;
        }

        let t = gs.characters[cn].temp as usize;

        let template_skills = gs.character_templates[t].skill;

        for n in 0..50usize {
            let temp_skill = template_skills[n];

            let ch = &mut gs.characters[cn];

            if ch.skill[n][0] == 0 && temp_skill[0] != 0 {
                ch.skill[n][0] = temp_skill[0];
                log::info!("added {} to {}", skills::skill_name(n), ch.get_name());
            }

            if temp_skill[2] < ch.skill[n][0] {
                let p = skillcost(
                    ch.skill[n][0] as i32,
                    ch.skill[n][3] as i32,
                    temp_skill[2] as i32,
                );
                log::info!(
                    "reduced {} on {} from {} to {}, added {} exp",
                    skills::skill_name(n),
                    ch.get_name(),
                    ch.skill[n][0],
                    temp_skill[2],
                    p
                );
                ch.skill[n][0] = temp_skill[2];
                ch.points += p;
            }

            ch.skill[n][1] = temp_skill[1];
            ch.skill[n][2] = temp_skill[2];
            ch.skill[n][3] = temp_skill[3];
        }
    }
    log::info!("Changed Skills.");
}

/// Port of `reset_item` from `populate.cpp`
/// Resets an item template and all instances
pub fn reset_item(gs: &mut GameState, n: usize) {
    if !(2..MAXTITEM).contains(&n) {
        return; // Never reset blank template (1)
    }

    let name = gs.item_templates[n].get_name().to_string();
    log::info!("Resetting item {} ({})", n, name);

    for in_id in 1..MAXITEM {
        let used = gs.items[in_id].used;
        let item_temp = gs.items[in_id].temp;
        let is_spell = (gs.items[in_id].flags & ItemFlags::IF_SPELL.bits()) != 0;

        if used != USE_ACTIVE {
            continue;
        }

        // Skip spell items
        if is_spell {
            continue;
        }

        if item_temp as usize != n {
            continue;
        }

        let item_name = gs.items[in_id].get_name().to_string();
        let carried = gs.items[in_id].carried;
        let x = gs.items[in_id].x;
        let y = gs.items[in_id].y;

        log::info!(" --> {} ({}) ({}, {},{})", item_name, in_id, carried, x, y);

        // Check if item should be reset or removed
        let template_flags = gs.item_templates[n].flags;
        let template_sprite = gs.item_templates[n].sprite[0];

        let should_reset = (template_flags
            & (ItemFlags::IF_TAKE.bits()
                | ItemFlags::IF_LOOK.bits()
                | ItemFlags::IF_LOOKSPECIAL.bits()
                | ItemFlags::IF_USE.bits()
                | ItemFlags::IF_USESPECIAL.bits()))
            != 0
            || carried != 0;

        if should_reset {
            // Reset item from template (for takeable/interactive items or carried items)
            let item_template = gs.item_templates[n];

            let x = gs.items[in_id].x;
            let y = gs.items[in_id].y;
            let carried = gs.items[in_id].carried;

            gs.items[in_id] = item_template;
            gs.items[in_id].x = x;
            gs.items[in_id].y = y;
            gs.items[in_id].carried = carried;
            gs.items[in_id].temp = n as u16;
        } else {
            // Remove item and place floor sprite (for non-interactive map items)
            let map_index = x as usize + y as usize * SERVER_MAPX as usize;

            gs.map[map_index].it = 0;
            gs.map[map_index].fsprite = template_sprite as u16;

            if (template_flags & ItemFlags::IF_MOVEBLOCK.bits()) != 0 {
                gs.map[map_index].flags |= MF_MOVEBLOCK as u64;
            }
            if (template_flags & ItemFlags::IF_SIGHTBLOCK.bits()) != 0 {
                gs.map[map_index].flags |= MF_SIGHTBLOCK as u64;
            }

            gs.items[in_id].used = USE_EMPTY;
        }
    }
}

/// Port of `reset_changed_items` from `populate.cpp`
/// Resets a predefined list of changed items
pub fn reset_changed_items(gs: &mut GameState) {
    let changelist: Vec<usize> = vec![];

    for n in changelist {
        reset_item(gs, n);
    }
}

/// Port of `pop_tick` from `populate.cpp`
/// Handles population ticking and resets
pub fn pop_tick(gs: &mut GameState) {
    const RESETTICKER: u32 = TICKS as u32 * 60;

    let ticker = gs.globals.ticker as u32;

    if ticker - gs.last_population_reset_tick >= RESETTICKER {
        gs.last_population_reset_tick = ticker;
        log::info!("Population tick: checking for resets");
    }

    // Check for character reset
    let reset_char_id = gs.globals.reset_char;
    if reset_char_id != 0 {
        reset_char(gs, reset_char_id as usize);
        gs.globals.reset_char = 0;
    }

    // Check for item reset
    let reset_item_id = gs.globals.reset_item;
    if reset_item_id != 0 {
        reset_item(gs, reset_item_id as usize);
        gs.globals.reset_item = 0;
    }
}

/// Port of `pop_reset_all` from `populate.cpp`
/// Resets all character and item templates
#[allow(dead_code)]
pub fn pop_reset_all(gs: &mut GameState) {
    for n in 1..MAXTCHARS {
        reset_char(gs, n);
    }
    for n in 1..MAXTITEM {
        reset_item(gs, n);
    }
    log::info!("Reset all templates");
}

/// Port of `pop_wipe` from `populate.cpp`
/// Wipes all dynamic game data
pub fn pop_wipe(gs: &mut GameState) {
    // Clear all characters
    for n in 1..MAXCHARS {
        let is_player = (gs.characters[n].flags & CharacterFlags::Player.bits()) != 0;

        if !is_player {
            gs.characters[n].used = USE_EMPTY;
        }
    }

    // Clear all items
    for n in 1..MAXITEM {
        gs.items[n].used = USE_EMPTY;
    }

    // Clear all effects
    for n in 1..MAXEFFECT {
        gs.effects[n].used = USE_EMPTY;
    }

    // Reset global statistics
    gs.globals.players_created = 0;
    gs.globals.npcs_created = 0;
    gs.globals.players_died = 0;
    gs.globals.npcs_died = 0;
    gs.globals.expire_cnt = 0;
    gs.globals.expire_run = 0;
    gs.globals.gc_cnt = 0;
    gs.globals.gc_run = 0;
    gs.globals.lost_cnt = 0;
    gs.globals.lost_run = 0;
    gs.globals.reset_char = 0;
    gs.globals.reset_item = 0;
    gs.globals.total_online_time = 0;
    gs.globals.uptime = 0;

    log::info!("Wiped all dynamic game data");
}

/// Port of `populate` from `populate.cpp`
/// Populates the world with NPCs
pub fn populate(gs: &mut GameState) {
    log::info!("Populating world...");

    // Iterate through all character templates and spawn respawnable NPCs
    for n in 1..MAXTCHARS {
        let used = gs.character_templates[n].used;
        let has_respawn = (gs.character_templates[n].flags & CharacterFlags::Respawn.bits()) != 0;

        if used != USE_EMPTY && has_respawn {
            if let Some(cn) = pop_create_char(gs, n, true) {
                log::debug!("Spawned NPC {} from template {}", cn, n);
            }
        }
    }

    log::info!("World populated");
}

/// Port of `pop_save_char` from `populate.cpp`
/// Saves a single character to disk
pub fn pop_save_char(nr: usize) {
    log::debug!("Saving character {}", nr);

    // TODO: Implement when persistence system is ready
}

/// Port of `pop_load_char` from `populate.cpp`
/// Loads a single character from disk
pub fn pop_load_char(nr: usize) {
    log::debug!("Loading character {}", nr);

    // TODO: Implement when persistence system is ready
}

/// Port of `pop_load_all_chars` from `populate.cpp`
/// Loads all characters from disk
pub fn pop_load_all_chars(_gs: &mut GameState) {
    log::info!("Loading all characters...");

    for nr in 1..MAXCHARS {
        pop_load_char(nr);
    }

    log::info!("All characters loaded");
}

/// Port of `pop_save_all_chars` from `populate.cpp`
/// Saves all characters to disk
pub fn pop_save_all_chars(gs: &mut GameState) {
    log::info!("Saving all characters...");

    for nr in 1..MAXCHARS {
        let is_player = (gs.characters[nr].flags & CharacterFlags::Player.bits()) != 0;

        if is_player {
            pop_save_char(nr);
        }
    }

    log::info!("All characters saved");
}
