use core::constants::{
    AT_AGIL, AT_BRAVE, AT_INT, AT_STREN, AT_WILL, DX_DOWN, MAXCHARS, MAXEFFECT, MAXITEM, MAXTCHARS,
    MAXTITEM, MF_INDOORS, MF_MOVEBLOCK, MF_SIGHTBLOCK, SERVER_MAPX, SERVER_MAPY, SK_BARTER,
    SK_BLAST, SK_BLESS, SK_CONCEN, SK_CURSE, SK_DAGGER, SK_DISPEL, SK_ENHANCE, SK_GHOST, SK_HAND,
    SK_HEAL, SK_IDENT, SK_IMMUN, SK_LIGHT, SK_LOCK, SK_MEDIT, SK_MSHIELD, SK_PERCEPT, SK_PROTECT,
    SK_RECALL, SK_REGEN, SK_REPAIR, SK_RESIST, SK_REST, SK_SENSE, SK_STEALTH, SK_STUN, SK_SURROUND,
    SK_SWORD, SK_TWOHAND, SK_WARCRY, TICKS, USE_ACTIVE, USE_EMPTY,
};

use {core::constants::CharacterFlags, core::constants::ItemFlags};

use crate::{
    driver::{self, use_item},
    effect::EffectManager,
    god::God,
    player,
    repository::Repository,
    state::State,
};

/// Port of `init_lights` from `populate.cpp`
/// Initialize lighting on the map
pub fn init_lights() {
    let mut cnt1 = 0;
    let mut cnt2 = 0;

    // First pass: clear all light and dlight values
    for y in 0..SERVER_MAPY as usize {
        for x in 0..SERVER_MAPX as usize {
            let m = x + y * SERVER_MAPX as usize;
            Repository::with_map_mut(|map| {
                map[m].light = 0;
                map[m].dlight = 0;
            });
        }
    }

    // Second pass: compute dlight for indoor tiles, then add lights from items
    for y in 0..SERVER_MAPY as usize {
        for x in 0..SERVER_MAPX as usize {
            let m = x + y * SERVER_MAPX as usize;

            // Compute daylight for indoor tiles first
            let is_indoors = Repository::with_map(|map| map[m].flags & MF_INDOORS as u64 != 0);

            if is_indoors {
                State::with_mut(|state| {
                    state.compute_dlight(x as i32, y as i32);
                });
                cnt2 += 1;
            }

            // Then add light from items
            let in_id = Repository::with_map(|map| map[m].it);

            if in_id == 0 {
                continue;
            }

            let (active, light_active, light_inactive) = Repository::with_items(|items| {
                (
                    items[in_id as usize].active,
                    items[in_id as usize].light[1],
                    items[in_id as usize].light[0],
                )
            });

            if active != 0 {
                if light_active != 0 {
                    State::with_mut(|state| {
                        state.do_add_light(x as i32, y as i32, light_active as i32);
                    });
                    cnt1 += 1;
                }
            } else {
                if light_inactive != 0 {
                    State::with_mut(|state| {
                        state.do_add_light(x as i32, y as i32, light_inactive as i32);
                    });
                    cnt1 += 1;
                }
            }
        }
    }

    log::info!("Initialized lights: {} items, {} indoor tiles", cnt1, cnt2);
}

/// Port of `pop_create_item` from `populate.cpp`
/// Creates items for NPCs based on alignment and template
pub fn pop_create_item(temp: usize, cn: usize) -> usize {
    let mut in_id = 0;
    let alignment = Repository::with_characters(|characters| characters[cn].alignment);

    // First check: Gorn uniques (1/150 chance)
    if in_id == 0 && alignment < 0 && rand::random::<u32>().is_multiple_of(150) {
        in_id = match temp {
            27 => God::create_item(542),  // bronze dagger
            28 => God::create_item(543),  // steel dagger
            29 => God::create_item(544),  // gold dagger
            30 => God::create_item(545),  // crystal dagger
            523 => God::create_item(546), // titan dagger
            31 => God::create_item(547),  // bronze sword
            32 => God::create_item(548),  // steel sword
            33 => God::create_item(549),  // gold sword
            34 => God::create_item(550),  // crystal sword
            524 => God::create_item(551), // titan sword
            35 => God::create_item(552),  // bronze two
            36 => God::create_item(553),  // steel two
            37 => God::create_item(554),  // gold two
            38 => God::create_item(555),  // crystal two
            125 => God::create_item(556), // titan two
            _ => None,
        }
        .unwrap_or(0);
    }

    // Second check: Kwai uniques (1/150 chance)
    if in_id == 0 && alignment < 0 && rand::random::<u32>().is_multiple_of(150) {
        in_id = match temp {
            27 => God::create_item(527),  // bronze dagger
            28 => God::create_item(528),  // steel dagger
            29 => God::create_item(529),  // gold dagger
            30 => God::create_item(530),  // crystal dagger
            523 => God::create_item(531), // titan dagger
            31 => God::create_item(532),  // bronze sword
            32 => God::create_item(533),  // steel sword
            33 => God::create_item(534),  // gold sword
            34 => God::create_item(535),  // crystal sword
            524 => God::create_item(536), // titan sword
            35 => God::create_item(537),  // bronze two
            36 => God::create_item(538),  // steel two
            37 => God::create_item(539),  // gold two
            38 => God::create_item(540),  // crystal two
            125 => God::create_item(541), // titan two
            _ => None,
        }
        .unwrap_or(0);
    }

    // Third check: Purple One uniques (1/150 chance)
    if in_id == 0 && alignment < 0 && rand::random::<u32>().is_multiple_of(150) {
        in_id = match temp {
            27 => God::create_item(572),  // bronze dagger
            28 => God::create_item(573),  // steel dagger
            29 => God::create_item(574),  // gold dagger
            30 => God::create_item(575),  // crystal dagger
            523 => God::create_item(576), // titan dagger
            31 => God::create_item(577),  // bronze sword
            32 => God::create_item(578),  // steel sword
            33 => God::create_item(579),  // gold sword
            34 => God::create_item(580),  // crystal sword
            524 => God::create_item(581), // titan sword
            35 => God::create_item(582),  // bronze two
            36 => God::create_item(583),  // steel two
            37 => God::create_item(584),  // gold two
            38 => God::create_item(585),  // crystal two
            125 => God::create_item(586), // titan two
            _ => None,
        }
        .unwrap_or(0);
    }

    // Fourth check: Skua uniques (1/150 chance)
    if in_id == 0 && alignment < 0 && rand::random::<u32>().is_multiple_of(150) {
        in_id = match temp {
            27 => God::create_item(280),  // bronze dagger
            28 => God::create_item(281),  // steel dagger
            29 => God::create_item(282),  // gold dagger
            30 => God::create_item(283),  // crystal dagger
            523 => God::create_item(525), // titan dagger
            31 => God::create_item(284),  // bronze sword
            32 => God::create_item(285),  // steel sword
            33 => God::create_item(286),  // gold sword
            34 => God::create_item(287),  // crystal sword
            524 => God::create_item(526), // titan sword
            35 => God::create_item(288),  // bronze two
            36 => God::create_item(289),  // steel two
            37 => God::create_item(290),  // gold two
            38 => God::create_item(291),  // crystal two
            125 => God::create_item(292), // titan two
            _ => None,
        }
        .unwrap_or(0);
    }

    // Default: create item from template
    if in_id == 0 {
        in_id = God::create_item(temp).unwrap_or(0);

        // Apply item damage for regular items
        if in_id != 0 {
            let max_damage = Repository::with_items(|items| items[in_id].max_damage);
            if max_damage > 0 {
                // 50% chance to age the item first
                if rand::random::<u32>().is_multiple_of(2) {
                    Repository::with_items_mut(|items| {
                        items[in_id].current_damage = max_damage + 1;
                    });
                    use_item::item_age(in_id);
                }
                // Set random damage
                Repository::with_items_mut(|items| {
                    items[in_id].current_damage = rand::random::<u32>() % max_damage;
                });
            }
        }
    } else {
        let char_name =
            Repository::with_characters(|characters| characters[cn].get_name().to_string());
        let item_name = Repository::with_items(|items| items[in_id].get_name().to_string());
        log::info!("{} got unique item {}.", char_name, item_name);
    }

    in_id
}

/// Port of `pop_create_bonus` from `populate.cpp`
/// Creates bonus items based on character rank
pub fn pop_create_bonus(cn: usize, _chance: i32) -> i32 {
    let points_tot = Repository::with_characters(|characters| characters[cn].points_tot);

    let in_id = if points_tot > 20000000 {
        // Very high rank - create special items
        let choice = rand::random::<u32>() % 12;
        match choice {
            0 => God::create_item(1107),  // Special item 1
            1 => God::create_item(1108),  // Special item 2
            2 => God::create_item(1109),  // Special item 3
            3 => God::create_item(1110),  // Special item 4
            4 => God::create_item(1111),  // Special item 5
            5 => God::create_item(1112),  // Special item 6
            6 => God::create_item(1113),  // Special item 7
            7 => God::create_item(1114),  // Special item 8
            8 => God::create_item(1115),  // Special item 9
            9 => God::create_item(1116),  // Special item 10
            10 => God::create_item(1117), // Special item 11
            _ => God::create_item(1118),  // Special item 12
        }
    } else {
        // Normal bonus items based on random choice
        let choice = rand::random::<u32>() % 50;
        if choice < 10 {
            God::create_item(1100) // Bonus item template 1
        } else if choice < 20 {
            God::create_item(1101) // Bonus item template 2
        } else if choice < 30 {
            God::create_item(1102) // Bonus item template 3
        } else if choice < 40 {
            God::create_item(1103) // Bonus item template 4
        } else {
            God::create_item(1104) // Bonus item template 5
        }
    };

    if let Some(in_id) = in_id {
        log::info!("Created bonus item {} for character {}", in_id, cn);
        in_id as i32
    } else {
        0
    }
}

/// Port of `pop_create_bonus_belt` from `populate.cpp`
/// Creates special rainbow belts with random skills
pub fn pop_create_bonus_belt(cn: usize) -> i32 {
    let points_tot = Repository::with_characters(|characters| characters[cn].points_tot);

    // Calculate rank (from points2rank - needs to be implemented elsewhere)
    let rank = if points_tot < 1000 {
        0
    } else {
        ((points_tot as f64).ln() / 10.0) as i32
    };

    if rank == 0 {
        return 0;
    }

    let in_id = God::create_item(1106); // Rainbow belt template
    if in_id.is_none() {
        return 0;
    }
    let in_id = in_id.unwrap();

    // Customize the belt item (clear template and set sprite/name/description)
    Repository::with_items_mut(|items| {
        items[in_id].temp = 0; // Clear template
        items[in_id].sprite[0] = 16964;
        let name_bytes = b"Rainbow Belt";
        items[in_id].name[..name_bytes.len()].copy_from_slice(name_bytes);
        items[in_id].name[name_bytes.len()..].fill(0);
        let desc_bytes = b"An ancient belt. It seems to be highly magical";
        items[in_id].description[..desc_bytes.len()].copy_from_slice(desc_bytes);
        items[in_id].description[desc_bytes.len()..].fill(0);
        let ref_bytes = b"rainbow belt";
        items[in_id].reference[..ref_bytes.len()].copy_from_slice(ref_bytes);
        items[in_id].reference[ref_bytes.len()..].fill(0);
    });

    log::info!(
        "Character {} with rank {} got Rainbow Belt (t={})",
        cn,
        rank,
        0
    );

    let mut num_skills = rand::random::<i32>() % rank;
    if num_skills == 0 {
        num_skills = 1; // Ensure at least 1 skill
    }

    // Update item properties
    Repository::with_items_mut(|items| {
        items[in_id].power += (5 * num_skills) as u32;
        items[in_id].value += (10000 * num_skills) as u32;
    });

    // Add random skills to belt
    for _ in 0..num_skills {
        let skill_number = rand::random::<i32>() % 40; // 0-39
        let mut skill_value = rand::random::<i32>() % rank;
        skill_value >>= 1; // Divide by 2, max is rank/2 (max 12)
        if skill_value == 0 {
            skill_value = 1; // Ensure at least 1
        }

        Repository::with_items_mut(|items| {
            let item = &mut items[in_id];
            match skill_number {
                // Attributes
                0 => {
                    // Bravery (AT_BRAVE)
                    item.attrib[AT_BRAVE as usize][0] += skill_value as i8;
                    if item.attrib[AT_BRAVE as usize][0] > 12 {
                        item.attrib[AT_BRAVE as usize][0] = 12;
                    }
                    item.attrib[AT_BRAVE as usize][2] = (10
                        + (item.attrib[AT_BRAVE as usize][0] as i32 * (rand::random::<i32>() % 7)))
                        as i8;
                }
                1 => {
                    // Willpower (AT_WILL)
                    item.attrib[AT_WILL as usize][0] += skill_value as i8;
                    if item.attrib[AT_WILL as usize][0] > 12 {
                        item.attrib[AT_WILL as usize][0] = 12;
                    }
                    item.attrib[AT_WILL as usize][2] = (10
                        + (item.attrib[AT_WILL as usize][0] as i32 * (rand::random::<i32>() % 7)))
                        as i8;
                }
                2 => {
                    // Intuition (AT_INT)
                    item.attrib[AT_INT as usize][0] += skill_value as i8;
                    if item.attrib[AT_INT as usize][0] > 12 {
                        item.attrib[AT_INT as usize][0] = 12;
                    }
                    item.attrib[AT_INT as usize][2] = (10
                        + (item.attrib[AT_INT as usize][0] as i32 * (rand::random::<i32>() % 7)))
                        as i8;
                }
                3 => {
                    // Agility (AT_AGIL)
                    item.attrib[AT_AGIL as usize][0] += skill_value as i8;
                    if item.attrib[AT_AGIL as usize][0] > 12 {
                        item.attrib[AT_AGIL as usize][0] = 12;
                    }
                    item.attrib[AT_AGIL as usize][2] = (10
                        + (item.attrib[AT_AGIL as usize][0] as i32 * (rand::random::<i32>() % 7)))
                        as i8;
                }
                4 => {
                    // Strength (AT_STREN)
                    item.attrib[AT_STREN as usize][0] += skill_value as i8;
                    if item.attrib[AT_STREN as usize][0] > 12 {
                        item.attrib[AT_STREN as usize][0] = 12;
                    }
                    item.attrib[AT_STREN as usize][2] = (10
                        + (item.attrib[AT_STREN as usize][0] as i32 * (rand::random::<i32>() % 7)))
                        as i8;
                }
                // HP
                5 => {
                    item.hp[0] += (skill_value * 5) as i16;
                    if item.hp[0] > 60 {
                        item.hp[0] = 60;
                    }
                    item.hp[2] = (50 + (item.hp[0] as i32 * (rand::random::<i32>() % 7))) as i16;
                }
                // Endurance
                6 => {
                    item.end[0] += (skill_value * 5) as i16;
                    if item.end[0] > 60 {
                        item.end[0] = 60;
                    }
                    item.end[2] = (50 + (item.end[0] as i32 * (rand::random::<i32>() % 7))) as i16;
                }
                // Mana
                7 => {
                    item.mana[0] += (skill_value * 5) as i16;
                    if item.mana[0] > 60 {
                        item.mana[0] = 60;
                    }
                    item.mana[2] =
                        (50 + (item.mana[0] as i32 * (rand::random::<i32>() % 7))) as i16;
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
                    item.skill[SK_WARCRY][0] += skill_value as i8;
                    if item.skill[SK_WARCRY][0] > 12 {
                        item.skill[SK_WARCRY][0] = 12;
                    }
                }
                // Hand to Hand
                10 => {
                    item.skill[SK_HAND][0] += skill_value as i8;
                    if item.skill[SK_HAND][0] > 12 {
                        item.skill[SK_HAND][0] = 12;
                    }
                    item.skill[SK_HAND][2] =
                        (item.skill[SK_HAND][0] as i32 * (rand::random::<i32>() % 7)) as i8;
                }
                // Sword
                11 => {
                    item.skill[SK_SWORD][0] += skill_value as i8;
                    if item.skill[SK_SWORD][0] > 12 {
                        item.skill[SK_SWORD][0] = 12;
                    }
                }
                // Dagger
                12 => {
                    item.skill[SK_DAGGER][0] += skill_value as i8;
                    if item.skill[SK_DAGGER][0] > 12 {
                        item.skill[SK_DAGGER][0] = 12;
                    }
                }
                // Two-Handed
                13 => {
                    item.skill[SK_TWOHAND][0] += skill_value as i8;
                    if item.skill[SK_TWOHAND][0] > 12 {
                        item.skill[SK_TWOHAND][0] = 12;
                    }
                }
                // Lockpick
                14 => {
                    item.skill[SK_LOCK][0] += skill_value as i8;
                    if item.skill[SK_LOCK][0] > 12 {
                        item.skill[SK_LOCK][0] = 12;
                    }
                    item.skill[SK_LOCK][2] =
                        (item.skill[SK_LOCK][0] as i32 * (rand::random::<i32>() % 7)) as i8;
                }
                // Stealth
                15 => {
                    item.skill[SK_STEALTH][0] += skill_value as i8;
                    if item.skill[SK_STEALTH][0] > 12 {
                        item.skill[SK_STEALTH][0] = 12;
                    }
                }
                // Perception
                16 => {
                    item.skill[SK_PERCEPT][0] += skill_value as i8;
                    if item.skill[SK_PERCEPT][0] > 12 {
                        item.skill[SK_PERCEPT][0] = 12;
                    }
                    item.skill[SK_PERCEPT][2] =
                        (item.skill[SK_PERCEPT][0] as i32 * (rand::random::<i32>() % 7)) as i8;
                }
                // Magic Shield
                17 => {
                    item.skill[SK_MSHIELD][0] += skill_value as i8;
                    if item.skill[SK_MSHIELD][0] > 12 {
                        item.skill[SK_MSHIELD][0] = 12;
                    }
                }
                // Barter
                18 => {
                    item.skill[SK_BARTER][0] += skill_value as i8;
                    if item.skill[SK_BARTER][0] > 12 {
                        item.skill[SK_BARTER][0] = 12;
                    }
                    item.skill[SK_BARTER][2] =
                        (item.skill[SK_BARTER][0] as i32 * (rand::random::<i32>() % 7)) as i8;
                }
                // Repair
                19 => {
                    item.skill[SK_REPAIR][0] += skill_value as i8;
                    if item.skill[SK_REPAIR][0] > 12 {
                        item.skill[SK_REPAIR][0] = 12;
                    }
                    item.skill[SK_REPAIR][2] =
                        (item.skill[SK_REPAIR][0] as i32 * (rand::random::<i32>() % 7)) as i8;
                }
                // Light
                20 => {
                    item.skill[SK_LIGHT][0] += skill_value as i8;
                    if item.skill[SK_LIGHT][0] > 12 {
                        item.skill[SK_LIGHT][0] = 12;
                    }
                    item.skill[SK_LIGHT][2] =
                        (item.skill[SK_LIGHT][0] as i32 * (rand::random::<i32>() % 7)) as i8;
                }
                // Recall
                21 => {
                    item.skill[SK_RECALL][0] += skill_value as i8;
                    if item.skill[SK_RECALL][0] > 12 {
                        item.skill[SK_RECALL][0] = 12;
                    }
                    item.skill[SK_RECALL][2] =
                        (item.skill[SK_RECALL][0] as i32 * (rand::random::<i32>() % 7)) as i8;
                }
                // Protect
                22 => {
                    item.skill[SK_PROTECT][0] += skill_value as i8;
                    if item.skill[SK_PROTECT][0] > 12 {
                        item.skill[SK_PROTECT][0] = 12;
                    }
                    item.skill[SK_PROTECT][2] =
                        (item.skill[SK_PROTECT][0] as i32 * (rand::random::<i32>() % 7)) as i8;
                }
                // Enhance
                23 => {
                    item.skill[SK_ENHANCE][0] += skill_value as i8;
                    if item.skill[SK_ENHANCE][0] > 12 {
                        item.skill[SK_ENHANCE][0] = 12;
                    }
                    item.skill[SK_ENHANCE][2] =
                        (item.skill[SK_ENHANCE][0] as i32 * (rand::random::<i32>() % 7)) as i8;
                }
                // Stun
                24 => {
                    item.skill[SK_STUN][0] += skill_value as i8;
                    if item.skill[SK_STUN][0] > 12 {
                        item.skill[SK_STUN][0] = 12;
                    }
                }
                // Curse
                25 => {
                    item.skill[SK_CURSE][0] += skill_value as i8;
                    if item.skill[SK_CURSE][0] > 12 {
                        item.skill[SK_CURSE][0] = 12;
                    }
                }
                // Bless
                26 => {
                    item.skill[SK_BLESS][0] += skill_value as i8;
                    if item.skill[SK_BLESS][0] > 12 {
                        item.skill[SK_BLESS][0] = 12;
                    }
                    item.skill[SK_BLESS][2] =
                        (item.skill[SK_BLESS][0] as i32 * (rand::random::<i32>() % 7)) as i8;
                }
                // Identify
                27 => {
                    item.skill[SK_IDENT][0] += skill_value as i8;
                    if item.skill[SK_IDENT][0] > 12 {
                        item.skill[SK_IDENT][0] = 12;
                    }
                    item.skill[SK_IDENT][2] =
                        (item.skill[SK_IDENT][0] as i32 * (rand::random::<i32>() % 7)) as i8;
                }
                // Resist
                28 => {
                    item.skill[SK_RESIST][0] += skill_value as i8;
                    if item.skill[SK_RESIST][0] > 12 {
                        item.skill[SK_RESIST][0] = 12;
                    }
                    item.skill[SK_RESIST][2] =
                        (item.skill[SK_RESIST][0] as i32 * (rand::random::<i32>() % 7)) as i8;
                }
                // Blast
                29 => {
                    item.skill[SK_BLAST][0] += skill_value as i8;
                    if item.skill[SK_BLAST][0] > 12 {
                        item.skill[SK_BLAST][0] = 12;
                    }
                }
                // Dispel
                30 => {
                    item.skill[SK_DISPEL][0] += skill_value as i8;
                    if item.skill[SK_DISPEL][0] > 12 {
                        item.skill[SK_DISPEL][0] = 12;
                    }
                }
                // Heal
                31 => {
                    item.skill[SK_HEAL][0] += skill_value as i8;
                    if item.skill[SK_HEAL][0] > 12 {
                        item.skill[SK_HEAL][0] = 12;
                    }
                    item.skill[SK_HEAL][2] =
                        (item.skill[SK_HEAL][0] as i32 * (rand::random::<i32>() % 7)) as i8;
                }
                // Ghost
                32 => {
                    item.skill[SK_GHOST][0] += skill_value as i8;
                    if item.skill[SK_GHOST][0] > 12 {
                        item.skill[SK_GHOST][0] = 12;
                    }
                }
                // Regeneration
                33 => {
                    item.skill[SK_REGEN][0] += skill_value as i8;
                    if item.skill[SK_REGEN][0] > 12 {
                        item.skill[SK_REGEN][0] = 12;
                    }
                }
                // Rest
                34 => {
                    item.skill[SK_REST][0] += skill_value as i8;
                    if item.skill[SK_REST][0] > 12 {
                        item.skill[SK_REST][0] = 12;
                    }
                    item.skill[SK_REST][2] =
                        (item.skill[SK_REST][0] as i32 * (rand::random::<i32>() % 7)) as i8;
                }
                // Meditation
                35 => {
                    item.skill[SK_MEDIT][0] += skill_value as i8;
                    if item.skill[SK_MEDIT][0] > 12 {
                        item.skill[SK_MEDIT][0] = 12;
                    }
                }
                // Sense
                36 => {
                    item.skill[SK_SENSE][0] += skill_value as i8;
                    if item.skill[SK_SENSE][0] > 12 {
                        item.skill[SK_SENSE][0] = 12;
                    }
                    item.skill[SK_SENSE][2] =
                        (item.skill[SK_SENSE][0] as i32 * (rand::random::<i32>() % 7)) as i8;
                }
                // Immunity
                37 => {
                    item.skill[SK_IMMUN][0] += skill_value as i8;
                    if item.skill[SK_IMMUN][0] > 12 {
                        item.skill[SK_IMMUN][0] = 12;
                    }
                }
                // Surround Hit
                38 => {
                    item.skill[SK_SURROUND][0] += skill_value as i8;
                    if item.skill[SK_SURROUND][0] > 12 {
                        item.skill[SK_SURROUND][0] = 12;
                    }
                }
                // Concentration
                39 => {
                    item.skill[SK_CONCEN][0] += skill_value as i8;
                    if item.skill[SK_CONCEN][0] > 12 {
                        item.skill[SK_CONCEN][0] = 12;
                    }
                }
                _ => {}
            }
        });
    }

    in_id as i32
}

/// Port of `pop_create_char` from `populate.cpp`
/// Creates a character from a template
pub fn pop_create_char(template_id: usize, drop: bool) -> Option<usize> {
    // Find a free character slot.
    let cn = match Repository::with_characters(|characters| {
        (1..MAXCHARS).find(|&i| characters[i].used == USE_EMPTY)
    }) {
        Some(index) => index,
        None => {
            log::error!("MAXCHARS reached!");
            return None;
        }
    };

    // Copy template and set initial fields (matches C++: ch[cn] = ch_temp[n]).
    Repository::with_characters_mut(|characters| {
        characters[cn] =
            Repository::with_character_templates(|char_templates| char_templates[template_id]);
        characters[cn].pass1 = rand::random::<u32>() % 0x3fffffff;
        characters[cn].pass2 = rand::random::<u32>() % 0x3fffffff;
        characters[cn].temp = template_id as u16;
    });

    let mut flag = false;
    let mut hasitems = false;

    // Create inventory items from template.
    for m in 0..40usize {
        let tmp_template = Repository::with_characters(|characters| characters[cn].item[m]);
        if tmp_template == 0 {
            continue;
        }

        let tmp_instance = God::create_item(tmp_template as usize).unwrap_or(0);
        if tmp_instance == 0 {
            flag = true;
            Repository::with_characters_mut(|characters| {
                characters[cn].item[m] = 0;
            });
        } else {
            Repository::with_items_mut(|items| {
                items[tmp_instance].carried = cn as u16;
            });
            Repository::with_characters_mut(|characters| {
                characters[cn].item[m] = tmp_instance as u32;
            });
            hasitems = true;
        }
    }

    // Create worn items from template (uses pop_create_item to preserve unique logic).
    for m in 0..20usize {
        let tmp_template = Repository::with_characters(|characters| characters[cn].worn[m]);
        if tmp_template == 0 {
            continue;
        }

        let tmp_instance = pop_create_item(tmp_template as usize, cn);
        if tmp_instance == 0 {
            flag = true;
            Repository::with_characters_mut(|characters| {
                characters[cn].worn[m] = 0;
            });
        } else {
            Repository::with_items_mut(|items| {
                items[tmp_instance].carried = cn as u16;
            });
            Repository::with_characters_mut(|characters| {
                characters[cn].worn[m] = tmp_instance as u32;
            });
            hasitems = true;
        }
    }

    // Clear spells from template.
    Repository::with_characters_mut(|characters| {
        for m in 0..20usize {
            if characters[cn].spell[m] != 0 {
                characters[cn].spell[m] = 0;
            }
        }
    });

    // Create carried item (citem) from template.
    let tmp_template = Repository::with_characters(|characters| characters[cn].citem);
    if tmp_template != 0 {
        let tmp_instance = God::create_item(tmp_template as usize).unwrap_or(0);
        if tmp_instance == 0 {
            flag = true;
            Repository::with_characters_mut(|characters| {
                characters[cn].citem = 0;
            });
        } else {
            Repository::with_items_mut(|items| {
                items[tmp_instance].carried = cn as u16;
            });
            Repository::with_characters_mut(|characters| {
                characters[cn].citem = tmp_instance as u32;
            });
            hasitems = true;
        }
    }

    // Roll back if any item creation failed.
    if flag {
        God::destroy_items(cn);
        Repository::with_characters_mut(|characters| {
            characters[cn].used = USE_EMPTY;
        });
        return None;
    }

    // Finalize stats (mana logic matches C++).
    Repository::with_characters_mut(|characters| {
        characters[cn].a_end = 1000000;
        characters[cn].a_hp = 1000000;

        if characters[cn].skill[SK_MEDIT][0] != 0 {
            characters[cn].a_mana = 1000000;
        } else {
            let r1 = (rand::random::<u32>() % 8) as i32;
            let r2 = (rand::random::<u32>() % 8) as i32;
            let r3 = (rand::random::<u32>() % 8) as i32;
            let r4 = (rand::random::<u32>() % 8) as i32;
            characters[cn].a_mana = r1 * r2 * r3 * r4 * 100;
        }

        characters[cn].dir = DX_DOWN;
        characters[cn].data[92] = TICKS * 60;
    });

    // Bonus item / belt logic (matches C++: only if evil and hasitems; only first free slot).
    let has_meditation =
        Repository::with_characters(|characters| characters[cn].skill[SK_MEDIT][0] != 0);
    let a_mana = Repository::with_characters(|characters| characters[cn].a_mana);
    let alignment = Repository::with_characters(|characters| characters[cn].alignment);

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
        if let Some(slot) = Repository::with_characters(|characters| {
            let items = characters[cn].item;
            items.iter().position(|&it| it == 0)
        }) {
            if rand::random::<u32>().is_multiple_of(chance as u32) {
                let tmp = pop_create_bonus(cn, chance);
                if tmp != 0 {
                    let tmp = tmp as usize;
                    Repository::with_items_mut(|items| {
                        items[tmp].carried = cn as u16;
                    });
                    Repository::with_characters_mut(|characters| {
                        characters[cn].item[slot] = tmp as u32;
                    });
                }
            }
        }

        // Rainbow belt: at most one, attempt on (new) first empty slot.
        if let Some(slot) = Repository::with_characters(|characters| {
            let items = characters[cn].item;
            items.iter().position(|&it| it == 0)
        }) {
            if rand::random::<u32>().is_multiple_of(10000) {
                let tmp = pop_create_bonus_belt(cn);
                if tmp != 0 {
                    let tmp = tmp as usize;
                    Repository::with_items_mut(|items| {
                        items[tmp].carried = cn as u16;
                    });
                    Repository::with_characters_mut(|characters| {
                        characters[cn].item[slot] = tmp as u32;
                    });
                }
            }
        }
    }

    // Drop character on map if requested (matches C++: exact coords, cleanup on failure).
    if drop {
        let (x, y) = Repository::with_characters(|characters| (characters[cn].x, characters[cn].y));

        if x < 0 || y < 0 || !God::drop_char(cn, x as usize, y as usize) {
            log::error!("Could not drop char template {}", template_id);
            God::destroy_items(cn);
            Repository::with_characters_mut(|characters| {
                characters[cn].used = USE_EMPTY;
            });
            return None;
        }
    }

    State::with(|state| state.do_update_char(cn));
    Repository::with_globals_mut(|globals| {
        globals.npcs_created += 1;
    });

    Some(cn)
}

/// Port of `reset_char` from `populate.cpp`
/// Resets a character template and all instances
pub fn reset_char(n: usize) {
    if !(1..MAXTCHARS).contains(&n) {
        log::error!("reset_char: invalid template {}", n);
        return;
    }

    let (used, has_respawn) = Repository::with_character_templates(|templates| {
        (
            templates[n].used,
            (templates[n].flags & CharacterFlags::Respawn.bits()) != 0,
        )
    });

    if used == USE_EMPTY || !has_respawn {
        log::error!(
            "reset_char: template {} is not in use or does not have respawn flag",
            n
        );
        return;
    }

    let name =
        Repository::with_character_templates(|templates| templates[n].get_name().to_string());
    log::info!("Resetting char {} ({})", n, name);

    // Recalculate character template points
    let mut pts = 0;
    let mut cnt = 0;

    Repository::with_character_templates(|templates| {
        // Count base attributes
        for z in 0..5 {
            pts += templates[n].attrib[z][0] as i32;
        }

        // Count HP
        for m in 50..templates[n].hp[0] as i32 {
            pts += m / 10 + 1;
        }

        // Count endurance
        for m in 50..templates[n].end[0] as i32 {
            pts += m / 10 + 1;
        }

        // Count mana
        for m in 50..templates[n].mana[0] as i32 {
            pts += m / 10 + 1;
        }

        // Count skills
        for z in 0..50 {
            for m in 0..templates[n].skill[z][0] as i32 {
                pts += m / 10 + 1;
            }
        }
    });

    Repository::with_character_templates_mut(|templates| {
        templates[n].points_tot = pts;
    });

    // Destroy all instances of this template (they will be respawned)
    for cn in 1..MAXCHARS {
        let (temp, used, char_name, x, y) = Repository::with_characters(|characters| {
            (
                characters[cn].temp,
                characters[cn].used,
                characters[cn].get_name().to_string(),
                characters[cn].x,
                characters[cn].y,
            )
        });

        if temp as usize == n && used == USE_ACTIVE {
            log::info!(" --> {} ({}) ({},{})", char_name, cn, x, y);

            // Destroy items and remove from map
            God::destroy_items(cn);
            player::plr_map_remove(cn);

            // Mark character as unused
            Repository::with_characters_mut(|characters| {
                characters[cn].used = USE_EMPTY;
            });

            cnt += 1;
        }
    }

    // Clean up effects referencing this template (type 2 = respawn timer)
    for m in 0..MAXEFFECT {
        let (effect_used, effect_type, data2) = Repository::with_effects(|effects| {
            (effects[m].used, effects[m].effect_type, effects[m].data[2])
        });

        if effect_used == USE_ACTIVE && effect_type == 2 && data2 == n as u32 {
            log::info!(" --> effect {}", m);
            Repository::with_effects_mut(|effects| {
                effects[m].used = USE_EMPTY;
            });
        }
    }

    // Clean up items carried by this template
    for m in 0..MAXITEM {
        let (item_used, carried) =
            Repository::with_items(|items| (items[m].used, items[m].carried));

        if item_used == USE_ACTIVE && carried as usize == n {
            let temp = Repository::with_items(|items| items[m].temp);
            Repository::with_items_mut(|items| {
                let item_template =
                    Repository::with_item_templates(|templates| templates[temp as usize]);
                items[m] = item_template;
                items[m].temp = temp;
            });
        }
    }

    if cnt != 1 {
        log::warn!("AUTO-RESPAWN: Found {} instances of {} ({})", cnt, name, n);
    }

    // Schedule respawn if template is still active
    let template_used = Repository::with_character_templates(|templates| templates[n].used);
    if template_used == USE_ACTIVE {
        let (template_x, template_y) =
            Repository::with_character_templates(|templates| (templates[n].x, templates[n].y));

        EffectManager::fx_add_effect(
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
pub fn pop_skill() {
    for cn in 1..MAXCHARS {
        let is_player = Repository::with_characters(|characters| {
            (characters[cn].flags & CharacterFlags::Player.bits()) != 0
                && characters[cn].used == USE_ACTIVE
        });
        if !is_player {
            continue;
        }

        let t = Repository::with_characters(|characters| characters[cn].temp as usize);

        let template_skills = Repository::with_character_templates(|templates| templates[t].skill);

        for n in 0..50usize {
            let temp_skill = template_skills[n];

            Repository::with_characters_mut(|characters| {
                let ch = &mut characters[cn];

                if ch.skill[n][0] == 0 && temp_skill[0] != 0 {
                    ch.skill[n][0] = temp_skill[0];
                    log::info!("added {} to {}", driver::skill_name(n), ch.get_name());
                }

                if temp_skill[2] < ch.skill[n][0] {
                    let p = skillcost(
                        ch.skill[n][0] as i32,
                        ch.skill[n][3] as i32,
                        temp_skill[2] as i32,
                    );
                    log::info!(
                        "reduced {} on {} from {} to {}, added {} exp",
                        driver::skill_name(n),
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
            });
        }
    }
    log::info!("Changed Skills.");
}

/// Port of `reset_item` from `populate.cpp`
/// Resets an item template and all instances
pub fn reset_item(n: usize) {
    if !(2..MAXTITEM).contains(&n) {
        return; // Never reset blank template (1)
    }

    let name = Repository::with_item_templates(|templates| templates[n].get_name().to_string());
    log::info!("Resetting item {} ({})", n, name);

    for in_id in 1..MAXITEM {
        let (used, item_temp, is_spell) = Repository::with_items(|items| {
            (
                items[in_id].used,
                items[in_id].temp,
                (items[in_id].flags & ItemFlags::IF_SPELL.bits()) != 0,
            )
        });

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

        let (item_name, carried, x, y) = Repository::with_items(|items| {
            (
                items[in_id].get_name().to_string(),
                items[in_id].carried,
                items[in_id].x,
                items[in_id].y,
            )
        });

        log::info!(" --> {} ({}) ({}, {},{})", item_name, in_id, carried, x, y);

        // Check if item should be reset or removed
        let (template_flags, template_sprite) = Repository::with_item_templates(|templates| {
            (templates[n].flags, templates[n].sprite[0])
        });

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
            Repository::with_items_mut(|items| {
                let item_template = Repository::with_item_templates(|templates| templates[n]);

                // Preserve certain fields
                let x = items[in_id].x;
                let y = items[in_id].y;
                let carried = items[in_id].carried;

                items[in_id] = item_template;
                items[in_id].x = x;
                items[in_id].y = y;
                items[in_id].carried = carried;
                items[in_id].temp = n as u16;
            });
        } else {
            // Remove item and place floor sprite (for non-interactive map items)
            let map_index = x as usize + y as usize * SERVER_MAPX as usize;

            Repository::with_map_mut(|map| {
                map[map_index].it = 0;
                map[map_index].fsprite = template_sprite as u16;

                if (template_flags & ItemFlags::IF_MOVEBLOCK.bits()) != 0 {
                    map[map_index].flags |= MF_MOVEBLOCK as u64;
                }
                if (template_flags & ItemFlags::IF_SIGHTBLOCK.bits()) != 0 {
                    map[map_index].flags |= MF_SIGHTBLOCK as u64;
                }
            });

            Repository::with_items_mut(|items| {
                items[in_id].used = USE_EMPTY;
            });
        }
    }
}

/// Port of `reset_changed_items` from `populate.cpp`
/// Resets a predefined list of changed items
pub fn reset_changed_items() {
    let changelist: Vec<usize> = vec![];

    for n in changelist {
        reset_item(n);
    }
}

/// Port of `pop_tick` from `populate.cpp`
/// Handles population ticking and resets
pub fn pop_tick() {
    const RESETTICKER: u32 = TICKS as u32 * 60;

    let ticker = Repository::with_globals(|globals| globals.ticker) as u32;

    if ticker - Repository::get_last_population_reset_tick() >= RESETTICKER {
        Repository::set_last_population_reset_tick(ticker);
        log::info!("Population tick: checking for resets");
    }

    // Check for character reset
    let reset_char_id = Repository::with_globals(|globals| globals.reset_char);
    if reset_char_id != 0 {
        reset_char(reset_char_id as usize);
        Repository::with_globals_mut(|globals| globals.reset_char = 0);
    }

    // Check for item reset
    let reset_item_id = Repository::with_globals(|globals| globals.reset_item);
    if reset_item_id != 0 {
        reset_item(reset_item_id as usize);
        Repository::with_globals_mut(|globals| globals.reset_item = 0);
    }
}

/// Port of `pop_reset_all` from `populate.cpp`
/// Resets all character and item templates
#[allow(dead_code)]
pub fn pop_reset_all() {
    for n in 1..MAXTCHARS {
        reset_char(n);
    }
    for n in 1..MAXTITEM {
        reset_item(n);
    }
    log::info!("Reset all templates");
}

/// Port of `pop_wipe` from `populate.cpp`
/// Wipes all dynamic game data
pub fn pop_wipe() {
    // Clear all characters
    for n in 1..MAXCHARS {
        let is_player = Repository::with_characters(|characters| {
            (characters[n].flags & CharacterFlags::Player.bits()) != 0
        });

        if !is_player {
            Repository::with_characters_mut(|characters| {
                characters[n].used = USE_EMPTY;
            });
        }
    }

    // Clear all items
    for n in 1..MAXITEM {
        Repository::with_items_mut(|items| {
            items[n].used = USE_EMPTY;
        });
    }

    // Clear all effects
    for n in 1..MAXEFFECT {
        Repository::with_effects_mut(|effects| {
            effects[n].used = USE_EMPTY;
        });
    }

    // Reset global statistics
    Repository::with_globals_mut(|globals| {
        globals.players_created = 0;
        globals.npcs_created = 0;
        globals.players_died = 0;
        globals.npcs_died = 0;
        globals.expire_cnt = 0;
        globals.expire_run = 0;
        globals.gc_cnt = 0;
        globals.gc_run = 0;
        globals.lost_cnt = 0;
        globals.lost_run = 0;
        globals.reset_char = 0;
        globals.reset_item = 0;
        globals.total_online_time = 0;
        globals.uptime = 0;
    });

    log::info!("Wiped all dynamic game data");
}

/// Port of `pop_remove` from `populate.cpp`
/// Saves all players to disk
#[allow(dead_code)]
pub fn pop_remove() {
    log::info!("Saving players...");

    // TODO: Implement actual file saving when persistence system is ready
    // This would open .tmp/char.dat, .tmp/item.dat, .tmp/global.dat
    // and write out all player data

    let mut chc = 0;

    for n in 1..MAXCHARS {
        let (used, is_player) = Repository::with_characters(|characters| {
            (
                characters[n].used,
                (characters[n].flags & CharacterFlags::Player.bits()) != 0,
            )
        });

        if used != USE_EMPTY && is_player {
            // TODO: Write character to file
            chc += 1;
        }
    }

    log::info!("Saved {} player characters", chc);
}

/// Port of `pop_load` from `populate.cpp`
/// Loads game data from disk
#[allow(dead_code)]
pub fn pop_load() {
    log::info!("Loading game data...");

    // TODO: Implement actual file loading when persistence system is ready
    // This would read from data files and populate the repository

    log::info!("Game data loaded");
}

/// Port of `populate` from `populate.cpp`
/// Populates the world with NPCs
pub fn populate() {
    log::info!("Populating world...");

    // Iterate through all character templates and spawn respawnable NPCs
    for n in 1..MAXTCHARS {
        let (used, has_respawn) = Repository::with_character_templates(|templates| {
            (
                templates[n].used,
                (templates[n].flags & CharacterFlags::Respawn.bits()) != 0,
            )
        });

        if used != USE_EMPTY && has_respawn {
            if let Some(cn) = pop_create_char(n, true) {
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
pub fn pop_load_all_chars() {
    log::info!("Loading all characters...");

    for nr in 1..MAXCHARS {
        pop_load_char(nr);
    }

    log::info!("All characters loaded");
}

/// Port of `pop_save_all_chars` from `populate.cpp`
/// Saves all characters to disk
pub fn pop_save_all_chars() {
    log::info!("Saving all characters...");

    for nr in 1..MAXCHARS {
        let is_player = Repository::with_characters(|characters| {
            (characters[nr].flags & CharacterFlags::Player.bits()) != 0
        });

        if is_player {
            pop_save_char(nr);
        }
    }

    log::info!("All characters saved");
}
