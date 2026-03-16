use core::constants::{
    CharacterFlags, ItemFlags, MAXCHARS, MAX_SPEEDTAB_SPEED_INDEX, MIN_SPEEDTAB_INDEX,
};
use core::ranks::{self, TOTAL_RANKS};
use core::types::FontColor;
use core::{skills, traits};

use crate::core::types::skilltab;
use crate::effect::EffectManager;
use crate::game_state::GameState;
use crate::god::God;
use crate::{driver, helpers};

impl GameState {
    /// Helper function to check if character wears a specific item
    /// Port of part of `really_update_char`
    pub(crate) fn char_wears_item(&mut self, cn: usize, item_template: u16) -> bool {
        for n in 0..20 {
            let item_idx = self.characters[cn].worn[n];
            if item_idx != 0 {
                if self.items[item_idx as usize].temp == item_template {
                    return true;
                }
            }
        }
        false
    }

    /// Port of `really_update_char(int cn)` from `svr_do.cpp`
    ///
    /// Recalculates all character stats from base values, worn items, and
    /// active spells. This is the central stat computation invoked after
    /// equipment changes, spell effects or any change that affects derived
    /// attributes. It computes:
    /// - Final attributes (strength, agility, etc.)
    /// - HP, endurance, and mana totals
    /// - Skill modifiers from attributes and items
    /// - Armor, weapon and gethit damage values
    /// - Light emission and infra-vision
    /// - Movement speed and temporary flags
    ///
    /// # Arguments
    /// * `cn` - Character id to recompute
    pub(crate) fn really_update_char(&mut self, cn: usize) {
        // Clear regeneration prevention flags and sprite override
        self.characters[cn].flags &= !(CharacterFlags::NoHpReg.bits()
            | CharacterFlags::NoEndReg.bits()
            | CharacterFlags::NoManaReg.bits());
        self.characters[cn].sprite_override = 0;

        // Check for NOMAGIC map flag
        let char_x = self.characters[cn].x;
        let char_y = self.characters[cn].y;
        let wears_466 = self.char_wears_item(cn, 466);
        let wears_481 = self.char_wears_item(cn, 481);

        let map_index = char_x as usize + char_y as usize * core::constants::SERVER_MAPX as usize;
        let has_nomagic_flag = self.map[map_index].flags & core::constants::MF_NOMAGIC as u64 != 0;

        if has_nomagic_flag && !wears_466 && !wears_481 {
            let already_has_nomagic =
                self.characters[cn].flags & CharacterFlags::NoMagic.bits() != 0;

            if !already_has_nomagic {
                self.characters[cn].flags |= CharacterFlags::NoMagic.bits();
                self.remove_spells(cn);
                self.do_character_log(cn, FontColor::Green, "You feel your magic fail.\n");
            }
        } else {
            let has_nomagic = self.characters[cn].flags & CharacterFlags::NoMagic.bits() != 0;

            if has_nomagic {
                self.characters[cn].flags &= !CharacterFlags::NoMagic.bits();
                self.characters[cn].set_do_update_flags();
                self.do_character_log(cn, FontColor::Green, "You feel your magic return.\n");
            }
        }

        let old_light = self.characters[cn].light;

        // Initialize stat accumulators
        let mut attrib_bonus = [0i32; 5];
        let mut hp_bonus = 0i32;
        let mut end_bonus = 0i32;
        let mut mana_bonus = 0i32;
        let mut skill_bonus = [0i32; 50];
        let mut armor = 0i32;
        let mut weapon = 0i32;
        let mut gethit = 0i32;
        let mut light = 0i32;
        let mut sublight = 0i32;
        let mut infra = 0u8;

        // Reset temp bonuses in character
        for n in 0..5 {
            self.characters[cn].attrib[n][4] = 0;
        }
        self.characters[cn].hp[4] = 0;
        self.characters[cn].end[4] = 0;
        self.characters[cn].mana[4] = 0;
        for n in 0..50 {
            self.characters[cn].skill[n][4] = 0;
        }
        self.characters[cn].armor = 0;
        self.characters[cn].weapon = 0;
        self.characters[cn].gethit_dam = 0;
        self.characters[cn].stunned = 0;
        self.characters[cn].light = 0;

        let char_has_nomagic = self.characters[cn].flags & CharacterFlags::NoMagic.bits() != 0;

        // Calculate bonuses from worn items
        for n in 0..20 {
            let item_idx = self.characters[cn].worn[n];
            if item_idx == 0 {
                continue;
            }

            let item = &mut self.items[item_idx as usize];

            if !char_has_nomagic {
                // Add magical bonuses
                for z in 0..5 {
                    attrib_bonus[z] += if item.active != 0 {
                        item.attrib[z][1] as i32
                    } else {
                        item.attrib[z][0] as i32
                    };
                }

                hp_bonus += if item.active != 0 {
                    item.hp[1] as i32
                } else {
                    item.hp[0] as i32
                };

                end_bonus += if item.active != 0 {
                    item.end[1] as i32
                } else {
                    item.end[0] as i32
                };

                mana_bonus += if item.active != 0 {
                    item.mana[1] as i32
                } else {
                    item.mana[0] as i32
                };

                for z in 0..50 {
                    skill_bonus[z] += if item.active != 0 {
                        item.skill[z][1] as i32
                    } else {
                        item.skill[z][0] as i32
                    };
                }
            }

            // Add physical bonuses (always apply)
            if item.active != 0 {
                armor += item.armor[1] as i32;
                gethit += item.gethit_dam[1] as i32;
                if item.weapon[1] as i32 > weapon {
                    weapon = item.weapon[1] as i32;
                }
                if item.light[1] as i32 > light {
                    light = item.light[1] as i32;
                } else if item.light[1] < 0 {
                    sublight -= item.light[1] as i32;
                }
            } else {
                armor += item.armor[0] as i32;
                gethit += item.gethit_dam[0] as i32;
                if item.weapon[0] as i32 > weapon {
                    weapon = item.weapon[0] as i32;
                }
                if item.light[0] as i32 > light {
                    light = item.light[0] as i32;
                } else if item.light[0] < 0 {
                    sublight -= item.light[0] as i32;
                }
            }
        }

        // Add permanent bonuses
        armor += self.characters[cn].armor_bonus as i32;
        weapon += self.characters[cn].weapon_bonus as i32;
        gethit += self.characters[cn].gethit_bonus as i32;
        light += self.characters[cn].light_bonus as i32;

        // Calculate bonuses from active spells
        if !char_has_nomagic {
            for n in 0..20 {
                let spell_idx = self.characters[cn].spell[n];
                if spell_idx == 0 {
                    continue;
                }

                let spell = &mut self.items[spell_idx as usize];

                for z in 0..5 {
                    attrib_bonus[z] += spell.attrib[z][1] as i32;
                }

                hp_bonus += spell.hp[1] as i32;
                end_bonus += spell.end[1] as i32;
                mana_bonus += spell.mana[1] as i32;

                for z in 0..50 {
                    skill_bonus[z] += spell.skill[z][1] as i32;
                }

                armor += spell.armor[1] as i32;
                weapon += spell.weapon[1] as i32;
                if spell.light[1] as i32 > light {
                    light = spell.light[1] as i32;
                } else if spell.light[1] < 0 {
                    sublight -= spell.light[1] as i32;
                }

                // Check for special spell effects
                if spell.temp == skills::SK_STUN as u16 || spell.temp == skills::SK_WARCRY2 as u16 {
                    self.characters[cn].stunned = 1;
                }

                if spell.hp[0] < 0 {
                    self.characters[cn].flags |= CharacterFlags::NoHpReg.bits();
                }
                if spell.end[0] < 0 {
                    self.characters[cn].flags |= CharacterFlags::NoEndReg.bits();
                }
                if spell.mana[0] < 0 {
                    self.characters[cn].flags |= CharacterFlags::NoManaReg.bits();
                }

                if spell.sprite_override != 0 {
                    self.characters[cn].sprite_override = spell.sprite_override as i16;
                }

                // Check for infrared vision components (items 635, 637, 639, 641)
                if spell.temp == 635 {
                    infra |= 1;
                }
                if spell.temp == 637 {
                    infra |= 2;
                }
                if spell.temp == 639 {
                    infra |= 4;
                }
                if spell.temp == 641 {
                    infra |= 8;
                }
            }
        }

        // Calculate final attributes
        for z in 0..5 {
            let mut final_attrib = self.characters[cn].attrib[z][0] as i32
                + self.characters[cn].attrib[z][1] as i32
                + attrib_bonus[z];

            final_attrib = final_attrib.clamp(1, 250);
            self.characters[cn].attrib[z][5] = final_attrib as u8;
        }

        // Calculate final HP
        let mut final_hp =
            self.characters[cn].hp[0] as i32 + self.characters[cn].hp[1] as i32 + hp_bonus;
        final_hp = final_hp.clamp(10, 999);
        self.characters[cn].hp[5] = final_hp as u16;

        // Calculate final endurance
        let mut final_end =
            self.characters[cn].end[0] as i32 + self.characters[cn].end[1] as i32 + end_bonus;
        final_end = final_end.clamp(10, 999);
        self.characters[cn].end[5] = final_end as u16;

        // Calculate final mana
        let mut final_mana =
            self.characters[cn].mana[0] as i32 + self.characters[cn].mana[1] as i32 + mana_bonus;
        final_mana = final_mana.clamp(10, 999);
        self.characters[cn].mana[5] = final_mana as u16;

        // Handle infrared vision
        let is_player = self.characters[cn].flags & CharacterFlags::Player.bits() != 0;

        if is_player {
            let has_infrared = self.characters[cn].flags & CharacterFlags::Infrared.bits() != 0;

            if infra == 15 && !has_infrared {
                self.characters[cn].flags |= CharacterFlags::Infrared.bits();
                self.do_character_log(cn, FontColor::Green, "You can see in the dark!\n");
            } else if infra != 15 && has_infrared {
                let is_god = self.characters[cn].flags & CharacterFlags::God.bits() != 0;

                if !is_god {
                    self.characters[cn].flags &= !CharacterFlags::Infrared.bits();
                    self.do_character_log(
                        cn,
                        FontColor::Red,
                        "You can no longer see in the dark!\n",
                    );
                }
            }
        }

        // Calculate final skills (with attribute bonuses)
        for z in 0..50 {
            let mut final_skill = self.characters[cn].skill[z][0] as i32
                + self.characters[cn].skill[z][1] as i32
                + skill_bonus[z];

            // Add attribute bonuses using the proper skill->attribute mapping from `skilltab`
            let attrs = skilltab::get_skill_attribs(z);
            let attrib_contribution = (self.characters[cn].attrib[attrs[0]][5] as i32
                + self.characters[cn].attrib[attrs[1]][5] as i32
                + self.characters[cn].attrib[attrs[2]][5] as i32)
                / 5;
            final_skill += attrib_contribution;
            final_skill = final_skill.clamp(1, 250);
            self.characters[cn].skill[z][5] = final_skill as u8;
        }

        // Set final armor
        armor = armor.clamp(0, 250);
        self.characters[cn].armor = armor as i16;

        // Set final weapon
        weapon = weapon.clamp(0, 250);
        self.characters[cn].weapon = weapon as i16;

        // Set final gethit damage
        gethit = gethit.clamp(0, 250);
        self.characters[cn].gethit_dam = gethit as i8;

        // Set final light
        light -= sublight;
        light = light.clamp(0, 250);
        self.characters[cn].light = light as u8;

        // Calculate speed based on mode
        let mut speed_calc = 10i32;
        let mode = self.characters[cn].mode;
        let agil = self.characters[cn].attrib[core::constants::AT_AGIL as usize][5] as i32;
        let stren = self.characters[cn].attrib[core::constants::AT_STREN as usize][5] as i32;
        let speed_mod = self.characters[cn].speed_mod as i32;

        if mode == 0 {
            // Sneak mode
            speed_calc = (agil + stren) / 50 + speed_mod + 12;
        } else if mode == 1 {
            // Normal mode
            speed_calc = (agil + stren) / 50 + speed_mod + 14;
        } else if mode == 2 {
            // Fast mode
            speed_calc = (agil + stren) / 50 + speed_mod + 16;
        }

        self.characters[cn].speed = 20 - speed_calc as i16;
        self.characters[cn].speed = self.characters[cn]
            .speed
            .clamp(MIN_SPEEDTAB_INDEX as i16, MAX_SPEEDTAB_SPEED_INDEX as i16);

        // Cap current stats at their maximums
        if self.characters[cn].a_hp > self.characters[cn].hp[5] as i32 * 1000 {
            self.characters[cn].a_hp = self.characters[cn].hp[5] as i32 * 1000;
        }
        if self.characters[cn].a_end > self.characters[cn].end[5] as i32 * 1000 {
            self.characters[cn].a_end = self.characters[cn].end[5] as i32 * 1000;
        }
        if self.characters[cn].a_mana > self.characters[cn].mana[5] as i32 * 1000 {
            self.characters[cn].a_mana = self.characters[cn].mana[5] as i32 * 1000;
        }

        // Update light if it changed
        let new_light = self.characters[cn].light;
        if old_light != new_light {
            let used = self.characters[cn].used;
            let x = self.characters[cn].x;
            let y = self.characters[cn].y;

            if used == core::constants::USE_ACTIVE
                && x > 0
                && x < core::constants::SERVER_MAPX as i16
                && y > 0
                && y < core::constants::SERVER_MAPY as i16
            {
                let idx = (x as i32 + y as i32 * core::constants::SERVER_MAPX) as usize;
                let map_char = self.map[idx].ch;

                if map_char == cn as u32 {
                    self.do_add_light(x as i32, y as i32, new_light as i32 - old_light as i32);
                }
            }
        }
    }

    /// Port of `do_regenerate(int cn)` from `svr_do.cpp`
    ///
    /// Handles HP/endurance/mana regeneration and related per-tick updates.
    ///
    /// Responsibilities:
    /// - Apply HP/END/MANA regeneration rules (including moon/mayhem effects)
    /// - Manage spell durations, shield behavior and active-item wear
    /// - Apply underwater damage and item tear/wear for active players
    /// - Clamp accumulated stats and set timers for low-resource states
    ///
    /// # Arguments
    /// * `cn` - Character id to regenerate (called every tick)
    pub(crate) fn do_regenerate(&mut self, cn: usize) {
        // Check if character is stoned - no regeneration if stoned
        let is_stoned = self.characters[cn].flags & CharacterFlags::Stoned.bits() != 0;

        if is_stoned {
            return;
        }

        // Determine moon multiplier for regen rates
        let mut moonmult = 20;

        let is_player = self.characters[cn].flags & CharacterFlags::Player.bits() != 0;
        let globs_flags = self.globals.flags;
        let newmoon = self.globals.newmoon != 0;
        let fullmoon = self.globals.fullmoon != 0;

        if ((globs_flags & core::constants::GF_MAYHEM != 0) || newmoon) && is_player {
            moonmult = 10; // Slower regen during mayhem or new moon
        }
        if fullmoon && is_player {
            moonmult = 40; // Faster regen during full moon
        }

        // Check for regeneration prevention flags
        let nohp = self.characters[cn].flags & CharacterFlags::NoHpReg.bits() != 0;
        let noend = self.characters[cn].flags & CharacterFlags::NoEndReg.bits() != 0;
        let nomana = self.characters[cn].flags & CharacterFlags::NoManaReg.bits() != 0;

        // Check if standing in underwater tile
        let x = self.characters[cn].x as usize;
        let y = self.characters[cn].y as usize;
        let map_idx = x + y * core::constants::SERVER_MAPX as usize;
        let uwater = self.map[map_idx].flags & core::constants::MF_UWATER as u64 != 0;

        let mut uwater_active = uwater;
        let mut hp_regen = false;
        let mut mana_regen = false;
        let mut gothp = 0i32;

        // Scale factor: convert per-tick values designed for the legacy 18 TPS
        // tick rate to the current tick rate, preserving wall-clock rates.
        let scale = |v: i32| -> i32 { v * core::constants::LEGACY_TICKS / core::constants::TICKS };

        // Process regeneration based on character status (if not stunned)
        let stunned = self.characters[cn].stunned != 0;

        if !stunned {
            let status = self.characters[cn].status;
            let base_status = helpers::ch_base_status(status as u8);

            match base_status {
                // Standing/idle states - regenerate normally
                0..=7 => {
                    if !noend {
                        self.characters[cn].a_end += scale(moonmult * 4);

                        // Add bonus from Rest skill
                        if self.characters[cn].skill[skills::SK_REST][0] != 0 {
                            self.characters[cn].a_end += scale(
                                self.characters[cn].skill[skills::SK_REST][5] as i32 * moonmult
                                    / 30,
                            );
                        }
                    }

                    if !nohp {
                        hp_regen = true;
                        self.characters[cn].a_hp += scale(moonmult * 2);
                        // C original: gothp += moonmult (tracks half the HP regen increment)
                        gothp += scale(moonmult);

                        // Add bonus from Regen skill
                        if self.characters[cn].skill[skills::SK_REGEN][0] != 0 {
                            let regen_bonus = scale(
                                self.characters[cn].skill[skills::SK_REGEN][5] as i32 * moonmult
                                    / 30,
                            );
                            self.characters[cn].a_hp += regen_bonus;
                            gothp += regen_bonus;
                        }
                    }

                    if !nomana {
                        let has_medit = self.characters[cn].skill[skills::SK_MEDIT][0] != 0;

                        if has_medit {
                            mana_regen = true;
                            self.characters[cn].a_mana += scale(moonmult);
                            self.characters[cn].a_mana += scale(
                                self.characters[cn].skill[skills::SK_MEDIT][5] as i32 * moonmult
                                    / 30,
                            );
                        }
                    }
                }

                // Walking/turning states - endurance based on mode
                16 | 24 | 32 | 40 | 48 | 60 | 72 | 84 | 96 | 100 | 104 | 108 | 112 | 116 | 120
                | 124 | 128 | 132 | 136 | 140 | 144 | 148 | 152 => {
                    let mode = self.characters[cn].mode;

                    if mode == 2 {
                        // Fast mode drains endurance
                        self.characters[cn].a_end -= scale(25);
                    } else if mode == 0 {
                        // Sneak mode regenerates endurance
                        if !noend {
                            self.characters[cn].a_end += scale(25);
                        }
                    }
                }

                // Attack states - endurance drain based on status2 and mode
                160 | 168 | 176 | 184 => {
                    let status2 = self.characters[cn].status2;
                    let mode = self.characters[cn].mode;

                    if status2 == 0 || status2 == 5 || status2 == 6 {
                        // Attack action
                        if mode == 1 {
                            self.characters[cn].a_end -= scale(12);
                        } else if mode == 2 {
                            self.characters[cn].a_end -= scale(50);
                        }
                    } else {
                        // Misc action
                        if mode == 2 {
                            self.characters[cn].a_end -= scale(25);
                        } else if mode == 0 {
                            if !noend {
                                self.characters[cn].a_end += scale(25);
                            }
                        }
                    }
                }

                _ => {
                    log::warn!("do_regenerate(): unknown ch_base_status {}.", base_status);
                }
            }
        }

        // Undead characters get bonus HP regeneration
        let is_undead = self.characters[cn].flags & CharacterFlags::Undead.bits() != 0;

        if is_undead {
            hp_regen = true;
            self.characters[cn].a_hp += scale(650);
            gothp += scale(650);
        }

        // Amulet of Ankh (item 768) provides additional regeneration
        let worn_neck = self.characters[cn].worn[core::constants::WN_NECK];
        if worn_neck != 0 {
            let is_ankh = self.items[worn_neck as usize].temp == 768;

            if is_ankh {
                let has_regen = self.characters[cn].skill[skills::SK_REGEN][0] != 0;
                let has_rest = self.characters[cn].skill[skills::SK_REST][0] != 0;
                let has_medit = self.characters[cn].skill[skills::SK_MEDIT][0] != 0;

                if has_regen {
                    self.characters[cn].a_hp += scale(
                        self.characters[cn].skill[skills::SK_REGEN][5] as i32 * moonmult / 60,
                    );
                }
                if has_rest {
                    self.characters[cn].a_end +=
                        scale(self.characters[cn].skill[skills::SK_REST][5] as i32 * moonmult / 60);
                }
                if has_medit {
                    self.characters[cn].a_mana += scale(
                        self.characters[cn].skill[skills::SK_MEDIT][5] as i32 * moonmult / 60,
                    );
                }
            }
        }

        // Cap accumulated stats at their maximums (max * 1000)
        if self.characters[cn].a_hp > self.characters[cn].hp[5] as i32 * 1000 {
            self.characters[cn].a_hp = self.characters[cn].hp[5] as i32 * 1000;
        }
        if self.characters[cn].a_end > self.characters[cn].end[5] as i32 * 1000 {
            self.characters[cn].a_end = self.characters[cn].end[5] as i32 * 1000;
        }
        if self.characters[cn].a_mana > self.characters[cn].mana[5] as i32 * 1000 {
            self.characters[cn].a_mana = self.characters[cn].mana[5] as i32 * 1000;
        }

        // Set timer when regenerating below 90% of max
        if hp_regen {
            let needs_timer = self.characters[cn].a_hp < self.characters[cn].hp[5] as i32 * 900;
            if needs_timer {
                self.characters[cn].data[92] = core::constants::TICKS * 60;
            }
        }

        if mana_regen {
            let needs_timer = self.characters[cn].a_mana < self.characters[cn].mana[5] as i32 * 900;
            if needs_timer {
                self.characters[cn].data[92] = core::constants::TICKS * 60;
            }
        }

        // Force to sneak mode if exhausted
        let is_exhausted = self.characters[cn].a_end < 1500;
        let mode = self.characters[cn].mode;

        if is_exhausted && mode != 0 {
            self.characters[cn].mode = 0;
            self.characters[cn].set_do_update_flags();

            self.do_character_log(cn, FontColor::Red, "You're exhausted.\n");
        }

        // Decrement escape timer
        if self.characters[cn].escape_timer > 0 {
            self.characters[cn].escape_timer -= 1;
        }

        // Process spell effects
        for spell_slot in 0..20 {
            let spell_item = self.characters[cn].spell[spell_slot];

            if spell_item == 0 {
                continue;
            }

            let is_permspell =
                self.items[spell_item as usize].flags & ItemFlags::IF_PERMSPELL.bits() != 0;

            if is_permspell {
                // Permanent spell - apply ongoing HP/end/mana drain/gain
                let hp_change = self.items[spell_item as usize].hp[0];
                let end_change = self.items[spell_item as usize].end[0];
                let mana_change = self.items[spell_item as usize].mana[0];

                let mut killed = false;
                let mut end_depleted = false;
                let mut mana_depleted = false;

                if hp_change != -1 {
                    self.characters[cn].a_hp += hp_change as i32;
                    if self.characters[cn].a_hp < 500 {
                        killed = true;
                    }
                }
                if end_change != -1 {
                    self.characters[cn].a_end += end_change as i32;
                    if self.characters[cn].a_end < 500 {
                        self.characters[cn].a_end = 500;
                        end_depleted = true;
                    }
                }
                if mana_change != -1 {
                    self.characters[cn].a_mana += mana_change as i32;
                    if self.characters[cn].a_mana < 500 {
                        self.characters[cn].a_mana = 500;
                        mana_depleted = true;
                    }
                }

                if killed {
                    let spell_name = self.items[spell_item as usize].get_name().to_string();
                    log::info!("Character {} killed by spell: {}", cn, spell_name);
                    self.do_character_log(
                        cn,
                        FontColor::Red,
                        &format!("The {} killed you!\n", spell_name),
                    );
                    self.do_area_log(
                        cn,
                        0,
                        0,
                        0,
                        FontColor::Red,
                        &format!("The {} killed {}.\n", spell_name, cn),
                    );
                    self.do_character_killed(cn, 0, false);
                    return;
                }

                if end_depleted {
                    let spell_name = self.items[spell_item as usize].get_name().to_string();
                    self.items[spell_item as usize].active = 0;
                    log::info!(
                        "{} ran out due to lack of endurance for cn={}",
                        spell_name,
                        cn
                    );
                }

                if mana_depleted {
                    let spell_name = self.items[spell_item as usize].get_name().to_string();
                    self.items[spell_item as usize].active = 0;
                    log::info!("{} ran out due to lack of mana for cn={}", spell_name, cn);
                }
            } else {
                // Temporary spell - decrement timer
                if self.items[spell_item as usize].active > 0 {
                    self.items[spell_item as usize].active -= 1;
                }

                let active = self.items[spell_item as usize].active;

                // Warn when spell is about to run out
                if active == core::constants::TICKS as u32 * 30 {
                    let spell_name = self.items[spell_item as usize].get_name().to_string();
                    let is_player_or_usurp = self.characters[cn].flags
                        & (CharacterFlags::Player.bits() | CharacterFlags::Usurp.bits())
                        != 0;
                    let temp = self.characters[cn].temp;
                    let companion_owner = self.characters[cn].data[63];

                    if is_player_or_usurp {
                        self.do_character_log(
                            cn,
                            FontColor::Red,
                            &format!("{} is about to run out.\n", spell_name),
                        );
                    } else if temp == core::constants::CT_COMPANION as u16 && companion_owner != 0 {
                        let co = companion_owner as usize;
                        if co > 0 && co < MAXCHARS {
                            let is_sane_player = self.characters[co].used
                                == core::constants::USE_ACTIVE
                                && self.characters[co].flags & CharacterFlags::Player.bits() != 0;

                            if is_sane_player {
                                let item_temp = self.items[spell_item as usize].temp;

                                // Only inform owner about certain spell types
                                if item_temp == skills::SK_BLESS as u16
                                    || item_temp == skills::SK_PROTECT as u16
                                    || item_temp == skills::SK_ENHANCE as u16
                                {
                                    let co_name = self.characters[co].get_name().to_string();

                                    self.do_sayx(
                                        cn,
                                        format!(
                                            "My spell {} is running out, {}.",
                                            spell_name, co_name,
                                        )
                                        .as_str(),
                                    );
                                }
                            }
                        }
                    }
                }

                // Check item temp for special handling
                let item_temp = self.items[spell_item as usize].temp;

                // Water breathing spell cancels underwater damage
                if item_temp == 649 {
                    uwater_active = false;
                }

                // Magic Shield spell - update armor based on remaining duration
                if item_temp == skills::SK_MSHIELD as u16 {
                    let old_armor = self.items[spell_item as usize].armor[1];
                    let new_armor = active / 1024 + 1;
                    let new_power = active / 256;

                    self.items[spell_item as usize].armor[1] = new_armor as i8;
                    self.items[spell_item as usize].power = new_power;

                    if old_armor != new_armor as i8 {
                        self.characters[cn].set_do_update_flags();
                    }
                }

                // Handle spell expiration
                if active == 0 {
                    let spell_name = self.items[spell_item as usize].get_name().to_string();

                    // Recall spell - teleport character
                    if item_temp == skills::SK_RECALL as u16 {
                        let char_used = self.characters[cn].used;

                        if char_used == core::constants::USE_ACTIVE {
                            let old_x = self.characters[cn].x;
                            let old_y = self.characters[cn].y;
                            let dest_x = self.items[spell_item as usize].data[0];
                            let dest_y = self.items[spell_item as usize].data[1];
                            let is_invisible =
                                self.characters[cn].flags & CharacterFlags::Invisible.bits() != 0;

                            if God::transfer_char(self, cn, dest_x as usize, dest_y as usize) {
                                if !is_invisible {
                                    EffectManager::fx_add_effect(
                                        self,
                                        12,
                                        0,
                                        old_x as i32,
                                        old_y as i32,
                                        0,
                                    );

                                    EffectManager::fx_add_effect(
                                        self,
                                        12,
                                        0,
                                        dest_x as i32,
                                        dest_y as i32,
                                        0,
                                    );
                                }
                            }

                            // Reset character state
                            self.characters[cn].status = 0;
                            self.characters[cn].attack_cn = 0;
                            self.characters[cn].skill_nr = 0;
                            self.characters[cn].goto_x = 0;
                            self.characters[cn].use_nr = 0;
                            self.characters[cn].misc_action = 0;
                            self.characters[cn].dir = core::constants::DX_DOWN;
                        }
                    } else {
                        self.do_character_log(
                            cn,
                            FontColor::Red,
                            &format!("{} ran out.\n", spell_name),
                        );
                    }

                    // Remove spell
                    self.items[spell_item as usize].used = core::constants::USE_EMPTY;
                    self.characters[cn].spell[spell_slot] = 0;
                    self.characters[cn].set_do_update_flags();
                }
            }
        }

        // Handle underwater damage for players
        if uwater_active {
            let is_player = self.characters[cn].flags & CharacterFlags::Player.bits() != 0;
            let is_immortal = self.characters[cn].flags & CharacterFlags::Immortal.bits() != 0;

            if is_player && !is_immortal {
                self.characters[cn].a_hp -= 250 + gothp;

                let is_dead = self.characters[cn].a_hp < 500;
                if is_dead {
                    self.do_character_killed(cn, 0, false);
                }
            }
        }

        // Handle item tear and wear for active players
        let used = self.characters[cn].used;
        let is_player = self.characters[cn].flags & CharacterFlags::Player.bits() != 0;

        if used == core::constants::USE_ACTIVE && is_player {
            driver::char_item_expire(self, cn);
        }
    }

    /// Set the update/save flags for a character (port of `do_update_char`)
    pub(crate) fn do_update_char(&mut self, cn: usize) {
        self.characters[cn].set_do_update_flags();
    }

    /// Remove all active spell items from a character (port of `remove_spells`)
    pub(crate) fn remove_spells(&mut self, cn: usize) {
        for n in 0..20 {
            let spell_item = self.characters[cn].spell[n];
            if spell_item == 0 {
                continue;
            }
            let in_idx = spell_item as usize;
            if in_idx < self.items.len() {
                self.items[in_idx].used = core::constants::USE_EMPTY;
            }
            self.characters[cn].spell[n] = 0;
        }
        self.do_update_char(cn);
    }

    /// Port of `do_raise_attrib(cn, nr)` from `svr_do.cpp`.
    ///
    /// Attempts to raise a base attribute using available character points.
    /// Validates bounds and point cost before incrementing the attribute and
    /// deducting points.
    ///
    /// # Arguments
    /// * `cn` - Character id
    /// * `attrib` - Attribute index (0..4)
    ///
    /// # Returns
    /// * `true` if the attribute was raised, `false` otherwise
    pub(crate) fn do_raise_attrib(&mut self, cn: usize, attrib: i32) -> bool {
        let attrib_idx = attrib as usize;
        if attrib_idx >= 5 {
            return false;
        }

        let current_val = self.characters[cn].attrib[attrib_idx][0];
        let max_val = self.characters[cn].attrib[attrib_idx][2];
        let diff = self.characters[cn].attrib[attrib_idx][3];
        let available_points = self.characters[cn].points;

        // Can't raise if current value is 0 or already at max
        if current_val == 0 || current_val >= max_val {
            return false;
        }

        // Calculate points needed to raise this attribute
        let points_needed = helpers::attrib_needed(current_val as i32, diff as i32);

        if points_needed > available_points {
            return false;
        }

        // Spend points and raise attribute
        self.characters[cn].points -= points_needed;
        self.characters[cn].attrib[attrib_idx][0] += 1;

        true
    }

    /// Port of `do_raise_hp(cn)` from `svr_do.cpp`.
    ///
    /// Attempts to increase the character's base HP at the cost of
    /// character points. Performs validation and updates derived stats.
    ///
    /// # Arguments
    /// * `cn` - Character id
    ///
    /// # Returns
    /// * `true` on success, `false` on failure
    pub(crate) fn do_raise_hp(&mut self, cn: usize) -> bool {
        let current_val = self.characters[cn].hp[0];
        let max_val = self.characters[cn].hp[2];
        let diff = self.characters[cn].hp[3];
        let available_points = self.characters[cn].points;

        if current_val == 0 || current_val >= max_val {
            return false;
        }

        let points_needed = helpers::hp_needed(current_val as i32, diff as i32);

        if points_needed > available_points {
            return false;
        }

        self.characters[cn].points -= points_needed;
        self.characters[cn].hp[0] += 1;

        true
    }

    /// Port of `do_raise_end(cn)` from `svr_do.cpp`.
    ///
    /// Attempts to increase the character's base endurance using available
    /// character points. Updates derived stats on success.
    ///
    /// # Arguments
    /// * `cn` - Character id
    ///
    /// # Returns
    /// * `true` on success, `false` on failure
    pub(crate) fn do_raise_end(&mut self, cn: usize) -> bool {
        let current_val = self.characters[cn].end[0];
        let max_val = self.characters[cn].end[2];
        let diff = self.characters[cn].end[3];
        let available_points = self.characters[cn].points;

        if current_val == 0 || current_val >= max_val {
            return false;
        }

        let points_needed = helpers::end_needed(current_val as i32, diff as i32);

        if points_needed > available_points {
            return false;
        }

        self.characters[cn].points -= points_needed;
        self.characters[cn].end[0] += 1;

        self.do_update_char(cn);

        true
    }

    /// Port of `do_raise_mana(cn)` from `svr_do.cpp`.
    ///
    /// Attempts to increase the character's base mana using available
    /// character points. Validates and updates derived statistics.
    ///
    /// # Arguments
    /// * `cn` - Character id
    ///
    /// # Returns
    /// * `true` on success, `false` on failure
    pub(crate) fn do_raise_mana(&mut self, cn: usize) -> bool {
        let current_val = self.characters[cn].mana[0];
        let max_val = self.characters[cn].mana[2];
        let diff = self.characters[cn].mana[3];
        let available_points = self.characters[cn].points;

        if current_val == 0 || current_val >= max_val {
            return false;
        }

        let points_needed = helpers::mana_needed(current_val as i32, diff as i32);

        if points_needed > available_points {
            return false;
        }

        self.characters[cn].points -= points_needed;
        self.characters[cn].mana[0] += 1;

        self.do_update_char(cn);

        true
    }

    /// Port of `do_raise_skill(cn, nr)` from `svr_do.cpp`.
    ///
    /// Attempts to raise a skill for the character using available points.
    /// Validates bounds, costs and updates the character on success.
    ///
    /// # Arguments
    /// * `cn` - Character id
    /// * `skill` - Skill index (0..49)
    ///
    /// # Returns
    /// * `true` when the skill was increased, `false` otherwise
    pub(crate) fn do_raise_skill(&mut self, cn: usize, skill: i32) -> bool {
        let skill_idx = skill as usize;
        if skill_idx >= 50 {
            return false;
        }

        let current_val = self.characters[cn].skill[skill_idx][0];
        let max_val = self.characters[cn].skill[skill_idx][2];
        let diff = self.characters[cn].skill[skill_idx][3];
        let available_points = self.characters[cn].points;

        if current_val == 0 || current_val >= max_val {
            return false;
        }

        let points_needed = helpers::skill_needed(current_val as i32, diff as i32);

        if points_needed > available_points {
            return false;
        }

        self.characters[cn].points -= points_needed;
        self.characters[cn].skill[skill_idx][0] += 1;
        self.characters[cn].set_do_update_flags();

        true
    }

    /// Port of `do_lower_hp(cn)` from `svr_do.cpp`.
    ///
    /// Permanently reduces base HP for the character and adjusts point
    /// distributions accordingly. Used when applying death penalties.
    ///
    /// # Arguments
    /// * `cn` - Character id
    ///
    /// # Returns
    /// * `true` when the operation succeeded
    pub(crate) fn do_lower_hp(&mut self, cn: usize) -> bool {
        let current_val = self.characters[cn].hp[0];

        if current_val < 11 {
            return false;
        }

        self.characters[cn].hp[0] -= 1;

        let new_val = self.characters[cn].hp[0];
        let diff = self.characters[cn].hp[3];

        let points_lost = helpers::hp_needed(new_val as i32, diff as i32);

        self.characters[cn].points_tot -= points_lost;

        self.do_update_char(cn);

        true
    }

    /// Port of `do_lower_mana(cn)` from `svr_do.cpp`.
    ///
    /// Permanently reduces base mana for the character and adjusts point
    /// totals accordingly. Used when applying death penalties.
    ///
    /// # Arguments
    /// * `cn` - Character id
    ///
    /// # Returns
    /// * `true` when the operation succeeded
    pub(crate) fn do_lower_mana(&mut self, cn: usize) -> bool {
        let current_val = self.characters[cn].mana[0];

        if current_val < 11 {
            return false;
        }

        self.characters[cn].mana[0] -= 1;

        let new_val = self.characters[cn].mana[0];
        let diff = self.characters[cn].mana[3];

        let points_lost = helpers::mana_needed(new_val as i32, diff as i32);

        self.characters[cn].points_tot -= points_lost;
        self.do_update_char(cn);

        true
    }

    /// Port of `do_check_new_level(cn)` from `svr_do.cpp`.
    ///
    /// Evaluates whether the character has reached the next rank threshold
    /// and grants the appropriate increases to HP/END/MANA. Also handles
    /// announcements and any herald/NPC notifications required on level up.
    ///
    /// # Arguments
    /// * `cn` - Character id to check for level advancement
    pub(crate) fn do_check_new_level(&mut self, cn: usize) {
        // Only for players
        if (self.characters[cn].flags & CharacterFlags::Player.bits()) == 0 {
            return;
        }

        let rank = core::ranks::points2rank(self.characters[cn].points_tot as u32) as usize;

        // Check if current rank is less than new rank
        if (self.characters[cn].data[45] as usize) < rank {
            let (hp, end, mana) = if (self.characters[cn].kindred
                & ((traits::KIN_TEMPLAR | traits::KIN_ARCHTEMPLAR) as i32))
                != 0
            {
                (15, 10, 5)
            } else if (self.characters[cn].kindred
                & ((traits::KIN_MERCENARY
                    | traits::KIN_SORCERER
                    | traits::KIN_WARRIOR
                    | traits::KIN_SEYAN_DU) as i32))
                != 0
            {
                (10, 10, 10)
            } else if (self.characters[cn].kindred
                & ((traits::KIN_HARAKIM | traits::KIN_ARCHHARAKIM) as i32))
                != 0
            {
                (5, 10, 15)
            } else {
                return; // Unknown kindred, don't proceed
            };

            let diff = rank - self.characters[cn].data[45] as usize;
            self.characters[cn].data[45] = rank as i32;

            // Log level up message
            if diff == 1 {
                self.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!(
                        "You rose a level! Congratulations! You received {} hitpoints, {} endurance and {} mana.\n",
                        hp * diff,
                        end * diff,
                        mana * diff
                    ),
                );
            } else {
                self.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!(
                        "You rose {} levels! Congratulations! You received {} hitpoints, {} endurance and {} mana.\n",
                        diff,
                        hp * diff,
                        end * diff,
                        mana * diff
                    ),
                );
            }

            // Find an NPC to announce the rank
            let temp = if (self.characters[cn].kindred & (traits::KIN_PURPLE as i32)) != 0 {
                core::constants::CT_PRIEST
            } else {
                core::constants::CT_LGUARD
            };

            // Find a character with appropriate template
            let mut herald_cn = 0;
            for n in 1..core::constants::MAXCHARS {
                if self.characters[n].used != core::constants::USE_ACTIVE {
                    continue;
                }
                if (self.characters[n].flags & CharacterFlags::Body.bits()) != 0 {
                    continue;
                }
                if self.characters[n].temp == temp as u16 {
                    herald_cn = n;
                    break;
                }
            }

            // Have the herald yell it out
            if herald_cn != 0 {
                let char_name = self.characters[cn].get_name().to_string();
                let rank_name = ranks::rank_name_by_index(rank);
                let message = format!(
                    "Hear ye, hear ye! {} has attained the rank of {}!",
                    char_name, rank_name
                );

                self.do_shout(herald_cn, &message);
            }

            // Award stat increases
            self.characters[cn].hp[1] = (hp * rank) as u16;
            self.characters[cn].end[1] = (end * rank) as u16;
            self.characters[cn].mana[1] = (mana * rank) as u16;

            self.do_update_char(cn);
        }
    }

    /// Port of `do_hurt(cn, co, dam, type)` from `svr_do.cpp`.
    ///
    /// Applies damage to a target character (`co`) inflicted by `cn`.
    /// This routine handles armor degradation, magical shields, damage
    /// scaling by type, experience awards for attackers, death handling,
    /// possible god-saves, and reactive gethit damage.
    ///
    /// # Arguments
    /// * `cn` - Attacker character id
    /// * `co` - Target character id
    /// * `dam` - Raw damage value (scaled internally)
    /// * `type_hurt` - Damage type code (influences scaling/FX)
    ///
    /// # Returns
    /// Actual damage dealt in game units (after internal scaling/truncation)
    pub(crate) fn do_hurt(&mut self, cn: usize, co: usize, dam: i32, type_hurt: i32) -> i32 {
        // Quick sanity/body check
        let is_body = (self.characters[co].flags & CharacterFlags::Body.bits()) != 0;
        if is_body {
            return 0;
        }

        // If a real player got hit, damage armour pieces
        let co_is_player = (self.characters[co].flags & CharacterFlags::Player.bits()) != 0;
        if co_is_player {
            driver::item_damage_armor(self, co, dam);
        }

        // Determine noexp conditions
        let mut noexp = 0;
        if cn != 0
            && (self.characters[cn].flags & CharacterFlags::Player.bits()) == 0
            && self.characters[cn].data[63] == co as i32
        {
            noexp = 1;
        }
        if (self.characters[co].flags & CharacterFlags::Player.bits()) != 0 {
            noexp = 1;
        }
        if self.characters[co].temp == core::constants::CT_COMPANION as u16
            && (self.characters[co].flags & CharacterFlags::Thrall.bits()) == 0
        {
            noexp = 1;
        }

        // Handle magical shields (SK_MSHIELD)
        let co_armor = self.characters[co].armor;
        let spells = self.characters[co].spell;
        let mut shield_updates: Vec<(usize, usize, i32, bool)> = Vec::new(); // (slot, item_idx, new_active, kill)

        for n in 0..20 {
            let in_idx = spells[n] as usize;
            if in_idx == 0 {
                continue;
            }

            let (item_temp, item_active) = if in_idx < self.items.len() {
                (self.items[in_idx].temp, self.items[in_idx].active)
            } else {
                (0u16, 0u32)
            };

            if item_temp == skills::SK_MSHIELD as u16 {
                let active = item_active as i32;
                let mut tmp = active / 1024 + 1;
                tmp = (dam + tmp - co_armor as i32) * 5;

                if tmp > 0 {
                    if tmp >= active {
                        shield_updates.push((n, in_idx, 0, true));
                    } else {
                        shield_updates.push((n, in_idx, active - tmp, false));
                    }
                }
            }
        }

        // Apply shield updates
        if !shield_updates.is_empty() {
            for (slot, in_idx, new_active, kill) in shield_updates {
                if kill {
                    self.characters[co].spell[slot] = 0;
                    if in_idx < self.items.len() {
                        self.items[in_idx].used = core::constants::USE_EMPTY;
                    }
                    self.do_update_char(co);
                } else {
                    if in_idx < self.items.len() {
                        self.items[in_idx].active = new_active as u32;
                        self.items[in_idx].armor[1] = (self.items[in_idx].active / 1024 + 1) as i8;
                        self.items[in_idx].power = self.items[in_idx].active / 256;
                    }
                    self.do_update_char(co);
                }
            }
        }

        // Re-read armor after shield updates, matching C behavior.
        let co_armor = self.characters[co].armor;

        // Compute damage scaling by type
        let mut dam = dam;
        if type_hurt == 0 {
            dam -= co_armor as i32;
            if dam < 0 {
                dam = 0;
            } else {
                dam *= 250;
            }
        } else if type_hurt == 3 {
            dam *= 1000;
        } else {
            dam -= co_armor as i32;
            if dam < 0 {
                dam = 0;
            } else {
                dam *= 750;
            }
        }

        // Immortal characters take no damage
        let is_immortal = (self.characters[co].flags & CharacterFlags::Immortal.bits()) != 0;
        if is_immortal {
            dam = 0;
        }

        // Notifications for visible hits
        if type_hurt != 3 {
            let cn_x = self.characters[cn].x;
            let cn_y = self.characters[cn].y;
            self.do_area_notify(
                cn as i32,
                co as i32,
                cn_x as i32,
                cn_y as i32,
                core::constants::NT_SEEHIT as i32,
                cn as i32,
                co as i32,
                0,
                0,
            );
            self.do_notify_character(
                co as u32,
                core::constants::NT_GOTHIT as i32,
                cn as i32,
                dam / 1000,
                0,
                0,
            );
            self.do_notify_character(
                cn as u32,
                core::constants::NT_DIDHIT as i32,
                co as i32,
                dam / 1000,
                0,
                0,
            );
        }

        if dam < 1 {
            return 0;
        }

        // Award some experience for damaging blows
        if type_hurt != 2 && type_hurt != 3 && noexp == 0 {
            let mut tmp = dam / 4000;
            if tmp > 0 && cn != 0 {
                tmp = helpers::scale_exps(&self.characters[cn], &self.characters[co], tmp);
                if tmp > 0 {
                    self.characters[cn].points += tmp;
                    self.characters[cn].points_tot += tmp;
                    self.do_check_new_level(cn);
                }
            }
        }

        // Set map injury flags and show FX
        if type_hurt != 1 {
            let co_x = self.characters[co].x;
            let co_y = self.characters[co].y;
            let idx = (co_x as i32 + co_y as i32 * core::constants::SERVER_MAPX) as usize;
            if dam < 10000 {
                self.map[idx].flags |= core::constants::MF_GFX_INJURED;
            } else if dam < 30000 {
                self.map[idx].flags |=
                    core::constants::MF_GFX_INJURED | core::constants::MF_GFX_INJURED1;
            } else if dam < 50000 {
                self.map[idx].flags |=
                    core::constants::MF_GFX_INJURED | core::constants::MF_GFX_INJURED2;
            } else {
                self.map[idx].flags |= core::constants::MF_GFX_INJURED
                    | core::constants::MF_GFX_INJURED1
                    | core::constants::MF_GFX_INJURED2;
            }
            crate::effect::EffectManager::fx_add_effect(
                self,
                core::constants::FX_INJURED as i32,
                8,
                co_x as i32,
                co_y as i32,
                0,
            );
        }

        // Combined map flags for arena checks (C includes both co/cn positions).
        let co_idx = (self.characters[co].x as i32
            + self.characters[co].y as i32 * core::constants::SERVER_MAPX)
            as usize;
        let mut mf_flags = self.map[co_idx].flags;
        if cn != 0 {
            let cn_idx = (self.characters[cn].x as i32
                + self.characters[cn].y as i32 * core::constants::SERVER_MAPX)
                as usize;
            mf_flags |= self.map[cn_idx].flags;
        }

        // God save check
        let will_die_hp = self.characters[co].a_hp - dam;
        let saved_by_god = (will_die_hp < 500) && (self.characters[co].luck >= 100);

        if saved_by_god {
            if (mf_flags & core::constants::MF_ARENA as u64) == 0
                && helpers::random_mod_i32(10000) < 5000 + self.characters[co].luck
            {
                // Save the character
                self.characters[co].a_hp = self.characters[co].hp[5] as i32 * 500;
                self.characters[co].luck /= 2;
                self.characters[co].data[44] += 1; // saved counter

                self.do_character_log(co, core::types::FontColor::Yellow, "A god reached down and saved you from the killing blow. You must have done the gods a favor sometime in the past!\n");
                let co_x = self.characters[co].x;
                let co_y = self.characters[co].y;
                self.do_area_log(
                    co,
                    0,
                    co_x as i32,
                    co_y as i32,
                    core::types::FontColor::Yellow,
                    &format!(
                        "A god reached down and saved {} from the killing blow.\n",
                        self.characters[co].get_name().to_string()
                    ),
                );
                crate::effect::EffectManager::fx_add_effect(
                    self,
                    6,
                    0,
                    co_x as i32,
                    co_y as i32,
                    0,
                );
                let temple_x = self.characters[co].temple_x as usize;
                let temple_y = self.characters[co].temple_y as usize;
                God::transfer_char(self, co, temple_x, temple_y);
                let new_x = self.characters[co].x;
                let new_y = self.characters[co].y;
                crate::effect::EffectManager::fx_add_effect(
                    self,
                    6,
                    0,
                    new_x as i32,
                    new_y as i32,
                    0,
                );

                self.do_notify_character(
                    cn as u32,
                    core::constants::NT_DIDKILL as i32,
                    co as i32,
                    0,
                    0,
                    0,
                );
                let cn_x = self.characters[cn].x as i32;
                let cn_y = self.characters[cn].y as i32;
                self.do_area_notify(
                    cn as i32,
                    co as i32,
                    cn_x,
                    cn_y,
                    core::constants::NT_SEEKILL as i32,
                    cn as i32,
                    co as i32,
                    0,
                    0,
                );
                return dam / 1000;
            }
        }

        // Subtract hp
        self.characters[co].a_hp -= dam;

        // Warn about low HP
        let cur_hp = self.characters[co].a_hp;
        if (500..8000).contains(&cur_hp) {
            self.do_character_log(
                co,
                core::types::FontColor::Red,
                "You're almost dead... Give running a try!\n",
            );
        }

        // Handle death
        if cur_hp < 500 {
            let cn_x = self.characters[cn].x as i32;
            let cn_y = self.characters[cn].y as i32;
            let co_name = self.characters[co].get_name().to_string();
            self.do_area_log(
                cn,
                co,
                cn_x,
                cn_y,
                core::types::FontColor::Red,
                &format!("{} is dead!\n", co_name),
            );
            let cn_name = self.characters[cn].get_name().to_string();
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("You killed {}.\n", co_name),
            );

            if (self.characters[cn].flags & CharacterFlags::Invisible.bits()) != 0 {
                self.do_character_log(
                    co,
                    core::types::FontColor::Red,
                    "Oh dear, that blow was fatal. Somebody killed you...\n",
                );
            } else {
                self.do_character_log(
                    co,
                    core::types::FontColor::Red,
                    &format!("Oh dear, that blow was fatal. {} killed you...\n", cn_name),
                );
            }

            self.do_notify_character(
                cn as u32,
                core::constants::NT_DIDKILL as i32,
                co as i32,
                0,
                0,
                0,
            );
            let cn_x = self.characters[cn].x as i32;
            let cn_y = self.characters[cn].y as i32;
            self.do_area_notify(
                cn as i32,
                co as i32,
                cn_x,
                cn_y,
                core::constants::NT_SEEKILL as i32,
                cn as i32,
                co as i32,
                0,
                0,
            );

            // Score and EXP handing (defer to helpers/stubs)
            if type_hurt != 2
                && cn != 0
                && (mf_flags & core::constants::MF_ARENA as u64) == 0
                && noexp == 0
            {
                let tmp = self.do_char_score(co);
                let rank = core::ranks::points2rank(self.characters[co].points_tot as u32) as i32;
                let mut tmp = tmp;
                let has_medit = self.characters[co].skill[skills::SK_MEDIT][0] != 0;
                if !has_medit {
                    let spells = self.characters[co].spell;
                    for n in 0..20 {
                        let in_idx = spells[n] as usize;
                        if in_idx == 0 {
                            continue;
                        }
                        let item_temp = self.items[in_idx].temp;
                        if item_temp == skills::SK_PROTECT as u16
                            || item_temp == skills::SK_ENHANCE as u16
                            || item_temp == skills::SK_BLESS as u16
                        {
                            tmp += tmp / 5;
                        }
                    }
                }

                self.do_character_killed(co, cn, false);
                if type_hurt != 2 && cn != 0 && cn != co {
                    self.do_give_exp(cn, tmp, 1, rank);
                }
            } else {
                self.do_character_killed(co, cn, false);
            }

            self.characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
        } else {
            // Reactive damage (gethit)
            if type_hurt == 0 {
                let gethit = self.characters[co].gethit_dam;
                if gethit > 0 {
                    let odam = helpers::random_mod_i32(gethit as i32) + 1;
                    // call do_hurt on attacker
                    self.do_hurt(co, cn, odam, 3);
                }
            }
        }

        dam / 1000
    }
}
