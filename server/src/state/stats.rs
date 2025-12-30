use core::constants::{CharacterFlags, ItemFlags, MAXCHARS};
use core::types::FontColor;
use rand::Rng;

use crate::effect::EffectManager;
use crate::god::God;
use crate::repository::Repository;
use crate::state::State;
use crate::{driver, helpers};

impl State {
    /// Helper function to check if character wears a specific item
    /// Port of part of `really_update_char`
    pub(crate) fn char_wears_item(&self, cn: usize, item_template: u16) -> bool {
        Repository::with_characters(|characters| {
            for n in 0..20 {
                let item_idx = characters[cn].worn[n];
                if item_idx != 0 {
                    let matches = Repository::with_items(|items| {
                        items[item_idx as usize].temp == item_template
                    });
                    if matches {
                        return true;
                    }
                }
            }
            false
        })
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
        Repository::with_characters_mut(|characters| {
            characters[cn].flags &= !(CharacterFlags::CF_NOHPREG.bits()
                | CharacterFlags::CF_NOENDREG.bits()
                | CharacterFlags::CF_NOMANAREG.bits());
            characters[cn].sprite_override = 0;
        });

        // Check for NOMAGIC map flag
        let (char_x, char_y, wears_466, wears_481) = Repository::with_characters(|characters| {
            (
                characters[cn].x,
                characters[cn].y,
                self.char_wears_item(cn, 466),
                self.char_wears_item(cn, 481),
            )
        });

        let map_index =
            (char_x as usize + char_y as usize * core::constants::SERVER_MAPX as usize) as usize;
        let has_nomagic_flag = Repository::with_map(|map| {
            map[map_index].flags & core::constants::MF_NOMAGIC as u64 != 0
        });

        if has_nomagic_flag && !wears_466 && !wears_481 {
            let already_has_nomagic = Repository::with_characters(|ch| {
                ch[cn].flags & CharacterFlags::CF_NOMAGIC.bits() != 0
            });

            if !already_has_nomagic {
                Repository::with_characters_mut(|characters| {
                    characters[cn].flags |= CharacterFlags::CF_NOMAGIC.bits();
                });
                self.remove_spells(cn);
                self.do_character_log(cn, FontColor::Green, "You feel your magic fail.\n");
            }
        } else {
            let has_nomagic = Repository::with_characters(|ch| {
                ch[cn].flags & CharacterFlags::CF_NOMAGIC.bits() != 0
            });

            if has_nomagic {
                Repository::with_characters_mut(|characters| {
                    characters[cn].flags &= !CharacterFlags::CF_NOMAGIC.bits();
                    characters[cn].set_do_update_flags();
                });
                self.do_character_log(cn, FontColor::Green, "You feel your magic return.\n");
            }
        }

        let old_light = Repository::with_characters(|characters| characters[cn].light);

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
        Repository::with_characters_mut(|characters| {
            for n in 0..5 {
                characters[cn].attrib[n][4] = 0;
            }
            characters[cn].hp[4] = 0;
            characters[cn].end[4] = 0;
            characters[cn].mana[4] = 0;
            for n in 0..50 {
                characters[cn].skill[n][4] = 0;
            }
            characters[cn].armor = 0;
            characters[cn].weapon = 0;
            characters[cn].gethit_dam = 0;
            characters[cn].stunned = 0;
            characters[cn].light = 0;
        });

        let char_has_nomagic =
            Repository::with_characters(|ch| ch[cn].flags & CharacterFlags::CF_NOMAGIC.bits() != 0);

        // Calculate bonuses from worn items
        for n in 0..20 {
            let item_idx = Repository::with_characters(|ch| ch[cn].worn[n]);
            if item_idx == 0 {
                continue;
            }

            Repository::with_items(|items| {
                let item = &items[item_idx as usize];

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
            });
        }

        // Add permanent bonuses
        Repository::with_characters(|characters| {
            armor += characters[cn].armor_bonus as i32;
            weapon += characters[cn].weapon_bonus as i32;
            gethit += characters[cn].gethit_bonus as i32;
            light += characters[cn].light_bonus as i32;
        });

        // Calculate bonuses from active spells
        if !char_has_nomagic {
            for n in 0..20 {
                let spell_idx = Repository::with_characters(|ch| ch[cn].spell[n]);
                if spell_idx == 0 {
                    continue;
                }

                Repository::with_items(|items| {
                    let spell = &items[spell_idx as usize];

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
                    if spell.temp == core::constants::SK_STUN as u16 || spell.temp == 59 {
                        // SK_WARCRY2 = 59
                        Repository::with_characters_mut(|characters| {
                            characters[cn].stunned = 1;
                        });
                    }

                    if spell.hp[0] < 0 {
                        Repository::with_characters_mut(|characters| {
                            characters[cn].flags |= CharacterFlags::CF_NOHPREG.bits();
                        });
                    }
                    if spell.end[0] < 0 {
                        Repository::with_characters_mut(|characters| {
                            characters[cn].flags |= CharacterFlags::CF_NOENDREG.bits();
                        });
                    }
                    if spell.mana[0] < 0 {
                        Repository::with_characters_mut(|characters| {
                            characters[cn].flags |= CharacterFlags::CF_NOMANAREG.bits();
                        });
                    }

                    if spell.sprite_override != 0 {
                        Repository::with_characters_mut(|characters| {
                            characters[cn].sprite_override = spell.sprite_override as i16;
                        });
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
                });
            }
        }

        // Calculate final attributes
        Repository::with_characters_mut(|characters| {
            for z in 0..5 {
                let mut final_attrib = characters[cn].attrib[z][0] as i32
                    + characters[cn].attrib[z][1] as i32
                    + attrib_bonus[z];
                if final_attrib < 1 {
                    final_attrib = 1;
                }
                if final_attrib > 250 {
                    final_attrib = 250;
                }
                characters[cn].attrib[z][5] = final_attrib as u8;
            }

            // Calculate final HP
            let mut final_hp = characters[cn].hp[0] as i32 + characters[cn].hp[1] as i32 + hp_bonus;
            if final_hp < 10 {
                final_hp = 10;
            }
            if final_hp > 999 {
                final_hp = 999;
            }
            characters[cn].hp[5] = final_hp as u16;

            // Calculate final endurance
            let mut final_end =
                characters[cn].end[0] as i32 + characters[cn].end[1] as i32 + end_bonus;
            if final_end < 10 {
                final_end = 10;
            }
            if final_end > 999 {
                final_end = 999;
            }
            characters[cn].end[5] = final_end as u16;

            // Calculate final mana
            let mut final_mana =
                characters[cn].mana[0] as i32 + characters[cn].mana[1] as i32 + mana_bonus;
            if final_mana < 10 {
                final_mana = 10;
            }
            if final_mana > 999 {
                final_mana = 999;
            }
            characters[cn].mana[5] = final_mana as u16;
        });

        // Handle infrared vision
        let is_player =
            Repository::with_characters(|ch| ch[cn].flags & CharacterFlags::CF_PLAYER.bits() != 0);

        if is_player {
            let has_infrared = Repository::with_characters(|ch| {
                ch[cn].flags & CharacterFlags::CF_INFRARED.bits() != 0
            });

            if infra == 15 && !has_infrared {
                Repository::with_characters_mut(|characters| {
                    characters[cn].flags |= CharacterFlags::CF_INFRARED.bits();
                });
                self.do_character_log(cn, FontColor::Green, "You can see in the dark!\n");
            } else if infra != 15 && has_infrared {
                let is_god = Repository::with_characters(|ch| {
                    ch[cn].flags & CharacterFlags::CF_GOD.bits() != 0
                });

                if !is_god {
                    Repository::with_characters_mut(|characters| {
                        characters[cn].flags &= !CharacterFlags::CF_INFRARED.bits();
                    });
                    self.do_character_log(
                        cn,
                        FontColor::Red,
                        "You can no longer see in the dark!\n",
                    );
                }
            }
        }

        // Calculate final skills (with attribute bonuses)
        // TODO: Need access to static_skilltab to properly calculate skill bonuses
        Repository::with_characters_mut(|characters| {
            for z in 0..50 {
                let mut final_skill = characters[cn].skill[z][0] as i32
                    + characters[cn].skill[z][1] as i32
                    + skill_bonus[z];

                // Add attribute bonuses (simplified - real implementation needs skilltab)
                // For now, just add a generic attribute bonus
                let attrib_contribution = (characters[cn].attrib[core::constants::AT_AGIL as usize]
                    [5] as i32
                    + characters[cn].attrib[core::constants::AT_STREN as usize][5] as i32
                    + characters[cn].attrib[core::constants::AT_INT as usize][5] as i32)
                    / 5;
                final_skill += attrib_contribution;

                if final_skill < 1 {
                    final_skill = 1;
                }
                if final_skill > 250 {
                    final_skill = 250;
                }
                characters[cn].skill[z][5] = final_skill as u8;
            }

            // Set final armor
            if armor < 0 {
                armor = 0;
            }
            if armor > 250 {
                armor = 250;
            }
            characters[cn].armor = armor as i16;

            // Set final weapon
            if weapon < 0 {
                weapon = 0;
            }
            if weapon > 250 {
                weapon = 250;
            }
            characters[cn].weapon = weapon as i16;

            // Set final gethit damage
            if gethit < 0 {
                gethit = 0;
            }
            if gethit > 250 {
                gethit = 250;
            }
            characters[cn].gethit_dam = gethit as i8;

            // Set final light
            light -= sublight;
            if light < 0 {
                light = 0;
            }
            if light > 250 {
                light = 250;
            }
            characters[cn].light = light as u8;

            // Calculate speed based on mode
            let mut speed_calc = 10i32;
            let mode = characters[cn].mode;
            let agil = characters[cn].attrib[core::constants::AT_AGIL as usize][5] as i32;
            let stren = characters[cn].attrib[core::constants::AT_STREN as usize][5] as i32;
            let speed_mod = characters[cn].speed_mod as i32;

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

            characters[cn].speed = 20 - speed_calc as i16;
            if characters[cn].speed < 0 {
                characters[cn].speed = 0;
            }
            if characters[cn].speed > 19 {
                characters[cn].speed = 19;
            }

            // Cap current stats at their maximums
            if characters[cn].a_hp > characters[cn].hp[5] as i32 * 1000 {
                characters[cn].a_hp = characters[cn].hp[5] as i32 * 1000;
            }
            if characters[cn].a_end > characters[cn].end[5] as i32 * 1000 {
                characters[cn].a_end = characters[cn].end[5] as i32 * 1000;
            }
            if characters[cn].a_mana > characters[cn].mana[5] as i32 * 1000 {
                characters[cn].a_mana = characters[cn].mana[5] as i32 * 1000;
            }
        });

        // Update light if it changed
        let new_light = Repository::with_characters(|ch| ch[cn].light);
        if old_light != new_light {
            let (used, x, y) = Repository::with_characters(|ch| (ch[cn].used, ch[cn].x, ch[cn].y));

            if used == core::constants::USE_ACTIVE
                && x > 0
                && x < core::constants::SERVER_MAPX as i16
                && y > 0
                && y < core::constants::SERVER_MAPY as i16
            {
                let map_char = Repository::with_map(|map| {
                    let idx = (x as i32 + y as i32 * core::constants::SERVER_MAPX) as usize;
                    map[idx].ch
                });

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
    pub(crate) fn do_regenerate(&self, cn: usize) {
        // Check if character is stoned - no regeneration if stoned
        let is_stoned =
            Repository::with_characters(|ch| ch[cn].flags & CharacterFlags::CF_STONED.bits() != 0);

        if is_stoned {
            return;
        }

        // Determine moon multiplier for regen rates
        let mut moonmult = 20;

        let (is_player, globs_flags, newmoon, fullmoon) = Repository::with_globals(|globs| {
            let char_is_player = Repository::with_characters(|ch| {
                ch[cn].flags & CharacterFlags::CF_PLAYER.bits() != 0
            });
            (
                char_is_player,
                globs.flags,
                globs.newmoon != 0,
                globs.fullmoon != 0,
            )
        });

        if ((globs_flags & core::constants::GF_MAYHEM != 0) || newmoon) && is_player {
            moonmult = 10; // Slower regen during mayhem or new moon
        }
        if fullmoon && is_player {
            moonmult = 40; // Faster regen during full moon
        }

        // Check for regeneration prevention flags
        let (nohp, noend, nomana) = Repository::with_characters(|ch| {
            let nohp = ch[cn].flags & CharacterFlags::CF_NOHPREG.bits() != 0;
            let noend = ch[cn].flags & CharacterFlags::CF_NOENDREG.bits() != 0;
            let nomana = ch[cn].flags & CharacterFlags::CF_NOMANAREG.bits() != 0;
            (nohp, noend, nomana)
        });

        // Check if standing in underwater tile
        let uwater = Repository::with_characters(|ch| {
            let x = ch[cn].x as usize;
            let y = ch[cn].y as usize;
            let map_idx = x + y * core::constants::SERVER_MAPX as usize;

            Repository::with_map(|map| map[map_idx].flags & core::constants::MF_UWATER as u64 != 0)
        });

        let mut uwater_active = uwater;
        let mut hp_regen = false;
        let mut mana_regen = false;
        let mut gothp = 0i32;

        // Process regeneration based on character status (if not stunned)
        let stunned = Repository::with_characters(|ch| ch[cn].stunned != 0);

        if !stunned {
            let status = Repository::with_characters(|ch| ch[cn].status);
            let base_status = Self::ch_base_status(status as u8);

            match base_status {
                // Standing/idle states - regenerate normally
                0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 => {
                    if !noend {
                        Repository::with_characters_mut(|ch| {
                            ch[cn].a_end += moonmult * 4;

                            // Add bonus from Rest skill
                            if ch[cn].skill[core::constants::SK_REST][0] != 0 {
                                ch[cn].a_end += ch[cn].skill[core::constants::SK_REST][5] as i32
                                    * moonmult
                                    / 30;
                            }
                        });
                    }

                    if !nohp {
                        hp_regen = true;
                        Repository::with_characters_mut(|ch| {
                            ch[cn].a_hp += moonmult * 2;
                            gothp += moonmult * 2;

                            // Add bonus from Regen skill
                            if ch[cn].skill[core::constants::SK_REGEN][0] != 0 {
                                let regen_bonus = ch[cn].skill[core::constants::SK_REGEN][5] as i32
                                    * moonmult
                                    / 30;
                                ch[cn].a_hp += regen_bonus;
                                gothp += regen_bonus;
                            }
                        });
                    }

                    if !nomana {
                        let has_medit = Repository::with_characters(|ch| {
                            ch[cn].skill[core::constants::SK_MEDIT][0] != 0
                        });

                        if has_medit {
                            mana_regen = true;
                            Repository::with_characters_mut(|ch| {
                                ch[cn].a_mana += moonmult;
                                ch[cn].a_mana += ch[cn].skill[core::constants::SK_MEDIT][5] as i32
                                    * moonmult
                                    / 30;
                            });
                        }
                    }
                }

                // Walking/turning states - endurance based on mode
                16 | 24 | 32 | 40 | 48 | 60 | 72 | 84 | 96 | 100 | 104 | 108 | 112 | 116 | 120
                | 124 | 128 | 132 | 136 | 140 | 144 | 148 | 152 => {
                    let mode = Repository::with_characters(|ch| ch[cn].mode);

                    if mode == 2 {
                        // Fast mode drains endurance
                        Repository::with_characters_mut(|ch| {
                            ch[cn].a_end -= 25;
                        });
                    } else if mode == 0 {
                        // Sneak mode regenerates endurance
                        if !noend {
                            Repository::with_characters_mut(|ch| {
                                ch[cn].a_end += 25;
                            });
                        }
                    }
                }

                // Attack states - endurance drain based on status2 and mode
                160 | 168 | 176 | 184 => {
                    let (status2, mode) =
                        Repository::with_characters(|ch| (ch[cn].status2, ch[cn].mode));

                    if status2 == 0 || status2 == 5 || status2 == 6 {
                        // Attack action
                        if mode == 1 {
                            Repository::with_characters_mut(|ch| {
                                ch[cn].a_end -= 12;
                            });
                        } else if mode == 2 {
                            Repository::with_characters_mut(|ch| {
                                ch[cn].a_end -= 50;
                            });
                        }
                    } else {
                        // Misc action
                        if mode == 2 {
                            Repository::with_characters_mut(|ch| {
                                ch[cn].a_end -= 25;
                            });
                        } else if mode == 0 {
                            if !noend {
                                Repository::with_characters_mut(|ch| {
                                    ch[cn].a_end += 25;
                                });
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
        let is_undead =
            Repository::with_characters(|ch| ch[cn].flags & CharacterFlags::CF_UNDEAD.bits() != 0);

        if is_undead {
            hp_regen = true;
            Repository::with_characters_mut(|ch| {
                ch[cn].a_hp += 650;
            });
            gothp += 650;
        }

        // Amulet of Ankh (item 768) provides additional regeneration
        let worn_neck = Repository::with_characters(|ch| ch[cn].worn[core::constants::WN_NECK]);
        if worn_neck != 0 {
            let is_ankh = Repository::with_items(|items| items[worn_neck as usize].temp == 768);

            if is_ankh {
                let (has_regen, has_rest, has_medit) = Repository::with_characters(|ch| {
                    (
                        ch[cn].skill[core::constants::SK_REGEN][0] != 0,
                        ch[cn].skill[core::constants::SK_REST][0] != 0,
                        ch[cn].skill[core::constants::SK_MEDIT][0] != 0,
                    )
                });

                Repository::with_characters_mut(|ch| {
                    if has_regen {
                        ch[cn].a_hp +=
                            ch[cn].skill[core::constants::SK_REGEN][5] as i32 * moonmult / 60;
                    }
                    if has_rest {
                        ch[cn].a_end +=
                            ch[cn].skill[core::constants::SK_REST][5] as i32 * moonmult / 60;
                    }
                    if has_medit {
                        ch[cn].a_mana +=
                            ch[cn].skill[core::constants::SK_MEDIT][5] as i32 * moonmult / 60;
                    }
                });
            }
        }

        // Cap accumulated stats at their maximums (max * 1000)
        Repository::with_characters_mut(|ch| {
            if ch[cn].a_hp > ch[cn].hp[5] as i32 * 1000 {
                ch[cn].a_hp = ch[cn].hp[5] as i32 * 1000;
            }
            if ch[cn].a_end > ch[cn].end[5] as i32 * 1000 {
                ch[cn].a_end = ch[cn].end[5] as i32 * 1000;
            }
            if ch[cn].a_mana > ch[cn].mana[5] as i32 * 1000 {
                ch[cn].a_mana = ch[cn].mana[5] as i32 * 1000;
            }
        });

        // Set timer when regenerating below 90% of max
        if hp_regen {
            let needs_timer =
                Repository::with_characters(|ch| ch[cn].a_hp < ch[cn].hp[5] as i32 * 900);
            if needs_timer {
                Repository::with_characters_mut(|ch| {
                    ch[cn].data[92] = core::constants::TICKS * 60;
                });
            }
        }

        if mana_regen {
            let needs_timer =
                Repository::with_characters(|ch| ch[cn].a_mana < ch[cn].mana[5] as i32 * 900);
            if needs_timer {
                Repository::with_characters_mut(|ch| {
                    ch[cn].data[92] = core::constants::TICKS * 60;
                });
            }
        }

        // Force to sneak mode if exhausted
        let (is_exhausted, mode) =
            Repository::with_characters(|ch| (ch[cn].a_end < 1500, ch[cn].mode));

        if is_exhausted && mode != 0 {
            Repository::with_characters_mut(|ch| {
                ch[cn].mode = 0;
                ch[cn].set_do_update_flags();
            });

            self.do_character_log(cn, FontColor::Red, "You're exhausted.\n");
        }

        // Decrement escape timer
        Repository::with_characters_mut(|ch| {
            if ch[cn].escape_timer > 0 {
                ch[cn].escape_timer -= 1;
            }
        });

        // Process spell effects
        for spell_slot in 0..20 {
            let spell_item = Repository::with_characters(|ch| ch[cn].spell[spell_slot]);

            if spell_item == 0 {
                continue;
            }

            let is_permspell = Repository::with_items(|items| {
                items[spell_item as usize].flags & ItemFlags::IF_PERMSPELL.bits() != 0
            });

            if is_permspell {
                // Permanent spell - apply ongoing HP/end/mana drain/gain
                let (hp_change, end_change, mana_change) = Repository::with_items(|items| {
                    (
                        items[spell_item as usize].hp[0],
                        items[spell_item as usize].end[0],
                        items[spell_item as usize].mana[0],
                    )
                });

                let mut killed = false;
                let mut end_depleted = false;
                let mut mana_depleted = false;

                Repository::with_characters_mut(|ch| {
                    if hp_change != -1 {
                        ch[cn].a_hp += hp_change as i32;
                        if ch[cn].a_hp < 500 {
                            killed = true;
                        }
                    }
                    if end_change != -1 {
                        ch[cn].a_end += end_change as i32;
                        if ch[cn].a_end < 500 {
                            ch[cn].a_end = 500;
                            end_depleted = true;
                        }
                    }
                    if mana_change != -1 {
                        ch[cn].a_mana += mana_change as i32;
                        if ch[cn].a_mana < 500 {
                            ch[cn].a_mana = 500;
                            mana_depleted = true;
                        }
                    }
                });

                if killed {
                    let spell_name =
                        Repository::with_items(|items| items[spell_item as usize].name.clone());
                    log::info!(
                        "Character {} killed by spell: {}",
                        cn,
                        String::from_utf8_lossy(&spell_name)
                    );
                    self.do_character_log(
                        cn,
                        FontColor::Red,
                        &format!("The {} killed you!\n", String::from_utf8_lossy(&spell_name)),
                    );
                    self.do_area_log(
                        cn,
                        0,
                        0,
                        0,
                        FontColor::Red,
                        &format!(
                            "The {} killed {}.\n",
                            String::from_utf8_lossy(&spell_name),
                            cn
                        ),
                    );
                    self.do_character_killed(0, cn);
                    return;
                }

                if end_depleted {
                    let spell_name =
                        Repository::with_items(|items| items[spell_item as usize].name.clone());
                    Repository::with_items_mut(|items| {
                        items[spell_item as usize].active = 0;
                    });
                    log::info!(
                        "{} ran out due to lack of endurance for cn={}",
                        String::from_utf8_lossy(&spell_name),
                        cn
                    );
                }

                if mana_depleted {
                    let spell_name =
                        Repository::with_items(|items| items[spell_item as usize].name.clone());
                    Repository::with_items_mut(|items| {
                        items[spell_item as usize].active = 0;
                    });
                    log::info!(
                        "{} ran out due to lack of mana for cn={}",
                        String::from_utf8_lossy(&spell_name),
                        cn
                    );
                }
            } else {
                // Temporary spell - decrement timer
                Repository::with_items_mut(|items| {
                    if items[spell_item as usize].active > 0 {
                        items[spell_item as usize].active -= 1;
                    }
                });

                let active = Repository::with_items(|items| items[spell_item as usize].active);

                // Warn when spell is about to run out
                if active == core::constants::TICKS as u32 * 30 {
                    let spell_name =
                        Repository::with_items(|items| items[spell_item as usize].name.clone());
                    let (is_player_or_usurp, temp, companion_owner) =
                        Repository::with_characters(|ch| {
                            (
                                ch[cn].flags
                                    & (CharacterFlags::CF_PLAYER.bits()
                                        | CharacterFlags::CF_USURP.bits())
                                    != 0,
                                ch[cn].temp,
                                ch[cn].data[63],
                            )
                        });

                    if is_player_or_usurp {
                        self.do_character_log(
                            cn,
                            FontColor::Red,
                            &format!(
                                "{} is about to run out.\n",
                                String::from_utf8_lossy(&spell_name)
                            ),
                        );
                    } else if temp == core::constants::CT_COMPANION as u16 && companion_owner != 0 {
                        let co = companion_owner as usize;
                        if co > 0 && co < MAXCHARS {
                            let is_sane_player = Repository::with_characters(|ch| {
                                ch[co].used == core::constants::USE_ACTIVE
                                    && ch[co].flags & CharacterFlags::CF_PLAYER.bits() != 0
                            });

                            if is_sane_player {
                                let item_temp =
                                    Repository::with_items(|items| items[spell_item as usize].temp);

                                // Only inform owner about certain spell types
                                if item_temp == core::constants::SK_BLESS as u16
                                    || item_temp == core::constants::SK_PROTECT as u16
                                    || item_temp == core::constants::SK_ENHANCE as u16
                                {
                                    let co_name =
                                        Repository::with_characters(|ch| ch[co].name.clone());

                                    self.do_sayx(
                                        cn,
                                        format!(
                                            "My spell {} is running out, {}.",
                                            String::from_utf8_lossy(&spell_name),
                                            String::from_utf8_lossy(&co_name),
                                        )
                                        .as_str(),
                                    );
                                }
                            }
                        }
                    }
                }

                // Check item temp for special handling
                let item_temp = Repository::with_items(|items| items[spell_item as usize].temp);

                // Water breathing spell cancels underwater damage
                if item_temp == 649 {
                    uwater_active = false;
                }

                // Magic Shield spell - update armor based on remaining duration
                if item_temp == core::constants::SK_MSHIELD as u16 {
                    let old_armor =
                        Repository::with_items(|items| items[spell_item as usize].armor[1]);
                    let new_armor = active / 1024 + 1;
                    let new_power = active / 256;

                    Repository::with_items_mut(|items| {
                        items[spell_item as usize].armor[1] = new_armor as i8;
                        items[spell_item as usize].power = new_power as u32;
                    });

                    if old_armor != new_armor as i8 {
                        Repository::with_characters_mut(|ch| {
                            ch[cn].set_do_update_flags();
                        });
                    }
                }

                // Handle spell expiration
                if active == 0 {
                    let spell_name =
                        Repository::with_items(|items| items[spell_item as usize].name.clone());

                    // Recall spell - teleport character
                    if item_temp == core::constants::SK_RECALL as u16 {
                        let char_used = Repository::with_characters(|ch| ch[cn].used);

                        if char_used == core::constants::USE_ACTIVE {
                            let (old_x, old_y, dest_x, dest_y, is_invisible) =
                                Repository::with_characters(|ch| {
                                    let dest = Repository::with_items(|items| {
                                        (
                                            items[spell_item as usize].data[0],
                                            items[spell_item as usize].data[1],
                                        )
                                    });
                                    (
                                        ch[cn].x,
                                        ch[cn].y,
                                        dest.0,
                                        dest.1,
                                        ch[cn].flags & CharacterFlags::CF_INVISIBLE.bits() != 0,
                                    )
                                });

                            if God::transfer_char(cn, dest_x as usize, dest_y as usize) {
                                if !is_invisible {
                                    EffectManager::fx_add_effect(
                                        12,
                                        0,
                                        old_x as i32,
                                        old_y as i32,
                                        0,
                                    );

                                    EffectManager::fx_add_effect(
                                        12,
                                        0,
                                        dest_x as i32,
                                        dest_y as i32,
                                        0,
                                    );
                                }
                            }

                            // Reset character state
                            Repository::with_characters_mut(|ch| {
                                ch[cn].status = 0;
                                ch[cn].attack_cn = 0;
                                ch[cn].skill_nr = 0;
                                ch[cn].goto_x = 0;
                                ch[cn].use_nr = 0;
                                ch[cn].misc_action = 0;
                                ch[cn].dir = core::constants::DX_DOWN;
                            });
                        }
                    } else {
                        self.do_character_log(
                            cn,
                            FontColor::Red,
                            &format!("{} ran out.\n", String::from_utf8_lossy(&spell_name)),
                        );
                    }

                    // Remove spell
                    Repository::with_items_mut(|items| {
                        items[spell_item as usize].used = core::constants::USE_EMPTY;
                    });
                    Repository::with_characters_mut(|ch| {
                        ch[cn].spell[spell_slot] = 0;
                        ch[cn].set_do_update_flags();
                    });
                }
            }
        }

        // Handle underwater damage for players
        if uwater_active {
            let (is_player, is_immortal) = Repository::with_characters(|ch| {
                (
                    ch[cn].flags & CharacterFlags::CF_PLAYER.bits() != 0,
                    ch[cn].flags & CharacterFlags::CF_IMMORTAL.bits() != 0,
                )
            });

            if is_player && !is_immortal {
                Repository::with_characters_mut(|ch| {
                    ch[cn].a_hp -= 250 + gothp;
                });

                let is_dead = Repository::with_characters(|ch| ch[cn].a_hp < 500);
                if is_dead {
                    self.do_character_killed(0, cn);
                }
            }
        }

        // Handle item tear and wear for active players
        let (used, is_player) = Repository::with_characters(|ch| {
            (
                ch[cn].used,
                ch[cn].flags & CharacterFlags::CF_PLAYER.bits() != 0,
            )
        });

        if used == core::constants::USE_ACTIVE && is_player {
            driver::char_item_expire(cn);
        }
    }

    /// Helper function to determine base status from full status value
    /// Port of ch_base_status from svr_tick.cpp
    pub(crate) fn ch_base_status(n: u8) -> u8 {
        if n < 4 {
            return n;
        }
        if n < 16 {
            return n;
        }
        if n < 24 {
            return 16;
        } // walk up
        if n < 32 {
            return 24;
        } // walk down
        if n < 40 {
            return 32;
        } // walk left
        if n < 48 {
            return 40;
        } // walk right
        if n < 60 {
            return 48;
        } // walk left+up
        if n < 72 {
            return 60;
        } // walk left+down
        if n < 84 {
            return 72;
        } // walk right+up
        if n < 96 {
            return 84;
        } // walk right+down
        if n < 100 {
            return 96;
        }
        if n < 104 {
            return 100;
        } // turn up to left
        if n < 108 {
            return 104;
        } // turn up to right
        if n < 112 {
            return 108;
        }
        if n < 116 {
            return 112;
        }
        if n < 120 {
            return 116;
        } // turn down to left
        if n < 124 {
            return 120;
        }
        if n < 128 {
            return 124;
        } // turn down to right
        if n < 132 {
            return 128;
        }
        if n < 136 {
            return 132;
        } // turn left to up
        if n < 140 {
            return 136;
        }
        if n < 144 {
            return 140;
        } // turn left to down
        if n < 148 {
            return 144;
        }
        if n < 152 {
            return 148;
        } // turn right to up
        if n < 156 {
            return 152;
        }
        if n < 160 {
            return 160;
        } // turn right to down
        if n < 164 {
            return 160;
        }
        if n < 168 {
            return 160;
        } // attack up
        if n < 176 {
            return 168;
        } // attack down
        if n < 184 {
            return 176;
        } // attack left
        if n < 192 {
            return 184;
        } // attack right

        n // default
    }

    /// Set the update/save flags for a character (port of `do_update_char`)
    pub(crate) fn do_update_char(&self, cn: usize) {
        Repository::with_characters_mut(|characters| {
            characters[cn].set_do_update_flags();
        });
    }

    /// Remove all active spell items from a character (port of `remove_spells`)
    pub(crate) fn remove_spells(&self, cn: usize) {
        for n in 0..20 {
            let spell_item = Repository::with_characters(|ch| ch[cn].spell[n]);
            if spell_item == 0 {
                continue;
            }
            let in_idx = spell_item as usize;
            Repository::with_items_mut(|items| {
                if in_idx < items.len() {
                    items[in_idx].used = core::constants::USE_EMPTY;
                }
            });
            Repository::with_characters_mut(|ch| {
                ch[cn].spell[n] = 0;
            });
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
    pub(crate) fn do_raise_attrib(&self, cn: usize, attrib: i32) -> bool {
        let attrib_idx = attrib as usize;
        if attrib_idx >= 5 {
            return false;
        }

        let (current_val, max_val, diff, available_points) = Repository::with_characters(|ch| {
            (
                ch[cn].attrib[attrib_idx][0],
                ch[cn].attrib[attrib_idx][2],
                ch[cn].attrib[attrib_idx][3],
                ch[cn].points,
            )
        });

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
        Repository::with_characters_mut(|ch| {
            ch[cn].points -= points_needed;
            ch[cn].attrib[attrib_idx][0] += 1;
        });

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
    pub(crate) fn do_raise_hp(&self, cn: usize) -> bool {
        let (current_val, max_val, diff, available_points) = Repository::with_characters(|ch| {
            (ch[cn].hp[0], ch[cn].hp[2], ch[cn].hp[3], ch[cn].points)
        });

        if current_val == 0 || current_val >= max_val {
            return false;
        }

        let points_needed = helpers::hp_needed(current_val as i32, diff as i32);

        if points_needed > available_points {
            return false;
        }

        Repository::with_characters_mut(|ch| {
            ch[cn].points -= points_needed;
            ch[cn].hp[0] += 1;
        });

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
    pub(crate) fn do_raise_end(&self, cn: usize) -> bool {
        let (current_val, max_val, diff, available_points) = Repository::with_characters(|ch| {
            (ch[cn].end[0], ch[cn].end[2], ch[cn].end[3], ch[cn].points)
        });

        if current_val == 0 || current_val >= max_val {
            return false;
        }

        let points_needed = helpers::end_needed(current_val as i32, diff as i32);

        if points_needed > available_points {
            return false;
        }

        Repository::with_characters_mut(|ch| {
            ch[cn].points -= points_needed;
            ch[cn].end[0] += 1;

            self.do_update_char(cn);
        });

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
    pub(crate) fn do_raise_mana(&self, cn: usize) -> bool {
        let (current_val, max_val, diff, available_points) = Repository::with_characters(|ch| {
            (
                ch[cn].mana[0],
                ch[cn].mana[2],
                ch[cn].mana[3],
                ch[cn].points,
            )
        });

        if current_val == 0 || current_val >= max_val {
            return false;
        }

        let points_needed = helpers::mana_needed(current_val as i32, diff as i32);

        if points_needed > available_points {
            return false;
        }

        Repository::with_characters_mut(|ch| {
            ch[cn].points -= points_needed;
            ch[cn].mana[0] += 1;

            self.do_update_char(cn);
        });

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
    pub(crate) fn do_raise_skill(&self, cn: usize, skill: i32) -> bool {
        let skill_idx = skill as usize;
        if skill_idx >= 50 {
            return false;
        }

        let (current_val, max_val, diff, available_points) = Repository::with_characters(|ch| {
            (
                ch[cn].skill[skill_idx][0],
                ch[cn].skill[skill_idx][2],
                ch[cn].skill[skill_idx][3],
                ch[cn].points,
            )
        });

        if current_val == 0 || current_val >= max_val {
            return false;
        }

        let points_needed = helpers::skill_needed(current_val as i32, diff as i32);

        if points_needed > available_points {
            return false;
        }

        Repository::with_characters_mut(|ch| {
            ch[cn].points -= points_needed;
            ch[cn].skill[skill_idx][0] += 1;
            ch[cn].set_do_update_flags();
        });

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
    pub(crate) fn do_lower_hp(&self, cn: usize) -> bool {
        let current_val = Repository::with_characters(|ch| ch[cn].hp[0]);

        if current_val < 11 {
            return false;
        }

        Repository::with_characters_mut(|ch| {
            ch[cn].hp[0] -= 1;
        });

        let new_val = Repository::with_characters(|ch| ch[cn].hp[0]);
        let diff = Repository::with_characters(|ch| ch[cn].hp[3]);

        let points_lost = helpers::hp_needed(new_val as i32, diff as i32);

        Repository::with_characters_mut(|ch| {
            ch[cn].points_tot -= points_lost;

            self.do_update_char(cn);
        });

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
    pub(crate) fn do_lower_mana(&self, cn: usize) -> bool {
        let current_val = Repository::with_characters(|ch| ch[cn].mana[0]);

        if current_val < 11 {
            return false;
        }

        Repository::with_characters_mut(|ch| {
            ch[cn].mana[0] -= 1;
        });

        let new_val = Repository::with_characters(|ch| ch[cn].mana[0]);
        let diff = Repository::with_characters(|ch| ch[cn].mana[3]);

        let points_lost = helpers::mana_needed(new_val as i32, diff as i32);

        Repository::with_characters_mut(|ch| {
            ch[cn].points_tot -= points_lost;
            self.do_update_char(cn);
        });

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
    pub(crate) fn do_check_new_level(&self, cn: usize) {
        Repository::with_characters_mut(|characters| {
            // Only for players
            if (characters[cn].flags & CharacterFlags::CF_PLAYER.bits()) == 0 {
                return;
            }

            let rank = crate::helpers::points2rank(characters[cn].points_tot as u32) as usize;

            // Check if current rank is less than new rank
            if (characters[cn].data[45] as usize) < rank {
                let (hp, end, mana) = if (characters[cn].kindred
                    & ((core::constants::KIN_TEMPLAR | core::constants::KIN_ARCHTEMPLAR) as i32))
                    != 0
                {
                    (15, 10, 5)
                } else if (characters[cn].kindred
                    & ((core::constants::KIN_MERCENARY
                        | core::constants::KIN_SORCERER
                        | core::constants::KIN_WARRIOR
                        | core::constants::KIN_SEYAN_DU) as i32))
                    != 0
                {
                    (10, 10, 10)
                } else if (characters[cn].kindred
                    & ((core::constants::KIN_HARAKIM | core::constants::KIN_ARCHHARAKIM) as i32))
                    != 0
                {
                    (5, 10, 15)
                } else {
                    return; // Unknown kindred, don't proceed
                };

                let diff = rank - characters[cn].data[45] as usize;
                characters[cn].data[45] = rank as i32;

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
                let temp = if (characters[cn].kindred & (core::constants::KIN_PURPLE as i32)) != 0 {
                    core::constants::CT_PRIEST
                } else {
                    core::constants::CT_LGUARD
                };

                // Find a character with appropriate template
                let mut herald_cn = 0;
                for n in 1..core::constants::MAXCHARS {
                    if characters[n].used != core::constants::USE_ACTIVE {
                        continue;
                    }
                    if (characters[n].flags & CharacterFlags::CF_BODY.bits()) != 0 {
                        continue;
                    }
                    if characters[n].temp == temp as u16 {
                        herald_cn = n;
                        break;
                    }
                }

                // Have the herald yell it out
                if herald_cn != 0 {
                    let char_name = String::from_utf8_lossy(&characters[cn].name)
                        .trim_matches('\0')
                        .to_string();
                    let rank_name = if rank < crate::helpers::RANK_NAMES.len() {
                        crate::helpers::RANK_NAMES[rank]
                    } else {
                        "Unknown Rank"
                    };
                    let message = format!(
                        "Hear ye, hear ye! {} has attained the rank of {}!",
                        char_name, rank_name
                    );

                    State::with(|state| state.do_shout(herald_cn, &message))
                }

                // Award stat increases
                characters[cn].hp[1] = (hp * rank) as u16;
                characters[cn].end[1] = (end * rank) as u16;
                characters[cn].mana[1] = (mana * rank) as u16;

                State::with(|state| {
                    state.do_update_char(cn);
                });
            }
        });
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
        let is_body =
            Repository::with_characters(|ch| (ch[co].flags & CharacterFlags::CF_BODY.bits()) != 0);
        if is_body {
            return 0;
        }

        // If a real player got hit, damage armour pieces
        let co_is_player = Repository::with_characters(|ch| {
            (ch[co].flags & CharacterFlags::CF_PLAYER.bits()) != 0
        });
        if co_is_player {
            driver::item_damage_armor(co, dam);
        }

        // Determine noexp conditions
        let mut noexp = 0;
        Repository::with_characters(|ch| {
            if cn != 0
                && (ch[cn].flags & CharacterFlags::CF_PLAYER.bits()) == 0
                && ch[cn].data[63] == co as i32
            {
                noexp = 1;
            }
            if (ch[co].flags & CharacterFlags::CF_PLAYER.bits()) != 0 {
                noexp = 1;
            }
            if ch[co].temp == core::constants::CT_COMPANION as u16
                && (ch[co].flags & CharacterFlags::CF_THRALL.bits()) == 0
            {
                noexp = 1;
            }
        });

        // Handle magical shields (SK_MSHIELD)
        let co_armor = Repository::with_characters(|ch| ch[co].armor);
        let spells = Repository::with_characters(|ch| ch[co].spell);
        let mut shield_updates: Vec<(usize, usize, i32, bool)> = Vec::new(); // (slot, item_idx, new_active, kill)

        for n in 0..20 {
            let in_idx = spells[n] as usize;
            if in_idx == 0 {
                continue;
            }

            let (item_temp, item_active) = Repository::with_items(|items| {
                if in_idx < items.len() {
                    (items[in_idx].temp, items[in_idx].active)
                } else {
                    (0u16, 0u32)
                }
            });

            if item_temp == core::constants::SK_MSHIELD as u16 {
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
                    Repository::with_characters_mut(|characters| {
                        characters[co].spell[slot] = 0;
                    });
                    Repository::with_items_mut(|items| {
                        if in_idx < items.len() {
                            items[in_idx].used = core::constants::USE_EMPTY;
                        }
                    });
                    self.do_update_char(co);
                } else {
                    Repository::with_items_mut(|items| {
                        if in_idx < items.len() {
                            items[in_idx].active = new_active as u32;
                            items[in_idx].armor[1] = (items[in_idx].active / 1024 + 1) as i8;
                            items[in_idx].power = items[in_idx].active / 256;
                        }
                    });
                    self.do_update_char(co);
                }
            }
        }

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
        let is_immortal = Repository::with_characters(|ch| {
            (ch[co].flags & CharacterFlags::CF_IMMORTAL.bits()) != 0
        });
        if is_immortal {
            dam = 0;
        }

        // Notifications for visible hits
        if type_hurt != 3 {
            let (cn_x, cn_y) = Repository::with_characters(|ch| (ch[cn].x, ch[cn].y));
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
                tmp = helpers::scale_exps(cn as i32, co as i32, tmp);
                if tmp > 0 {
                    Repository::with_characters_mut(|characters| {
                        characters[cn].points += tmp;
                        characters[cn].points_tot += tmp;
                    });
                    self.do_check_new_level(cn);
                }
            }
        }

        // Set map injury flags and show FX
        if type_hurt != 1 {
            let (co_x, co_y) = Repository::with_characters(|ch| (ch[co].x, ch[co].y));
            Repository::with_map_mut(|map| {
                let idx =
                    (co_x as i32 + co_y as i32 * core::constants::SERVER_MAPX as i32) as usize;
                if dam < 10000 {
                    map[idx].flags |= core::constants::MF_GFX_INJURED as u64;
                } else if dam < 30000 {
                    map[idx].flags |=
                        (core::constants::MF_GFX_INJURED | core::constants::MF_GFX_INJURED1) as u64;
                } else if dam < 50000 {
                    map[idx].flags |=
                        (core::constants::MF_GFX_INJURED | core::constants::MF_GFX_INJURED2) as u64;
                } else {
                    map[idx].flags |= (core::constants::MF_GFX_INJURED
                        | core::constants::MF_GFX_INJURED1
                        | core::constants::MF_GFX_INJURED2)
                        as u64;
                }
            });
            crate::effect::EffectManager::fx_add_effect(
                core::constants::FX_INJURED as i32,
                8,
                co_x as i32,
                co_y as i32,
                0,
            );
        }

        // God save check
        let saved_by_god = Repository::with_characters(|ch| {
            let will_die_hp = ch[co].a_hp - dam;
            (will_die_hp < 500) && (ch[co].luck >= 100)
        });

        if saved_by_god {
            let mf_arena = Repository::with_map(|map| {
                let idx = (Repository::with_characters(|ch| ch[co].x as i32)
                    + Repository::with_characters(|ch| ch[co].y as i32)
                        * core::constants::SERVER_MAPX as i32) as usize;
                map[idx].flags & core::constants::MF_ARENA as u64
            });

            let mut rng = rand::thread_rng();
            if mf_arena == 0
                && rng.gen_range(0..10000) < 5000 + Repository::with_characters(|ch| ch[co].luck)
            {
                // Save the character
                Repository::with_characters_mut(|characters| {
                    characters[co].a_hp = characters[co].hp[5] as i32 * 500;
                    characters[co].luck /= 2;
                    characters[co].data[44] += 1; // saved counter
                });

                self.do_character_log(co, core::types::FontColor::Yellow, "A god reached down and saved you from the killing blow. You must have done the gods a favor sometime in the past!\n");
                let (co_x, co_y) = Repository::with_characters(|ch| (ch[co].x, ch[co].y));
                self.do_area_log(
                    co,
                    0,
                    co_x as i32,
                    co_y as i32,
                    core::types::FontColor::Yellow,
                    &format!(
                        "A god reached down and saved {} from the killing blow.\n",
                        Repository::with_characters(|ch| ch[co].get_name().to_string())
                    ),
                );
                crate::effect::EffectManager::fx_add_effect(6, 0, co_x as i32, co_y as i32, 0);
                God::transfer_char(
                    co,
                    Repository::with_characters(|ch| ch[co].temple_x as usize),
                    Repository::with_characters(|ch| ch[co].temple_y as usize),
                );
                let (new_x, new_y) = Repository::with_characters(|ch| (ch[co].x, ch[co].y));
                crate::effect::EffectManager::fx_add_effect(6, 0, new_x as i32, new_y as i32, 0);

                Repository::with_characters_mut(|characters| {
                    characters[cn].data[44] += 1;
                });

                self.do_notify_character(
                    cn as u32,
                    core::constants::NT_DIDKILL as i32,
                    co as i32,
                    0,
                    0,
                    0,
                );
                self.do_area_notify(
                    cn as i32,
                    co as i32,
                    Repository::with_characters(|ch| ch[cn].x as i32),
                    Repository::with_characters(|ch| ch[cn].y as i32),
                    core::constants::NT_SEEKILL as i32,
                    cn as i32,
                    co as i32,
                    0,
                    0,
                );
                return (dam / 1000) as i32;
            }
        }

        // Subtract hp
        Repository::with_characters_mut(|characters| {
            characters[co].a_hp -= dam;
        });

        // Warn about low HP
        let cur_hp = Repository::with_characters(|ch| ch[co].a_hp);
        if cur_hp < 8000 && cur_hp >= 500 {
            self.do_character_log(
                co,
                core::types::FontColor::Red,
                "You're almost dead... Give running a try!\n",
            );
        }

        // Handle death
        if cur_hp < 500 {
            self.do_area_log(
                cn,
                co,
                Repository::with_characters(|ch| ch[cn].x as i32),
                Repository::with_characters(|ch| ch[cn].y as i32),
                core::types::FontColor::Red,
                &format!(
                    "{} is dead!\n",
                    Repository::with_characters(|ch| ch[co].get_name().to_string())
                ),
            );
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!(
                    "You killed {}.\n",
                    Repository::with_characters(|ch| ch[co].get_name().to_string())
                ),
            );

            if Repository::with_characters(|ch| {
                (ch[cn].flags & CharacterFlags::CF_INVISIBLE.bits()) != 0
            }) {
                self.do_character_log(
                    co,
                    core::types::FontColor::Red,
                    "Oh dear, that blow was fatal. Somebody killed you...\n",
                );
            } else {
                self.do_character_log(
                    co,
                    core::types::FontColor::Red,
                    &format!(
                        "Oh dear, that blow was fatal. {} killed you...\n",
                        Repository::with_characters(|ch| ch[cn].get_name().to_string())
                    ),
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
            self.do_area_notify(
                cn as i32,
                co as i32,
                Repository::with_characters(|ch| ch[cn].x as i32),
                Repository::with_characters(|ch| ch[cn].y as i32),
                core::constants::NT_SEEKILL as i32,
                cn as i32,
                co as i32,
                0,
                0,
            );

            // Score and EXP handing (defer to helpers/stubs)
            if type_hurt != 2
                && cn != 0
                && Repository::with_map(|map| {
                    let idx = (Repository::with_characters(|ch| ch[co].x as i32)
                        + Repository::with_characters(|ch| ch[co].y as i32)
                            * core::constants::SERVER_MAPX as i32)
                        as usize;
                    map[idx].flags & core::constants::MF_ARENA as u64 == 0
                })
                && noexp == 0
            {
                let tmp = self.do_char_score(co);
                let rank =
                    helpers::points2rank(
                        Repository::with_characters(|ch| ch[co].points_tot as u32) as u32
                    ) as i32;
                // Some bonuses for spells are handled in do_give_exp/do_char_killed
                self.do_character_killed(co, cn);
                if type_hurt != 2 && cn != 0 && cn != co {
                    self.do_give_exp(cn, tmp, 1, rank);
                }
            } else {
                self.do_character_killed(co, cn);
            }

            Repository::with_characters_mut(|characters| {
                characters[cn].cerrno = core::constants::ERR_SUCCESS as u16;
            });
        } else {
            // Reactive damage (gethit)
            if type_hurt == 0 {
                let gethit = Repository::with_characters(|ch| ch[co].gethit_dam);
                if gethit > 0 {
                    let mut rng = rand::thread_rng();
                    let odam = rng.gen_range(0..(gethit as i32)) + 1;
                    // call do_hurt on attacker
                    self.do_hurt(co, cn, odam, 3);
                }
            }
        }

        dam / 1000
    }
}
