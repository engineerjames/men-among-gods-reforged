use core::constants::{GF_CLOSEENEMY, GF_LOOTING, GF_MAYHEM, GF_SPEEDY};
use core::string_operations::c_string_to_str;
use core::types::FontColor;

use crate::effect::EffectManager;
use crate::enums::CharacterFlags;
use crate::god::God;
use crate::player::cl_list;
use crate::repository::Repository;
use crate::state::State;
use crate::{driver, helpers};

impl State {
    /// Creates a note item with custom text for the character.
    ///
    /// # Arguments
    ///
    /// * `cn` - Character number (index)
    /// * `text` - The note text to create
    pub(crate) fn do_create_note(&self, cn: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        if text.len() >= 199 {
            return;
        }

        log::info!("created note: {}.", text);

        for m in 0..40 {
            let slot = Repository::with_characters(|chars| chars[cn].item[m]);
            if slot == 0 {
                if let Some(in_idx) = God::create_item(132) {
                    Repository::with_items_mut(|items| {
                        items[in_idx].temp = 0;
                        items[in_idx].description = [0; 200];
                        let bytes = text.as_bytes();
                        let length_to_copy =
                            std::cmp::min(bytes.len(), items[in_idx].description.len());
                        items[in_idx].description[..length_to_copy]
                            .copy_from_slice(&bytes[..length_to_copy]);

                        items[in_idx].flags |= core::constants::ItemFlags::IF_NOEXPIRE.bits();
                        items[in_idx].carried = cn as u16;
                    });
                    Repository::with_characters_mut(|chars| {
                        chars[cn].item[m] = in_idx as u32;
                        chars[cn].set_do_update_flags();
                    });
                    self.do_update_char(cn);
                    return;
                }
            }
        }

        self.do_character_log(
            cn,
            core::types::FontColor::Red,
            "You failed to create a note. Inventory is full!\n",
        );
    }

    /// Performs an emote action for the character, broadcasting to the area.
    ///
    /// # Arguments
    ///
    /// * `cn` - Character number (index)
    /// * `text` - The emote text
    pub(crate) fn do_emote(&self, cn: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        if text.contains('%') {
            return;
        }

        let shutup =
            Repository::with_characters(|ch| (ch[cn].flags & CharacterFlags::ShutUp.bits()) != 0);
        let invis = Repository::with_characters(|ch| {
            (ch[cn].flags & CharacterFlags::Invisible.bits()) != 0
        });

        if shutup {
            self.do_character_log(cn, core::types::FontColor::Red, "You feel guilty.\n");
            log::info!("emote: feels guilty ({})", text);
        } else if invis {
            self.do_area_log(
                0,
                0,
                Repository::with_characters(|ch| ch[cn].x as i32),
                Repository::with_characters(|ch| ch[cn].y as i32),
                core::types::FontColor::Green,
                &format!("Somebody {}.\n", text),
            );
            log::info!("emote(inv): {}", text);
        } else {
            let name = Repository::with_characters(|ch| ch[cn].get_name().to_string());
            self.do_area_log(
                0,
                0,
                Repository::with_characters(|ch| ch[cn].x as i32),
                Repository::with_characters(|ch| ch[cn].y as i32),
                core::types::FontColor::Green,
                &format!("{} {}.\n", name, text),
            );
            log::info!("emote: {}", text);
        }
    }

    /// Port of `do_become_skua(int cn)` from `svr_do.cpp`
    ///
    /// Transform character into a Skua.
    pub(crate) fn do_become_skua(&self, cn: usize) {
        // Ported from svr_do.cpp
        let is_purple = Repository::with_characters(|characters| {
            (characters[cn].kindred & core::constants::KIN_PURPLE as i32) != 0
        });

        if !is_purple {
            self.do_character_log(cn, FontColor::Red, "Hmm. Nothing happened.\n");
            return;
        }

        let ticker = Repository::with_globals(|globals| globals.ticker);
        let attack_time = Repository::with_characters(|characters| {
            characters[cn].data[core::constants::CHD_ATTACKTIME]
        });

        let days = (ticker - attack_time) / (60 * core::constants::TICKS) / 60 / 24;
        if days < 30 {
            self.do_character_log(
                cn,
                FontColor::Red,
                &format!("You have {} days of penance left.\n", 30 - days),
            );
            return;
        }

        self.do_character_log(cn, FontColor::Red, " \n");
        self.do_character_log(
            cn,
            FontColor::Red,
            "You feel the presence of a god again. You feel protected.  Your desire to kill subsides.\n",
        );
        self.do_character_log(cn, FontColor::Red, " \n");
        self.do_character_log(
            cn,
            FontColor::Red,
            "\"THE GOD SKUA WELCOMES YOU, MORTAL! YOUR BONDS OF SLAVERY ARE BROKEN!\"\n",
        );
        self.do_character_log(cn, FontColor::Red, " \n");
        self.do_character_log(cn, FontColor::Green, "Player killing flag cleared.\n");
        self.do_character_log(cn, FontColor::Red, " \n");

        let (x, y) = Repository::with_characters(|characters| (characters[cn].x, characters[cn].y));

        Repository::with_characters_mut(|characters| {
            characters[cn].kindred &= !(core::constants::KIN_PURPLE as i32);
            characters[cn].data[core::constants::CHD_ATTACKTIME] = 0;
            characters[cn].data[core::constants::CHD_ATTACKVICT] = 0;
            characters[cn].temple_x = 512;
            characters[cn].temple_y = 512;
        });

        chlog!(cn, "Converted to skua. ({} days elapsed)", days);

        EffectManager::fx_add_effect(5, 0, x as i32, y as i32, 0);
    }

    /// Port of `do_make_soulstone(int cn, int cexp)` from `svr_do.cpp`
    ///
    /// Create a soulstone item.
    pub(crate) fn do_make_soulstone(&self, cn: usize, cexp: i32) {
        if let Some(in_idx) = God::create_item(1146) {
            let rank = crate::helpers::points2rank(cexp as u32);

            Repository::with_items_mut(|items| {
                let it = &mut items[in_idx];

                // set name
                it.name = [0; 40];
                for (i, &b) in b"Soulstone".iter().enumerate() {
                    it.name[i] = b;
                }

                // set reference
                it.reference = [0; 40];
                for (i, &b) in b"soulstone".iter().enumerate() {
                    it.reference[i] = b;
                }

                // set description
                let desc = format!("Level {} soulstone, holding {} exp.", rank, cexp);
                it.description = [0; 200];
                let desc_bytes = desc.as_bytes();
                let length_to_copy = std::cmp::min(desc_bytes.len(), it.description.len());
                it.description[..length_to_copy].copy_from_slice(&desc_bytes[..length_to_copy]);

                it.data[0] = rank;
                it.data[1] = cexp as u32;
                it.temp = 0;
                it.driver = 68;
            });

            God::give_character_item(cn, in_idx);
        }
    }

    /// Port of `do_list_all_flags(int cn, u64 flag)` from `svr_do.cpp`
    ///
    /// List all characters with a specific flag.
    pub(crate) fn do_list_all_flags(&self, cn: usize, flag: u64) {
        for n in 1..core::constants::MAXCHARS {
            let show = Repository::with_characters(|chars| {
                let ch = &chars[n];
                if ch.used == core::constants::USE_EMPTY {
                    return false;
                }
                if (ch.flags & CharacterFlags::Player.bits()) == 0 {
                    return false;
                }
                (ch.flags & flag) != 0
            });

            if show {
                let name = Repository::with_characters(|ch| ch[n].get_name().to_string());
                self.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("{:04} {}\n", n, name),
                );
            }
        }
    }

    /// Port of `do_list_net(int cn, int co)` from `svr_do.cpp`
    ///
    /// List network information for a character.
    pub(crate) fn do_list_net(&self, cn: usize, co: usize) {
        let header = Repository::with_characters(|chars| {
            format!(
                "{} is know to log on from the following addresses:\n",
                chars[co].get_name()
            )
        });
        self.do_character_log(cn, core::types::FontColor::Yellow, &header);

        for n in 80..90 {
            let ip = Repository::with_characters(|chars| chars[co].data[n]);
            let a = (ip & 255) as u8;
            let b = ((ip >> 8) & 255) as u8;
            let c = ((ip >> 16) & 255) as u8;
            let d = ((ip >> 24) & 255) as u8;
            self.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!("{}.{}.{}.{}\n", a, b, c, d),
            );
        }
    }

    /// Port of `do_respawn(int cn, int co)` from `svr_do.cpp`
    ///
    /// Admin command to respawn a character.
    pub(crate) fn do_respawn(&self, cn: usize, co: usize) {
        if !(1..core::constants::MAXTCHARS).contains(&co) {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "That template number is a bit strange, don't you think so, dude?\n",
            );
            return;
        }
        Repository::with_globals_mut(|globals| {
            globals.reset_char = co as i32;
        });
    }

    /// Port of `do_npclist(int cn, char* name)` from `svr_do.cpp`
    ///
    /// List NPCs matching a name pattern.
    pub(crate) fn do_npclist(&self, cn: usize, name: &str) {
        if name.is_empty() {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "Gimme a name to work with, dude!\n",
            );
            return;
        }
        if name.len() < 3 || name.len() > 35 {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "What kind of name is that, dude?\n",
            );
            return;
        }

        let mut foundalive = 0;
        let mut foundtemp = 0;

        for n in 1..core::constants::MAXCHARS {
            let matched = Repository::with_characters(|chars| {
                let ch = &chars[n];
                if ch.used == core::constants::USE_EMPTY {
                    return false;
                }
                if (ch.flags & CharacterFlags::Player.bits()) != 0 {
                    return false;
                }
                ch.get_name().to_lowercase().contains(&name.to_lowercase())
            });

            if matched {
                foundalive += 1;
                let (n_name, n_desc) = Repository::with_characters(|chars| {
                    (
                        chars[n].get_name().to_string(),
                        c_string_to_str(&chars[n].description).to_string(),
                    )
                });
                self.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("C{:4} {:20.20} {:.20}\n", n, n_name, n_desc),
                );
            }
        }

        for n in 1..core::constants::MAXTCHARS {
            let matched = Repository::with_character_templates(|temps| {
                if temps[n].used == core::constants::USE_EMPTY {
                    return false;
                }
                if (temps[n].flags & CharacterFlags::Player.bits()) != 0 {
                    return false;
                }
                let name_s = c_string_to_str(&temps[n].name);
                name_s.to_lowercase().contains(&name.to_lowercase())
            });

            if matched {
                foundtemp += 1;
                let (t_name, t_desc) = Repository::with_character_templates(|temps| {
                    let name_s = temps[n].get_name().to_string();
                    let desc_s = c_string_to_str(&temps[n].description).to_string();
                    (name_s, desc_s)
                });
                self.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("T{:4} {:20.20} {:.20}\n", n, t_name, t_desc),
                );
            }
        }

        if foundalive != 0 || foundtemp != 0 {
            self.do_character_log(cn, core::types::FontColor::Yellow, " \n");
        }
        self.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!(
                "{} characters, {} templates by that name\n",
                foundalive, foundtemp
            ),
        );
    }

    /// Port of `do_leave(int cn)` from `svr_do.cpp`
    ///
    /// Make character leave current location/mode.
    pub(crate) fn do_leave(&self, cn: usize) {
        let name = Repository::with_characters(|ch| ch[cn].get_name().to_string());
        self.do_announce(cn, 0, &format!("{} left the game.\n", name));
        Repository::with_characters_mut(|characters| {
            characters[cn].flags |= CharacterFlags::NoWho.bits() | CharacterFlags::Invisible.bits();
        });
    }

    /// Port of `do_enter(int cn)` from `svr_do.cpp`
    ///
    /// Make character enter a location/mode.
    pub(crate) fn do_enter(&self, cn: usize) {
        Repository::with_characters_mut(|characters| {
            characters[cn].flags &=
                !(CharacterFlags::NoWho.bits() | CharacterFlags::Invisible.bits());
        });
        let name = Repository::with_characters(|ch| ch[cn].get_name().to_string());
        self.do_announce(cn, 0, &format!("{} entered the game.\n", name));
    }

    /// Port of `do_stat(int cn)` from `svr_do.cpp`
    ///
    /// Display character statistics.
    pub(crate) fn do_stat(&self, cn: usize) {
        Repository::with_globals(|globals| {
            self.do_character_log(
                cn,
                core::types::FontColor::Blue,
                &format!("items: {}/{}\n", globals.item_cnt, core::constants::MAXITEM),
            );
            self.do_character_log(
                cn,
                core::types::FontColor::Blue,
                &format!(
                    "chars: {}/{}\n",
                    globals.character_cnt,
                    core::constants::MAXCHARS
                ),
            );
            self.do_character_log(
                cn,
                core::types::FontColor::Blue,
                &format!(
                    "effes: {}/{}\n",
                    globals.effect_cnt,
                    core::constants::MAXEFFECT
                ),
            );

            self.do_character_log(
                cn,
                core::types::FontColor::Blue,
                &format!("newmoon={}\n", globals.newmoon),
            );
            self.do_character_log(
                cn,
                core::types::FontColor::Blue,
                &format!("fullmoon={}\n", globals.fullmoon),
            );
            self.do_character_log(
                cn,
                core::types::FontColor::Blue,
                &format!("mdday={} (%28={})\n", globals.mdday, globals.mdday % 28),
            );

            self.do_character_log(
                cn,
                core::types::FontColor::Blue,
                &format!(
                    "mayhem={}, looting={}, close={}, cap={}, speedy={}\n",
                    if (globals.flags & core::constants::GF_MAYHEM) != 0 {
                        "yes"
                    } else {
                        "no"
                    },
                    if (globals.flags & core::constants::GF_LOOTING) != 0 {
                        "yes"
                    } else {
                        "no"
                    },
                    if (globals.flags & core::constants::GF_CLOSEENEMY) != 0 {
                        "yes"
                    } else {
                        "no"
                    },
                    if (globals.flags & core::constants::GF_CAP) != 0 {
                        "yes"
                    } else {
                        "no"
                    },
                    if (globals.flags & core::constants::GF_SPEEDY) != 0 {
                        "yes"
                    } else {
                        "no"
                    }
                ),
            );
        });
    }

    /// Port of `do_become_purple(int cn)` from `svr_do.cpp`
    ///
    /// Transform character into Purple faction.
    pub(crate) fn do_become_purple(&self, cn: usize) {
        // Ported from svr_do.cpp
        let ticker = Repository::with_globals(|globals| globals.ticker);
        let last = Repository::with_characters(|characters| {
            characters[cn].data[core::constants::CHD_RIDDLER]
        });
        let is_purple = Repository::with_characters(|characters| {
            (characters[cn].kindred & core::constants::KIN_PURPLE as i32) != 0
        });

        if ticker - last < core::constants::TICKS * 60 && !is_purple {
            self.do_character_log(cn, FontColor::Red, " \n");
            self.do_character_log(
                cn,
                FontColor::Red,
                "You feel a god leave you. You feel alone. Scared. Unprotected.\n",
            );
            self.do_character_log(cn, FontColor::Red, " \n");
            self.do_character_log(
                cn,
                FontColor::Red,
                "Another presence enters your mind. You feel hate. Lust. Rage. A Purple Cloud engulfs you.\n",
            );
            self.do_character_log(cn, FontColor::Red, " \n");
            self.do_character_log(
                cn,
                FontColor::Red,
                "\"THE GOD OF THE PURPLE WELCOMES YOU, MORTAL! MAY YOU BE A GOOD SLAVE!\"\n",
            );
            self.do_character_log(cn, FontColor::Red, " \n");
            self.do_character_log(
                cn,
                FontColor::Green,
                "Player killing flag set. May you enjoy the killing.\n",
            );
            self.do_character_log(cn, FontColor::Red, " \n");

            let (x, y) =
                Repository::with_characters(|characters| (characters[cn].x, characters[cn].y));

            Repository::with_characters_mut(|characters| {
                characters[cn].kindred |= core::constants::KIN_PURPLE as i32;
                characters[cn].temple_x = 558;
                characters[cn].temple_y = 542;
            });

            self.do_update_char(cn);

            chlog!(cn, "Converted to purple. ({} days elapsed)", 0);

            EffectManager::fx_add_effect(5, 0, x as i32, y as i32, 0);
        } else {
            self.do_character_log(cn, FontColor::Red, "Hmm. Nothing happened.\n");
        }
    }

    /// Port of `do_command(int cn, char* ptr)` from `svr_do.cpp`
    ///
    /// Process a command from a character.
    pub(crate) fn do_command(&mut self, cn: usize, ptr: &str) {
        // Tokenize up to 10 args. Mimics the original C++ behaviour: quoted tokens
        // or alnum tokens, and `args[n]` points to the remainder starting at next token.
        let mut arg: [String; 10] = Default::default();
        let mut args: [Option<&str>; 10] = [None; 10];

        let mut pos = 0usize;
        let bytes = ptr.as_bytes();
        let len = bytes.len();

        for n in 0..10 {
            // skip initial whitespace
            while pos < len && bytes[pos].is_ascii_whitespace() {
                pos += 1;
            }
            if pos >= len {
                break;
            }

            let mut token = String::new();
            if bytes[pos] == b'"' {
                // quoted
                pos += 1;
                while pos < len && bytes[pos] != b'"' && token.len() < 39 {
                    token.push(bytes[pos] as char);
                    pos += 1;
                }
                if pos < len && bytes[pos] == b'"' {
                    pos += 1;
                }
            } else {
                while pos < len && (bytes[pos] as char).is_ascii_alphanumeric() && token.len() < 39
                {
                    token.push(bytes[pos] as char);
                    pos += 1;
                }
            }

            // skip whitespace after token
            while pos < len && bytes[pos].is_ascii_whitespace() {
                pos += 1;
            }

            arg[n] = token;

            if pos < len {
                // Point to remainder starting at this position
                args[n] = Some(&ptr[pos..]);
            } else {
                args[n] = None;
            }

            if pos >= len {
                break;
            }
        }

        let cmd = arg[0].to_lowercase();

        // Read flags for this character
        let (f_gg, f_c, f_g, f_i, f_s, f_p, f_u, f_sh, f_pol) =
            Repository::with_characters(|characters| {
                let flags = characters[cn].flags;
                (
                    (flags & core::constants::CharacterFlags::CF_GREATERGOD.bits()) != 0,
                    (flags & core::constants::CharacterFlags::CF_CREATOR.bits()) != 0,
                    (flags & core::constants::CharacterFlags::CF_GOD.bits()) != 0,
                    (flags & core::constants::CharacterFlags::CF_IMP.bits()) != 0,
                    (flags & core::constants::CharacterFlags::CF_STAFF.bits()) != 0,
                    (flags & core::constants::CharacterFlags::CF_PLAYER.bits()) != 0,
                    (flags & core::constants::CharacterFlags::CF_USURP.bits()) != 0,
                    (flags & core::constants::CharacterFlags::CF_SHUTUP.bits()) != 0,
                    (flags
                        & (core::constants::CharacterFlags::CF_POH_LEADER.bits()
                            | core::constants::CharacterFlags::CF_GOD.bits()))
                        != 0,
                )
            });

        let f_m = !f_p;
        let f_gi = f_g || f_i;
        let f_giu = f_gi || f_u;
        let f_gius = f_giu || f_s;

        // helper closures
        let starts = |s: &str| cmd.starts_with(s);
        let arg_get = |i: usize| arg.get(i).map(|s| s.as_str()).unwrap_or("");
        let args_get = |i: usize| args.get(i).and_then(|o| *o).unwrap_or("");
        let parse_usize = |s: &str| s.parse::<usize>().unwrap_or(0usize);
        let parse_i32 = |s: &str| s.parse::<i32>().unwrap_or(0i32);

        let first = cmd.chars().next().unwrap_or('\0');

        log::debug!("Command received from {}: cmd={} ptr={}", cn, cmd, ptr);

        match first {
            'a' => {
                if starts("afk") && f_p {
                    log::debug!("Processing afk command for {}", cn);
                    self.do_afk(cn, args_get(0));
                    return;
                }
                if starts("allow") && f_p {
                    log::debug!("Processing allow command for {}", cn);
                    self.do_allow(cn, parse_usize(arg_get(1)));
                    return;
                }
                if starts("announce") && f_gius {
                    log::debug!("Processing announce command for {}", cn);
                    self.do_announce(cn, cn, args_get(0));
                    return;
                }
                if starts("addban") && f_gi {
                    log::debug!("Processing addban command for {}", cn);
                    God::add_ban(cn, parse_usize(arg_get(1)));
                    return;
                }
            }
            'b' => {
                if starts("bow") && !f_sh {
                    log::debug!("Processing bow command for {}", cn);
                    Repository::with_characters_mut(|characters| {
                        characters[cn].misc_action = core::constants::DR_BOW as u16;
                    });
                    return;
                }
                if starts("balance") && !f_m {
                    log::debug!("Processing balance command for {}", cn);
                    self.do_balance(cn);
                    return;
                }
                if starts("black") && f_g {
                    log::debug!("Processing black command for {}", cn);
                    God::set_flag(
                        cn,
                        arg_get(1),
                        core::constants::CharacterFlags::CF_BLACK.bits(),
                    );
                    return;
                }
                if starts("build") && f_c {
                    log::debug!("Processing build command for {}", cn);
                    God::build(cn, parse_i32(arg_get(1)) as u32);
                    return;
                }
            }
            'c' => {
                if starts("cap") && f_g {
                    // TODO: `set_cap(int cn,int nr)` from original C++
                    // Original call: set_cap(cn, atoi(arg[1]));
                    // Not implemented elsewhere in Rust yet; preserve as TODO.
                    log::warn!("TODO: set_cap not implemented - call set_cap({}, arg1)", cn);
                    self.do_character_log(cn, FontColor::Red, "cap command not implemented\n");
                    return;
                }
                if starts("caution") && f_gius {
                    log::debug!("Processing caution command for {}", cn);
                    self.do_caution(cn, cn, args_get(0));
                    return;
                }
                if starts("ccp") && f_i {
                    log::debug!("Processing ccp command for {}", cn);
                    God::set_flag(
                        cn,
                        arg_get(1),
                        core::constants::CharacterFlags::CF_CCP.bits(),
                    );
                    return;
                }
                if starts("closenemey") && f_g {
                    log::debug!("Processing closeenemy command for {}", cn);
                    God::set_gflag(cn, GF_CLOSEENEMY);
                    return;
                }
                if starts("create") && f_g {
                    log::debug!("Processing create command for {}", cn);
                    God::create(cn, parse_i32(arg_get(1)));
                    return;
                }
                if starts("creator") && f_gg {
                    log::debug!("Processing creator command for {}", cn);
                    God::set_flag(
                        cn,
                        arg_get(1),
                        core::constants::CharacterFlags::CF_CREATOR.bits(),
                    );
                    return;
                }
            }
            'd' => {
                if starts("deposit") && !f_m {
                    log::debug!("Processing deposit command for {}", cn);
                    self.do_deposit(cn, parse_i32(arg_get(1)), parse_i32(arg_get(2)));
                    return;
                }
                if starts("depot") && !f_m {
                    log::debug!("Processing depot command for {}", cn);
                    self.do_depot(cn);
                    return;
                }
                if starts("delban") && f_giu {
                    log::debug!("Processing delban command for {}", cn);
                    God::del_ban(cn, parse_usize(arg_get(1)));
                    return;
                }
                if starts("diffi") && f_g {
                    // TODO: Intentionally left unimplemented - wtf was this for?
                    log::warn!("TODO: diffi command not implemented - original purpose unclear");
                    return;
                }
            }
            'e' => {
                if starts("effect") && f_g {
                    // TODO: `effectlist(int cn)` from original C++
                    // Original call: effectlist(cn);
                    // No Rust equivalent found; leave TODO for later implementation.
                    log::warn!(
                        "TODO: effectlist not implemented - would list active effects for {}",
                        cn
                    );
                    self.do_character_log(cn, FontColor::Red, "effectlist not implemented\n");
                    return;
                }
                if starts("emote") {
                    log::debug!("Processing emote command for {}", cn);
                    self.do_emote(cn, args_get(0));
                    return;
                }
                if starts("enemy") && f_giu {
                    log::debug!("Processing enemy command for {}", cn);
                    self.do_enemy(cn, arg_get(1), arg_get(2));
                    return;
                }
                if starts("enter") && f_gi {
                    log::debug!("Processing enter command for {}", cn);
                    self.do_enter(cn);
                    return;
                }
                if starts("exit") && f_u {
                    log::debug!("Processing exit command for {}", cn);
                    God::exit_usurp(cn);
                    return;
                }
                if starts("eras") && f_g {
                    return; // to avoid ambiguity with "erase"
                }
                if starts("erase") && f_g {
                    log::debug!("Processing erase command for {}", cn);
                    God::erase(cn, parse_usize(arg_get(1)), 0);
                    return;
                }
            }
            'f' => {
                if starts("fightback") {
                    log::debug!("Processing fightback command for {}", cn);
                    self.do_fightback(cn);
                    return;
                }
                if starts("follow") && !f_m {
                    log::debug!("Processing follow command for {}", cn);
                    self.do_follow(cn, args_get(0));
                    return;
                }
                if starts("force") && f_giu {
                    log::debug!("Processing force command for {}", cn);
                    God::force(cn, arg_get(1), args_get(1));
                    return;
                }
            }
            'g' => {
                if starts("gtell") && !f_m {
                    log::debug!("Processing gtell command for {}", cn);
                    self.do_gtell(cn, args_get(0));
                    return;
                }
                if starts("gold") {
                    log::debug!("Processing gold command for {}", cn);
                    self.do_gold(cn, parse_i32(arg_get(1)));
                    return;
                }
                if starts("golden") && f_g {
                    log::debug!("Processing golden command for {}", cn);
                    God::set_flag(
                        cn,
                        arg_get(1),
                        core::constants::CharacterFlags::CF_GOLDEN.bits(),
                    );
                    return;
                }
                if starts("group") && !f_m {
                    log::debug!("Processing group command for {}", cn);
                    self.do_group(cn, args_get(0));
                    return;
                }
                if starts("gargoyle") && f_gi {
                    log::debug!("Processing gargoyle command for {}", cn);
                    God::gargoyle(cn);
                    return;
                }
                if starts("ggold") && f_g {
                    log::debug!("Processing ggold command for {}", cn);
                    God::gold_char(
                        cn,
                        parse_usize(arg_get(1)),
                        parse_i32(arg_get(2)),
                        parse_i32(arg_get(3)),
                    );
                    return;
                }
                if starts("give") && f_giu {
                    log::debug!("Processing give command for {}", cn);
                    self.do_god_give(cn, parse_usize(arg_get(1)));
                    return;
                }
                if starts("goto") && f_giu {
                    log::debug!("Processing goto command for {}", cn);
                    God::goto(cn, cn, arg_get(1), arg_get(2));
                    return;
                }
                if starts("god") && f_g {
                    log::debug!("Processing god command for {}", cn);
                    God::set_flag(
                        cn,
                        arg_get(1),
                        core::constants::CharacterFlags::CF_GOD.bits(),
                    );
                    return;
                }
                if starts("greatergod") && f_gg {
                    log::debug!("Processing greatergod command for {}", cn);
                    God::set_flag(
                        cn,
                        arg_get(1),
                        core::constants::CharacterFlags::CF_GREATERGOD.bits(),
                    );
                    return;
                }

                if starts("greaterinv") && f_gg {
                    log::debug!("Processing greaterinv command for {}", cn);
                    God::set_flag(
                        cn,
                        arg_get(1),
                        core::constants::CharacterFlags::CF_GREATERINV.bits(),
                    );
                    return;
                }

                if starts("grolm") && f_gi {
                    log::debug!("Processing grolm command for {}", cn);
                    God::grolm(cn);
                    return;
                }

                if starts("grolminfo") && f_gi {
                    log::debug!("Processing grolminfo command for {}", cn);
                    God::grolm_info(cn);
                    return;
                }

                if starts("grolmstart") && f_g {
                    log::debug!("Processing grolmstart command for {}", cn);
                    God::grolm_start(cn);
                    return;
                }
            }
            'h' => {
                if starts("help") {
                    log::debug!("Processing help command for {}", cn);
                    self.do_help(cn, arg_get(1));
                    return;
                }
            }
            'i' => {
                if starts("ignore") && !f_m {
                    log::debug!("Processing ignore command for {}", cn);
                    self.do_ignore(cn, arg_get(1), 0);
                    return;
                }
                if starts("iignore") && !f_m {
                    log::debug!("Processing iignore command for {}", cn);
                    self.do_ignore(cn, arg_get(1), 1);
                    return;
                }
                if starts("iinfo") && f_g {
                    log::debug!("Processing iinfo command for {}", cn);
                    God::iinfo(cn, parse_usize(arg_get(1)));
                    return;
                }
                if (starts("immortal") || starts("imm")) && f_u {
                    log::debug!("Processing immortal command for {}", cn);
                    God::set_flag(
                        cn,
                        arg_get(1),
                        core::constants::CharacterFlags::CF_IMMORTAL.bits(),
                    );
                    return;
                }
                if (starts("immortal") || starts("imm")) && f_g {
                    log::debug!("Processing god-immortal command for {}", cn);
                    God::set_flag(
                        cn,
                        arg_get(1),
                        core::constants::CharacterFlags::CF_IMMORTAL.bits(),
                    );
                    return;
                }
                if starts("imp") && f_g {
                    log::debug!("Processing imp command for {}", cn);
                    God::set_flag(
                        cn,
                        arg_get(1),
                        core::constants::CharacterFlags::CF_IMP.bits(),
                    );
                    return;
                }
                if starts("info") && f_gius {
                    log::debug!("Processing info command for {}", cn);
                    God::info(cn, parse_usize(arg_get(1)));
                    return;
                }
                if starts("init") && f_g {
                    log::warn!("TODO: init command not implemented -- this used to init badwords but we do it differently now.");
                    self.do_character_log(cn, FontColor::Green, "Done.\n");
                    return;
                }
                if (starts("infrared") || starts("infra")) && f_giu {
                    log::debug!("Processing infrared command for {}", cn);
                    God::set_flag(
                        cn,
                        arg_get(1),
                        core::constants::CharacterFlags::CF_INFRARED.bits(),
                    );
                    return;
                }
                if (starts("invisible") || starts("inv")) && f_giu {
                    log::debug!("Processing invisible command for {}", cn);
                    God::set_flag(
                        cn,
                        arg_get(1),
                        core::constants::CharacterFlags::CF_INVISIBLE.bits(),
                    );
                    return;
                }
                if starts("ipshow") && f_giu {
                    log::debug!("Processing ipshow command for {}", cn);
                    self.do_list_net(cn, parse_usize(arg_get(1)));
                    return;
                }
                if starts("itell") && f_giu {
                    log::debug!("Processing itell command for {}", cn);
                    self.do_itell(cn, args_get(0));
                    return;
                }
            }
            'k' => {
                if starts("kick") && f_giu {
                    log::debug!("Processing kick command for {}", cn);
                    God::kick(cn, parse_usize(arg_get(1)));
                    return;
                }
            }
            'l' => {
                if starts("lag") && !f_m {
                    log::debug!("Processing lag command for {}", cn);
                    self.do_lag(cn, parse_i32(arg_get(1)));
                    return;
                }
                if starts("leave") && f_gi {
                    log::debug!("Processing leave command for {}", cn);
                    self.do_leave(cn);
                    return;
                }
                if starts("look") && f_gius {
                    log::debug!("Processing look command for {}", cn);
                    // do_look_char expects numbers in original; use parse
                    self.do_look_char(cn, parse_usize(arg_get(1)), 1, 0, 0);
                    return;
                }
                if (starts("lookdepot") || starts("lookd")) && f_gg {
                    log::debug!("Processing lookdepot command for {}", cn);
                    self.do_look_player_depot(cn, parse_usize(arg_get(1)));
                    return;
                }
                if (starts("lookinv") || starts("looki")) && f_gg {
                    log::debug!("Processing lookinv command for {}", cn);
                    self.do_look_player_inventory(cn, parse_usize(arg_get(1)));
                    return;
                }
                if (starts("lookequip") || starts("looke")) && f_gg {
                    log::debug!("Processing lookequip command for {}", cn);
                    self.do_look_player_equipment(cn, parse_usize(arg_get(1)));
                    return;
                }
                if starts("looting") && f_g {
                    log::debug!("Processing looting command for {}", cn);
                    God::set_gflag(cn, GF_LOOTING);
                    return;
                }
                if starts("lower") && f_g {
                    log::debug!("Processing lower command for {}", cn);
                    God::lower_char(cn, parse_usize(arg_get(1)), parse_i32(arg_get(2)));
                    return;
                }
                if starts("luck") && f_giu {
                    log::debug!("Processing luck command for {}", cn);
                    God::luck(cn, parse_usize(arg_get(1)), parse_i32(arg_get(2)));
                    return;
                }
                if starts("listban") && f_giu {
                    log::debug!("Processing listban command for {}", cn);
                    God::list_ban(cn);
                    return;
                }
                if starts("listimps") && f_giu {
                    log::debug!("Processing listimps command for {}", cn);
                    God::implist(cn);
                    return;
                }
                if starts("listgolden") && f_giu {
                    log::debug!("Processing listgolden command for {}", cn);
                    self.do_list_all_flags(cn, core::constants::CharacterFlags::CF_GOLDEN.bits());
                    return;
                }
                if starts("listblack") && f_giu {
                    log::debug!("Processing listblack command for {}", cn);
                    self.do_list_all_flags(cn, core::constants::CharacterFlags::CF_BLACK.bits());
                    return;
                }
            }
            'm' => {
                if starts("mayhem") && f_g {
                    log::debug!("Processing mayhem command for {}", cn);
                    God::set_gflag(cn, GF_MAYHEM);
                    return;
                }
                if starts("mark") && f_giu {
                    log::debug!("Processing mark command for {}", cn);
                    self.do_mark(cn, parse_usize(arg_get(1)), args_get(1));
                    return;
                }
                if starts("me") {
                    log::debug!("Processing me command for {}", cn);
                    self.do_emote(cn, args_get(0));
                    return;
                }
                if starts("mirror") && f_giu {
                    log::debug!("Processing mirror command for {}", cn);
                    God::mirror(cn, arg_get(1), arg_get(2));
                    return;
                }
                if starts("mailpass") && f_g {
                    // TODO: Left unimplemented for now
                    log::warn!("TODO: mailpass command not implemented");
                    //God::mail_password(cn, arg_get(1), arg_get(2));
                    return;
                }
            }
            'n' => {
                if starts("noshout") && !f_m {
                    log::debug!("Processing noshout command for {}", cn);
                    self.do_noshout(cn);
                    return;
                }
                if starts("nostaff") && f_giu {
                    log::debug!("Processing nostaff command for {}", cn);
                    self.do_nostaff(cn);
                    return;
                }
                if starts("notell") && !f_m {
                    log::debug!("Processing notell command for {}", cn);
                    self.do_notell(cn);
                    return;
                }
                if starts("name") && f_giu {
                    log::debug!("Processing name command for {}", cn);
                    God::set_name(cn, parse_usize(arg_get(1)), args_get(1));
                    return;
                }
                if starts("nodesc") && f_giu {
                    log::debug!("Processing nodesc command for {}", cn);
                    God::reset_description(cn, parse_usize(arg_get(1)));
                    return;
                }
                if starts("nolist") && f_gi {
                    log::debug!("Processing nolist command for {}", cn);
                    God::set_flag(
                        cn,
                        arg_get(1),
                        core::constants::CharacterFlags::CF_NOLIST.bits(),
                    );
                    return;
                }
                if starts("noluck") && f_giu {
                    log::debug!("Processing noluck command for {}", cn);
                    God::luck(cn, parse_usize(arg_get(1)), -parse_i32(arg_get(2)));
                    return;
                }
                if starts("nowho") && f_gi {
                    log::debug!("Processing nowho command for {}", cn);
                    God::set_flag(
                        cn,
                        arg_get(1),
                        core::constants::CharacterFlags::CF_NOWHO.bits(),
                    );
                    return;
                }
                if starts("npclist") && f_giu {
                    log::debug!("Processing npclist command for {}", cn);
                    self.do_npclist(cn, args_get(0));
                    return;
                }
            }
            'p' => {
                if starts("password") && f_g {
                    log::debug!("Processing others-password command for {}", cn);
                    // change another's password
                    God::change_pass(cn, parse_usize(arg_get(1)), arg_get(2));
                    return;
                }
                if starts("password") {
                    log::debug!("Processing own-password command for {}", cn);
                    // change own password
                    God::change_pass(cn, cn, arg_get(1));
                    return;
                }
                if starts("pent") {
                    log::debug!("Processing pent command for {}", cn);
                    self.do_check_pent_count(cn);
                    return;
                }
                if starts("poh") && f_pol {
                    log::debug!("Processing poh command for {}", cn);
                    God::set_flag(cn, arg_get(1), CharacterFlags::Poh.bits());
                    return;
                }
                if starts("pol") && f_pol {
                    log::debug!("Processing pol command for {}", cn);
                    God::set_flag(cn, arg_get(1), CharacterFlags::PohLeader.bits());
                    return;
                }

                if starts("prof") && f_g {
                    log::debug!("Processing prof command for {}", cn);
                    God::set_flag(cn, arg_get(1), CharacterFlags::PohLeader.bits());
                    return;
                }

                if starts("purple") && !f_g && !f_m {
                    log::debug!("Processing become_purple command for {}", cn);
                    self.do_become_purple(cn);
                    return;
                }

                if starts("purple") && f_g {
                    log::debug!("Processing set_purple command for {}", cn);
                    God::set_purple(cn, parse_usize(arg_get(1)));
                    return;
                }

                if starts("perase") && f_g {
                    log::debug!("Processing perase command for {}", cn);
                    God::erase(cn, parse_usize(arg_get(1)), 1);
                    return;
                }

                if starts("pktcnt") && f_g {
                    // TODO: pkt_list();
                    log::warn!("TODO: pktcnt command not implemented - original purpose unclear");
                    return;
                }

                if starts("pktcl") && f_g {
                    log::debug!("Processing pktcl command for {}", cn);
                    cl_list();
                    return;
                }
            }
            'r' => {
                if starts("rank") {
                    log::debug!("Processing rank command for {}", cn);
                    self.do_view_exp_to_rank(cn);
                    return;
                }

                if starts("raise") && f_giu {
                    log::debug!("Processing raise command for {}", cn);
                    God::raise_char(cn, parse_usize(arg_get(1)), parse_i32(arg_get(2)));
                    return;
                }

                if starts("recall") && f_giu {
                    log::debug!("Processing recall command for {}", cn);
                    God::goto(cn, cn, "512", "512");
                    return;
                }

                if starts("respawn") && f_giu {
                    log::debug!("Processing respawn command for {}", cn);
                    self.do_respawn(cn, parse_usize(arg_get(1)));
                    return;
                }
            }
            's' => {
                if starts("shout") {
                    log::debug!("Processing shout command for {}", cn);
                    self.do_shout(cn, args_get(0));
                    return;
                }

                if starts("safe") && f_g {
                    log::debug!("Processing safe command for {}", cn);
                    God::set_flag(
                        cn,
                        arg_get(1),
                        core::constants::CharacterFlags::CF_SAFE.bits(),
                    );
                    return;
                }

                if starts("save") && f_g {
                    log::debug!("Processing save command for {}", cn);
                    God::save(cn, parse_usize(arg_get(1)));
                    return;
                }

                if starts("seen") {
                    log::debug!("Processing seen command for {}", cn);
                    self.do_seen(cn, arg_get(1));
                    return;
                }

                if starts("send") {
                    log::debug!("Processing send command for {}", cn);
                    God::goto(cn, parse_usize(arg_get(1)), arg_get(2), arg_get(3));
                    return;
                }

                if starts("shutup") && f_gius {
                    log::debug!("Processing shutup command for {}", cn);
                    God::shutup(cn, parse_usize(arg_get(1)));
                    return;
                }

                if starts("skill") && f_g {
                    log::debug!("Processing skill command for {}", cn);
                    God::skill(
                        cn,
                        parse_usize(arg_get(1)),
                        driver::skill_lookup(arg_get(2)),
                        parse_i32(arg_get(3)),
                    );
                    return;
                }

                if starts("skua") {
                    log::debug!("Processing skua command for {}", cn);
                    self.do_become_skua(cn);
                    return;
                }

                if starts("slap") && f_giu {
                    log::debug!("Processing slap command for {}", cn);
                    God::slap(cn, parse_usize(arg_get(1)));
                    return;
                }

                if starts("sort") {
                    log::debug!("Processing sort command for {}", cn);
                    self.do_sort(cn, arg_get(1));
                    return;
                }

                if starts("soulstone") && f_g {
                    log::debug!("Processing soulstone command for {}", cn);
                    self.do_make_soulstone(cn, parse_i32(arg_get(1)));
                    return;
                }

                if starts("speedy") && f_g {
                    log::debug!("Processing speedy command for {}", cn);
                    God::set_gflag(cn, GF_SPEEDY);
                    return;
                }

                if starts("spellignore") && !f_m {
                    log::debug!("Processing spellignore command for {}", cn);
                    self.do_spellignore(cn);
                    return;
                }

                if starts("sprite") && f_giu {
                    log::debug!("Processing sprite command for {}", cn);
                    God::spritechange(cn, parse_usize(arg_get(1)), parse_i32(arg_get(2)));
                    return;
                }

                if starts("stell") && f_giu {
                    log::debug!("Processing stell command for {}", cn);
                    State::with(|state| state.do_stell(cn, args_get(0)));
                    return;
                }

                if starts("stat") && f_g {
                    log::debug!("Processing stat command for {}", cn);
                    self.do_stat(cn);
                    return;
                }

                if starts("staff") && f_g {
                    log::debug!("Processing staff command for {}", cn);
                    God::set_flag(
                        cn,
                        arg_get(1),
                        core::constants::CharacterFlags::CF_STAFF.bits(),
                    );
                    return;
                }

                if starts("steal") && f_gg {
                    log::debug!("Processing steal command for {}", cn);
                    self.do_steal_player(cn, arg_get(1), arg_get(2));
                    return;
                }

                if starts("summon") && f_g {
                    log::debug!("Processing summon command for {}", cn);
                    God::summon(cn, arg_get(1), arg_get(2), arg_get(3));
                    return;
                }
            }
            't' => {
                if starts("tell") {
                    log::debug!("Processing tell command for {}", cn);
                    self.do_tell(cn, arg_get(1), args_get(1));
                    return;
                }

                if starts("tavern") && f_g && !f_m {
                    log::debug!("Processing tavern command for {}", cn);
                    God::tavern(cn);
                    return;
                }

                if starts("temple") && f_giu {
                    log::debug!("Processing temple command for {}", cn);
                    God::goto(cn, cn, "800", "800");
                    return;
                }

                if starts("thrall") && f_giu {
                    log::debug!("Processing thrall command for {}", cn);
                    God::thrall(cn, arg_get(1), arg_get(2));
                    return;
                }

                if starts("time") {
                    log::debug!("Processing time command for {}", cn);
                    helpers::show_time(cn);
                    return;
                }

                if starts("tinfo") && f_g {
                    log::debug!("Processing tinfo command for {}", cn);
                    God::tinfo(cn, parse_usize(arg_get(1)));
                    return;
                }

                if starts("top") && f_g {
                    log::debug!("Processing top command for {}", cn);
                    God::top(cn);
                    return;
                }
            }
            'u' => {
                if starts("unique") && f_g {
                    log::debug!("Processing unique command for {}", cn);
                    God::unique(cn);
                    return;
                }
                if starts("usurp") && f_giu {
                    log::debug!("Processing usurp command for {}", cn);
                    God::usurp(cn, parse_usize(arg_get(1)));
                    return;
                }
            }
            'w' => {
                if starts("who") {
                    log::debug!("Processing who command for {}", cn);
                    if f_gius {
                        God::who(cn);
                    } else {
                        God::user_who(cn);
                    }
                    return;
                }
                if starts("wave") && !f_sh {
                    log::debug!("Processing wave command for {}", cn);
                    Repository::with_characters_mut(|characters| {
                        characters[cn].misc_action = core::constants::DR_WAVE as u16;
                    });
                    return;
                }
                if starts("withdraw") && !f_m {
                    log::debug!("Processing withdraw command for {}", cn);
                    self.do_withdraw(cn, parse_i32(arg_get(1)), parse_i32(arg_get(2)));
                    return;
                }
                if starts("write") && f_giu {
                    log::debug!("Processing write command for {}", cn);
                    self.do_create_note(cn, args_get(0));
                    return;
                }
            }
            _ => {}
        }

        // Unknown command
        self.do_character_log(cn, FontColor::Red, &format!("Unknown command #{}\n", cmd));
    }
}
