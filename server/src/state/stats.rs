use core::constants::{
    CharacterFlags, ItemFlags, MAX_SPEEDTAB_SPEED_INDEX, MAXCHARS, MIN_SPEEDTAB_INDEX,
};
use core::ranks;
use core::talent_trees::{
    available_talent_points, grant_talent_points, talent_stat_bonuses, total_points_spent,
};
use core::types::FontColor;
use core::{skills, traits};

use crate::effect::EffectManager;
use crate::game_state::GameState;
use crate::god::God;
use crate::{driver, helpers, points};

impl GameState {
    /// Helper function to check if character wears a specific item
    /// Port of part of `really_update_char`
    pub(crate) fn char_wears_item(&mut self, cn: usize, item_template: u16) -> bool {
        for n in 0..20 {
            let item_idx = self.characters[cn].worn[n];
            if item_idx != 0 && self.items[item_idx as usize].temp == item_template {
                return true;
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
        helpers::sync_weapon_skill(&mut self.characters[cn].skill);

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
        let has_nomagic_flag =
            self.map[map_index].flags & u64::from(core::constants::MF_NOMAGIC) != 0;

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
                for (z, bonus) in attrib_bonus.iter_mut().enumerate().take(5) {
                    *bonus += if item.active != 0 {
                        i32::from(item.attrib[z][1])
                    } else {
                        i32::from(item.attrib[z][0])
                    };
                }

                hp_bonus += if item.active != 0 {
                    i32::from(item.hp[1])
                } else {
                    i32::from(item.hp[0])
                };

                end_bonus += if item.active != 0 {
                    i32::from(item.end[1])
                } else {
                    i32::from(item.end[0])
                };

                mana_bonus += if item.active != 0 {
                    i32::from(item.mana[1])
                } else {
                    i32::from(item.mana[0])
                };

                let modifier_idx = if item.active != 0 { 1 } else { 0 };
                helpers::add_canonical_skill_bonuses(&mut skill_bonus, &item.skill, modifier_idx);
            }

            // Add physical bonuses (always apply)
            if item.active != 0 {
                armor += i32::from(item.armor[1]);
                gethit += i32::from(item.gethit_dam[1]);
                if i32::from(item.weapon[1]) > weapon {
                    weapon = i32::from(item.weapon[1]);
                }
                if i32::from(item.light[1]) > light {
                    light = i32::from(item.light[1]);
                } else if item.light[1] < 0 {
                    sublight -= i32::from(item.light[1]);
                }
            } else {
                armor += i32::from(item.armor[0]);
                gethit += i32::from(item.gethit_dam[0]);
                if i32::from(item.weapon[0]) > weapon {
                    weapon = i32::from(item.weapon[0]);
                }
                if i32::from(item.light[0]) > light {
                    light = i32::from(item.light[0]);
                } else if item.light[0] < 0 {
                    sublight -= i32::from(item.light[0]);
                }
            }
        }

        // Add permanent bonuses
        armor += i32::from(self.characters[cn].armor_bonus);
        weapon += i32::from(self.characters[cn].weapon_bonus);
        gethit += i32::from(self.characters[cn].gethit_bonus);
        light += i32::from(self.characters[cn].light_bonus);

        // Calculate bonuses from active spells
        if !char_has_nomagic {
            for n in 0..20 {
                let spell_idx = self.characters[cn].spell[n];
                if spell_idx == 0 {
                    continue;
                }

                let spell = &mut self.items[spell_idx as usize];

                for (z, bonus) in attrib_bonus.iter_mut().enumerate().take(5) {
                    *bonus += i32::from(spell.attrib[z][1]);
                }

                hp_bonus += i32::from(spell.hp[1]);
                end_bonus += i32::from(spell.end[1]);
                mana_bonus += i32::from(spell.mana[1]);

                helpers::add_canonical_skill_bonuses(&mut skill_bonus, &spell.skill, 1);

                armor += i32::from(spell.armor[1]);
                weapon += i32::from(spell.weapon[1]);
                if i32::from(spell.light[1]) > light {
                    light = i32::from(spell.light[1]);
                } else if spell.light[1] < 0 {
                    sublight -= i32::from(spell.light[1]);
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

        let talent_bonuses = talent_stat_bonuses(
            self.characters[cn].kindred,
            &self.characters[cn].future1,
            &self.characters[cn].attrib,
            &self.characters[cn].skill,
        );
        for (z, bonus) in attrib_bonus.iter_mut().enumerate() {
            *bonus += talent_bonuses.attrib[z];
        }
        for (z, bonus) in skill_bonus.iter_mut().enumerate() {
            *bonus += talent_bonuses.skill[z];
        }
        hp_bonus += talent_bonuses.hp_flat;
        mana_bonus += talent_bonuses.mana_flat;
        end_bonus += talent_bonuses.end_flat;

        // Calculate final attributes
        for (z, &bonus) in attrib_bonus.iter().enumerate().take(5) {
            let mut final_attrib = i32::from(self.characters[cn].attrib[z][0])
                + i32::from(self.characters[cn].attrib[z][1])
                + bonus;

            final_attrib = final_attrib.clamp(1, 250);
            self.characters[cn].attrib[z][5] = final_attrib as u8;
        }

        // Calculate final HP
        let mut final_hp =
            i32::from(self.characters[cn].hp[0]) + i32::from(self.characters[cn].hp[1]) + hp_bonus;
        final_hp = final_hp.clamp(10, 999);
        self.characters[cn].hp[5] = final_hp as u16;

        // Calculate final endurance
        let mut final_end = i32::from(self.characters[cn].end[0])
            + i32::from(self.characters[cn].end[1])
            + end_bonus;
        final_end = final_end.clamp(10, 999);
        self.characters[cn].end[5] = final_end as u16;

        // Calculate final mana
        let mut final_mana = i32::from(self.characters[cn].mana[0])
            + i32::from(self.characters[cn].mana[1])
            + mana_bonus;
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
        for (z, &bonus) in skill_bonus.iter().enumerate().take(50) {
            let mut final_skill = i32::from(self.characters[cn].skill[z][0])
                + i32::from(self.characters[cn].skill[z][1])
                + bonus;

            // Add attribute bonuses using the proper skill->attribute mapping from `skills`
            let attrs = skills::get_skill_attribs(z);
            let attrib_contribution = (i32::from(self.characters[cn].attrib[attrs[0]][5])
                + i32::from(self.characters[cn].attrib[attrs[1]][5])
                + i32::from(self.characters[cn].attrib[attrs[2]][5]))
                / 5;
            final_skill += attrib_contribution;
            final_skill = final_skill.clamp(1, 250);
            self.characters[cn].skill[z][5] = final_skill as u8;
        }

        // Apply talent-derived armor/weapon percent bonuses to the aggregated
        // (items + permanent + spells) totals before clamping.
        if talent_bonuses.armor_percent != 0 {
            armor += (armor as f32 * (talent_bonuses.armor_percent as f32 / 100.0)).round() as i32;
        }
        if talent_bonuses.weapon_percent != 0 {
            weapon +=
                (weapon as f32 * (talent_bonuses.weapon_percent as f32 / 100.0)).round() as i32;
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
        let agil = i32::from(self.characters[cn].attrib[core::constants::AT_AGIL as usize][5]);
        let stren = i32::from(self.characters[cn].attrib[core::constants::AT_STREN as usize][5]);
        let speed_mod = i32::from(self.characters[cn].speed_mod);

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
        if self.characters[cn].a_hp > i32::from(self.characters[cn].hp[5]) * 1000 {
            self.characters[cn].a_hp = i32::from(self.characters[cn].hp[5]) * 1000;
        }
        if self.characters[cn].a_end > i32::from(self.characters[cn].end[5]) * 1000 {
            self.characters[cn].a_end = i32::from(self.characters[cn].end[5]) * 1000;
        }
        if self.characters[cn].a_mana > i32::from(self.characters[cn].mana[5]) * 1000 {
            self.characters[cn].a_mana = i32::from(self.characters[cn].mana[5]) * 1000;
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
                let idx = (i32::from(x) + i32::from(y) * core::constants::SERVER_MAPX) as usize;
                let map_char = self.map[idx].ch;

                if map_char == cn as u32 {
                    self.do_add_light(
                        i32::from(x),
                        i32::from(y),
                        i32::from(new_light) - i32::from(old_light),
                    );
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
        let uwater = self.map[map_idx].flags & u64::from(core::constants::MF_UWATER) != 0;

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
                                i32::from(self.characters[cn].skill[skills::SK_REST][5]) * moonmult
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
                                i32::from(self.characters[cn].skill[skills::SK_REGEN][5])
                                    * moonmult
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
                                i32::from(self.characters[cn].skill[skills::SK_MEDIT][5])
                                    * moonmult
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
                        } else if mode == 0 && !noend {
                            self.characters[cn].a_end += scale(25);
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
                        i32::from(self.characters[cn].skill[skills::SK_REGEN][5]) * moonmult / 60,
                    );
                }
                if has_rest {
                    self.characters[cn].a_end += scale(
                        i32::from(self.characters[cn].skill[skills::SK_REST][5]) * moonmult / 60,
                    );
                }
                if has_medit {
                    self.characters[cn].a_mana += scale(
                        i32::from(self.characters[cn].skill[skills::SK_MEDIT][5]) * moonmult / 60,
                    );
                }
            }
        }

        // Cap accumulated stats at their maximums (max * 1000)
        if self.characters[cn].a_hp > i32::from(self.characters[cn].hp[5]) * 1000 {
            self.characters[cn].a_hp = i32::from(self.characters[cn].hp[5]) * 1000;
        }
        if self.characters[cn].a_end > i32::from(self.characters[cn].end[5]) * 1000 {
            self.characters[cn].a_end = i32::from(self.characters[cn].end[5]) * 1000;
        }
        if self.characters[cn].a_mana > i32::from(self.characters[cn].mana[5]) * 1000 {
            self.characters[cn].a_mana = i32::from(self.characters[cn].mana[5]) * 1000;
        }

        // Set timer when regenerating below 90% of max
        if hp_regen {
            let needs_timer = self.characters[cn].a_hp < i32::from(self.characters[cn].hp[5]) * 900;
            if needs_timer {
                self.characters[cn].data[92] = core::constants::TICKS * 60;
            }
        }

        if mana_regen {
            let needs_timer =
                self.characters[cn].a_mana < i32::from(self.characters[cn].mana[5]) * 900;
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
                    self.characters[cn].a_hp += i32::from(hp_change);
                    if self.characters[cn].a_hp < 500 {
                        killed = true;
                    }
                }
                if end_change != -1 {
                    self.characters[cn].a_end += i32::from(end_change);
                    if self.characters[cn].a_end < 500 {
                        self.characters[cn].a_end = 500;
                        end_depleted = true;
                    }
                }
                if mana_change != -1 {
                    self.characters[cn].a_mana += i32::from(mana_change);
                    if self.characters[cn].a_mana < 500 {
                        self.characters[cn].a_mana = 500;
                        mana_depleted = true;
                    }
                }

                if killed {
                    let spell_name = self.items[spell_item as usize].get_name().to_owned();
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
                    let spell_name = self.items[spell_item as usize].get_name().to_owned();
                    self.items[spell_item as usize].active = 0;
                    log::info!(
                        "{} ran out due to lack of endurance for cn={}",
                        spell_name,
                        cn
                    );
                }

                if mana_depleted {
                    let spell_name = self.items[spell_item as usize].get_name().to_owned();
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
                    let spell_name = self.items[spell_item as usize].get_name().to_owned();
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
                                    let co_name = self.characters[co].get_name().to_owned();

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

                // Parasite / Contagion damage-over-time with caster lifesteal.
                // Each spell-item stores the caster in `data[0]` and ticks
                // once per second based on remaining duration. Damage scales
                // with `power`; the caster heals for 25% of damage dealt.
                if item_temp == skills::SK_PARASITE as u16
                    || item_temp == skills::SK_CONTAGION as u16
                {
                    let item = &self.items[spell_item as usize];
                    let duration = item.duration as i32;
                    let active_i = active as i32;
                    let power = item.power as i32;
                    let caster = item.data[0] as usize;
                    // Tick once per second of real time.
                    let elapsed = duration - active_i;
                    if elapsed > 0 && elapsed % core::constants::TICKS == 0 {
                        let base_dam = (power / 4).max(1);
                        let dam = if item_temp == skills::SK_CONTAGION as u16 {
                            base_dam * 2
                        } else {
                            base_dam
                        };
                        let dam_unit = dam * 1000;
                        // Apply DoT directly without going through do_hurt to
                        // avoid amplifying with armor (the parasite eats
                        // flesh from the inside).
                        self.characters[cn].a_hp -= dam_unit;
                        // Lifesteal: caster regains 25% of damage dealt, if
                        // still alive and a sane character index.
                        if core::types::Character::is_sane_character(caster)
                            && caster != cn
                            && self.characters[caster].used == core::constants::USE_ACTIVE
                        {
                            let heal = dam_unit / 4;
                            self.characters[caster].a_hp += heal;
                            let max_hp = i32::from(self.characters[caster].hp[5]) * 1000;
                            if self.characters[caster].a_hp > max_hp {
                                self.characters[caster].a_hp = max_hp;
                            }
                        }
                        if self.characters[cn].a_hp < 500 {
                            self.characters[cn].a_hp = 500;
                            let spell_name = self.items[spell_item as usize].get_name().to_owned();
                            self.do_character_log(
                                cn,
                                FontColor::Red,
                                &format!("The {} killed you!\n", spell_name),
                            );
                            self.do_character_killed(cn, caster, false);
                            return;
                        }
                    }
                }

                // Rains of Renewal heal-over-time. Ticks once per second of
                // real time, restoring HP scaled by the caster's skill power.
                if item_temp == skills::SK_RAINS_OF_RENEWAL as u16 {
                    let item = &self.items[spell_item as usize];
                    let duration = item.duration as i32;
                    let active_i = active as i32;
                    let power = item.power as i32;
                    let elapsed = duration - active_i;
                    if elapsed > 0 && elapsed % core::constants::TICKS == 0 {
                        let heal = (power / 4).max(1) * 1000;
                        self.characters[cn].a_hp += heal;
                        let max_hp = i32::from(self.characters[cn].hp[5]) * 1000;
                        if self.characters[cn].a_hp > max_hp {
                            self.characters[cn].a_hp = max_hp;
                        }
                    }
                }

                // Handle spell expiration
                if active == 0 {
                    let spell_name = self.items[spell_item as usize].get_name().to_owned();

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

                            if God::transfer_char(self, cn, dest_x as usize, dest_y as usize)
                                && !is_invisible
                            {
                                EffectManager::fx_add_effect(
                                    self,
                                    12,
                                    0,
                                    i32::from(old_x),
                                    i32::from(old_y),
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
        let points_needed = points::attrib_needed(i32::from(current_val), i32::from(diff));

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

        let points_needed = points::hp_needed(i32::from(current_val), i32::from(diff));

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

        let points_needed = points::end_needed(i32::from(current_val), i32::from(diff));

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

        let points_needed = points::mana_needed(i32::from(current_val), i32::from(diff));

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
        helpers::sync_weapon_skill(&mut self.characters[cn].skill);

        let skill_idx = skills::canonicalize_weapon_skill(skill as usize);
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

        let points_needed = points::skill_needed(i32::from(current_val), i32::from(diff));

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

        let points_lost = points::hp_needed(i32::from(new_val), i32::from(diff));

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

        let points_lost = points::mana_needed(i32::from(new_val), i32::from(diff));

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

            let old_rank = self.characters[cn].data[45] as usize;
            let diff = rank - old_rank;

            let crossed_milestones =
                u16::from(ranks::talent_points_awarded_between(old_rank, rank));
            let total_entitlement = u16::from(ranks::talent_points_awarded_between(0, rank));
            let current_total_talent_points =
                u16::from(available_talent_points(&self.characters[cn].future1))
                    + (total_points_spent(&self.characters[cn].future1) as u16);

            let remaining_entitlement =
                total_entitlement.saturating_sub(current_total_talent_points);
            let talent_points = crossed_milestones.min(remaining_entitlement) as u8;

            self.characters[cn].data[45] = rank as i32;
            if talent_points > 0 {
                grant_talent_points(&mut self.characters[cn].future1, talent_points);
            }

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

            if talent_points == 1 {
                self.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    "You gained 1 talent point. Open the Talents panel to spend it.\n",
                );
            } else if talent_points > 1 {
                self.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!(
                        "You gained {} talent points. Open the Talents panel to spend them.\n",
                        talent_points
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
                let char_name = self.characters[cn].get_name().to_owned();
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

            let player_id = self.characters[cn].player as usize;
            if player_id > 0 && player_id < self.players.len() && self.players[player_id].usnr == cn
            {
                crate::player::commands::send_set_char_talents(self, player_id);
            }
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

        for (n, &spell_ref) in spells[..20].iter().enumerate() {
            let in_idx = spell_ref as usize;
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
                tmp = (dam + tmp - i32::from(co_armor)) * 5;

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
            dam -= i32::from(co_armor);
            if dam < 0 {
                dam = 0;
            } else {
                dam *= 250;
            }
        } else if type_hurt == 3 {
            dam *= 1000;
        } else {
            dam -= i32::from(co_armor);
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
                i32::from(cn_x),
                i32::from(cn_y),
                i32::from(core::constants::NT_SEEHIT),
                cn as i32,
                co as i32,
                0,
                0,
            );
            self.do_notify_character(
                co as u32,
                i32::from(core::constants::NT_GOTHIT),
                cn as i32,
                dam / 1000,
                0,
                0,
            );
            self.do_notify_character(
                cn as u32,
                i32::from(core::constants::NT_DIDHIT),
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
            let idx = (i32::from(co_x) + i32::from(co_y) * core::constants::SERVER_MAPX) as usize;
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
                i32::from(core::constants::FX_INJURED),
                8,
                i32::from(co_x),
                i32::from(co_y),
                0,
            );
        }

        // Combined map flags for arena checks (C includes both co/cn positions).
        let co_idx = (i32::from(self.characters[co].x)
            + i32::from(self.characters[co].y) * core::constants::SERVER_MAPX)
            as usize;
        let mut mf_flags = self.map[co_idx].flags;
        if cn != 0 {
            let cn_idx = (i32::from(self.characters[cn].x)
                + i32::from(self.characters[cn].y) * core::constants::SERVER_MAPX)
                as usize;
            mf_flags |= self.map[cn_idx].flags;
        }

        // God save check
        let will_die_hp = self.characters[co].a_hp - dam;
        let saved_by_god = (will_die_hp < 500) && (self.characters[co].luck >= 100);

        if saved_by_god
            && (mf_flags & u64::from(core::constants::MF_ARENA)) == 0
            && helpers::random_mod_i32(10000) < 5000 + self.characters[co].luck
        {
            // Save the character
            self.characters[co].a_hp = i32::from(self.characters[co].hp[5]) * 500;
            self.characters[co].luck /= 2;
            self.characters[co].data[44] += 1; // saved counter

            self.do_character_log(co, core::types::FontColor::Yellow, "A god reached down and saved you from the killing blow. You must have done the gods a favor sometime in the past!\n");
            let co_x = self.characters[co].x;
            let co_y = self.characters[co].y;
            self.do_area_log(
                co,
                0,
                i32::from(co_x),
                i32::from(co_y),
                core::types::FontColor::Yellow,
                &format!(
                    "A god reached down and saved {} from the killing blow.\n",
                    self.characters[co].get_name().to_owned()
                ),
            );
            crate::effect::EffectManager::fx_add_effect(
                self,
                6,
                0,
                i32::from(co_x),
                i32::from(co_y),
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
                i32::from(new_x),
                i32::from(new_y),
                0,
            );

            self.do_notify_character(
                cn as u32,
                i32::from(core::constants::NT_DIDKILL),
                co as i32,
                0,
                0,
                0,
            );
            let cn_x = i32::from(self.characters[cn].x);
            let cn_y = i32::from(self.characters[cn].y);
            self.do_area_notify(
                cn as i32,
                co as i32,
                cn_x,
                cn_y,
                i32::from(core::constants::NT_SEEKILL),
                cn as i32,
                co as i32,
                0,
                0,
            );
            return dam / 1000;
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
            let cn_x = i32::from(self.characters[cn].x);
            let cn_y = i32::from(self.characters[cn].y);
            let co_name = self.characters[co].get_name().to_owned();
            self.do_area_log(
                cn,
                co,
                cn_x,
                cn_y,
                core::types::FontColor::Red,
                &format!("{} is dead!\n", co_name),
            );
            let cn_name = self.characters[cn].get_name().to_owned();
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
                i32::from(core::constants::NT_DIDKILL),
                co as i32,
                0,
                0,
                0,
            );
            let cn_x = i32::from(self.characters[cn].x);
            let cn_y = i32::from(self.characters[cn].y);
            self.do_area_notify(
                cn as i32,
                co as i32,
                cn_x,
                cn_y,
                i32::from(core::constants::NT_SEEKILL),
                cn as i32,
                co as i32,
                0,
                0,
            );

            // Score and EXP handing (defer to helpers/stubs)
            if type_hurt != 2
                && cn != 0
                && (mf_flags & u64::from(core::constants::MF_ARENA)) == 0
                && noexp == 0
            {
                let tmp = self.do_char_score(co);
                let rank = core::ranks::points2rank(self.characters[co].points_tot as u32) as i32;
                let mut tmp = tmp;
                let has_medit = self.characters[co].skill[skills::SK_MEDIT][0] != 0;
                if !has_medit {
                    let spells = self.characters[co].spell;
                    for &spell_ref in &spells[..20] {
                        let in_idx = spell_ref as usize;
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
                    let odam = helpers::random_mod_i32(i32::from(gethit)) + 1;
                    // call do_hurt on attacker
                    self.do_hurt(co, cn, odam, 3);
                }
            }
        }

        dam / 1000
    }
}

#[cfg(test)]
mod tests {
    use core::{
        constants::USE_ACTIVE,
        skills::{self, SkillIndex},
        traits,
    };

    use crate::test_helpers::{add_test_player, with_test_gs};
    use crate::{driver, game_state::GameState};

    fn seed_weapon_skill_baseline(gs: &mut GameState, cn: usize) {
        for attrib_idx in 0..5 {
            gs.characters[cn].attrib[attrib_idx][SkillIndex::BaseValue as usize] = 30;
        }

        gs.characters[cn].skill[skills::SK_WEAPON][SkillIndex::BaseValue as usize] = 50;
        gs.characters[cn].skill[skills::SK_WEAPON][SkillIndex::MaxValue as usize] = 100;
    }

    fn weapon_skill_total(gs: &GameState, cn: usize) -> i32 {
        i32::from(gs.characters[cn].skill[skills::SK_WEAPON][SkillIndex::TotalValue as usize])
    }

    fn weapon_skill_attribute_contribution(gs: &GameState, cn: usize) -> i32 {
        let attrs = skills::get_skill_attribs(skills::SK_WEAPON);
        (i32::from(gs.characters[cn].attrib[attrs[0]][SkillIndex::TotalValue as usize])
            + i32::from(gs.characters[cn].attrib[attrs[1]][SkillIndex::TotalValue as usize])
            + i32::from(gs.characters[cn].attrib[attrs[2]][SkillIndex::TotalValue as usize]))
            / 5
    }

    fn set_legacy_weapon_bonuses(skill: &mut [[i8; 3]; skills::MAX_SKILLS], modifier_idx: usize) {
        skill[skills::SK_HAND][modifier_idx] = 10;
        skill[skills::SK_DAGGER][modifier_idx] = 10;
        skill[skills::SK_TWOHAND][modifier_idx] = 10;
    }

    #[test]
    fn active_spell_legacy_weapon_bonuses_collapse_per_source() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            seed_weapon_skill_baseline(gs, cn);
            gs.really_update_char(cn);
            let baseline = weapon_skill_total(gs, cn);

            let spell_idx = 10;
            gs.items[spell_idx] = core::types::Item::default();
            gs.items[spell_idx].used = USE_ACTIVE;
            set_legacy_weapon_bonuses(&mut gs.items[spell_idx].skill, 1);
            gs.characters[cn].spell[0] = spell_idx as u32;

            gs.really_update_char(cn);

            assert_eq!(weapon_skill_total(gs, cn) - baseline, 10);
        });
    }

    #[test]
    fn worn_item_legacy_weapon_bonuses_collapse_per_source() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            seed_weapon_skill_baseline(gs, cn);
            gs.really_update_char(cn);
            let baseline = weapon_skill_total(gs, cn);

            let item_idx = 10;
            gs.items[item_idx] = core::types::Item::default();
            gs.items[item_idx].used = USE_ACTIVE;
            set_legacy_weapon_bonuses(&mut gs.items[item_idx].skill, 0);
            gs.characters[cn].worn[0] = item_idx as u32;

            gs.really_update_char(cn);

            assert_eq!(weapon_skill_total(gs, cn) - baseline, 10);
        });
    }

    #[test]
    fn separate_weapon_bonus_sources_still_stack() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            seed_weapon_skill_baseline(gs, cn);
            gs.really_update_char(cn);
            let baseline = weapon_skill_total(gs, cn);

            let first_spell_idx = 10;
            gs.items[first_spell_idx] = core::types::Item::default();
            gs.items[first_spell_idx].used = USE_ACTIVE;
            set_legacy_weapon_bonuses(&mut gs.items[first_spell_idx].skill, 1);
            gs.characters[cn].spell[0] = first_spell_idx as u32;

            let second_spell_idx = 11;
            gs.items[second_spell_idx] = core::types::Item::default();
            gs.items[second_spell_idx].used = USE_ACTIVE;
            gs.items[second_spell_idx].skill[skills::SK_WEAPON][1] = 10;
            gs.characters[cn].spell[1] = second_spell_idx as u32;

            gs.really_update_char(cn);

            assert_eq!(weapon_skill_total(gs, cn) - baseline, 20);
        });
    }

    #[test]
    fn bless_spell_does_not_add_direct_weapon_skill_bonuses() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            seed_weapon_skill_baseline(gs, cn);
            gs.item_templates[1].used = USE_ACTIVE;
            gs.really_update_char(cn);
            let baseline_total = weapon_skill_total(gs, cn);
            let baseline_attribute_contribution = weapon_skill_attribute_contribution(gs, cn);

            assert!(driver::spell_bless(gs, cn, cn, 50));

            let spell_idx = gs.characters[cn].spell[0] as usize;
            assert_ne!(spell_idx, 0);
            assert_eq!(gs.items[spell_idx].skill[skills::SK_WEAPON][1], 0);
            for legacy_skill in skills::LEGACY_WEAPON_SKILLS {
                assert_eq!(gs.items[spell_idx].skill[legacy_skill][1], 0);
            }

            let blessed_total = weapon_skill_total(gs, cn);
            let blessed_attribute_contribution = weapon_skill_attribute_contribution(gs, cn);

            assert_eq!(
                blessed_total - baseline_total,
                blessed_attribute_contribution - baseline_attribute_contribution
            );
        });
    }

    #[test]
    fn rank_up_grants_talent_points_only_for_milestone_ranks() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.characters[cn].kindred = traits::KIN_MERCENARY as i32;
            gs.characters[cn].used = USE_ACTIVE;
            gs.characters[cn].data[45] = 0;
            gs.characters[cn].points_tot = core::ranks::RANK_THRESHOLDS[3] as i32;

            gs.do_check_new_level(cn);

            assert_eq!(gs.characters[cn].future1[0], 2);
            assert_eq!(gs.characters[cn].data[45], 3);
        });
    }

    #[test]
    fn rank_up_to_non_milestone_does_not_grant_talent_point() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.characters[cn].kindred = traits::KIN_MERCENARY as i32;
            gs.characters[cn].used = USE_ACTIVE;
            gs.characters[cn].data[45] = 1;
            gs.characters[cn].points_tot = core::ranks::RANK_THRESHOLDS[2] as i32;

            gs.do_check_new_level(cn);

            assert_eq!(gs.characters[cn].future1[0], 0);
            assert_eq!(gs.characters[cn].data[45], 2);
        });
    }

    #[test]
    fn rank_check_without_new_rank_does_not_grant_talent_points() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.characters[cn].kindred = traits::KIN_MERCENARY as i32;
            gs.characters[cn].used = USE_ACTIVE;
            gs.characters[cn].data[45] = 3;
            gs.characters[cn].points_tot = core::ranks::RANK_THRESHOLDS[3] as i32;

            gs.do_check_new_level(cn);

            assert_eq!(gs.characters[cn].future1[0], 0);
        });
    }

    #[test]
    fn rank_up_does_not_over_grant_when_entitlement_already_met() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.characters[cn].kindred = traits::KIN_MERCENARY as i32;
            gs.characters[cn].used = USE_ACTIVE;
            gs.characters[cn].data[45] = 0;
            gs.characters[cn].points_tot = core::ranks::RANK_THRESHOLDS[3] as i32;
            gs.characters[cn].future1[0] = 2;

            gs.do_check_new_level(cn);

            assert_eq!(gs.characters[cn].future1[0], 2);
            assert_eq!(gs.characters[cn].data[45], 3);
        });
    }

    #[test]
    fn rank_up_grants_only_missing_entitlement_points() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.characters[cn].kindred = traits::KIN_MERCENARY as i32;
            gs.characters[cn].used = USE_ACTIVE;
            gs.characters[cn].data[45] = 0;
            gs.characters[cn].points_tot = core::ranks::RANK_THRESHOLDS[3] as i32;
            gs.characters[cn].future1[0] = 1;

            gs.do_check_new_level(cn);

            assert_eq!(gs.characters[cn].future1[0], 2);
            assert_eq!(gs.characters[cn].data[45], 3);
        });
    }
}
