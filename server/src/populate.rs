use crate::{driver, god::God, repository::Repository, state::State};

/// Port of `init_lights` from `populate.cpp`
/// Initialize lighting on the map
pub fn init_lights() {
    let mut cnt1 = 0;
    let mut cnt2 = 0;

    // First pass: clear all light and dlight values
    for y in 0..core::constants::SERVER_MAPY as usize {
        for x in 0..core::constants::SERVER_MAPX as usize {
            let m = x + y * core::constants::SERVER_MAPX as usize;
            Repository::with_map_mut(|map| {
                map[m].light = 0;
                map[m].dlight = 0;
            });
        }
    }

    // Second pass: compute dlight for indoor tiles, then add lights from items
    for y in 0..core::constants::SERVER_MAPY as usize {
        for x in 0..core::constants::SERVER_MAPX as usize {
            let m = x + y * core::constants::SERVER_MAPX as usize;

            // Compute daylight for indoor tiles first
            let is_indoors =
                Repository::with_map(|map| map[m].flags & core::constants::MF_INDOORS as u64 != 0);

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

    // Check for evil alignment special items (1/150 chance, multiple checks)
    if alignment < 0 && rand::random::<u32>() % 150 == 0 {
        in_id = match temp {
            27 => God::create_item(603),  // Dagger
            28 => God::create_item(604),  // Short Sword
            29 => God::create_item(605),  // Long Sword
            30 => God::create_item(606),  // Two-Handed Sword
            523 => God::create_item(607), // Claymore
            31 => God::create_item(608),  // Axe
            32 => God::create_item(609),  // Battle Axe
            33 => God::create_item(610),  // Two-Handed Axe
            34 => God::create_item(611),  // Staff
            524 => God::create_item(612), // Halberd
            35 => God::create_item(613),  // Dagger
            36 => God::create_item(614),  // Bone Club
            37 => God::create_item(615),  // Mace
            38 => God::create_item(616),  // Flail
            125 => God::create_item(617), // Warhammer
            _ => None,
        }
        .unwrap_or(0);
    }

    // Second check (armor)
    if in_id == 0 && alignment < 0 && rand::random::<u32>() % 150 == 0 {
        in_id = match temp {
            27 => God::create_item(618),  // Leather Helm
            28 => God::create_item(619),  // Chain Helm
            29 => God::create_item(620),  // Plate Helm
            30 => God::create_item(621),  // Great Helm
            523 => God::create_item(622), // War Helm
            31 => God::create_item(623),  // Leather Armor
            32 => God::create_item(624),  // Chain Armor
            33 => God::create_item(625),  // Plate Armor
            34 => God::create_item(626),  // Robe
            524 => God::create_item(627), // War Armor
            35 => God::create_item(628),  // Leather Gloves
            36 => God::create_item(629),  // Chain Gloves
            37 => God::create_item(630),  // Plate Gloves
            38 => God::create_item(631),  // Great Gloves
            125 => God::create_item(632), // War Gloves
            _ => None,
        }
        .unwrap_or(0);
    }

    // Third check (boots)
    if in_id == 0 && alignment < 0 && rand::random::<u32>() % 150 == 0 {
        in_id = match temp {
            27 => God::create_item(633),  // Leather Boots
            28 => God::create_item(634),  // Chain Boots
            29 => God::create_item(635),  // Plate Boots
            30 => God::create_item(636),  // Great Boots
            523 => God::create_item(637), // War Boots
            31 => God::create_item(638),  // Leather Belt
            32 => God::create_item(639),  // Chain Belt
            33 => God::create_item(640),  // Plate Belt
            34 => God::create_item(641),  // Sash
            524 => God::create_item(642), // War Belt
            35 => God::create_item(643),  // Leather Pants
            36 => God::create_item(644),  // Chain Pants
            37 => God::create_item(645),  // Plate Pants
            38 => God::create_item(646),  // Great Pants
            125 => God::create_item(647), // War Pants
            _ => None,
        }
        .unwrap_or(0);
    }

    // Fourth check (shields/cloaks)
    if in_id == 0 && alignment < 0 && rand::random::<u32>() % 150 == 0 {
        in_id = match temp {
            27 => God::create_item(648),  // Leather Shield
            28 => God::create_item(649),  // Chain Shield
            29 => God::create_item(650),  // Plate Shield
            30 => God::create_item(651),  // Great Shield
            523 => God::create_item(652), // War Shield
            31 => God::create_item(653),  // Leather Cloak
            32 => God::create_item(654),  // Chain Cloak
            33 => God::create_item(655),  // Plate Cloak
            34 => God::create_item(656),  // Robe Cloak
            524 => God::create_item(657), // War Cloak
            35 => God::create_item(658),  // Amulet
            36 => God::create_item(659),  // Ring
            37 => God::create_item(660),  // Bracelet
            38 => God::create_item(661),  // Earring
            125 => God::create_item(662), // Necklace
            _ => None,
        }
        .unwrap_or(0);
    }

    // Default: create item from template
    if in_id == 0 {
        let citem = Repository::with_characters(|characters| characters[cn].citem);
        if citem != 0 {
            in_id = God::create_item(Repository::with_items(|items| {
                items[citem as usize].temp as usize
            }))
            .unwrap_or(0);
        }
    } else {
        log::info!(
            "Created special item {} for character {} (template {})",
            in_id,
            cn,
            temp
        );
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
        skill_value = skill_value >> 1; // Divide by 2, max is rank/2 (max 12)
        if skill_value == 0 {
            skill_value = 1; // Ensure at least 1
        }

        Repository::with_items_mut(|items| {
            let item = &mut items[in_id];
            match skill_number {
                // Attributes
                0 => {
                    // Bravery (AT_BRAVE)
                    item.attrib[core::constants::AT_BRAVE as usize][0] += skill_value as i8;
                    if item.attrib[core::constants::AT_BRAVE as usize][0] > 12 {
                        item.attrib[core::constants::AT_BRAVE as usize][0] = 12;
                    }
                    item.attrib[core::constants::AT_BRAVE as usize][2] =
                        (10 + (item.attrib[core::constants::AT_BRAVE as usize][0] as i32
                            * (rand::random::<i32>() % 7))) as i8;
                }
                1 => {
                    // Willpower (AT_WILL)
                    item.attrib[core::constants::AT_WILL as usize][0] += skill_value as i8;
                    if item.attrib[core::constants::AT_WILL as usize][0] > 12 {
                        item.attrib[core::constants::AT_WILL as usize][0] = 12;
                    }
                    item.attrib[core::constants::AT_WILL as usize][2] =
                        (10 + (item.attrib[core::constants::AT_WILL as usize][0] as i32
                            * (rand::random::<i32>() % 7))) as i8;
                }
                2 => {
                    // Intuition (AT_INT)
                    item.attrib[core::constants::AT_INT as usize][0] += skill_value as i8;
                    if item.attrib[core::constants::AT_INT as usize][0] > 12 {
                        item.attrib[core::constants::AT_INT as usize][0] = 12;
                    }
                    item.attrib[core::constants::AT_INT as usize][2] =
                        (10 + (item.attrib[core::constants::AT_INT as usize][0] as i32
                            * (rand::random::<i32>() % 7))) as i8;
                }
                3 => {
                    // Agility (AT_AGIL)
                    item.attrib[core::constants::AT_AGIL as usize][0] += skill_value as i8;
                    if item.attrib[core::constants::AT_AGIL as usize][0] > 12 {
                        item.attrib[core::constants::AT_AGIL as usize][0] = 12;
                    }
                    item.attrib[core::constants::AT_AGIL as usize][2] =
                        (10 + (item.attrib[core::constants::AT_AGIL as usize][0] as i32
                            * (rand::random::<i32>() % 7))) as i8;
                }
                4 => {
                    // Strength (AT_STREN)
                    item.attrib[core::constants::AT_STREN as usize][0] += skill_value as i8;
                    if item.attrib[core::constants::AT_STREN as usize][0] > 12 {
                        item.attrib[core::constants::AT_STREN as usize][0] = 12;
                    }
                    item.attrib[core::constants::AT_STREN as usize][2] =
                        (10 + (item.attrib[core::constants::AT_STREN as usize][0] as i32
                            * (rand::random::<i32>() % 7))) as i8;
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
                    item.skill[core::constants::SK_WARCRY][0] += skill_value as i8;
                    if item.skill[core::constants::SK_WARCRY][0] > 12 {
                        item.skill[core::constants::SK_WARCRY][0] = 12;
                    }
                }
                // Hand to Hand
                10 => {
                    item.skill[core::constants::SK_HAND][0] += skill_value as i8;
                    if item.skill[core::constants::SK_HAND][0] > 12 {
                        item.skill[core::constants::SK_HAND][0] = 12;
                    }
                    item.skill[core::constants::SK_HAND][2] =
                        (item.skill[core::constants::SK_HAND][0] as i32
                            * (rand::random::<i32>() % 7)) as i8;
                }
                // Sword
                11 => {
                    item.skill[core::constants::SK_SWORD][0] += skill_value as i8;
                    if item.skill[core::constants::SK_SWORD][0] > 12 {
                        item.skill[core::constants::SK_SWORD][0] = 12;
                    }
                }
                // Dagger
                12 => {
                    item.skill[core::constants::SK_DAGGER][0] += skill_value as i8;
                    if item.skill[core::constants::SK_DAGGER][0] > 12 {
                        item.skill[core::constants::SK_DAGGER][0] = 12;
                    }
                }
                // Two-Handed
                13 => {
                    item.skill[core::constants::SK_TWOHAND][0] += skill_value as i8;
                    if item.skill[core::constants::SK_TWOHAND][0] > 12 {
                        item.skill[core::constants::SK_TWOHAND][0] = 12;
                    }
                }
                // Lockpick
                14 => {
                    item.skill[core::constants::SK_LOCK][0] += skill_value as i8;
                    if item.skill[core::constants::SK_LOCK][0] > 12 {
                        item.skill[core::constants::SK_LOCK][0] = 12;
                    }
                    item.skill[core::constants::SK_LOCK][2] =
                        (item.skill[core::constants::SK_LOCK][0] as i32
                            * (rand::random::<i32>() % 7)) as i8;
                }
                // Stealth
                15 => {
                    item.skill[core::constants::SK_STEALTH][0] += skill_value as i8;
                    if item.skill[core::constants::SK_STEALTH][0] > 12 {
                        item.skill[core::constants::SK_STEALTH][0] = 12;
                    }
                }
                // Perception
                16 => {
                    item.skill[core::constants::SK_PERCEPT][0] += skill_value as i8;
                    if item.skill[core::constants::SK_PERCEPT][0] > 12 {
                        item.skill[core::constants::SK_PERCEPT][0] = 12;
                    }
                    item.skill[core::constants::SK_PERCEPT][2] =
                        (item.skill[core::constants::SK_PERCEPT][0] as i32
                            * (rand::random::<i32>() % 7)) as i8;
                }
                // Magic Shield
                17 => {
                    item.skill[core::constants::SK_MSHIELD][0] += skill_value as i8;
                    if item.skill[core::constants::SK_MSHIELD][0] > 12 {
                        item.skill[core::constants::SK_MSHIELD][0] = 12;
                    }
                }
                // Barter
                18 => {
                    item.skill[core::constants::SK_BARTER][0] += skill_value as i8;
                    if item.skill[core::constants::SK_BARTER][0] > 12 {
                        item.skill[core::constants::SK_BARTER][0] = 12;
                    }
                    item.skill[core::constants::SK_BARTER][2] =
                        (item.skill[core::constants::SK_BARTER][0] as i32
                            * (rand::random::<i32>() % 7)) as i8;
                }
                // Repair
                19 => {
                    item.skill[core::constants::SK_REPAIR][0] += skill_value as i8;
                    if item.skill[core::constants::SK_REPAIR][0] > 12 {
                        item.skill[core::constants::SK_REPAIR][0] = 12;
                    }
                    item.skill[core::constants::SK_REPAIR][2] =
                        (item.skill[core::constants::SK_REPAIR][0] as i32
                            * (rand::random::<i32>() % 7)) as i8;
                }
                // Light
                20 => {
                    item.skill[core::constants::SK_LIGHT][0] += skill_value as i8;
                    if item.skill[core::constants::SK_LIGHT][0] > 12 {
                        item.skill[core::constants::SK_LIGHT][0] = 12;
                    }
                    item.skill[core::constants::SK_LIGHT][2] =
                        (item.skill[core::constants::SK_LIGHT][0] as i32
                            * (rand::random::<i32>() % 7)) as i8;
                }
                // Recall
                21 => {
                    item.skill[core::constants::SK_RECALL][0] += skill_value as i8;
                    if item.skill[core::constants::SK_RECALL][0] > 12 {
                        item.skill[core::constants::SK_RECALL][0] = 12;
                    }
                    item.skill[core::constants::SK_RECALL][2] =
                        (item.skill[core::constants::SK_RECALL][0] as i32
                            * (rand::random::<i32>() % 7)) as i8;
                }
                // Protect
                22 => {
                    item.skill[core::constants::SK_PROTECT][0] += skill_value as i8;
                    if item.skill[core::constants::SK_PROTECT][0] > 12 {
                        item.skill[core::constants::SK_PROTECT][0] = 12;
                    }
                    item.skill[core::constants::SK_PROTECT][2] =
                        (item.skill[core::constants::SK_PROTECT][0] as i32
                            * (rand::random::<i32>() % 7)) as i8;
                }
                // Enhance
                23 => {
                    item.skill[core::constants::SK_ENHANCE][0] += skill_value as i8;
                    if item.skill[core::constants::SK_ENHANCE][0] > 12 {
                        item.skill[core::constants::SK_ENHANCE][0] = 12;
                    }
                    item.skill[core::constants::SK_ENHANCE][2] =
                        (item.skill[core::constants::SK_ENHANCE][0] as i32
                            * (rand::random::<i32>() % 7)) as i8;
                }
                // Stun
                24 => {
                    item.skill[core::constants::SK_STUN][0] += skill_value as i8;
                    if item.skill[core::constants::SK_STUN][0] > 12 {
                        item.skill[core::constants::SK_STUN][0] = 12;
                    }
                }
                // Curse
                25 => {
                    item.skill[core::constants::SK_CURSE][0] += skill_value as i8;
                    if item.skill[core::constants::SK_CURSE][0] > 12 {
                        item.skill[core::constants::SK_CURSE][0] = 12;
                    }
                }
                // Bless
                26 => {
                    item.skill[core::constants::SK_BLESS][0] += skill_value as i8;
                    if item.skill[core::constants::SK_BLESS][0] > 12 {
                        item.skill[core::constants::SK_BLESS][0] = 12;
                    }
                    item.skill[core::constants::SK_BLESS][2] =
                        (item.skill[core::constants::SK_BLESS][0] as i32
                            * (rand::random::<i32>() % 7)) as i8;
                }
                // Identify
                27 => {
                    item.skill[core::constants::SK_IDENT][0] += skill_value as i8;
                    if item.skill[core::constants::SK_IDENT][0] > 12 {
                        item.skill[core::constants::SK_IDENT][0] = 12;
                    }
                    item.skill[core::constants::SK_IDENT][2] =
                        (item.skill[core::constants::SK_IDENT][0] as i32
                            * (rand::random::<i32>() % 7)) as i8;
                }
                // Resist
                28 => {
                    item.skill[core::constants::SK_RESIST][0] += skill_value as i8;
                    if item.skill[core::constants::SK_RESIST][0] > 12 {
                        item.skill[core::constants::SK_RESIST][0] = 12;
                    }
                    item.skill[core::constants::SK_RESIST][2] =
                        (item.skill[core::constants::SK_RESIST][0] as i32
                            * (rand::random::<i32>() % 7)) as i8;
                }
                // Blast
                29 => {
                    item.skill[core::constants::SK_BLAST][0] += skill_value as i8;
                    if item.skill[core::constants::SK_BLAST][0] > 12 {
                        item.skill[core::constants::SK_BLAST][0] = 12;
                    }
                }
                // Dispel
                30 => {
                    item.skill[core::constants::SK_DISPEL][0] += skill_value as i8;
                    if item.skill[core::constants::SK_DISPEL][0] > 12 {
                        item.skill[core::constants::SK_DISPEL][0] = 12;
                    }
                }
                // Heal
                31 => {
                    item.skill[core::constants::SK_HEAL][0] += skill_value as i8;
                    if item.skill[core::constants::SK_HEAL][0] > 12 {
                        item.skill[core::constants::SK_HEAL][0] = 12;
                    }
                    item.skill[core::constants::SK_HEAL][2] =
                        (item.skill[core::constants::SK_HEAL][0] as i32
                            * (rand::random::<i32>() % 7)) as i8;
                }
                // Ghost
                32 => {
                    item.skill[core::constants::SK_GHOST][0] += skill_value as i8;
                    if item.skill[core::constants::SK_GHOST][0] > 12 {
                        item.skill[core::constants::SK_GHOST][0] = 12;
                    }
                }
                // Regeneration
                33 => {
                    item.skill[core::constants::SK_REGEN][0] += skill_value as i8;
                    if item.skill[core::constants::SK_REGEN][0] > 12 {
                        item.skill[core::constants::SK_REGEN][0] = 12;
                    }
                }
                // Rest
                34 => {
                    item.skill[core::constants::SK_REST][0] += skill_value as i8;
                    if item.skill[core::constants::SK_REST][0] > 12 {
                        item.skill[core::constants::SK_REST][0] = 12;
                    }
                    item.skill[core::constants::SK_REST][2] =
                        (item.skill[core::constants::SK_REST][0] as i32
                            * (rand::random::<i32>() % 7)) as i8;
                }
                // Meditation
                35 => {
                    item.skill[core::constants::SK_MEDIT][0] += skill_value as i8;
                    if item.skill[core::constants::SK_MEDIT][0] > 12 {
                        item.skill[core::constants::SK_MEDIT][0] = 12;
                    }
                }
                // Sense
                36 => {
                    item.skill[core::constants::SK_SENSE][0] += skill_value as i8;
                    if item.skill[core::constants::SK_SENSE][0] > 12 {
                        item.skill[core::constants::SK_SENSE][0] = 12;
                    }
                    item.skill[core::constants::SK_SENSE][2] =
                        (item.skill[core::constants::SK_SENSE][0] as i32
                            * (rand::random::<i32>() % 7)) as i8;
                }
                // Immunity
                37 => {
                    item.skill[core::constants::SK_IMMUN][0] += skill_value as i8;
                    if item.skill[core::constants::SK_IMMUN][0] > 12 {
                        item.skill[core::constants::SK_IMMUN][0] = 12;
                    }
                }
                // Surround Hit
                38 => {
                    item.skill[core::constants::SK_SURROUND][0] += skill_value as i8;
                    if item.skill[core::constants::SK_SURROUND][0] > 12 {
                        item.skill[core::constants::SK_SURROUND][0] = 12;
                    }
                }
                // Concentration
                39 => {
                    item.skill[core::constants::SK_CONCEN][0] += skill_value as i8;
                    if item.skill[core::constants::SK_CONCEN][0] > 12 {
                        item.skill[core::constants::SK_CONCEN][0] = 12;
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
pub fn pop_create_char(n: usize, drop: bool) -> usize {
    let cn = God::create_char(n, true);
    if cn.is_none() {
        return 0;
    }
    let cn = cn.unwrap() as usize;

    // Set initial state
    Repository::with_characters_mut(|characters| {
        characters[cn].a_end = 1000000;
        characters[cn].a_hp = 1000000;

        let has_meditation = characters[cn].skill[core::constants::SK_MEDIT][0] != 0;
        if has_meditation {
            characters[cn].a_mana = characters[cn].mana[5] as i32 * 100;
        } else {
            characters[cn].a_mana = 1000000;
        }

        characters[cn].dir = core::constants::DX_DOWN;
        characters[cn].data[92] = core::constants::TICKS * 60;
    });

    // Create bonus items based on mana level
    let a_mana = Repository::with_characters(|characters| characters[cn].a_mana);
    let has_meditation = Repository::with_characters(|characters| {
        characters[cn].skill[core::constants::SK_MEDIT][0] != 0
    });

    let mut chance = 25;
    if !has_meditation && a_mana > 15 * 100 {
        chance = 50;
    }
    if !has_meditation && a_mana > 30 * 100 {
        chance = 100;
    }
    if !has_meditation && a_mana > 65 * 100 {
        chance = 200;
    }

    let alignment = Repository::with_characters(|characters| characters[cn].alignment);
    if alignment < 0 {
        // Create bonus items for evil characters
        for _ in 0..4 {
            if rand::random::<u32>() % chance == 0 {
                let bonus = pop_create_bonus(cn, chance as i32);
                if bonus != 0 {
                    God::give_character_item(cn, bonus as usize);
                }
            }
        }

        // Check for special belt
        if rand::random::<u32>() % 10000 == 0 {
            let belt = pop_create_bonus_belt(cn);
            if belt != 0 {
                God::give_character_item(cn, belt as usize);
            }
        }
    }

    // Drop character on map if requested
    if drop {
        let (x, y) = Repository::with_character_templates(|templates| {
            (templates[n].x as usize, templates[n].y as usize)
        });

        if !God::drop_char_fuzzy(cn, x, y) {
            log::error!("Failed to drop character {} at ({}, {})", cn, x, y);
        }
    }

    State::with(|state| state.do_update_char(cn));

    Repository::with_globals_mut(|globals| {
        globals.npcs_created += 1;
    });

    cn
}

/// Port of `reset_char` from `populate.cpp`
/// Resets a character template and all instances
pub fn reset_char(n: usize) {
    if n < 1 || n >= core::constants::MAXTCHARS {
        return;
    }

    let (used, has_respawn) = Repository::with_character_templates(|templates| {
        (
            templates[n].used,
            (templates[n].flags & core::constants::CharacterFlags::CF_RESPAWN.bits()) != 0,
        )
    });

    if used == core::constants::USE_EMPTY || !has_respawn {
        return;
    }

    let name = Repository::with_character_templates(|templates| {
        String::from_utf8_lossy(&templates[n].name).to_string()
    });
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

    // Update all instances of this template
    for cn in 1..core::constants::MAXCHARS {
        let temp = Repository::with_characters(|characters| characters[cn].temp);
        if temp as usize == n {
            Repository::with_characters_mut(|characters| {
                let char_template =
                    Repository::with_character_templates(|templates| templates[n]);

                // Preserve certain fields
                let pass1 = characters[cn].pass1;
                let pass2 = characters[cn].pass2;
                let x = characters[cn].x;
                let y = characters[cn].y;

                characters[cn] = char_template;
                characters[cn].pass1 = pass1;
                characters[cn].pass2 = pass2;
                characters[cn].x = x;
                characters[cn].y = y;
                characters[cn].temp = n as u16;
            });
            cnt += 1;
        }
    }

    // Update effects referencing this template
    for m in 0..core::constants::MAXEFFECT {
        let data0 = Repository::with_effects(|effects| effects[m].data[0]);
        if data0 == n as u32 {
            Repository::with_effects_mut(|effects| {
                effects[m].data[1] = 1; // Mark for respawn
            });
        }
    }

    // Update items carried by template
    for m in 0..core::constants::MAXITEM {
        let carried = Repository::with_items(|items| items[m].carried);
        if carried as usize == n {
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
        log::warn!("Reset char {}: found {} instances", n, cnt);
    }

    let template_used = Repository::with_character_templates(|templates| templates[n].used);
    if template_used == core::constants::USE_ACTIVE {
        log::info!("Marked template {} for respawn", n);
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
    for cn in 1..core::constants::MAXCHARS {
        let is_player = Repository::with_characters(|characters| {
            (characters[cn].flags & core::constants::CharacterFlags::CF_PLAYER.bits()) != 0
                && characters[cn].used == core::constants::USE_ACTIVE
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
    if n < 2 || n >= core::constants::MAXTITEM {
        return; // Never reset blank template (1)
    }

    let name = Repository::with_item_templates(|templates| {
        String::from_utf8_lossy(&templates[n].name).to_string()
    });
    log::info!("Resetting item {} ({})", n, name);

    for in_id in 1..core::constants::MAXITEM {
        let temp = Repository::with_items(|items| items[in_id].temp);
        if temp as usize != n {
            continue;
        }

        let used = Repository::with_items(|items| items[in_id].used);
        if used == core::constants::USE_EMPTY {
            continue;
        }

        // Reset item from template
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

        log::debug!("Reset item instance {}", in_id);
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
    const RESETTICKER: i32 = core::constants::TICKS * 60;

    static mut LAST_RESET: i32 = 0;

    let ticker = Repository::with_globals(|globals| globals.ticker);

    unsafe {
        if ticker - LAST_RESET >= RESETTICKER {
            LAST_RESET = ticker;
            log::info!("Population tick: checking for resets");
        }
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
    for n in 1..core::constants::MAXTCHARS {
        reset_char(n);
    }
    for n in 1..core::constants::MAXTITEM {
        reset_item(n);
    }
    log::info!("Reset all templates");
}

/// Port of `pop_wipe` from `populate.cpp`
/// Wipes all dynamic game data
pub fn pop_wipe() {
    // Clear all characters
    for n in 1..core::constants::MAXCHARS {
        let is_player = Repository::with_characters(|characters| {
            (characters[n].flags & core::constants::CharacterFlags::CF_PLAYER.bits()) != 0
        });

        if !is_player {
            Repository::with_characters_mut(|characters| {
                characters[n].used = core::constants::USE_EMPTY;
            });
        }
    }

    // Clear all items
    for n in 1..core::constants::MAXITEM {
        Repository::with_items_mut(|items| {
            items[n].used = core::constants::USE_EMPTY;
        });
    }

    // Clear all effects
    for n in 1..core::constants::MAXEFFECT {
        Repository::with_effects_mut(|effects| {
            effects[n].used = core::constants::USE_EMPTY;
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

    for n in 1..core::constants::MAXCHARS {
        let (used, is_player) = Repository::with_characters(|characters| {
            (
                characters[n].used,
                (characters[n].flags & core::constants::CharacterFlags::CF_PLAYER.bits()) != 0,
            )
        });

        if used != core::constants::USE_EMPTY && is_player {
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
    for n in 1..core::constants::MAXTCHARS {
        let (used, has_respawn) = Repository::with_character_templates(|templates| {
            (
                templates[n].used,
                (templates[n].flags & core::constants::CharacterFlags::CF_RESPAWN.bits()) != 0,
            )
        });

        if used != core::constants::USE_EMPTY && has_respawn {
            let cn = pop_create_char(n, true);
            if cn != 0 {
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

    for nr in 1..core::constants::MAXCHARS {
        pop_load_char(nr);
    }

    log::info!("All characters loaded");
}

/// Port of `pop_save_all_chars` from `populate.cpp`
/// Saves all characters to disk
pub fn pop_save_all_chars() {
    log::info!("Saving all characters...");

    for nr in 1..core::constants::MAXCHARS {
        let is_player = Repository::with_characters(|characters| {
            (characters[nr].flags & core::constants::CharacterFlags::CF_PLAYER.bits()) != 0
        });

        if is_player {
            pop_save_char(nr);
        }
    }

    log::info!("All characters saved");
}
