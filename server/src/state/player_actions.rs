use core::constants::CharacterFlags;
use core::types::FontColor;

use crate::god::God;
use crate::repository::Repository;
use crate::state::State;
use crate::{driver, helpers};

impl State {
    /// Port of `do_swap_item(int cn, int n)` from `svr_do.cpp`
    ///
    /// Swap the carried item (citem) with an equipment slot.
    /// Performs various prerequisite checks including attributes, skills, HP/END/MANA requirements,
    /// faction restrictions, rank requirements, and placement validation.
    ///
    /// # Arguments
    /// * `cn` - Character index
    /// * `n` - Equipment slot index (0-19, but only 0-11 are valid worn slots)
    ///
    /// # Returns
    /// * The slot number on success
    /// * -1 on failure
    pub(crate) fn do_swap_item(&self, cn: usize, n: usize) -> i32 {
        const AT_TEXT: [&str; 5] = [
            "not brave enough",
            "not determined enough",
            "not intuitive enough",
            "not agile enough",
            "not strong enough",
        ];

        let result = Repository::with_characters_mut(|characters| {
            // Check if citem has high bit set (invalid state)
            if (characters[cn].citem & 0x80000000) != 0 {
                return -1;
            }

            // Sanity check slot range
            if n > 19 {
                return -1;
            }

            let tmp = characters[cn].citem as usize;

            // Check prerequisites if there's an item to equip
            if tmp != 0 {
                let check_result = Repository::with_items_mut(|items| {
                    // Driver 52: Personal item with character binding
                    if items[tmp].driver == 52 && items[tmp].data[0] as usize != cn {
                        if items[tmp].data[0] == 0 {
                            // Bind item to character
                            items[tmp].data[0] = cn as u32;

                            // Engrave character name into description
                            let current_desc = String::from_utf8_lossy(&items[tmp].description)
                                .trim_matches('\0')
                                .to_string();
                            let char_name = String::from_utf8_lossy(&characters[cn].name)
                                .trim_matches('\0')
                                .to_string();
                            let new_desc = format!(
                                "{} Engraved in it are the letters \"{}\".",
                                current_desc, char_name
                            );

                            if new_desc.len() < 200 {
                                let desc_bytes = new_desc.as_bytes();
                                items[tmp].description[..desc_bytes.len().min(200)]
                                    .copy_from_slice(&desc_bytes[..desc_bytes.len().min(200)]);
                            }
                        } else {
                            let item_ref = String::from_utf8_lossy(&items[tmp].reference)
                                .trim_matches('\0')
                                .to_string();
                            self.do_character_log(
                                cn,
                                FontColor::Red,
                                &format!(
                                    "The gods frown at your attempt to wear another ones {}.\n",
                                    item_ref
                                ),
                            );
                            return -1;
                        }
                    }

                    // Check attribute requirements
                    for m in 0..5 {
                        if items[tmp].attrib[m][2] > characters[cn].attrib[m][0] as i8 {
                            self.do_character_log(
                                cn,
                                FontColor::Red,
                                &format!("You're {} to use that.\n", AT_TEXT[m]),
                            );
                            return -1;
                        }
                    }

                    // Check skill requirements
                    for m in 0..50 {
                        if items[tmp].skill[m][2] > characters[cn].skill[m][0] as i8 {
                            self.do_character_log(
                                cn,
                                FontColor::Red,
                                "You don't know how to use that.\n",
                            );
                            return -1;
                        }
                        if items[tmp].skill[m][2] != 0 && characters[cn].skill[m][0] == 0 {
                            self.do_character_log(
                                cn,
                                FontColor::Red,
                                "You don't know how to use that.\n",
                            );
                            return -1;
                        }
                    }

                    // Check HP/END/MANA requirements
                    if items[tmp].hp[2] > characters[cn].hp[0] as i16 {
                        self.do_character_log(
                            cn,
                            FontColor::Red,
                            "You don't have enough life force to use that.\n",
                        );
                        return -1;
                    }
                    if items[tmp].end[2] > characters[cn].end[0] as i16 {
                        self.do_character_log(
                            cn,
                            FontColor::Red,
                            "You don't have enough endurance to use that.\n",
                        );
                        return -1;
                    }
                    if items[tmp].mana[2] > characters[cn].mana[0] as i16 {
                        self.do_character_log(
                            cn,
                            FontColor::Red,
                            "You don't have enough mana to use that.\n",
                        );
                        return -1;
                    }

                    // Check faction/kindred restrictions
                    if (items[tmp].driver == 18
                        && (characters[cn].kindred & core::constants::KIN_PURPLE as i32) != 0)
                        || (items[tmp].driver == 39
                            && (characters[cn].kindred & core::constants::KIN_PURPLE as i32) == 0)
                        || (items[tmp].driver == 40
                            && (characters[cn].kindred & core::constants::KIN_SEYAN_DU as i32) == 0)
                    {
                        self.do_character_log(cn, FontColor::Red, "Ouch. That hurt.\n");
                        return -1;
                    }

                    // Check rank requirement
                    if items[tmp].min_rank
                        > helpers::points2rank(characters[cn].points_tot as u32) as i8
                    {
                        self.do_character_log(
                            cn,
                            FontColor::Red,
                            "You're not experienced enough to use that.\n",
                        );
                        return -1;
                    }

                    // Check for correct placement
                    use core::constants::*;
                    let placement_ok = match n {
                        WN_HEAD => (items[tmp].placement & PL_HEAD) != 0,
                        WN_NECK => (items[tmp].placement & PL_NECK) != 0,
                        WN_BODY => (items[tmp].placement & PL_BODY) != 0,
                        WN_ARMS => (items[tmp].placement & PL_ARMS) != 0,
                        WN_BELT => (items[tmp].placement & PL_BELT) != 0,
                        WN_LEGS => (items[tmp].placement & PL_LEGS) != 0,
                        WN_FEET => (items[tmp].placement & PL_FEET) != 0,
                        WN_LHAND => {
                            if (items[tmp].placement & PL_SHIELD) == 0 {
                                false
                            } else {
                                // Check if right hand has two-handed weapon
                                let rhand_item = characters[cn].worn[WN_RHAND] as usize;
                                if rhand_item != 0
                                    && (items[rhand_item].placement & PL_TWOHAND) != 0
                                {
                                    false
                                } else {
                                    true
                                }
                            }
                        }
                        WN_RHAND => {
                            if (items[tmp].placement & PL_WEAPON) == 0 {
                                false
                            } else if (items[tmp].placement & PL_TWOHAND) != 0
                                && characters[cn].worn[WN_LHAND] != 0
                            {
                                false
                            } else {
                                true
                            }
                        }
                        WN_CLOAK => (items[tmp].placement & PL_CLOAK) != 0,
                        WN_RRING | WN_LRING => (items[tmp].placement & PL_RING) != 0,
                        _ => false,
                    };

                    if !placement_ok {
                        return -1;
                    }

                    0 // Success
                });

                if check_result == -1 {
                    return -1;
                }
            }

            // Perform the swap
            let tmp = characters[cn].citem;
            characters[cn].citem = characters[cn].worn[n];
            characters[cn].worn[n] = tmp;

            characters[cn].set_do_update_flags();

            n as i32
        });

        result
    }

    pub fn use_labtransfer2(&self, cn: usize, co: usize) {
        // Port of use_labtransfer2 from helper.cpp
        // If cn is a companion and its master matches the corpse owner, notify master and teleport them.
        let maybe_cc = Repository::with_characters(|ch| ch[cn].data[63] as usize);
        let is_companion =
            Repository::with_characters(|ch| ch[cn].temp == core::constants::CT_COMPANION as u16);

        if is_companion && maybe_cc == Repository::with_characters(|ch| ch[co].data[0] as usize) {
            let cc = maybe_cc;
            self.do_character_log(
                cc,
                core::types::FontColor::Yellow,
                "Your Companion killed your enemy.\n",
            );
            driver::finish_laby_teleport(
                cc,
                Repository::with_characters(|ch| ch[co].data[1] as usize),
                Repository::with_characters(|ch| ch[co].data[2] as usize),
            );
            God::transfer_char(cn, 512, 512);
            log::info!("Labkeeper room solved by GC: cc={}", cc);
            return;
        }

        // If the corpse's designated killer isn't cn, inform and bail out
        let corpse_owner = Repository::with_characters(|ch| ch[co].data[0] as usize);
        if corpse_owner != cn {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "Sorry, this killing does not count, as you're not the designated killer.\n",
            );
            log::info!(
                "Sorry, this killing does not count, as you're not the designated killer (cn={})",
                cn
            );
            return;
        }

        // Finish teleport for the player
        let tx = Repository::with_characters(|ch| ch[co].data[1] as usize);
        let ty = Repository::with_characters(|ch| ch[co].data[2] as usize);
        driver::finish_laby_teleport(cn, tx, ty);
        log::info!("Solved Labkeeper Room: cn={}", cn);

        // If cn has a GC in data[64] which is sane and a companion, transfer it as well
        let cc2 = Repository::with_characters(|ch| ch[cn].data[64] as usize);
        // The C++ checks IS_SANENPC(cc) && IS_COMPANION(cc). We'll approximate by checking used/temp flags.
        if cc2 != 0 {
            let is_sane_and_companion = Repository::with_characters(|ch| {
                ch[cc2].used != core::constants::USE_EMPTY
                    && (ch[cc2].temp == core::constants::CT_COMPANION as u16)
            });
            if is_sane_and_companion {
                God::transfer_char(cc2, 512, 512);
            }
        }
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
    pub fn do_char_score(&self, cn: usize) -> i32 {
        let pts = Repository::with_characters(|characters| characters[cn].points_tot);
        let pts = if pts < 0 { 0 } else { pts } as f64;
        ((pts.sqrt() as i32) / 7) + 7
    }

    /// Port of `do_seen(int cn, char* cco)` from `svr_do.cpp`
    ///
    /// Tell when a certain player last logged on.
    ///
    /// # Arguments
    /// * `cn` - Character asking about last seen time
    /// * `target_name` - Name or ID of character to look up
    pub fn do_seen(&self, cn: usize, target_name: &str) {
        if target_name.is_empty() {
            self.do_character_log(cn, core::types::FontColor::Red, "When was WHO last seen?\n");
            return;
        }

        Repository::with_characters(|characters| {
            // Numeric lookup only for deities
            let co = if target_name.chars().next().unwrap_or('a').is_ascii_digit() {
                if (characters[cn].flags
                    & (CharacterFlags::CF_IMP | CharacterFlags::CF_GOD | CharacterFlags::CF_USURP)
                        .bits())
                    == 0
                {
                    0
                } else {
                    target_name.parse::<usize>().unwrap_or(0)
                }
            } else {
                // TODO: Implement do_lookup_char_self - for now just return 0
                log::info!(
                    "TODO: Implement do_lookup_char_self for target_name={}",
                    target_name
                );
                0
            };

            if co == 0 {
                self.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("I've never heard of {}.\n", target_name),
                );
                return;
            }

            if (characters[co].flags & CharacterFlags::CF_PLAYER.bits()) == 0 {
                let co_name = String::from_utf8_lossy(&characters[co].name)
                    .trim_matches('\0')
                    .to_string();
                self.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    &format!("{} is not a player.\n", co_name),
                );
                return;
            }

            if (characters[cn].flags & CharacterFlags::CF_GOD.bits()) == 0
                && (characters[co].flags & CharacterFlags::CF_GOD.bits()) != 0
            {
                self.do_character_log(
                    cn,
                    core::types::FontColor::Red,
                    "No one knows when the gods where last seen.\n",
                );
                return;
            }

            if (characters[cn].flags & (CharacterFlags::CF_IMP | CharacterFlags::CF_GOD).bits())
                != 0
            {
                // God view: detailed timestamp
                let last = std::cmp::max(characters[co].login_date, characters[co].logout_date);
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i32;

                let co_name = String::from_utf8_lossy(&characters[co].name)
                    .trim_matches('\0')
                    .to_string();

                // Format timestamps
                use chrono::{TimeZone, Utc};
                let last_dt = Utc.timestamp_opt(last as i64, 0).unwrap();
                let now_dt = Utc.timestamp_opt(now as i64, 0).unwrap();

                self.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!(
                        "{} was last seen on {} (time now: {})\n",
                        co_name,
                        last_dt.format("%Y-%m-%d %H:%M:%S"),
                        now_dt.format("%Y-%m-%d %H:%M:%S")
                    ),
                );

                if characters[co].used == core::constants::USE_ACTIVE
                    && (characters[co].flags & CharacterFlags::CF_INVISIBLE.bits()) == 0
                {
                    self.do_character_log(
                        cn,
                        core::types::FontColor::Yellow,
                        &format!("PS: {} is online right now!\n", co_name),
                    );
                }
            } else {
                // Normal player view: relative time
                let last_date =
                    (std::cmp::max(characters[co].login_date, characters[co].logout_date)
                        / (24 * 3600)) as i32;
                let current_date = (std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i32)
                    / (24 * 3600);
                let days = current_date - last_date;

                let when = match days {
                    0 => "earlier today".to_string(),
                    1 => "yesterday".to_string(),
                    2 => "the day before yesterday".to_string(),
                    _ => format!("{} days ago", days),
                };

                let co_name = String::from_utf8_lossy(&characters[co].name)
                    .trim_matches('\0')
                    .to_string();
                self.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("{} was last seen {}.\n", co_name, when),
                );
            }
        });
    }

    /// Port of `do_follow(int cn, char* name)` from `svr_do.cpp`
    ///
    /// Set character to follow another character.
    pub(crate) fn do_follow(&self, cn: usize, name: &str) {
        if name.is_empty() {
            let co = Repository::with_characters(|ch| ch[cn].data[10] as usize);
            if co != 0 {
                let target = Repository::with_characters(|ch| ch[co].get_name().to_string());
                self.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!(
                        "You're following {}; type '#follow self' to stop.\n",
                        target
                    ),
                );
            } else {
                self.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    "You're not following anyone.\n",
                );
            }
            return;
        }

        let co = self.do_lookup_char_self(name, cn) as usize;
        if co == 0 {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("Sorry, I cannot find {}.\n", name),
            );
            return;
        }
        if co == cn {
            self.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "Now following no one.\n",
            );
            Repository::with_characters_mut(|chars| {
                chars[cn].data[10] = 0;
                chars[cn].goto_x = 0;
            });
            return;
        }

        let invis_src = Repository::with_characters(|ch| {
            ch[co].flags & (CharacterFlags::CF_INVISIBLE.bits() | CharacterFlags::CF_NOWHO.bits())
                != 0
        });
        if invis_src {
            // approximate invis_level checks skipped
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("Sorry, I cannot find {}.\n", name),
            );
            return;
        }

        Repository::with_characters_mut(|chars| {
            chars[cn].data[10] = co as i32;
        });
        let target = Repository::with_characters(|ch| ch[co].get_name().to_string());
        self.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!("Now following {}.\n", target),
        );
    }

    /// Port of `do_ignore(int cn, char* name, int flag)` from `svr_do.cpp`
    ///
    /// Add or remove a character from the ignore list.
    pub(crate) fn do_ignore(&self, cn: usize, name: &str, flag: i32) {
        let base = if flag == 0 { 30 } else { 50 };
        if name.is_empty() {
            self.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "Your ignore group consists of:\n",
            );
            for n in base..(base + 10) {
                let co = Repository::with_characters(|ch| ch[cn].data[n] as usize);
                if co == 0 {
                    continue;
                }
                let nm = Repository::with_characters(|ch| ch[co].get_name().to_string());
                self.do_character_log(cn, core::types::FontColor::Yellow, &format!("{}\n", nm));
            }
            return;
        }

        let co = self.do_lookup_char(name) as usize;
        if co == 0 {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("Sorry, I cannot find \"{}\".\n", name),
            );
            return;
        }
        if co == cn {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "Ignoring yourself won't do you much good.\n",
            );
            return;
        }

        for n in base..(base + 10) {
            if Repository::with_characters(|ch| ch[cn].data[n] as usize) == co {
                Repository::with_characters_mut(|ch| ch[cn].data[n] = 0);
                let nm = Repository::with_characters(|ch| ch[co].get_name().to_string());
                self.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("{} removed from your ignore group.\n", nm),
                );
                return;
            }
        }

        if Repository::with_characters(|ch| (ch[co].flags & CharacterFlags::CF_PLAYER.bits()) == 0)
        {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("Sorry, {} is not a player.\n", name),
            );
            return;
        }

        for n in base..(base + 10) {
            if Repository::with_characters(|ch| ch[cn].data[n]) == 0 {
                Repository::with_characters_mut(|ch| ch[cn].data[n] = co as i32);
                let nm = Repository::with_characters(|ch| ch[co].get_name().to_string());
                self.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("{} added to your ignore group.\n", nm),
                );
                return;
            }
        }
        self.do_character_log(
            cn,
            core::types::FontColor::Red,
            "Sorry, I can only handle ten ignore group members.\n",
        );
    }

    /// Port of `do_group(int cn, char* name)` from `svr_do.cpp`
    ///
    /// Invite someone to join group or manage group membership.
    pub(crate) fn do_group(&self, cn: usize, name: &str) {
        if name.is_empty() {
            self.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "Your group consists of:\n",
            );
            let me = Repository::with_characters(|ch| ch[cn].get_name().to_string());
            self.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!("{}-15.15s ...\n", me),
            );
            return;
        }

        let co = self.do_lookup_char(name) as usize;
        if co == 0 {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("Sorry, I cannot find \"{}\".\n", name),
            );
            return;
        }
        if co == cn {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "You're automatically part of your own group.\n",
            );
            return;
        }
        if Repository::with_characters(|ch| (ch[co].flags & CharacterFlags::CF_PLAYER.bits()) == 0)
        {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                &format!("Sorry, {} is not a player.\n", name),
            );
            return;
        }

        for n in 1..10 {
            if Repository::with_characters(|ch| ch[cn].data[n] as usize) == co {
                Repository::with_characters_mut(|ch| ch[cn].data[n] = 0);
                let nm = Repository::with_characters(|ch| ch[co].get_name().to_string());
                self.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("{} removed from your group.\n", nm),
                );
                self.do_character_log(
                    co,
                    core::types::FontColor::Red,
                    &format!(
                        "You are no longer part of {}'s group.\n",
                        Repository::with_characters(|ch| ch[cn].get_name().to_string())
                    ),
                );
                return;
            }
        }

        for n in 1..10 {
            if Repository::with_characters(|ch| ch[cn].data[n]) == 0 {
                Repository::with_characters_mut(|ch| ch[cn].data[n] = co as i32);
                let nm = Repository::with_characters(|ch| ch[co].get_name().to_string());
                self.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("{} added to your group.\n", nm),
                );
                self.do_character_log(
                    co,
                    core::types::FontColor::Red,
                    &format!(
                        "You are now part of {}'s group.\n",
                        Repository::with_characters(|ch| ch[cn].get_name().to_string())
                    ),
                );
                return;
            }
        }
        self.do_character_log(
            cn,
            core::types::FontColor::Red,
            "Sorry, I can only handle ten group members.\n",
        );
    }

    /// Port of `do_allow(int cn, int co)` from `svr_do.cpp`
    ///
    /// Allow another character to take items from you.
    pub(crate) fn do_allow(&self, cn: usize, co: usize) {
        Repository::with_characters_mut(|ch| ch[cn].data[core::constants::CHD_ALLOW] = co as i32);
        if co != 0 {
            let name = Repository::with_characters(|ch| ch[co].get_name().to_string());
            self.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!("{} is now allowed to access your grave.\n", name),
            );
        } else {
            self.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "Nobody may now access your grave.\n",
            );
        }
    }

    /// Port of `do_mark(int cn, int co, char* msg)` from `svr_do.cpp`
    ///
    /// Mark a character for tracking or special handling.
    pub(crate) fn do_mark(&self, cn: usize, co: usize, msg: &str) {
        if !core::types::Character::is_sane_character(co) {
            self.do_character_log(cn, core::types::FontColor::Red, "That's not a player\n");
            return;
        }
        if msg.is_empty() {
            Repository::with_characters_mut(|ch| ch[co].text[3] = [0; 160]);
            let old = Repository::with_characters(|ch| {
                String::from_utf8_lossy(&ch[co].text[3]).to_string()
            });
            self.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!(
                    "Removed mark \"{}\" from {}\n",
                    old,
                    Repository::with_characters(|ch| ch[co].get_name().to_string())
                ),
            );
            return;
        }
        let mut buf = [0u8; 160];
        let bytes = msg.as_bytes();
        for i in 0..std::cmp::min(bytes.len(), 159) {
            buf[i] = bytes[i];
        }
        Repository::with_characters_mut(|ch| ch[co].text[3] = buf);
        self.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!(
                "Marked {} with \"{}\"\n",
                Repository::with_characters(|ch| ch[co].get_name().to_string()),
                msg
            ),
        );
    }

    /// Port of `do_afk(int cn, char* msg)` from `svr_do.cpp`
    ///
    /// Set or clear AFK status with optional message.
    pub(crate) fn do_afk(&self, cn: usize, msg: &str) {
        Repository::with_characters_mut(|ch| {
            if ch[cn].data[core::constants::CHD_AFK] != 0 {
                ch[cn].data[core::constants::CHD_AFK] = 0;
                self.do_character_log(cn, core::types::FontColor::Yellow, "Back.\n");
            } else {
                ch[cn].data[core::constants::CHD_AFK] = 1;
                if !msg.is_empty() {
                    self.do_character_log(
                        cn,
                        core::types::FontColor::Yellow,
                        "Away. Use #afk again to show you're back. Message:\n",
                    );
                    let bytes = msg.as_bytes();
                    for i in 0..std::cmp::min(bytes.len(), 48) {
                        ch[cn].text[0][i] = bytes[i];
                    }
                    self.do_character_log(
                        cn,
                        core::types::FontColor::Blue,
                        &format!("  \"{}\"\n", String::from_utf8_lossy(&ch[cn].text[0])),
                    );
                } else {
                    self.do_character_log(
                        cn,
                        core::types::FontColor::Yellow,
                        "Away. Use #afk again to show you're back.\n",
                    );
                    ch[cn].text[0][0] = 0;
                }
            }
        });
    }

    /// Port of `do_help(int cn, char* topic)` from `svr_do.cpp`
    ///
    /// Display help information for a topic.
    pub(crate) fn do_help(&self, cn: usize, _topic: &str) {
        self.do_character_log(
            cn,
            core::types::FontColor::Green,
            "The following commands are available:\n",
        );
        self.do_character_log(
            cn,
            core::types::FontColor::Green,
            " #afk <message>         away from keyboard.\n",
        );
        self.do_character_log(
            cn,
            core::types::FontColor::Green,
            " #allow <player>        to access your grave.\n",
        );
        self.do_character_log(
            cn,
            core::types::FontColor::Green,
            " #gold <amount>         get X gold coins.\n",
        );
        self.do_character_log(
            cn,
            core::types::FontColor::Green,
            " #group <player>        group with player.\n",
        );
        self.do_character_log(
            cn,
            core::types::FontColor::Green,
            " #who                   see who's online.\n",
        );
    }

    /// Port of `do_fightback(int cn)` from `svr_do.cpp`
    ///
    /// Toggle automatic fight-back when attacked.
    pub(crate) fn do_fightback(&self, cn: usize) {
        Repository::with_characters_mut(|chars| {
            if chars[cn].data[11] != 0 {
                chars[cn].data[11] = 0;
                self.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    "Auto-Fightback enabled.\n",
                );
            } else {
                chars[cn].data[11] = 1;
                self.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    "Auto-Fightback disabled.\n",
                );
            }
        });
    }
}
