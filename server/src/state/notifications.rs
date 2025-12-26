use crate::driver::Driver;
use crate::repository::Repository;
use crate::state::State;
use core::constants::{CharacterFlags, MAXCHARS};

impl State {
    /// Send a notification to all characters in an area around a position.
    /// Excludes the source characters `cn` and `co` from receiving the notification.
    ///
    /// Port of `do_area_notify` from `svr_do.cpp`
    ///
    /// # Parameters
    /// - `cn`: Source character 1 (excluded from notification)
    /// - `co`: Source character 2 (excluded from notification)
    /// - `xs`: X coordinate of the center position
    /// - `ys`: Y coordinate of the center position
    /// - `notify_type`: Type of notification to send
    /// - `dat1`, `dat2`, `dat3`, `dat4`: Data parameters for the notification
    pub(crate) fn notify_area(
        &self,
        cn: i32,
        co: i32,
        xs: i32,
        ys: i32,
        notify_type: i32,
        dat1: i32,
        dat2: i32,
        dat3: i32,
        dat4: i32,
    ) {
        Repository::with_map(|map| {
            for y in std::cmp::max(0, ys - core::constants::AREA_SIZE)
                ..std::cmp::min(
                    core::constants::SERVER_MAPY,
                    ys + core::constants::AREA_SIZE + 1,
                )
            {
                let m = y * core::constants::SERVER_MAPX as i32;
                for x in std::cmp::max(0, xs - core::constants::AREA_SIZE)
                    ..std::cmp::min(
                        core::constants::SERVER_MAPX,
                        xs + core::constants::AREA_SIZE + 1,
                    )
                {
                    let cc = map[(x + m) as usize].ch;

                    if cc != 0 && cc != cn as u32 && cc != co as u32 {
                        self.do_notify_character(cc, notify_type, dat1, dat2, dat3, dat4);
                    }
                }
            }
        });
    }

    /// Find the 3 closest NPCs to the shouting NPC and send them a notification.
    /// This is used for NPCs to call for help or alert nearby NPCs.
    ///
    /// **Warning**: Use this sparingly! It uses quite a bit of computation time.
    ///
    /// Port of `do_npc_shout` from `svr_do.cpp`
    ///
    /// # Parameters
    /// - `cn`: The NPC doing the shouting
    /// - `shout_type`: Type of notification to send
    /// - `dat1`, `dat2`, `dat3`, `dat4`: Data parameters for the notification
    pub(crate) fn npc_notify_dists(
        &self,
        cn: usize,
        shout_type: i32,
        dat1: i32,
        dat2: i32,
        dat3: i32,
        dat4: i32,
    ) {
        Repository::with_characters(|characters| {
            let mut best: [i32; 3] = [99; 3];
            let mut bestn: [i32; 3] = [0; 3];

            if characters[cn].data[52] == 3 {
                for co in 1..core::constants::MAXCHARS {
                    if co != cn
                        && characters[co].used == core::constants::USE_ACTIVE
                        && characters[co].flags & CharacterFlags::CF_BODY.bits() == 0
                    {
                        if characters[co].flags
                            & (CharacterFlags::CF_PLAYER | CharacterFlags::CF_USURP).bits()
                            != 0
                        {
                            continue;
                        }

                        if characters[co].data[53] != characters[cn].data[52] {
                            continue;
                        }

                        // TODO: This distance calculation seems incorrect potentially -- doublecheck
                        let distance = (characters[cn].x as i32 - characters[co].x as i32).abs()
                            + (characters[cn].y as i32 - characters[co].y as i32).abs();

                        if distance < best[0] {
                            best[2] = best[1];
                            bestn[2] = bestn[1];
                            best[1] = best[0];
                            bestn[1] = bestn[0];
                            best[0] = distance;
                            bestn[0] = co as i32;
                        } else if distance < best[1] {
                            best[2] = best[1];
                            bestn[2] = bestn[1];
                            best[1] = distance;
                            bestn[1] = co as i32;
                        }
                        // } else if distance < best[3] {
                        //     // TODO: Pretty sure [3] isn't safe
                        //     best[3] = distance;
                        //     bestn[3] = co as i32;
                        // }
                    }
                }

                for i in 0..bestn.len() {
                    if bestn[i] != 0 {
                        self.do_notify_character(
                            bestn[i] as u32,
                            shout_type,
                            dat1,
                            dat2,
                            dat3,
                            dat4,
                        );
                    }
                }
            } else {
                for co in 1..core::constants::MAXCHARS {
                    if co != cn
                        && characters[co].used == core::constants::USE_ACTIVE
                        && characters[co].flags & CharacterFlags::CF_BODY.bits() == 0
                    {
                        if characters[co].flags
                            & (CharacterFlags::CF_PLAYER | CharacterFlags::CF_USURP).bits()
                            != 0
                        {
                            continue;
                        }

                        if characters[co].data[53] != characters[cn].data[52] {
                            continue;
                        }

                        self.do_notify_character(co as u32, shout_type, dat1, dat2, dat3, dat4);
                    }
                }
            }
        });
    }

    /// Calculate a character's score based on their total points.
    /// Score is computed as: (sqrt(points_tot) / 7) + 7
    ///
    /// Port of `do_char_score` from `svr_do.cpp`
    ///
    /// # Parameters
    /// - `cn`: Character index
    ///
    /// # Returns
    /// The calculated score value
    pub(crate) fn get_score(&self, cn: usize) -> i32 {
        let pts = Repository::with_characters(|characters| characters[cn].points_tot);
        let pts = if pts < 0 { 0 } else { pts } as f64;
        ((pts.sqrt() as i32) / 7) + 7
    }
}

/// Remove a character from all other characters' enemy lists.
/// This is typically called when a character logs out or dies.
///
/// Port of `remove_enemy` from `svr_do.cpp`
///
/// # Parameters
/// - `co`: Character index to remove from enemy lists
pub(crate) fn remove_from_enemies(co: usize) {
    Repository::with_characters_mut(|characters| {
        for n in 1..MAXCHARS as usize {
            for m in 0..4 {
                if characters[n].enemy[m] as usize == co {
                    characters[n].enemy[m] = 0;
                }
            }
        }
    });
}
