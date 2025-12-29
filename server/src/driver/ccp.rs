#[cfg(feature = "REAL_CCP")]
mod real_ccp {
    use crate::core::constants;
    use crate::enums::CharacterFlags;
    use crate::repository::Repository;
    use crate::state::State;
    use std::sync::LazyLock;
    use std::sync::Mutex;

    #[derive(Clone)]
    struct CcpMem {
        lastshout: i64,
        fighting: i32,
        enemy_strength: i32,
        level: i32,
        sector: Vec<Vec<i32>>, // [SERVER_MAPX/10][SERVER_MAPY/10]
    }

    impl CcpMem {
        /// Creates a new `CcpMem` instance with default values.
        ///
        /// # Returns
        ///
        /// A new `CcpMem` struct with initialized fields.
        fn new() -> Self {
            let sx = (constants::SERVER_MAPX / 10) as usize;
            let sy = (constants::SERVER_MAPY / 10) as usize;
            CcpMem {
                lastshout: 0,
                fighting: 0,
                enemy_strength: 0,
                level: 1,
                sector: vec![vec![0; sy]; sx],
            }
        }
    }

    static CCP_MEMS: LazyLock<Mutex<Vec<Option<CcpMem>>>> =
        LazyLock::new(|| Mutex::new(vec![None; constants::MAXCHARS as usize]));

    /// Provides mutable access to the `CcpMem` for a given character number, initializing if needed.
    ///
    /// # Arguments
    ///
    /// * `cn` - Character number (index)
    /// * `f` - Closure to execute with mutable reference to `CcpMem`
    ///
    /// # Returns
    ///
    /// The return value of the closure `f`.
    fn get_ccp_mem_mut<F, R>(cn: usize, f: F) -> R
    where
        F: FnOnce(&mut CcpMem) -> R,
    {
        let mut guard = CCP_MEMS.lock().unwrap();
        if guard.get(cn).is_none() {
            if cn < guard.len() {
                guard[cn] = Some(CcpMem::new());
            }
        }
        let slot = &mut guard[cn];
        if slot.is_none() {
            guard[cn] = Some(CcpMem::new());
        }
        f(slot.as_mut().unwrap())
    }

    /// Adjusts the sector score for the character's current sector by a given value.
    ///
    /// # Arguments
    ///
    /// * `cn` - Character number (index)
    /// * `val` - Value to add to the sector score
    pub fn ccp_sector_score(cn: usize, val: i32) {
        get_ccp_mem_mut(cn, |mem| {
            let (x, y) = Repository::with_characters(|ch| (ch[cn].x as i32, ch[cn].y as i32));
            let sx = (x / 10) as usize;
            let sy = (y / 10) as usize;
            if sx < mem.sector.len() && sy < mem.sector[0].len() {
                mem.sector[sx][sy] += val;
            }
        });
    }

    /// Handles a tell (private message) to a CCP character.
    ///
    /// # Arguments
    ///
    /// * `cn` - CCP character number
    /// * `co` - Sender character number
    /// * `text` - Message text
    pub fn ccp_tell(cn: usize, co: usize, text: &str) {
        // Mirror C++ behaviour: if target is CCP, ignore; otherwise send a canned reply.
        if Repository::with_characters(|ch| {
            (ch[co].flags & CharacterFlags::ComputerControlledPlayer.bits() as u64) != 0
        }) {
            return;
        }

        State::with(|state| {
            state.do_character_log(
                cn,
                core::types::FontColor::Red,
                "Sorry, I'm just a robot, I cannot really talk back.",
            );
        });
    }

    /// Handles a shout message directed at a CCP character.
    ///
    /// # Arguments
    ///
    /// * `cn` - CCP character number
    /// * `co` - Sender character number
    /// * `text` - Message text
    pub fn ccp_shout(cn: usize, co: usize, text: &str) {
        // simplified port of original logic
        get_ccp_mem_mut(cn, |mem| {
            if mem.lastshout
                > Repository::with_globals(|g| g.ticker as i64) + (constants::TICKS as i64) * 10
            {
                return;
            }
            if text.contains(&Repository::with_characters(|ch| ch[cn].name.clone())) {
                crate::server::do_shout(cn, "I'm just a robot, stop shouting my name!");
                mem.lastshout = Repository::with_globals(|g| g.ticker as i64);
            }
        });
    }

    /// Sets the enemy for the CCP character and determines enemy strength.
    ///
    /// # Arguments
    ///
    /// * `cn` - CCP character number
    /// * `co` - Enemy character number
    ///
    /// # Returns
    ///
    /// Returns 1 after setting the enemy.
    pub fn ccp_set_enemy(cn: usize, co: usize) -> i32 {
        get_ccp_mem_mut(cn, |mem| {
            mem.fighting = co as i32;

            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    format!(
                        "co pts={}, cn pts={}\\n",
                        Repository::with_characters(|ch| ch[co].points_tot),
                        Repository::with_characters(|ch| ch[cn].points_tot)
                    )
                    .as_str(),
                );
            });

            let co_pts = Repository::with_characters(|ch| ch[co].points_tot as i64);
            let cn_pts = Repository::with_characters(|ch| ch[cn].points_tot as i64);

            mem.enemy_strength = if co_pts > cn_pts * 4 {
                7
            } else if co_pts > cn_pts * 2 {
                6
            } else if co_pts > (cn_pts * 3) / 2 {
                5
            } else if co_pts > cn_pts {
                4
            } else if co_pts > (cn_pts * 2) / 3 {
                3
            } else if co_pts > cn_pts / 2 {
                2
            } else if co_pts > cn_pts / 4 {
                1
            } else {
                0
            };

            State::with(|state| {
                state.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    format!(
                        "set enemy {} ({}), strength={}\\n",
                        co,
                        Repository::with_characters(|ch| String::from_utf8_lossy(&ch[co].name)),
                        mem.enemy_strength
                    )
                    .as_str(),
                );
            });

            1
        })
    }

    /// Handles logic when the CCP character is attacked by another character.
    ///
    /// # Arguments
    ///
    /// * `cn` - CCP character number
    /// * `co` - Attacker character number
    pub fn ccp_gotattack(cn: usize, co: usize) {
        get_ccp_mem_mut(cn, |mem| {
            if mem.fighting == co as i32 {
                return;
            }
            if mem.fighting == 0 {
                Repository::with_characters_mut(|ch| ch[cn].attack_cn = co as i32);
                ccp_set_enemy(cn, co);
                return;
            }
        });
    }

    /// Handles logic when the CCP character sees another character.
    ///
    /// # Arguments
    ///
    /// * `cn` - CCP character number
    /// * `co` - Seen character number
    pub fn ccp_seen(cn: usize, co: usize) {
        get_ccp_mem_mut(cn, |mem| {
            if Repository::with_characters(|ch| ch[co].attack_cn) == cn as i32 {
                if Repository::with_characters(|ch| ch[co].points_tot)
                    > Repository::with_characters(|ch| ch[cn].points_tot)
                {
                    ccp_sector_score(cn, -10000);
                }
            }

            if mem.fighting == co as i32 {
                return;
            }

            if Repository::with_characters(|ch| ch[cn].a_hp)
                < Repository::with_characters(|ch| ch[cn].hp[5]) * 800
            {
                return;
            }
            if Repository::with_characters(|ch| ch[cn].a_mana)
                < Repository::with_characters(|ch| ch[cn].hp[5]) * 800
            {
                return;
            }

            if (Repository::with_characters(|ch| ch[co].flags)
                & CharacterFlags::Player.bits() as u64)
                != 0
            {
                return;
            }
            if Repository::with_characters(|ch| ch[co].alignment) > 0 {
                return;
            }
            if crate::server::is_companion(co) {
                return;
            }

            if !crate::server::do_char_can_see(cn, co) {
                return;
            }
            if !crate::server::can_go(
                Repository::with_characters(|ch| ch[cn].x),
                Repository::with_characters(|ch| ch[cn].y),
                Repository::with_characters(|ch| ch[co].x),
                Repository::with_characters(|ch| ch[co].y),
            ) {
                return;
            }

            if Repository::with_characters(|ch| ch[co].points_tot)
                > (Repository::with_characters(|ch| ch[cn].points_tot) * 3) / 2
            {
                return;
            }

            if Repository::with_characters(|ch| ch[co].points_tot)
                < Repository::with_characters(|ch| ch[cn].points_tot) / 3
            {
                let co_pts = Repository::with_characters(|ch| ch[co].points_tot);
                let cn_pts = Repository::with_characters(|ch| ch[cn].points_tot);
                if co_pts > cn_pts {
                    ccp_sector_score(cn, -3000);
                }
                return;
            }

            ccp_set_enemy(cn, co);
            ccp_sector_score(cn, 1000);
        });
    }

    /// Attempts to raise the CCP character's attributes or skills, or level up.
    ///
    /// # Arguments
    ///
    /// * `cn` - CCP character number
    ///
    /// # Returns
    ///
    /// Returns 1 if an attribute or skill was raised, or level was increased.
    pub fn ccp_raise(cn: usize) -> i32 {
        let skill_list = [
            crate::core::enums::SK_DAGGER,
            crate::core::enums::SK_MEDIT,
            crate::core::enums::SK_BLAST,
            crate::core::enums::SK_CURSE,
            crate::core::enums::SK_STUN,
            crate::core::enums::SK_ENHANCE,
            crate::core::enums::SK_PROTECT,
            crate::core::enums::SK_BLESS,
            crate::core::enums::SK_MSHIELD,
        ];

        for n in 0..5 {
            if Repository::with_characters(|ch| ch[cn].attrib[n][0]) < get_ccp_mem_level(cn) {
                return crate::server::do_raise_attrib(cn, n as i32);
            }
        }

        for &s in skill_list.iter() {
            if Repository::with_characters(|ch| ch[cn].skill[s as usize][0]) != 0
                && Repository::with_characters(|ch| ch[cn].skill[s as usize][0])
                    < get_ccp_mem_level(cn) * 2
            {
                return crate::server::do_raise_skill(cn, s as i32);
            }
        }

        // increment mem->level
        get_ccp_mem_mut(cn, |mem| mem.level += 1);
        crate::server::do_char_log(
            cn,
            1,
            format!("raise new level={}\\n", get_ccp_mem_level(cn)).as_str(),
        );
        1
    }

    /// Gets the current level stored in the CCP character's memory.
    ///
    /// # Arguments
    ///
    /// * `cn` - CCP character number
    ///
    /// # Returns
    ///
    /// The current level as an i32.
    fn get_ccp_mem_level(cn: usize) -> i32 {
        let mut lvl = 1;
        get_ccp_mem_mut(cn, |mem| lvl = mem.level);
        lvl
    }

    /// Handles logic when the CCP character gains experience.
    ///
    /// # Arguments
    ///
    /// * `cn` - CCP character number
    pub fn ccp_gotexp(cn: usize) {
        let mut dontpanic = 10;
        while dontpanic > 0 && ccp_raise(cn) != 0 {
            dontpanic -= 1;
        }
    }

    /// Handles messages sent to the CCP character, dispatching to appropriate handlers.
    ///
    /// # Arguments
    ///
    /// * `cn` - CCP character number
    /// * `msg_type` - Message type identifier
    /// * `dat1` - First data parameter (meaning depends on message type)
    /// * `_dat2`, `_dat3`, `_dat4` - Unused data parameters
    pub fn ccp_msg(cn: usize, msg_type: i32, dat1: i32, _dat2: i32, _dat3: i32, _dat4: i32) {
        match msg_type {
            crate::core::enums::NT_GOTMISS | crate::core::enums::NT_GOTHIT => {
                ccp_gotattack(cn, dat1 as usize)
            }
            crate::core::enums::NT_SEE => ccp_seen(cn, dat1 as usize),
            crate::core::enums::NT_GOTEXP => ccp_gotexp(cn),
            _ => {}
        }
    }

    /// Checks if the CCP character is at their recall (temple) point.
    ///
    /// # Arguments
    ///
    /// * `cn` - CCP character number
    ///
    /// # Returns
    ///
    /// `true` if at recall point, `false` otherwise.
    pub fn ccp_at_recall_point(cn: usize) -> bool {
        let (x, y, tx, ty) = Repository::with_characters(|ch| {
            (ch[cn].x, ch[cn].y, ch[cn].temple_x, ch[cn].temple_y)
        });
        ((x - tx).abs() + (y - ty).abs()) < 5
    }

    /// Moves the CCP character toward the sector with the highest score.
    ///
    /// # Arguments
    ///
    /// * `cn` - CCP character number
    pub fn ccp_goto_sector(cn: usize) {
        get_ccp_mem_mut(cn, |mem| {
            let (x, y) = Repository::with_characters(|ch| (ch[cn].x as i32, ch[cn].y as i32));
            let mut bx = (x / 10) as i32;
            let mut by = (y / 10) as i32;
            let mut best = mem.sector[bx as usize][by as usize];

            let sx = (constants::SERVER_MAPX / 10) as i32;
            let sy = (constants::SERVER_MAPY / 10) as i32;

            if bx > 0 && mem.sector[(bx - 1) as usize][by as usize] > best {
                best = mem.sector[(bx - 1) as usize][by as usize];
                bx -= 1;
            }
            if bx < sx - 1 && mem.sector[(bx + 1) as usize][by as usize] > best {
                best = mem.sector[(bx + 1) as usize][by as usize];
                bx += 1;
            }
            if by > 0 && mem.sector[bx as usize][(by - 1) as usize] > best {
                best = mem.sector[bx as usize][(by - 1) as usize];
                by -= 1;
            }
            if by < sy - 1 && mem.sector[bx as usize][(by + 1) as usize] > best {
                best = mem.sector[bx as usize][(by + 1) as usize];
                by += 1;
            }

            crate::server::do_char_log(
                cn,
                1,
                format!(
                    "x={}, y={}, best={}, bx={}, by={}\\n",
                    x / 10,
                    y / 10,
                    best,
                    bx,
                    by
                )
                .as_str(),
            );

            let ticker = Repository::with_globals(|g| g.ticker as i32);
            let rx = (bx * 10) + (ticker % 7) + 2;
            let ry = (by * 10) + (ticker % 7) + 2;

            if rx == bx && ry == by {
                return;
            }

            Repository::with_characters_mut(|ch| {
                ch[cn].goto_x = rx as i16;
                ch[cn].goto_y = ry as i16;
            });

            mem.sector[bx as usize][by as usize] -= 5;
        });
    }

    /// Main driver function for CCP character AI logic.
    ///
    /// # Arguments
    ///
    /// * `cn` - CCP character number
    pub fn ccp_driver(cn: usize) {
        get_ccp_mem_mut(cn, |mem| {
            let can_recall = Repository::with_characters(|ch| ch[cn].a_hp < ch[cn].hp[5] * 500);
            if can_recall
                && !ccp_at_recall_point(cn)
                && crate::server::npc_try_spell(cn, cn, crate::core::enums::SK_RECALL)
            {
                return;
            }

            if Repository::with_characters(|ch| ch[cn].a_mana) > 1000 * 30 {
                if Repository::with_characters(|ch| ch[cn].a_hp)
                    < Repository::with_characters(|ch| ch[cn].hp[5]) * 750
                    && crate::server::npc_try_spell(cn, cn, crate::core::enums::SK_HEAL)
                {
                    return;
                }
                if crate::server::npc_try_spell(cn, cn, crate::core::enums::SK_PROTECT) {
                    return;
                }
                if crate::server::npc_try_spell(cn, cn, crate::core::enums::SK_ENHANCE) {
                    return;
                }
                if crate::server::npc_try_spell(cn, cn, crate::core::enums::SK_BLESS) {
                    return;
                }
                if crate::server::npc_try_spell(cn, cn, crate::core::enums::SK_MSHIELD) {
                    return;
                }
            }

            if mem.fighting != 0 && !crate::server::do_char_can_see(cn, mem.fighting as usize) {
                mem.fighting = 0;
            }

            if mem.fighting != 0 {
                let co = mem.fighting as usize;
                Repository::with_characters_mut(|ch| ch[cn].attack_cn = co as i32);
                Repository::with_characters_mut(|ch| {
                    ch[cn].goto_x = 0;
                    ch[cn].goto_y = 0;
                });
                if Repository::with_characters(|ch| ch[cn].a_mana) > 1000 * 30 {
                    if mem.enemy_strength > 1 {
                        if crate::server::npc_try_spell(cn, co, crate::core::enums::SK_CURSE) {
                            return;
                        }
                        if crate::server::npc_try_spell(cn, co, crate::core::enums::SK_STUN) {
                            return;
                        }
                        if mem.enemy_strength > 2 {
                            if crate::server::npc_try_spell(cn, co, crate::core::enums::SK_BLAST) {
                                return;
                            }
                        }
                    }
                }
            }

            if Repository::with_characters(|ch| ch[cn].attack_cn) != 0
                || Repository::with_characters(|ch| ch[cn].goto_x) != 0
                || Repository::with_characters(|ch| ch[cn].misc_action) != 0
            {
                return;
            }

            if Repository::with_characters(|ch| {
                ch[cn].skill[crate::core::enums::SK_MEDIT as usize][0]
            }) != 0
                && Repository::with_characters(|ch| ch[cn].a_mana)
                    < Repository::with_characters(|ch| ch[cn].mana[5] * 900)
            {
                return;
            }
            if Repository::with_characters(|ch| ch[cn].a_hp)
                < Repository::with_characters(|ch| ch[cn].hp[5]) * 900
            {
                return;
            }

            ccp_sector_score(cn, -1);
            ccp_goto_sector(cn);
        });
    }
}
