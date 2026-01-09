use core::{constants::CharacterFlags, string_operations::c_string_to_str};

use crate::{god::God, player, populate, repository::Repository, state::State};

pub struct EffectManager {}

impl EffectManager {
    /// Port of `can_drop(int m)` from `svr_effect.cpp`
    /// Checks if an item can be dropped at the given map index
    pub fn can_drop(map_index: usize) -> bool {
        Repository::with_map(|map| {
            if map[map_index].ch != 0
                || map[map_index].to_ch != 0
                || map[map_index].it != 0
                || (map[map_index].flags & core::constants::MF_MOVEBLOCK as u64) != 0
                || (map[map_index].flags & core::constants::MF_DEATHTRAP as u64) != 0
                || map[map_index].fsprite != 0
            {
                return false;
            }
            true
        })
    }

    /// Port of `is_beam(int in)` from `svr_effect.cpp`
    /// Checks if an item is an active beam (template 453)
    pub fn is_beam(item_id: usize) -> bool {
        if item_id == 0 {
            return false;
        }

        Repository::with_items(|items| {
            if items[item_id].temp != 453 {
                return false;
            }
            if items[item_id].active == 0 {
                return false;
            }
            true
        })
    }

    /// Port of `effect_tick(void)` from `svr_effect.cpp`
    /// Main effect processing function called every tick
    pub fn effect_tick() {
        let mut cnt = 0;

        for n in 1..core::constants::MAXEFFECT {
            let (used, effect_type) = Repository::with_effects(|effects| {
                (effects[n].used, effects[n].effect_type as i32)
            });

            if used == core::constants::USE_EMPTY {
                continue;
            }
            cnt += 1;

            if used != core::constants::USE_ACTIVE {
                continue;
            }

            match effect_type {
                1 => Self::handle_effect_type_1(n),
                2 => Self::handle_effect_type_2(n),
                3 => Self::handle_effect_type_3(n),
                4 => Self::handle_effect_type_4(n),
                5 => Self::handle_effect_type_5(n),
                6 => Self::handle_effect_type_6(n),
                7 => Self::handle_effect_type_7(n),
                8 => Self::handle_effect_type_8(n),
                9 => Self::handle_effect_type_9(n),
                10 => Self::handle_effect_type_10(n),
                11 => Self::handle_effect_type_11(n),
                12 => Self::handle_effect_type_12(n),
                _ => {}
            }
        }

        Repository::with_globals_mut(|globals| {
            globals.effect_cnt = cnt;
        });
    }

    /// Type 1: Remove injury flag from map
    /// Handle effect type 1: remove map injury graphics and expire
    ///
    /// Internal helper invoked by `effect_tick` when processing effects of
    /// type 1. Decrements duration and clears map injury flags when expired.
    fn handle_effect_type_1(n: usize) {
        Repository::with_effects_mut(|effects| {
            effects[n].duration -= 1;

            if effects[n].duration == 0 {
                effects[n].used = core::constants::USE_EMPTY;

                let map_index = effects[n].data[0] as usize
                    + effects[n].data[1] as usize * core::constants::SERVER_MAPX as usize;

                Repository::with_map_mut(|map| {
                    map[map_index].flags &= !(core::constants::MF_GFX_INJURED
                        | core::constants::MF_GFX_INJURED1
                        | core::constants::MF_GFX_INJURED2);
                });
            }
        });
    }

    /// Type 2: Timer for character respawn
    /// Handle effect type 2: respawn timer
    ///
    /// Internal helper for timed respawn effects. When the timer reaches
    /// zero, it attempts to reserve the map tile and transitions the effect
    /// to type 8 (respawn mist) if successful.
    fn handle_effect_type_2(n: usize) {
        Repository::with_effects_mut(|effects| {
            if effects[n].duration > 0 {
                effects[n].duration -= 1;
            }

            if effects[n].duration == 0 {
                let map_index = effects[n].data[0] as usize
                    + effects[n].data[1] as usize * core::constants::SERVER_MAPX as usize;

                // Check if target position is clear
                if player::plr_check_target(map_index) {
                    Repository::with_map_mut(|map| {
                        map[map_index].flags |= core::constants::MF_MOVEBLOCK as u64;
                    });
                    effects[n].effect_type = 8;
                }
            }
        });
    }

    /// Type 3: Death mist
    /// Handle effect type 3: death mist (grave placement)
    ///
    /// Drives the death-mist animation and, at the appropriate tick, will
    /// either create a grave/tomb or destroy items when space is unavailable.
    fn handle_effect_type_3(n: usize) {
        Repository::with_effects_mut(|effects| {
            effects[n].duration += 1;

            let map_index = effects[n].data[0] as usize
                + effects[n].data[1] as usize * core::constants::SERVER_MAPX as usize;

            if effects[n].duration == 19 {
                effects[n].used = core::constants::USE_EMPTY;

                Repository::with_map_mut(|map| {
                    map[map_index].flags &= !core::constants::MF_GFX_DEATH;
                });
            } else {
                Repository::with_map_mut(|map| {
                    map[map_index].flags &= !core::constants::MF_GFX_DEATH;
                    map[map_index].flags |= (effects[n].duration as u64) << 40;
                });

                if effects[n].duration == 9 {
                    let co = effects[n].data[2] as usize;
                    player::plr_map_remove(co);

                    let m = Self::find_drop_position(map_index);

                    if m == 0 {
                        let temp =
                            Repository::with_characters(|characters| characters[co].temp as usize);

                        log::info!("Character {} could not drop grave", co);

                        God::destroy_items(co);

                        Repository::with_characters_mut(|characters| {
                            characters[co].used = core::constants::USE_EMPTY;

                            if (characters[co].flags & CharacterFlags::Respawn.bits()) != 0 {
                                Repository::with_character_templates(|char_templates| {
                                    Self::fx_add_effect(
                                        2,
                                        core::constants::TICKS * 60 * 5
                                            + rand::random::<i32>().abs()
                                                % (core::constants::TICKS * 60 * 10),
                                        char_templates[temp].x as i32,
                                        char_templates[temp].y as i32,
                                        temp as i32,
                                    );
                                });
                            }
                        });
                    } else {
                        Self::handle_grave_creation(m, co, effects[n].data[3] as i32);
                    }
                }
            }
        });
    }

    /// Type 4: Tombstone
    /// Handle effect type 4: tombstone completion
    ///
    /// Finalizes a tombstone/effect sequence, creating the tombstone item
    /// and placing it on the map when its duration completes.
    fn handle_effect_type_4(n: usize) {
        Repository::with_effects_mut(|effects| {
            effects[n].duration += 1;

            let map_index = effects[n].data[0] as usize
                + effects[n].data[1] as usize * core::constants::SERVER_MAPX as usize;

            if effects[n].duration == 29 {
                effects[n].used = core::constants::USE_EMPTY;
                let co = effects[n].data[2] as usize;

                Repository::with_map_mut(|map| {
                    map[map_index].flags &= !core::constants::MF_GFX_TOMB;
                    // NOTE: `!X as u64` parses as `(!X) as u64` in Rust (negation before cast),
                    // which would truncate the mask to 32 bits and accidentally clear unrelated
                    // high flag bits. We want to widen to u64 first, then negate.
                    map[map_index].flags &= !(core::constants::MF_MOVEBLOCK as u64);
                });

                let in_id = God::create_item(170);
                if let Some(in_id) = in_id {
                    Repository::with_items_mut(|items| {
                        Repository::with_characters(|characters| {
                            items[in_id].data[0] = co as u32;

                            if characters[co].data[99] != 0 {
                                items[in_id].max_age[0] *= 4;
                            }

                            Repository::with_globals(|globals| {
                                let day_suffix = match globals.mdday {
                                    1 => "st",
                                    2 => "nd",
                                    3 => "rd",
                                    _ => "th",
                                };

                                let killer_name = if effects[n].data[3] != 0 {
                                    c_string_to_str(
                                        &characters[effects[n].data[3] as usize].reference,
                                    )
                                } else {
                                    "unknown causes"
                                };

                                let character_name = c_string_to_str(&characters[co].reference);

                                let description_string = format!(
                                    "Here rests {}, killed by {} on the {}{} day of the Year {}.",
                                    character_name,
                                    killer_name,
                                    globals.mdday,
                                    day_suffix,
                                    globals.mdyear
                                );

                                let mut desc_bytes = [0u8; 200];
                                let bytes_to_copy = description_string.as_bytes().len().min(199);
                                desc_bytes[..bytes_to_copy].copy_from_slice(
                                    &description_string.as_bytes()[..bytes_to_copy],
                                );
                                items[in_id].description = desc_bytes;
                            });

                            God::drop_item(
                                in_id,
                                effects[n].data[0] as usize,
                                effects[n].data[1] as usize,
                            );

                            Repository::with_items(|items| {
                                Repository::with_characters_mut(|characters| {
                                    characters[co].x = items[in_id].x as i16;
                                    characters[co].y = items[in_id].y as i16;
                                });
                            });

                            log::info!("Grave done for character {}", co);
                        });
                    });
                }
            } else {
                Repository::with_map_mut(|map| {
                    map[map_index].flags &= !core::constants::MF_GFX_TOMB;
                    map[map_index].flags |= (effects[n].duration as u64) << 35;
                });
            }
        });
    }

    /// Type 5: Evil magic
    /// Handle effect type 5: evil magic animation
    ///
    /// Updates evil-magic graphic flags and expires the effect after the
    /// configured duration.
    fn handle_effect_type_5(n: usize) {
        Repository::with_effects_mut(|effects| {
            effects[n].duration += 1;

            let map_index = effects[n].data[0] as usize
                + effects[n].data[1] as usize * core::constants::SERVER_MAPX as usize;

            if effects[n].duration == 8 {
                effects[n].used = core::constants::USE_EMPTY;
                Repository::with_map_mut(|map| {
                    map[map_index].flags &= !core::constants::MF_GFX_EMAGIC;
                });
            } else {
                Repository::with_map_mut(|map| {
                    map[map_index].flags &= !core::constants::MF_GFX_EMAGIC;
                    map[map_index].flags |= (effects[n].duration as u64) << 45;
                });
            }
        });
    }

    /// Type 6: Good magic
    /// Handle effect type 6: good magic animation
    ///
    /// Updates good-magic graphic flags and expires the effect after the
    /// configured duration.
    fn handle_effect_type_6(n: usize) {
        Repository::with_effects_mut(|effects| {
            effects[n].duration += 1;

            let map_index = effects[n].data[0] as usize
                + effects[n].data[1] as usize * core::constants::SERVER_MAPX as usize;

            if effects[n].duration == 8 {
                effects[n].used = core::constants::USE_EMPTY;
                Repository::with_map_mut(|map| {
                    map[map_index].flags &= !core::constants::MF_GFX_GMAGIC;
                });
            } else {
                Repository::with_map_mut(|map| {
                    map[map_index].flags &= !core::constants::MF_GFX_GMAGIC;
                    map[map_index].flags |= (effects[n].duration as u64) << 48;
                });
            }
        });
    }

    /// Type 7: Caster magic
    /// Handle effect type 7: caster magic animation
    ///
    /// Updates caster-magic graphic flags and expires the effect after the
    /// configured duration.
    fn handle_effect_type_7(n: usize) {
        Repository::with_effects_mut(|effects| {
            effects[n].duration += 1;

            let map_index = effects[n].data[0] as usize
                + effects[n].data[1] as usize * core::constants::SERVER_MAPX as usize;

            if effects[n].duration == 8 {
                effects[n].used = core::constants::USE_EMPTY;
                Repository::with_map_mut(|map| {
                    map[map_index].flags &= !core::constants::MF_GFX_CMAGIC;
                });
            } else {
                Repository::with_map_mut(|map| {
                    map[map_index].flags &= !core::constants::MF_GFX_CMAGIC;
                    map[map_index].flags |= (effects[n].duration as u64) << 51;
                });
            }
        });
    }

    /// Type 8: Respawn mist
    /// Handle effect type 8: respawn mist
    ///
    /// Handles the visual respawn mist effect and, at mid-life, may spawn
    /// the NPC via the populate helper when the tile becomes available.
    fn handle_effect_type_8(n: usize) {
        Repository::with_effects_mut(|effects| {
            effects[n].duration += 1;

            let map_index = effects[n].data[0] as usize
                + effects[n].data[1] as usize * core::constants::SERVER_MAPX as usize;

            if effects[n].duration == 19 {
                effects[n].used = core::constants::USE_EMPTY;
                Repository::with_map_mut(|map| {
                    map[map_index].flags &= !core::constants::MF_GFX_DEATH;
                });
            } else {
                Repository::with_map_mut(|map| {
                    map[map_index].flags &= !core::constants::MF_GFX_DEATH;
                    map[map_index].flags |= (effects[n].duration as u64) << 40;
                });

                if effects[n].duration == 9 {
                    Repository::with_map_mut(|map| {
                        // See note above about cast/negation precedence.
                        map[map_index].flags &= !(core::constants::MF_MOVEBLOCK as u64);
                    });

                    if let Some(_cn) = populate::pop_create_char(effects[n].data[2] as usize, true)
                    {
                        let respawn_flag = Repository::with_character_templates(|char_templates| {
                            (char_templates[effects[n].data[2] as usize].flags
                                & CharacterFlags::Respawn.bits())
                                != 0
                        });

                        if respawn_flag {
                            effects[n].effect_type = 2;
                            effects[n].duration = (core::constants::TICKS * 60 * 5) as u32;

                            Repository::with_map_mut(|map| {
                                map[map_index].flags &= !core::constants::MF_GFX_DEATH;
                            });
                        }
                    }
                }
            }
        });
    }

    /// Type 9: Controlled item animation with optional monster creation
    /// Handle effect type 9: controlled item animation / optional spawn
    ///
    /// Animates an item and, when complete, optionally creates a monster
    /// at the item's location and removes the item.
    fn handle_effect_type_9(n: usize) {
        Repository::with_effects_mut(|effects| {
            effects[n].duration -= 1;

            let in_id = effects[n].data[0] as usize;

            if (effects[n].duration & 1) == 0 {
                Repository::with_items_mut(|items| {
                    items[in_id].status[1] += 1;
                });
            }

            if effects[n].duration == 0 {
                let (x, y) = Repository::with_items(|items| (items[in_id].x, items[in_id].y));

                let map_index = x as usize + y as usize * core::constants::SERVER_MAPX as usize;

                Repository::with_map_mut(|map| {
                    map[map_index].it = 0;
                });

                if effects[n].data[1] != 0 {
                    if let Some(cn) = populate::pop_create_char(effects[n].data[1] as usize, false)
                    {
                        God::drop_char(cn, x as usize, y as usize);
                        Repository::with_characters_mut(|characters| {
                            characters[cn].dir = core::constants::DX_RIGHTUP;
                        });
                        player::plr_reset_status(cn);
                    }
                }

                effects[n].used = core::constants::USE_EMPTY;

                Repository::with_items_mut(|items| {
                    items[in_id].used = core::constants::USE_EMPTY;
                });
            }
        });
    }

    /// Type 10: Respawn object
    /// Handle effect type 10: respawn object
    ///
    /// Attempts to respawn a map object (item) after a delay. If the tile is
    /// blocked (e.g. beams present) the respawn is rescheduled.
    fn handle_effect_type_10(n: usize) {
        Repository::with_effects_mut(|effects| {
            if effects[n].duration > 0 {
                effects[n].duration -= 1;
            } else {
                let map_index = effects[n].data[0] as usize
                    + effects[n].data[1] as usize * core::constants::SERVER_MAPX as usize;

                // Check if object isn't allowed to respawn (supporting beams for mine)
                if Self::check_surrounding_beams(map_index) {
                    effects[n].duration = (core::constants::TICKS * 60 * 15) as u32;
                    return;
                }

                let in2 = Repository::with_map(|map| map[map_index].it);

                Repository::with_map_mut(|map| {
                    map[map_index].it = 0;
                });

                let in_id = God::create_item(effects[n].data[2] as usize);

                if let Some(in_id) = in_id {
                    let drop_success = God::drop_item(
                        in_id,
                        effects[n].data[0] as usize,
                        effects[n].data[1] as usize,
                    );

                    if !drop_success {
                        effects[n].duration = (core::constants::TICKS * 60) as u32;
                        Repository::with_items_mut(|items| {
                            items[in_id].used = core::constants::USE_EMPTY;
                        });
                        Repository::with_map_mut(|map| {
                            map[map_index].it = in2;
                        });
                    } else {
                        effects[n].used = core::constants::USE_EMPTY;
                        if in2 != 0 {
                            Repository::with_items_mut(|items| {
                                items[in2 as usize].used = core::constants::USE_EMPTY;
                            });
                        }
                        State::with_mut(|state| {
                            state.reset_go(effects[n].data[0] as i32, effects[n].data[1] as i32);
                        });
                    }
                }
            }
        });
    }

    /// Type 11: Remove queued spell flags
    /// Handle effect type 11: queued-spell flag remover
    ///
    /// Clears queued-spell flags on the target character when the effect
    /// duration elapses.
    fn handle_effect_type_11(n: usize) {
        Repository::with_effects_mut(|effects| {
            effects[n].duration -= 1;

            if effects[n].duration < 1 {
                effects[n].used = core::constants::USE_EMPTY;

                Repository::with_characters_mut(|characters| {
                    characters[effects[n].data[0] as usize].data[96] &= !effects[n].data[1] as i32;
                });
            }
        });
    }

    /// Type 12: Death mist (alternative)
    /// Handle effect type 12: alternative death mist
    ///
    /// Similar to type 3 but used for alternative death-mist sequences; it
    /// increments the animation and clears map flags on completion.
    fn handle_effect_type_12(n: usize) {
        Repository::with_effects_mut(|effects| {
            effects[n].duration += 1;

            let map_index = effects[n].data[0] as usize
                + effects[n].data[1] as usize * core::constants::SERVER_MAPX as usize;

            if effects[n].duration == 19 {
                effects[n].used = core::constants::USE_EMPTY;
                Repository::with_map_mut(|map| {
                    map[map_index].flags &= !core::constants::MF_GFX_DEATH;
                });
            } else {
                Repository::with_map_mut(|map| {
                    map[map_index].flags &= !core::constants::MF_GFX_DEATH;
                    map[map_index].flags |= (effects[n].duration as u64) << 40;
                });
            }
        });
    }

    /// Port of `fx_add_effect` from `svr_effect.cpp`
    pub fn fx_add_effect(
        effect_type: i32,
        duration: i32,
        d1: i32,
        d2: i32,
        d3: i32,
    ) -> Option<usize> {
        Repository::with_effects_mut(|effects| {
            for n in 1..core::constants::MAXEFFECT {
                if effects[n].used == core::constants::USE_EMPTY {
                    effects[n].used = core::constants::USE_ACTIVE;
                    effects[n].effect_type = effect_type as u8;
                    effects[n].duration = duration as u32;
                    effects[n].flags = 0;
                    effects[n].data[0] = d1 as u32;
                    effects[n].data[1] = d2 as u32;
                    effects[n].data[2] = d3 as u32;
                    return Some(n);
                }
            }
            None
        })
    }

    // Helper functions

    /// Find a nearby map cell suitable for dropping items.
    ///
    /// Scans a set of offsets around `base_map_index` returning the first
    /// index where `can_drop` returns true. Returns `0` when none found.
    fn find_drop_position(base_map_index: usize) -> usize {
        let offsets = [
            0,
            1,
            -1,
            core::constants::SERVER_MAPX,
            -core::constants::SERVER_MAPX,
            1 + core::constants::SERVER_MAPX,
            1 - core::constants::SERVER_MAPX,
            -1 + core::constants::SERVER_MAPX,
            -1 - core::constants::SERVER_MAPX,
            2,
            -2,
            2 * core::constants::SERVER_MAPX,
            -2 * core::constants::SERVER_MAPX,
            2 + core::constants::SERVER_MAPX,
            2 - core::constants::SERVER_MAPX,
            -2 + core::constants::SERVER_MAPX,
            -2 - core::constants::SERVER_MAPX,
            1 + 2 * core::constants::SERVER_MAPX,
            1 - 2 * core::constants::SERVER_MAPX,
            -1 + 2 * core::constants::SERVER_MAPX,
            -1 - 2 * core::constants::SERVER_MAPX,
            2 + 2 * core::constants::SERVER_MAPX,
            2 - 2 * core::constants::SERVER_MAPX,
            -2 + 2 * core::constants::SERVER_MAPX,
            -2 - 2 * core::constants::SERVER_MAPX,
        ];

        for offset in offsets.iter() {
            let m = (base_map_index as i32 + offset) as usize;
            if Self::can_drop(m) {
                return m;
            }
        }

        0
    }

    /// Create a grave/tomb at `map_index` for character `co` if items/gold exist.
    ///
    /// If the character has items or gold the tile is reserved and an effect
    /// is added to create a tombstone. Otherwise items are destroyed and the
    /// character slot may be freed or scheduled for respawn.
    fn handle_grave_creation(map_index: usize, co: usize, killer_cn: i32) {
        let (has_items, has_gold) = Repository::with_characters(|characters| {
            let mut flag = false;

            for z in 0..40 {
                if characters[co].item[z] != 0 {
                    flag = true;
                    break;
                }
            }

            if !flag {
                for z in 0..20 {
                    if characters[co].worn[z] != 0 {
                        flag = true;
                        break;
                    }
                }
            }

            if characters[co].citem != 0 {
                flag = true;
            }

            let has_gold = characters[co].gold != 0;

            (flag, has_gold)
        });

        if has_items || has_gold {
            Repository::with_map_mut(|map| {
                map[map_index].flags |= core::constants::MF_MOVEBLOCK as u64;
            });

            let fn_idx = Self::fx_add_effect(
                4,
                0,
                (map_index % core::constants::SERVER_MAPX as usize) as i32,
                (map_index / core::constants::SERVER_MAPX as usize) as i32,
                co as i32,
            );

            if let Some(fn_idx) = fn_idx {
                Repository::with_effects_mut(|effects| {
                    effects[fn_idx].data[3] = killer_cn as u32;
                });
            }
        } else {
            let temp = Repository::with_characters(|characters| characters[co].temp as usize);

            God::destroy_items(co);

            Repository::with_characters_mut(|characters| {
                characters[co].used = core::constants::USE_EMPTY;

                if temp != 0 && (characters[co].flags & CharacterFlags::Respawn.bits()) != 0 {
                    if temp == 189 || temp == 561 {
                        Self::fx_add_effect(
                            2,
                            core::constants::TICKS * 60 * 20
                                + rand::random::<i32>().abs() % (core::constants::TICKS * 60 * 5),
                            Repository::with_character_templates(|char_templates| {
                                char_templates[temp].x as i32
                            }),
                            Repository::with_character_templates(|char_templates| {
                                char_templates[temp].y as i32
                            }),
                            temp as i32,
                        );
                    } else {
                        Self::fx_add_effect(
                            2,
                            core::constants::TICKS * 60 * 4
                                + rand::random::<i32>().abs() % (core::constants::TICKS * 60),
                            Repository::with_character_templates(|char_templates| {
                                char_templates[temp].x as i32
                            }),
                            Repository::with_character_templates(|char_templates| {
                                char_templates[temp].y as i32
                            }),
                            temp as i32,
                        );
                    }

                    log::info!("Respawn {} ({}): YES", co, &characters[co].get_name());
                } else {
                    log::info!("Respawn {} ({}): NO", co, &characters[co].get_name());
                }
            });
        }
    }

    /// Check the neighborhood of `map_index` for active beam items.
    ///
    /// Returns `true` if any nearby tile contains an active beam item (used
    /// to prevent object respawn in beam-protected areas).
    fn check_surrounding_beams(map_index: usize) -> bool {
        let offsets = [
            0,
            -1,
            1,
            -core::constants::SERVER_MAPX,
            core::constants::SERVER_MAPX,
            -2,
            2,
            -2 * core::constants::SERVER_MAPX,
            2 * core::constants::SERVER_MAPX,
            -1 + core::constants::SERVER_MAPX,
            1 + core::constants::SERVER_MAPX,
            -1 - core::constants::SERVER_MAPX,
            1 - core::constants::SERVER_MAPX,
            -2 + core::constants::SERVER_MAPX,
            2 + core::constants::SERVER_MAPX,
            -2 - core::constants::SERVER_MAPX,
            2 - core::constants::SERVER_MAPX,
            -1 + 2 * core::constants::SERVER_MAPX,
            1 + 2 * core::constants::SERVER_MAPX,
            -1 - 2 * core::constants::SERVER_MAPX,
            1 - 2 * core::constants::SERVER_MAPX,
            -2 + 2 * core::constants::SERVER_MAPX,
            2 + 2 * core::constants::SERVER_MAPX,
            -2 - 2 * core::constants::SERVER_MAPX,
            2 - 2 * core::constants::SERVER_MAPX,
        ];

        Repository::with_map(|map| {
            for offset in offsets.iter() {
                let m = (map_index as i32 + offset) as usize;
                if m < map.len() && Self::is_beam(map[m].it as usize) {
                    return true;
                }
            }
            false
        })
    }
}
