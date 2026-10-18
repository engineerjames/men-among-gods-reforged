use crate::effect::EffectManager;
use crate::game_state::GameState;
use crate::god::God;
use crate::helpers::{self};
use core::constants::{
    AT_AGIL, AT_BRAVE, AT_INT, AT_STREN, AT_WILL, CharacterFlags, ItemFlags, MAXITEM, USE_ACTIVE,
    USE_EMPTY, WN_RHAND,
};
use core::skills::{SK_IMMUN, SK_RESIST};
use core::string_operations::c_string_to_str;
use core::traits;

/// Handles the legacy `use_shrine` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `_item_idx` - Value passed through to the legacy `use_shrine` hook.
///
/// # Returns
///
/// * `true` when `use_shrine` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_shrine` is outside the corresponding game-state collection.
pub fn use_shrine(gs: &mut GameState, cn: usize, _item_idx: usize) -> bool {
    if cn == 0 {
        return false;
    }

    let in2 = gs.characters[cn].citem as usize;

    if in2 == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "You get the feeling that it would be apropriate to give the gods a present.\n",
        );
        return false;
    }

    // Special-case: ONE FIRE POINT / ONE FAKE POINT
    if (in2 & 0x80000000) == 0 {
        let desc = {
            if in2 >= gs.items.len() {
                String::new()
            } else {
                c_string_to_str(&gs.items[in2].description).to_owned()
            }
        };

        if desc == "ONE FIRE POINT" || desc == "ONE FAKE POINT" {
            let is_fire = desc == "ONE FIRE POINT";

            if is_fire {
                {
                    gs.characters[cn].data[70] += 1;
                };
                let fp = gs.characters[cn].data[70];
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!(
                        "One fire point accounted for. You now have {} fire points.\n",
                        fp
                    ),
                );
            } else {
                let fp = gs.characters[cn].data[70];
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("Err, that's a fake point. You have {} fire points.\n", fp),
                );
            }

            if in2 < gs.items.len() {
                gs.items[in2].used = USE_EMPTY;
            };
            gs.characters[cn].citem = 0;

            let (better, worse, equal, bestval, bestcn, bestcount) = {
                let mut better = 0;
                let mut worse = 0;
                let mut equal = 0;
                let mut bestval = 0;
                let mut bestcn = 0;
                let mut bestcount = 0;

                for m in 1..core::constants::MAXCHARS {
                    if gs.characters[m].used == core::constants::USE_EMPTY {
                        continue;
                    }
                    if (gs.characters[m].flags & CharacterFlags::Player.bits()) == 0 {
                        continue;
                    }
                    if gs.characters[m].data[70] == 0 {
                        continue;
                    }

                    if gs.characters[m].data[70] > gs.characters[cn].data[70] {
                        better += 1;
                    } else if gs.characters[m].data[70] < gs.characters[cn].data[70] {
                        worse += 1;
                    } else {
                        equal += 1;
                    }

                    if gs.characters[m].data[70] > bestval {
                        bestval = gs.characters[m].data[70];
                        bestcn = m;
                        bestcount = 0;
                    }
                    if gs.characters[m].data[70] == bestval {
                        bestcount += 1;
                    }
                }

                (better, worse, equal, bestval, bestcn, bestcount)
            };

            if equal > 1 {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!(
                        "Your rank is {}, there are {} participating players of the same rank, {} are worse.\n",
                        better + 1,
                        equal - 1,
                        worse
                    ),
                );
            } else {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!(
                        "Your rank is {} and {} participating players are worse.\n",
                        better + 1,
                        worse
                    ),
                );
            }

            gs.do_character_log(cn, core::types::FontColor::Yellow, " \n");

            if bestcount == 1 {
                let name = { gs.characters[bestcn].get_name().to_owned() };
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!(
                        "First place holder is {} with {} fire points.\n",
                        name, bestval
                    ),
                );
            } else {
                gs.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!(
                        "First place is shared by {} players, all with {} fire points:\n",
                        bestcount, bestval
                    ),
                );
                gs.do_character_log(cn, core::types::FontColor::Yellow, " \n");

                for m in 1..core::constants::MAXCHARS {
                    if gs.characters[m].used == core::constants::USE_EMPTY {
                        continue;
                    }
                    if (gs.characters[m].flags & CharacterFlags::Player.bits()) == 0 {
                        continue;
                    }
                    if gs.characters[m].data[70] == 0 {
                        continue;
                    }
                    if gs.characters[m].data[70] == bestval {
                        let name = gs.characters[m].get_name();
                        gs.do_character_log(
                            cn,
                            core::types::FontColor::Yellow,
                            &format!("{}\n", name),
                        );
                    }
                }
            }

            gs.do_character_log(cn, core::types::FontColor::Yellow, " \n");

            return false;
        }
    }

    // Calculate item value
    let val = if (in2 & 0x80000000) != 0 {
        // Money
        let val = (in2 & 0x7fffffff) as i32;
        gs.characters[cn].citem = 0;

        val
    } else {
        // Item
        let value = {
            let mut val = gs.items[in2].value;
            if (gs.items[in2].flags & ItemFlags::IF_UNIQUE.bits()) != 0 {
                val *= 4;
            }
            val
        };

        gs.items[in2].used = USE_EMPTY;
        gs.characters[cn].citem = 0;
        value as i32
    };

    let val = val + crate::helpers::random_mod(val as u32 + 1) as i32;

    // Calculate rank threshold
    let rank = {
        let r = core::ranks::points2rank(gs.characters[cn].points_tot as u32) as i32 + 1;
        r * r * r * 4
    };

    // Check if offering is acceptable
    if val >= rank {
        // Restore mana
        let mana_restored = {
            if gs.characters[cn].a_mana < i32::from(gs.characters[cn].mana[5]) * 1000 {
                gs.characters[cn].a_mana = i32::from(gs.characters[cn].mana[5]) * 1000;
                true
            } else {
                false
            }
        };

        if mana_restored {
            gs.do_character_log(
                cn,
                core::types::FontColor::Green,
                "You feel the hand of the Goddess of Magic touch your mind.\n",
            );
            let (x, y) = {
                (
                    i32::from(gs.characters[cn].x),
                    i32::from(gs.characters[cn].y),
                )
            };

            EffectManager::fx_add_effect(gs, 6, 0, x, y, 0);
        }

        // Determine message based on value
        let message = if val >= rank * 64 {
            "The gods are madly in love with your offer.\n"
        } else if val >= rank * 32 {
            "The gods love your offer very much.\n"
        } else if val >= rank * 16 {
            "The gods love your offer.\n"
        } else if val >= rank * 8 {
            "The gods are very pleased with your offer.\n"
        } else if val >= rank * 4 {
            "The gods are pleased with your offer.\n"
        } else if val >= rank * 2 {
            "The gods deemed your offer apropriate.\n"
        } else {
            "The gods accepted your offer.\n"
        };

        gs.do_character_log(cn, core::types::FontColor::Yellow, message);

        // Increase luck
        if val != 0 && rank != 0 {
            let m = val / rank;
            gs.characters[cn].luck += m;
        }
    } else {
        // Offering not good enough
        let (message, luck_change) = if val < rank / 8 {
            ("You have angered the gods with your unworthy gift.\n", -2)
        } else if val < rank / 4 {
            ("The gods sneer at your gift.\n", -1)
        } else if val < rank / 2 {
            ("The gods think you're cheap.\n", 0)
        } else {
            (
                "You feel that it takes more than this to please the gods.\n",
                0,
            )
        };

        gs.do_character_log(cn, core::types::FontColor::Yellow, message);

        if luck_change != 0 {
            gs.characters[cn].luck += luck_change;
        }
    }

    // Show luck status
    gs.do_character_log(cn, core::types::FontColor::Yellow, " \n");

    let luck = gs.characters[cn].luck;
    let luck_message = if luck < -10000 {
        "You feel that the gods are mad at you.\n"
    } else if luck < 0 {
        "You feel that the gods are angry at you.\n"
    } else if luck < 100 {
        "You feel that the gods stance towards you is neutral.\n"
    } else if luck < 1000 {
        "You feel that the gods are pleased with you.\n"
    } else {
        "You feel that the gods are very fond of you.\n"
    };

    gs.do_character_log(cn, core::types::FontColor::Yellow, luck_message);

    true
}

/// Handles the legacy `use_seyan_shrine` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_seyan_shrine` hook.
///
/// # Returns
///
/// * `true` when `use_seyan_shrine` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_seyan_shrine` is outside the corresponding game-state collection.
pub fn use_seyan_shrine(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    if cn == 0 {
        return false;
    }

    // Check if character is Seyan'Du
    let is_seyan = (gs.characters[cn].kindred & traits::KIN_SEYAN_DU as i32) != 0;

    if !is_seyan {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "You have the feeling you're in the wrong place here.\n",
        );
        return false;
    }

    // Check for existing Seyan'Du sword (driver 40)
    let mut in2 = gs.characters[cn].worn[WN_RHAND] as usize;

    let sword_valid = if in2 != 0 {
        gs.items[in2].driver == 40 && gs.items[in2].data[0] == cn as u32
    } else {
        false
    };

    // If no valid sword, replace old ones and create new one
    if !sword_valid {
        // Remove old swords (driver 40 for this character)
        for n in 1..MAXITEM {
            if n >= gs.items.len() {
                break;
            }
            let should_replace = gs.items[n].used == USE_ACTIVE
                && gs.items[n].driver == 40
                && gs.items[n].data[0] == cn as u32;

            if should_replace {
                // Replace with broken sword (template 683)
                let (x, y, carried) = (gs.items[n].x, gs.items[n].y, gs.items[n].carried);

                if let Some(broken_sword) = God::create_item(gs, 683) {
                    if broken_sword == 0 {
                        continue;
                    }

                    gs.items[broken_sword].x = x;
                    gs.items[broken_sword].y = y;
                    gs.items[broken_sword].carried = carried;
                    gs.items[broken_sword].temp = 683;
                    gs.items[broken_sword].flags |= ItemFlags::IF_UPDATE.bits();

                    gs.items[n].used = USE_EMPTY;
                }
            }
        }

        // Check luck requirement
        let luck = gs.characters[cn].luck;
        if luck < 50 {
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "Kwai, the great goddess of war, deemed you unworthy to receive a new blade.\n",
            );
            return false;
        }

        // Create new Seyan'Du sword (template 682)
        let Some(new_sword) = God::create_item(gs, 682) else {
            return false;
        };
        if new_sword == 0 {
            return false;
        }
        in2 = new_sword;
        God::give_character_item(gs, cn, in2);
        {
            gs.items[in2].data[0] = cn as u32;
        };
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Kwai, the great goddess of war, deemed you worthy to receive a new blade.\n",
        );

        gs.characters[cn].luck -= 50;
    }

    // Mark this shrine as visited
    let shrine_bit = gs.items[item_idx].data[0];
    let already_visited = { (gs.characters[cn].data[21] as u32 & shrine_bit) != 0 };

    if !already_visited {
        gs.characters[cn].data[21] |= shrine_bit as i32;

        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "You found a new shrine of Kwai!\n",
        );
        gs.characters[cn].luck += 10;
    }

    // Count visited shrines
    let visited_bits = {
        let mut count = 0;
        let mut bit = 1u32;
        while bit != 0 {
            if (gs.characters[cn].data[21] & bit as i32) != 0 {
                count += 1;
            }
            bit = bit.wrapping_shl(1);
        }
        count
    };

    gs.do_character_log(
        cn,
        core::types::FontColor::Green,
        &format!(
            "You have visited {} of the 20 shrines of Kwai.\n",
            visited_bits
        ),
    );

    // Update sword weapon power based on shrines visited
    let cn_name = gs.characters[cn].name;

    gs.items[in2].weapon[0] = 15 + visited_bits * 4;

    // Grant 1 of all attributes per shrine visited
    gs.items[in2].attrib[AT_BRAVE as usize][0] = i16::from(visited_bits);
    gs.items[in2].attrib[AT_WILL as usize][0] = i16::from(visited_bits);
    gs.items[in2].attrib[AT_INT as usize][0] = i16::from(visited_bits);
    gs.items[in2].attrib[AT_STREN as usize][0] = i16::from(visited_bits);
    gs.items[in2].attrib[AT_AGIL as usize][0] = i16::from(visited_bits);

    // Grant 25 hp/end/mana per 5 shrines visited
    let bonus_stats = (visited_bits / 5) * 25;
    log::info!(
        "Character {} visited {} shrines, granting {} bonus stats to sword.",
        cn,
        visited_bits,
        bonus_stats
    );
    gs.items[in2].hp[0] = i16::from(bonus_stats);
    gs.items[in2].end[0] = i16::from(bonus_stats);
    gs.items[in2].mana[0] = i16::from(bonus_stats);

    // Every shrine beyond 10 should also give +5 immunity and resistance
    let bonus_resist = visited_bits.saturating_sub_unsigned(10) * 5;

    if bonus_resist > 0 {
        log::info!(
            "Character {} visited {} shrines, granting {} bonus resistances to sword.",
            cn,
            visited_bits,
            bonus_resist
        );
        gs.items[in2].skill[SK_IMMUN][0] = i16::from(bonus_resist);
        gs.items[in2].skill[SK_RESIST][0] = i16::from(bonus_resist);
    } else {
        log::info!(
            "Character {} visited {} shrines, granting no bonus resistances to sword.",
            cn,
            visited_bits
        );
    }

    gs.items[in2].flags |= ItemFlags::IF_UPDATE.bits();
    gs.items[in2].temp = 0;
    let description = format!(
        "A huge, two-handed sword, engraved with runes and magic symbols. It bears the name {}.",
        c_string_to_str(&cn_name)
    );
    let desc_bytes = description.as_bytes();
    let len = desc_bytes.len().min(gs.items[in2].description.len());
    gs.items[in2].description[..len].copy_from_slice(&desc_bytes[..len]);
    if len < gs.items[in2].description.len() {
        gs.items[in2].description[len..].fill(0);
    }

    gs.do_update_char(cn);

    false
}

/// Handles the legacy `use_lab8_shrine` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_lab8_shrine` hook.
///
/// # Returns
///
/// * `true` when `use_lab8_shrine` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_lab8_shrine` is outside the corresponding game-state collection.
pub fn use_lab8_shrine(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    // data[0] = item accepted as offering
    // data[1] = item returned as gift

    if cn == 0 {
        return false;
    }

    let offer = gs.characters[cn].citem as usize;
    if offer == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "You get the feeling that it would be appropriate to give the Goddess a present.\n",
        );
        return false;
    }

    // Money citems carry the 0x80000000 bit and are not item indices.
    if (offer & 0x80000000) != 0 || u32::from(gs.items[offer].temp) != gs.items[item_idx].data[0] {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "The Goddess only wants her property back, and rejects your offer.\n",
        );
        return false;
    }

    // Accept offering
    {
        gs.characters[cn].citem = 0;
    };
    {
        gs.items[offer].used = USE_EMPTY;
    };

    // Log the offering
    let (offer_ref, shrine_ref) = {
        (
            c_string_to_str(&gs.items[offer].reference).to_owned(),
            c_string_to_str(&gs.items[item_idx].reference).to_owned(),
        )
    };
    log::info!("Offered {} at {}", offer_ref, shrine_ref);

    // Create and give gift
    let gift_template = gs.items[item_idx].data[1];
    let gift = God::create_item(gs, gift_template as usize);

    if !God::give_character_item(gs, cn, gift.unwrap()) {
        // If inventory full, put in carried
        {
            gs.characters[cn].citem = gift.unwrap() as u32;
        };
        {
            gs.items[gift.unwrap()].carried = cn as u16;
        };
    }

    let gift_ref = { c_string_to_str(&gs.items[gift.unwrap()].reference).to_owned() };
    gs.do_character_log(
        cn,
        core::types::FontColor::Yellow,
        &format!("The Goddess has given you a {} in return!\n", gift_ref),
    );

    true
}

/// Handles the legacy `use_lab8_moneyshrine` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `use_lab8_moneyshrine` hook.
///
/// # Returns
///
/// * `true` when `use_lab8_moneyshrine` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `use_lab8_moneyshrine` is outside the corresponding game-state collection.
pub fn use_lab8_moneyshrine(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    // data[0] = minimum offering accepted
    // data[1] = teleport coordinate x
    // data[2] = teleport coordinate y

    if cn == 0 {
        return false;
    }

    let offer = gs.characters[cn].citem;
    if offer == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "You get the feeling that it would be appropriate to give the Goddess a present.\n",
        );
        return false;
    }

    // Check if it's money
    if (offer & 0x80000000) == 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Only money is accepted at this shrine.\n",
        );
        return false;
    }

    let amount = offer & 0x7fffffff;
    let min_offering = gs.items[item_idx].data[0];

    if amount < min_offering {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Your offering is not sufficient, and was rejected.\n",
        );
        return false;
    }

    // Log offering
    let shrine_ref = c_string_to_str(&gs.items[item_idx].reference).to_owned();
    log::info!("offered {}G at {}", amount / 100, shrine_ref);

    // Accept money and teleport
    {
        gs.characters[cn].citem = 0;
    };

    let (dest_x, dest_y) = (gs.items[item_idx].data[1], gs.items[item_idx].data[2]);
    God::transfer_char(gs, cn, dest_x as usize, dest_y as usize);

    // Restore mana if needed
    let (a_mana, max_mana) = {
        (
            gs.characters[cn].a_mana,
            i32::from(gs.characters[cn].mana[5]) * 1000,
        )
    };

    if a_mana < max_mana {
        {
            gs.characters[cn].a_mana = i32::from(gs.characters[cn].mana[5]) * 1000;
        };
        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "You feel the hand of the Goddess of Magic touch your mind.\n",
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

    true
}

/// Handles the legacy `change_to_archtemplar` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `change_to_archtemplar` is outside the corresponding game-state collection.
pub fn change_to_archtemplar(gs: &mut GameState, cn: usize) {
    // Check agility requirement
    let agility = gs.characters[cn].attrib[AT_AGIL as usize][0];
    if agility < 90 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Your agility is too low. There is still room for improvement.\n",
        );
        return;
    }

    // Check strength requirement
    let strength = gs.characters[cn].attrib[AT_STREN as usize][0];
    if strength < 90 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Your strength is too low. There is still room for improvement.\n",
        );
        return;
    }

    // Change race based on gender
    let (is_male, name) = {
        (
            (gs.characters[cn].kindred as u32 & traits::KIN_MALE) != 0,
            gs.characters[cn].get_name().to_owned(),
        )
    };

    let new_race = if is_male { 544 } else { 549 };
    God::minor_racechange(gs, cn, new_race);
    gs.do_character_log(
        cn,
        core::types::FontColor::Yellow,
        &format!(
            "You are truly worthy to become a Archtemplar. Congratulations, {}.\n",
            name
        ),
    );
}

/// Handles the legacy `change_to_archharakim` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `change_to_archharakim` is outside the corresponding game-state collection.
pub fn change_to_archharakim(gs: &mut GameState, cn: usize) {
    // Check willpower requirement
    let willpower = gs.characters[cn].attrib[AT_WILL as usize][0];
    if willpower < 90 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Your willpower is too low. There is still room for improvement.\n",
        );
        return;
    }

    // Check intuition requirement
    let intuition = gs.characters[cn].attrib[AT_INT as usize][0];
    if intuition < 90 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Your intuition is too low. There is still room for improvement.\n",
        );
        return;
    }

    // Change race based on gender
    let (is_male, name) = {
        (
            (gs.characters[cn].kindred as u32 & traits::KIN_MALE) != 0,
            gs.characters[cn].get_name().to_owned(),
        )
    };

    let new_race = if is_male { 545 } else { 550 };
    God::minor_racechange(gs, cn, new_race);

    gs.do_character_log(
        cn,
        core::types::FontColor::Yellow,
        &format!(
            "You are truly worthy to become a Archharakim. Congratulations, {}.\n",
            name
        ),
    );
}

/// Handles the legacy `change_to_warrior` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `change_to_warrior` is outside the corresponding game-state collection.
pub fn change_to_warrior(gs: &mut GameState, cn: usize) {
    // Check agility requirement
    let agility = gs.characters[cn].attrib[AT_AGIL as usize][0];
    if agility < 60 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Your agility is too low. There is still room for improvement.\n",
        );
        return;
    }

    // Check strength requirement
    let strength = gs.characters[cn].attrib[AT_STREN as usize][0];
    if strength < 60 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Your strength is too low. There is still room for improvement.\n",
        );
        return;
    }

    // Change race based on gender
    let (is_male, name) = {
        (
            (gs.characters[cn].kindred as u32 & traits::KIN_MALE) != 0,
            gs.characters[cn].get_name().to_owned(),
        )
    };

    let new_race = if is_male { 547 } else { 552 };
    God::minor_racechange(gs, cn, new_race);

    gs.do_character_log(
        cn,
        core::types::FontColor::Yellow,
        &format!(
            "You are truly worthy to become a Warrior. Congratulations, {}.\n",
            name
        ),
    );
}

/// Handles the legacy `change_to_sorcerer` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `change_to_sorcerer` is outside the corresponding game-state collection.
pub fn change_to_sorcerer(gs: &mut GameState, cn: usize) {
    // Check willpower requirement
    let willpower = gs.characters[cn].attrib[AT_WILL as usize][0];
    if willpower < 60 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Your willpower is too low. There is still room for improvement.\n",
        );
        return;
    }

    // Check intuition requirement
    let intuition = gs.characters[cn].attrib[AT_INT as usize][0];
    if intuition < 60 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Your intuition is too low. There is still room for improvement.\n",
        );
        return;
    }

    // Change race based on gender
    let (is_male, name) = {
        (
            (gs.characters[cn].kindred as u32 & traits::KIN_MALE) != 0,
            gs.characters[cn].get_name().to_owned(),
        )
    };

    let new_race = if is_male { 546 } else { 551 };
    God::minor_racechange(gs, cn, new_race);

    gs.do_character_log(
        cn,
        core::types::FontColor::Yellow,
        &format!(
            "You are truly worthy to become a Sorcerer. Congratulations, {}.\n",
            name
        ),
    );
}

/// Handles the legacy `shrine_of_change` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `_item_idx` - Value passed through to the legacy `shrine_of_change` hook.
///
/// # Returns
///
/// * `true` when `shrine_of_change` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `shrine_of_change` is outside the corresponding game-state collection.
pub fn shrine_of_change(gs: &mut GameState, cn: usize, _item_idx: usize) -> bool {
    // Requires specific potions to change character class
    // Potion of Life (148) -> Archtemplar/Archharakim
    // Greater Healing Potion (127/274) -> Warrior
    // Greater Mana Potion (131/273) -> Sorcerer

    if cn == 0 {
        return false;
    }

    let citem = gs.characters[cn].citem as usize;
    if citem == 0 || (citem & 0x80000000) != 0 {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Read the notes, my friend.\n",
        );
        return false;
    }

    let (citem_temp, kindred) = {
        let temp = gs.items[citem].temp;
        let kindred = gs.characters[cn].kindred;
        (temp, kindred)
    };

    // Potion of life -> Archtemplar/Archharakim
    if citem_temp == 148 {
        if (kindred as u32 & traits::KIN_TEMPLAR) != 0 {
            change_to_archtemplar(gs, cn);
        } else if (kindred as u32 & traits::KIN_HARAKIM) != 0 {
            change_to_archharakim(gs, cn);
        } else {
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "You are neither Templar nor Harakim.\n",
            );
        }
        return false;
    }

    // Greater healing potion -> Warrior
    if citem_temp == 127 || citem_temp == 274 {
        if (kindred as u32 & traits::KIN_MERCENARY) != 0 {
            change_to_warrior(gs, cn);
        } else {
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "You are not a Mercenary.\n",
            );
        }
        return false;
    }

    // Greater mana potion -> Sorcerer
    if citem_temp == 131 || citem_temp == 273 {
        if (kindred as u32 & traits::KIN_MERCENARY) != 0 {
            change_to_sorcerer(gs, cn);
        } else {
            gs.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "You are not a Mercenary.\n",
            );
        }
        return false;
    }

    gs.do_character_log(
        cn,
        core::types::FontColor::Yellow,
        "Read the notes, my friend.\n",
    );
    false
}

/// Handles the legacy `explorer_point` item-use hook.
///
/// # Arguments
///
/// * `gs` - Active game state used by this legacy driver hook.
/// * `cn` - Character index executing this legacy driver hook.
/// * `item_idx` - Value passed through to the legacy `explorer_point` hook.
///
/// # Returns
///
/// * `true` when `explorer_point` reports that it handled the legacy action, otherwise `false`.
///
/// # Panics
///
/// * Panics if any legacy id or index parameter used by `explorer_point` is outside the corresponding game-state collection.
pub fn explorer_point(gs: &mut GameState, cn: usize, item_idx: usize) -> bool {
    // data[0-3] = bitmask for visited flags (stored in character data[46-49])
    // data[4] = base experience reward

    // Check if already visited
    let (data0, data1, data2, data3, char_data46, char_data47, char_data48, char_data49) = {
        let d0 = gs.items[item_idx].data[0];
        let d1 = gs.items[item_idx].data[1];
        let d2 = gs.items[item_idx].data[2];
        let d3 = gs.items[item_idx].data[3];
        {
            (
                d0,
                d1,
                d2,
                d3,
                gs.characters[cn].data[46],
                gs.characters[cn].data[47],
                gs.characters[cn].data[48],
                gs.characters[cn].data[49],
            )
        }
    };

    if ((char_data46 & data0 as i32) == 0)
        && ((char_data47 & data1 as i32) == 0)
        && ((char_data48 & data2 as i32) == 0)
        && ((char_data49 & data3 as i32) == 0)
    {
        // Mark as visited
        gs.characters[cn].data[46] |= data0 as i32;
        gs.characters[cn].data[47] |= data1 as i32;
        gs.characters[cn].data[48] |= data2 as i32;
        gs.characters[cn].data[49] |= data3 as i32;
        gs.characters[cn].luck += 10;

        gs.do_character_log(
            cn,
            core::types::FontColor::Red,
            "You found a new exploration point!\n",
        );

        // Calculate experience reward
        let (base_exp, points_tot) = {
            let base = gs.items[item_idx].data[4];
            let pts = gs.characters[cn].points_tot;
            (base, pts)
        };

        let mut exp = base_exp / 2 + helpers::random_mod(base_exp);
        exp = std::cmp::min(points_tot as u32 / 10, exp); // Not more than 10% of total experience
        exp += helpers::random_mod(exp / 10 + 1); // Some more randomness

        log::info!(
            "exp point giving {} ({}) exp, char has {} exp",
            exp,
            base_exp,
            points_tot
        );

        gs.do_give_exp(cn, exp as i32, 0, -1);
        crate::player::commands::resend_completion_data_for_character(gs, cn);
    } else {
        gs.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            "Hmm. Seems somewhat familiar. You've been here before...\n",
        );
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{add_test_player, with_test_gs};
    use core::skills::SK_WEAPON;
    use core::string_operations::write_ascii_into_fixed;

    type PromotionCase = (fn(&mut GameState, usize), usize, i32, u16);

    /// Installs the minimum character template data needed for `minor_racechange`.
    fn install_character_template(
        gs: &mut GameState,
        template_id: usize,
        kindred: i32,
        skill_bonus: u16,
    ) {
        gs.character_templates[template_id] = core::types::Character::default();
        gs.character_templates[template_id].used = USE_ACTIVE;
        gs.character_templates[template_id].kindred = kindred;
        gs.character_templates[template_id].sprite = template_id as u16;
        gs.character_templates[template_id].skill[SK_WEAPON][1] = skill_bonus;
    }

    /// Puts an active test item in the global item array and returns its index.
    fn install_item(gs: &mut GameState, item_idx: usize, temp: u16) -> usize {
        gs.items[item_idx] = core::types::Item::default();
        gs.items[item_idx].used = USE_ACTIVE;
        gs.items[item_idx].temp = temp;
        write_ascii_into_fixed(&mut gs.items[item_idx].reference, "test item");
        item_idx
    }

    #[test]
    fn change_promotions_apply_template_kindred_and_skills() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let cases: &[PromotionCase] = &[
                (
                    change_to_archtemplar,
                    544,
                    (traits::KIN_ARCHTEMPLAR | traits::KIN_MALE) as i32,
                    11,
                ),
                (
                    change_to_archharakim,
                    545,
                    (traits::KIN_ARCHHARAKIM | traits::KIN_MALE) as i32,
                    12,
                ),
                (
                    change_to_warrior,
                    547,
                    (traits::KIN_WARRIOR | traits::KIN_MALE) as i32,
                    13,
                ),
                (
                    change_to_sorcerer,
                    546,
                    (traits::KIN_SORCERER | traits::KIN_MALE) as i32,
                    14,
                ),
            ];

            for &(change, template_id, expected_kindred, expected_skill) in cases {
                install_character_template(gs, template_id, expected_kindred, expected_skill);
                gs.characters[cn].kindred = traits::KIN_MALE as i32;
                for attr in [AT_AGIL, AT_STREN, AT_WILL, AT_INT] {
                    gs.characters[cn].attrib[attr as usize][0] = 90;
                }

                change(gs, cn);

                assert_eq!(gs.characters[cn].temp, template_id as u16);
                assert_eq!(gs.characters[cn].kindred, expected_kindred);
                assert_eq!(gs.characters[cn].skill[SK_WEAPON][1], expected_skill);
            }
        });
    }

    #[test]
    fn change_to_warrior_guard_keeps_character_unchanged_when_attributes_are_low() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            install_character_template(
                gs,
                547,
                (traits::KIN_WARRIOR | traits::KIN_MALE) as i32,
                99,
            );
            gs.characters[cn].kindred = (traits::KIN_MERCENARY | traits::KIN_MALE) as i32;
            gs.characters[cn].attrib[AT_AGIL as usize][0] = 59;
            gs.characters[cn].attrib[AT_STREN as usize][0] = 90;

            change_to_warrior(gs, cn);

            assert_eq!(gs.characters[cn].temp, 0);
            assert_eq!(
                gs.characters[cn].kindred,
                (traits::KIN_MERCENARY | traits::KIN_MALE) as i32
            );
            assert_eq!(gs.characters[cn].skill[SK_WEAPON][1], 0);
        });
    }

    #[test]
    fn shrine_of_change_dispatches_mercenary_healing_potion_to_warrior() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let potion = install_item(gs, 1, 127);
            install_character_template(gs, 547, (traits::KIN_WARRIOR | traits::KIN_MALE) as i32, 7);
            gs.characters[cn].kindred = (traits::KIN_MERCENARY | traits::KIN_MALE) as i32;
            gs.characters[cn].attrib[AT_AGIL as usize][0] = 60;
            gs.characters[cn].attrib[AT_STREN as usize][0] = 60;
            gs.characters[cn].citem = potion as u32;

            assert!(!shrine_of_change(gs, cn, 0));

            assert_eq!(gs.characters[cn].temp, 547);
            assert_eq!(
                gs.characters[cn].kindred,
                (traits::KIN_WARRIOR | traits::KIN_MALE) as i32
            );
        });
    }

    #[test]
    fn explorer_point_rewards_only_first_visit() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let point = install_item(gs, 1, 0);
            gs.items[point].data[0] = 0b10;
            gs.items[point].data[1] = 0b100;
            gs.items[point].data[4] = 100;
            gs.characters[cn].points_tot = 1_000;

            assert!(explorer_point(gs, cn, point));
            let first_points = gs.characters[cn].points;
            let first_total = gs.characters[cn].points_tot;

            assert_ne!(first_points, 0);
            assert_eq!(gs.characters[cn].data[46], 0b10);
            assert_eq!(gs.characters[cn].data[47], 0b100);
            assert_eq!(gs.characters[cn].luck, 10);

            assert!(explorer_point(gs, cn, point));

            assert_eq!(gs.characters[cn].points, first_points);
            assert_eq!(gs.characters[cn].points_tot, first_total);
            assert_eq!(gs.characters[cn].luck, 10);
        });
    }

    #[test]
    fn lab8_moneyshrine_rejects_non_money_and_accepts_sufficient_money() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let shrine = install_item(gs, 1, 0);
            let token = install_item(gs, 2, 123);
            gs.items[shrine].data[0] = 500;
            gs.items[shrine].data[1] = 10;
            gs.items[shrine].data[2] = 10;
            gs.characters[cn].citem = token as u32;

            assert!(!use_lab8_moneyshrine(gs, cn, shrine));
            assert_eq!(gs.characters[cn].citem, token as u32);

            gs.characters[cn].citem = 0x8000_0000 | 500;
            gs.characters[cn].mana[5] = 3;
            gs.characters[cn].a_mana = 0;

            assert!(use_lab8_moneyshrine(gs, cn, shrine));

            assert_eq!(gs.characters[cn].citem, 0);
            assert_eq!(gs.characters[cn].a_mana, 3_000);
        });
    }

    #[test]
    fn use_shrine_fire_point_consumes_real_point_and_ignores_fake_point_reward() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let real_point = install_item(gs, 1, 0);
            write_ascii_into_fixed(&mut gs.items[real_point].description, "ONE FIRE POINT");
            gs.characters[cn].citem = real_point as u32;

            assert!(!use_shrine(gs, cn, 0));

            assert_eq!(gs.characters[cn].data[70], 1);
            assert_eq!(gs.items[real_point].used, USE_EMPTY);
            assert_eq!(gs.characters[cn].citem, 0);

            let fake_point = install_item(gs, 2, 0);
            write_ascii_into_fixed(&mut gs.items[fake_point].description, "ONE FAKE POINT");
            gs.characters[cn].citem = fake_point as u32;

            assert!(!use_shrine(gs, cn, 0));

            assert_eq!(gs.characters[cn].data[70], 1);
            assert_eq!(gs.items[fake_point].used, USE_EMPTY);
            assert_eq!(gs.characters[cn].citem, 0);
        });
    }

    #[test]
    fn lab8_shrine_money_offer_should_be_rejected_without_indexing_items() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            let shrine = install_item(gs, 1, 0);
            gs.characters[cn].citem = 0x8000_0000 | 500;

            assert!(!use_lab8_shrine(gs, cn, shrine));
            assert_eq!(gs.characters[cn].citem, 0x8000_0000 | 500);
        });
    }
}
