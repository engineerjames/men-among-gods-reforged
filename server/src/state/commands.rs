use core::constants::{CharacterFlags, GF_CLOSEENEMY, GF_LOOTING, GF_MAYHEM, GF_SPEEDY};
use core::string_operations::c_string_to_str;
use core::types::FontColor;

use crate::effect::EffectManager;
use crate::game_state::GameState;
use crate::god::God;
use crate::{driver, helpers};

fn atoi_i32(s: &str) -> i32 {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }

    let mut sign: i64 = 1;
    if i < bytes.len() {
        if bytes[i] == b'-' {
            sign = -1;
            i += 1;
        } else if bytes[i] == b'+' {
            i += 1;
        }
    }

    let mut acc: i64 = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if !b.is_ascii_digit() {
            break;
        }
        acc = acc.saturating_mul(10).saturating_add((b - b'0') as i64);
        i += 1;
    }

    let v = acc.saturating_mul(sign);
    v.clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

fn atoi_u32(s: &str) -> u32 {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }

    // no sign for u32; match common C patterns: leading '-' yields 0
    if i < bytes.len() && (bytes[i] == b'-' || bytes[i] == b'+') {
        if bytes[i] == b'-' {
            return 0;
        }
        i += 1;
    }

    let mut acc: u64 = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if !b.is_ascii_digit() {
            break;
        }
        acc = acc.saturating_mul(10).saturating_add((b - b'0') as u64);
        i += 1;
    }

    acc.min(u32::MAX as u64) as u32
}

fn atoi_usize(s: &str) -> usize {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }

    if i < bytes.len() && (bytes[i] == b'-' || bytes[i] == b'+') {
        if bytes[i] == b'-' {
            return 0;
        }
        i += 1;
    }

    let mut acc: u128 = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if !b.is_ascii_digit() {
            break;
        }
        acc = acc.saturating_mul(10).saturating_add((b - b'0') as u128);
        i += 1;
    }

    acc.min(usize::MAX as u128) as usize
}

const ALL_COMMANDS: &'static [&str; 126] = &[
    "addban",
    "afk",
    "allow",
    "announce",
    "balance",
    "black",
    "bow",
    "build",
    "cap",
    "caution",
    "ccp",
    "closenemey",
    "create",
    "createspecial",
    "creator",
    "delban",
    "deposit",
    "depot",
    "emote",
    "enemy",
    "enter",
    "eras",
    "erase",
    "exit",
    "fightback",
    "follow",
    "force",
    "gargoyle",
    "ggold",
    "give",
    "god",
    "gold",
    "golden",
    "goto",
    "greatergod",
    "greaterinv",
    "grolm",
    "grolminfo",
    "grolmstart",
    "group",
    "gtell",
    "help",
    "ignore",
    "iignore",
    "iinfo",
    "immortal",
    "imp",
    "info",
    "infra",
    "infrared",
    "init",
    "invisible",
    "ipshow",
    "itell",
    "kick",
    "lag",
    "leave",
    "listban",
    "listblack",
    "listgolden",
    "listimps",
    "look",
    "lookdepot",
    "lookequip",
    "lookinv",
    "looting",
    "lower",
    "luck",
    "mailpass",
    "mark",
    "mayhem",
    "me",
    "mirror",
    "name",
    "network",
    "nodesc",
    "noluck",
    "nolist",
    "noshout",
    "nostaff",
    "notell",
    "nowho",
    "npclist",
    "password",
    "pent",
    "perase",
    "poh",
    "pol",
    "prof",
    "purple",
    "raise",
    "rank",
    "recall",
    "respawn",
    "safe",
    "save",
    "seen",
    "send",
    "shout",
    "shutup",
    "skill",
    "skua",
    "slap",
    "soulstone",
    "sort",
    "speedy",
    "spellignore",
    "sprite",
    "staff",
    "stat",
    "steal",
    "stell",
    "summon",
    "tavern",
    "tell",
    "temple",
    "thrall",
    "time",
    "tinfo",
    "top",
    "unique",
    "usurp",
    "wave",
    "who",
    "withdraw",
    "write",
];

fn match_command(input: &str) -> Option<&'static str> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }

    let input_lower = input.to_ascii_lowercase();
    let input_len = input_lower.len();

    // Allow a small number of mismatches for typo-tolerance, scaled by input length.
    // Keep this conservative to avoid returning arbitrary commands.
    let max_mismatches = match input_len {
        0..=4 => 0,
        5..=7 => 1,
        _ => 2,
    };

    let mut best: Option<(&'static str, usize)> = None;

    for &cmd in ALL_COMMANDS {
        if cmd.len() < input_len {
            continue;
        }

        let mut mismatches = 0usize;
        for (a, b) in input_lower.bytes().zip(cmd.bytes()) {
            if a != b {
                mismatches += 1;
                if mismatches > max_mismatches {
                    break;
                }
            }
        }

        if mismatches > max_mismatches {
            continue;
        }

        match best {
            None => best = Some((cmd, mismatches)),
            Some((best_cmd, best_score)) => {
                // Prefer fewer mismatches; tie-break to shorter command (more specific for prefixes).
                if mismatches < best_score
                    || (mismatches == best_score && cmd.len() < best_cmd.len())
                {
                    best = Some((cmd, mismatches));
                }
            }
        }
    }

    best.map(|(cmd, _)| cmd)
}

impl GameState {
    /// Creates a note item with custom text for the character.
    ///
    /// # Arguments
    ///
    /// * `cn` - Character number (index)
    /// * `text` - The note text to create
    pub(crate) fn do_create_note(&mut self, cn: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        if text.len() >= 199 {
            return;
        }

        log::info!("created note: {}.", text);

        for m in 0..40 {
            let slot = self.characters[cn].item[m];
            if slot == 0 {
                if let Some(in_idx) = God::create_item(self, 132) {
                    self.items[in_idx].temp = 0;
                    self.items[in_idx].description = [0; 200];
                    let bytes = text.as_bytes();
                    let length_to_copy =
                        std::cmp::min(bytes.len(), self.items[in_idx].description.len());
                    self.items[in_idx].description[..length_to_copy]
                        .copy_from_slice(&bytes[..length_to_copy]);

                    self.items[in_idx].flags |= core::constants::ItemFlags::IF_NOEXPIRE.bits();
                    self.items[in_idx].carried = cn as u16;

                    self.characters[cn].item[m] = in_idx as u32;
                    self.characters[cn].set_do_update_flags();

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
    pub(crate) fn do_emote(&mut self, cn: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        if text.contains('%') {
            return;
        }

        let shutup = (self.characters[cn].flags & CharacterFlags::ShutUp.bits()) != 0;
        let invis = (self.characters[cn].flags & CharacterFlags::Invisible.bits()) != 0;

        if shutup {
            self.do_character_log(cn, core::types::FontColor::Red, "You feel guilty.\n");
            log::info!("emote: feels guilty ({})", text);
        } else if invis {
            self.do_area_log(
                0,
                0,
                self.characters[cn].x as i32,
                self.characters[cn].y as i32,
                core::types::FontColor::Green,
                &format!("Somebody {}.\n", text),
            );
            log::info!("emote(inv): {}", text);
        } else {
            let name = self.characters[cn].get_name().to_string();
            self.do_area_log(
                0,
                0,
                self.characters[cn].x as i32,
                self.characters[cn].y as i32,
                core::types::FontColor::Green,
                &format!("{} {}.\n", name, text),
            );
            log::info!("emote: {}", text);
        }
    }

    /// Port of `do_become_skua(int cn)` from `svr_do.cpp`
    ///
    /// Transform character into a Skua.
    pub(crate) fn do_become_skua(&mut self, cn: usize) {
        // Ported from svr_do.cpp
        let is_purple = (self.characters[cn].kindred & core::constants::KIN_PURPLE as i32) != 0;

        if !is_purple {
            self.do_character_log(cn, FontColor::Red, "Hmm. Nothing happened.\n");
            return;
        }

        let ticker = self.globals.ticker;
        let attack_time = self.characters[cn].data[core::constants::CHD_ATTACKTIME];

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

        let (x, y) = (self.characters[cn].x, self.characters[cn].y);

        self.characters[cn].kindred &= !(core::constants::KIN_PURPLE as i32);
        self.characters[cn].data[core::constants::CHD_ATTACKTIME] = 0;
        self.characters[cn].data[core::constants::CHD_ATTACKVICT] = 0;
        self.characters[cn].temple_x = 512;
        self.characters[cn].temple_y = 512;

        chlog!(cn, "Converted to skua. ({} days elapsed)", days);

        EffectManager::fx_add_effect(5, 0, x as i32, y as i32, 0);
    }

    /// Port of `do_make_soulstone(int cn, int cexp)` from `svr_do.cpp`
    ///
    /// Create a soulstone item.
    pub(crate) fn do_make_soulstone(&mut self, cn: usize, cexp: i32) {
        if let Some(in_idx) = God::create_item(self, 1146) {
            let rank = core::ranks::points2rank(cexp as u32);

            let it = &mut self.items[in_idx];

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

            God::give_character_item(self, cn, in_idx);
        }
    }

    /// Port of `do_list_all_flags(int cn, u64 flag)` from `svr_do.cpp`
    ///
    /// List all characters with a specific flag.
    pub(crate) fn do_list_all_flags(&mut self, cn: usize, flag: u64) {
        for n in 1..core::constants::MAXCHARS {
            let ch = &mut self.characters[n];
            let show = {
                if ch.used == core::constants::USE_EMPTY {
                    false
                } else if (ch.flags & CharacterFlags::Player.bits()) == 0 {
                    false
                } else {
                    (ch.flags & flag) != 0
                }
            };

            if show {
                let name = self.characters[n].get_name().to_string();
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
    pub(crate) fn do_list_net(&mut self, cn: usize, co: usize) {
        let header = format!(
            "{} is know to log on from the following addresses:\n",
            self.characters[co].get_name()
        );
        self.do_character_log(cn, core::types::FontColor::Yellow, &header);

        for n in 80..90 {
            let ip = self.characters[co].data[n];
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
    pub(crate) fn do_respawn(&mut self, cn: usize, co: usize) {
        if !(1..core::constants::MAXTCHARS).contains(&co) {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "That template number is a bit strange, don't you think so, dude?\n",
            );
            return;
        }
        self.globals.reset_char = co as i32;
    }

    /// Port of `do_npclist(int cn, char* name)` from `svr_do.cpp`
    ///
    /// List NPCs matching a name pattern.
    pub(crate) fn do_npclist(&mut self, cn: usize, name: &str) {
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
            let ch = &mut self.characters[n];
            let matched = {
                if ch.used == core::constants::USE_EMPTY {
                    false
                } else if (ch.flags & CharacterFlags::Player.bits()) != 0 {
                    false
                } else {
                    ch.get_name().to_lowercase().contains(&name.to_lowercase())
                }
            };

            if matched {
                foundalive += 1;
                let n_name = self.characters[n].get_name().to_string();
                let n_desc = c_string_to_str(&mut self.characters[n].description).to_string();
                self.do_character_log(
                    cn,
                    core::types::FontColor::Yellow,
                    &format!("C{:4} {:20.20} {:.20}\n", n, n_name, n_desc),
                );
            }
        }

        for n in 1..core::constants::MAXTCHARS {
            let matched = {
                if self.character_templates[n].used == core::constants::USE_EMPTY {
                    false
                } else if (self.character_templates[n].flags & CharacterFlags::Player.bits()) != 0 {
                    false
                } else {
                    let name_s = c_string_to_str(&mut self.character_templates[n].name);
                    name_s.to_lowercase().contains(&name.to_lowercase())
                }
            };

            if matched {
                foundtemp += 1;
                let t_name = self.character_templates[n].get_name().to_string();
                let t_desc =
                    c_string_to_str(&mut self.character_templates[n].description).to_string();
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
    pub(crate) fn do_leave(&mut self, cn: usize) {
        let name = self.characters[cn].get_name().to_string();
        self.do_announce(cn, 0, &format!("{} left the game.\n", name));
        self.characters[cn].flags |=
            CharacterFlags::NoWho.bits() | CharacterFlags::Invisible.bits();
    }

    /// Port of `do_enter(int cn)` from `svr_do.cpp`
    ///
    /// Make character enter a location/mode.
    pub(crate) fn do_enter(&mut self, cn: usize) {
        self.characters[cn].flags &=
            !(CharacterFlags::NoWho.bits() | CharacterFlags::Invisible.bits());
        let name = self.characters[cn].get_name().to_string();
        self.do_announce(cn, 0, &format!("{} entered the game.\n", name));
    }

    /// Port of `do_stat(int cn)` from `svr_do.cpp`
    ///
    /// Display character statistics.
    pub(crate) fn do_stat(&mut self, cn: usize) {
        self.do_character_log(
            cn,
            core::types::FontColor::Blue,
            &format!(
                "items: {}/{}\n",
                self.globals.item_cnt,
                core::constants::MAXITEM
            ),
        );
        self.do_character_log(
            cn,
            core::types::FontColor::Blue,
            &format!(
                "chars: {}/{}\n",
                self.globals.character_cnt,
                core::constants::MAXCHARS
            ),
        );
        self.do_character_log(
            cn,
            core::types::FontColor::Blue,
            &format!(
                "effes: {}/{}\n",
                self.globals.effect_cnt,
                core::constants::MAXEFFECT
            ),
        );

        self.do_character_log(
            cn,
            core::types::FontColor::Blue,
            &format!("newmoon={}\n", self.globals.newmoon),
        );
        self.do_character_log(
            cn,
            core::types::FontColor::Blue,
            &format!("fullmoon={}\n", self.globals.fullmoon),
        );
        self.do_character_log(
            cn,
            core::types::FontColor::Blue,
            &format!(
                "mdday={} (%28={})\n",
                self.globals.mdday,
                self.globals.mdday % 28
            ),
        );

        self.do_character_log(
            cn,
            core::types::FontColor::Blue,
            &format!(
                "mayhem={}, looting={}, close={}, cap={}, speedy={}\n",
                if (self.globals.flags & core::constants::GF_MAYHEM) != 0 {
                    "yes"
                } else {
                    "no"
                },
                if (self.globals.flags & core::constants::GF_LOOTING) != 0 {
                    "yes"
                } else {
                    "no"
                },
                if (self.globals.flags & core::constants::GF_CLOSEENEMY) != 0 {
                    "yes"
                } else {
                    "no"
                },
                if (self.globals.flags & core::constants::GF_CAP) != 0 {
                    "yes"
                } else {
                    "no"
                },
                if (self.globals.flags & core::constants::GF_SPEEDY) != 0 {
                    "yes"
                } else {
                    "no"
                }
            ),
        );
    }

    /// Port of `do_become_purple(int cn)` from `svr_do.cpp`
    ///
    /// Transform character into Purple faction.
    pub(crate) fn do_become_purple(&mut self, cn: usize) {
        // Ported from svr_do.cpp
        let ticker = self.globals.ticker;
        let last = self.characters[cn].data[core::constants::CHD_RIDDLER];
        let is_purple = (self.characters[cn].kindred & core::constants::KIN_PURPLE as i32) != 0;

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

            let (x, y) = (self.characters[cn].x, self.characters[cn].y);

            self.characters[cn].kindred |= core::constants::KIN_PURPLE as i32;
            self.characters[cn].temple_x = 558;
            self.characters[cn].temple_y = 542;

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
        let (f_gg, f_c, f_g, f_i, f_s, f_p, f_u, f_sh, f_pol) = {
            let flags = self.characters[cn].flags;
            (
                (flags & CharacterFlags::GreaterGod.bits()) != 0,
                (flags & CharacterFlags::Creator.bits()) != 0,
                (flags & CharacterFlags::God.bits()) != 0,
                (flags & CharacterFlags::Imp.bits()) != 0,
                (flags & CharacterFlags::Staff.bits()) != 0,
                (flags & CharacterFlags::Player.bits()) != 0,
                (flags & CharacterFlags::Usurp.bits()) != 0,
                (flags & CharacterFlags::ShutUp.bits()) != 0,
                (flags & (CharacterFlags::PohLeader.bits() | CharacterFlags::God.bits())) != 0,
            )
        };

        let f_m = !f_p;
        let f_gi = f_g || f_i;
        let f_giu = f_gi || f_u;
        let f_gius = f_giu || f_s;

        // helper closures
        let arg_get = |i: usize| arg.get(i).map(|s| s.as_str()).unwrap_or("");
        let args_get = |i: usize| args.get(i).and_then(|o| *o).unwrap_or("");
        // Match original C behavior: numeric parsing is atoi-like (stops at first non-digit).
        let parse_usize = |s: &str| atoi_usize(s);
        let parse_i32 = |s: &str| atoi_i32(s);
        let parse_u32 = |s: &str| atoi_u32(s);

        log::debug!("Command received from {}: cmd={} ptr={}", cn, cmd, ptr);

        let matched_cmd = match_command(&cmd);

        match matched_cmd {
            Some("afk") if f_p => {
                log::debug!("Processing afk command for {}", cn);
                self.do_afk(cn, args_get(0));
                return;
            }
            Some("allow") if f_p => {
                log::debug!("Processing allow command for {}", cn);
                self.do_allow(cn, parse_usize(arg_get(1)));
                return;
            }
            Some("announce") if f_gius => {
                log::debug!("Processing announce command for {}", cn);
                self.do_announce(cn, cn, args_get(0));
                return;
            }
            Some("addban") if f_gi => {
                log::debug!("Processing addban command for {}", cn);
                God::add_ban(self, cn, parse_usize(arg_get(1)));
                return;
            }
            Some("bow") if !f_sh => {
                log::debug!("Processing bow command for {}", cn);
                self.characters[cn].misc_action = core::constants::DR_BOW as u16;
                return;
            }
            Some("balance") if !f_m => {
                log::debug!("Processing balance command for {}", cn);
                self.do_balance(cn);
                return;
            }
            Some("black") if f_g => {
                log::debug!("Processing black command for {}", cn);
                God::set_flag(self, cn, arg_get(1), CharacterFlags::Black.bits());
                return;
            }
            Some("build") if f_c => {
                log::debug!("Processing build command for {}", cn);
                God::build(self, cn, parse_i32(arg_get(1)) as u32);
                return;
            }
            Some("cap") if f_g => {
                // TODO: `set_cap(int cn,int nr)` from original C++
                // Original call: set_cap(cn, atoi(arg[1]));
                // Not implemented elsewhere in Rust yet; preserve as TODO.
                log::warn!("TODO: set_cap not implemented - call set_cap({}, arg1)", cn);
                self.do_character_log(cn, FontColor::Red, "cap command not implemented\n");
                return;
            }
            Some("caution") if f_gius => {
                log::debug!("Processing caution command for {}", cn);
                self.do_caution(cn, cn, args_get(0));
                return;
            }
            Some("ccp") if f_i => {
                log::debug!("Processing ccp command for {}", cn);
                God::set_flag(
                    self,
                    cn,
                    arg_get(1),
                    CharacterFlags::ComputerControlledPlayer.bits(),
                );
                return;
            }
            Some("closenemey") if f_g => {
                log::debug!("Processing closeenemy command for {}", cn);
                God::set_gflag(self, cn, GF_CLOSEENEMY);
                return;
            }
            Some("create") if f_g => {
                log::debug!("Processing create command for {}", cn);
                God::create(self, cn, parse_i32(arg_get(1)));
                return;
            }
            Some("createspecial") if f_g => {
                log::debug!("Processing createspecial command for {}", cn);
                God::create_special(self, cn, arg_get(1), arg_get(2), arg_get(3));
                return;
            }
            Some("creator") if f_gg => {
                log::debug!("Processing creator command for {}", cn);
                God::set_flag(self, cn, arg_get(1), CharacterFlags::Creator.bits());
                return;
            }
            Some("deposit") if !f_m => {
                log::debug!("Processing deposit command for {}", cn);
                self.do_deposit(cn, parse_i32(arg_get(1)), parse_i32(arg_get(2)));
                return;
            }
            Some("depot") if !f_m => {
                log::debug!("Processing depot command for {}", cn);
                self.do_depot(cn);
                return;
            }
            Some("delban") if f_giu => {
                log::debug!("Processing delban command for {}", cn);
                God::del_ban(self, cn, parse_usize(arg_get(1)));
                return;
            }
            Some("emote") => {
                log::debug!("Processing emote command for {}", cn);
                self.do_emote(cn, args_get(0));
                return;
            }
            Some("enemy") if f_giu => {
                log::debug!("Processing enemy command for {}", cn);
                self.do_enemy(cn, arg_get(1), arg_get(2));
                return;
            }
            Some("enter") if f_gi => {
                log::debug!("Processing enter command for {}", cn);
                self.do_enter(cn);
                return;
            }
            Some("exit") if f_u => {
                log::debug!("Processing exit command for {}", cn);
                God::exit_usurp(self, cn);
                return;
            }
            Some("eras") if f_g => {
                return;
            }
            Some("erase") if f_g => {
                log::debug!("Processing erase command for {}", cn);
                God::erase(self, cn, parse_usize(arg_get(1)), 0);
                return;
            }
            Some("fightback") => {
                log::debug!("Processing fightback command for {}", cn);
                self.do_fightback(cn);
                return;
            }
            Some("follow") if !f_m => {
                log::debug!("Processing follow command for {}", cn);
                self.do_follow(cn, args_get(0));
                return;
            }
            Some("force") if f_giu => {
                log::debug!("Processing force command for {}", cn);
                God::force(self, cn, arg_get(1), args_get(1));
                return;
            }
            Some("gtell") if !f_m => {
                log::debug!("Processing gtell command for {}", cn);
                self.do_gtell(cn, args_get(0));
                return;
            }
            Some("gold") => {
                log::debug!("Processing gold command for {}", cn);
                self.do_gold(cn, parse_i32(arg_get(1)));
                return;
            }
            Some("golden") if f_g => {
                log::debug!("Processing golden command for {}", cn);
                God::set_flag(self, cn, arg_get(1), CharacterFlags::Golden.bits());
                return;
            }
            Some("group") if !f_m => {
                log::debug!("Processing group command for {}", cn);
                self.do_group(cn, args_get(0));
                return;
            }
            Some("gargoyle") if f_gi => {
                log::debug!("Processing gargoyle command for {}", cn);
                God::gargoyle(self, cn);
                return;
            }
            Some("ggold") if f_g => {
                log::debug!("Processing ggold command for {}", cn);
                God::gold_char(
                    self,
                    cn,
                    arg_get(1),
                    parse_u32(arg_get(2)),
                    parse_u32(arg_get(3)),
                );
                return;
            }
            Some("give") if f_giu => {
                log::debug!("Processing give command for {}", cn);
                self.do_god_give(cn, parse_usize(arg_get(1)));
                return;
            }
            Some("goto") if f_giu => {
                log::debug!("Processing goto command for {}", cn);
                God::goto(self, cn, cn, arg_get(1), arg_get(2));
                return;
            }
            Some("god") if f_g => {
                log::debug!("Processing god command for {}", cn);
                God::set_flag(self, cn, arg_get(1), CharacterFlags::God.bits());
                return;
            }
            Some("greatergod") if f_gg => {
                log::debug!("Processing greatergod command for {}", cn);
                God::set_flag(self, cn, arg_get(1), CharacterFlags::GreaterGod.bits());
                return;
            }
            Some("greaterinv") if f_gg => {
                log::debug!("Processing greaterinv command for {}", cn);
                God::set_flag(self, cn, arg_get(1), CharacterFlags::GreaterInv.bits());
                return;
            }
            Some("grolm") if f_gi => {
                log::debug!("Processing grolm command for {}", cn);
                God::grolm(self, cn);
                return;
            }
            Some("grolminfo") if f_gi => {
                log::debug!("Processing grolminfo command for {}", cn);
                God::grolm_info(self, cn);
                return;
            }
            Some("grolmstart") if f_g => {
                log::debug!("Processing grolmstart command for {}", cn);
                God::grolm_start(self, cn);
                return;
            }
            Some("help") => {
                log::debug!("Processing help command for {}", cn);
                self.do_help(cn, arg_get(1));
                return;
            }
            Some("ignore") if !f_m => {
                log::debug!("Processing ignore command for {}", cn);
                self.do_ignore(cn, arg_get(1), 0);
                return;
            }
            Some("iignore") if !f_m => {
                log::debug!("Processing iignore command for {}", cn);
                self.do_ignore(cn, arg_get(1), 1);
                return;
            }
            Some("iinfo") if f_g => {
                log::debug!("Processing iinfo command for {}", cn);
                God::iinfo(self, cn, parse_usize(arg_get(1)));
                return;
            }
            Some("immortal") if f_u || f_g => {
                log::debug!("Processing immortal command for {}", cn);
                God::set_flag(self, cn, arg_get(1), CharacterFlags::Immortal.bits());
                return;
            }
            Some("imp") if f_g => {
                log::debug!("Processing imp command for {}", cn);
                God::set_flag(self, cn, arg_get(1), CharacterFlags::Imp.bits());
                return;
            }
            Some("info") if f_gius => {
                log::debug!("Processing info command for {}", cn);
                let target = if arg_get(1).is_empty() {
                    cn
                } else {
                    parse_usize(arg_get(1))
                };
                God::info(self, cn, target);
                return;
            }
            Some("infra") | Some("infrared") if f_giu => {
                log::debug!("Processing infrared command for {}", cn);
                God::set_flag(self, cn, arg_get(1), CharacterFlags::Infrared.bits());
                return;
            }
            Some("invisible") if f_giu => {
                log::debug!("Processing invisible command for {}", cn);
                God::set_flag(self, cn, arg_get(1), CharacterFlags::Invisible.bits());
                return;
            }
            Some("ipshow") if f_giu => {
                log::debug!("Processing ipshow command for {}", cn);
                self.do_list_net(cn, parse_usize(arg_get(1)));
                return;
            }
            Some("itell") if f_giu => {
                log::debug!("Processing itell command for {}", cn);
                self.do_itell(cn, args_get(0));
                return;
            }
            Some("kick") if f_giu => {
                log::debug!("Processing kick command for {}", cn);
                God::kick(self, cn, parse_usize(arg_get(1)));
                return;
            }
            Some("lag") if !f_m => {
                log::debug!("Processing lag command for {}", cn);
                self.do_lag(cn, parse_i32(arg_get(1)));
                return;
            }
            Some("leave") if f_gi => {
                log::debug!("Processing leave command for {}", cn);
                self.do_leave(cn);
                return;
            }
            Some("look") if f_gius => {
                log::debug!("Processing look command for {}", cn);
                // do_look_char expects numbers in original; use parse
                self.do_look_char(cn, parse_usize(arg_get(1)), 1, 0, 0);
                return;
            }
            Some("lookdepot") if f_gg => {
                log::debug!("Processing lookdepot command for {}", cn);
                self.do_look_player_depot(cn, parse_usize(arg_get(1)));
                return;
            }
            Some("lookinv") if f_gg => {
                log::debug!("Processing lookinv command for {}", cn);
                self.do_look_player_inventory(cn, parse_usize(arg_get(1)));
                return;
            }
            Some("lookequip") if f_gg => {
                log::debug!("Processing lookequip command for {}", cn);
                self.do_look_player_equipment(cn, parse_usize(arg_get(1)));
                return;
            }
            Some("looting") if f_g => {
                log::debug!("Processing looting command for {}", cn);
                God::set_gflag(self, cn, GF_LOOTING);
                return;
            }
            Some("lower") if f_g => {
                log::debug!("Processing lower command for {}", cn);
                God::lower_char(self, cn, arg_get(1), arg_get(2));
                return;
            }
            Some("luck") if f_giu => {
                log::debug!("Processing luck command for {}", cn);
                God::luck(self, cn, parse_usize(arg_get(1)), parse_i32(arg_get(2)));
                return;
            }
            Some("listban") if f_giu => {
                log::debug!("Processing listban command for {}", cn);
                God::list_ban(self, cn);
                return;
            }
            Some("listimps") if f_giu => {
                log::debug!("Processing listimps command for {}", cn);
                God::implist(self, cn);
                return;
            }
            Some("listgolden") if f_giu => {
                log::debug!("Processing listgolden command for {}", cn);
                self.do_list_all_flags(cn, CharacterFlags::Golden.bits());
                return;
            }
            Some("listblack") if f_giu => {
                log::debug!("Processing listblack command for {}", cn);
                self.do_list_all_flags(cn, CharacterFlags::Black.bits());
                return;
            }
            Some("mayhem") if f_g => {
                log::debug!("Processing mayhem command for {}", cn);
                God::set_gflag(self, cn, GF_MAYHEM);
                return;
            }
            Some("mark") if f_giu => {
                log::debug!("Processing mark command for {}", cn);
                self.do_mark(cn, parse_usize(arg_get(1)), args_get(1));
                return;
            }
            Some("me") => {
                log::debug!("Processing me command for {}", cn);
                self.do_emote(cn, args_get(0));
                return;
            }
            Some("mirror") if f_giu => {
                log::debug!("Processing mirror command for {}", cn);
                God::mirror(self, cn, arg_get(1), arg_get(2));
                return;
            }
            Some("noshout") if !f_m => {
                log::debug!("Processing noshout command for {}", cn);
                self.do_noshout(cn);
                return;
            }
            Some("nostaff") if f_giu => {
                log::debug!("Processing nostaff command for {}", cn);
                self.do_nostaff(cn);
                return;
            }
            Some("notell") if !f_m => {
                log::debug!("Processing notell command for {}", cn);
                self.do_notell(cn);
                return;
            }
            Some("name") if f_giu => {
                log::debug!("Processing name command for {}", cn);
                God::set_name(self, cn, parse_usize(arg_get(1)), args_get(1));
                return;
            }
            Some("nodesc") if f_giu => {
                log::debug!("Processing nodesc command for {}", cn);
                God::reset_description(self, cn, parse_usize(arg_get(1)));
                return;
            }
            Some("nolist") if f_gi => {
                log::debug!("Processing nolist command for {}", cn);
                God::set_flag(self, cn, arg_get(1), CharacterFlags::NoList.bits());
                return;
            }
            Some("noluck") if f_giu => {
                log::debug!("Processing noluck command for {}", cn);
                God::luck(self, cn, parse_usize(arg_get(1)), -parse_i32(arg_get(2)));
                return;
            }
            Some("nowho") if f_gi => {
                log::debug!("Processing nowho command for {}", cn);
                God::set_flag(self, cn, arg_get(1), CharacterFlags::NoWho.bits());
                return;
            }
            Some("npclist") if f_giu => {
                log::debug!("Processing npclist command for {}", cn);
                self.do_npclist(cn, args_get(0));
                return;
            }
            Some("password") => {
                if f_g {
                    log::debug!("Processing others-password command for {}", cn);
                    // change another's password
                    God::change_pass(self, cn, parse_usize(arg_get(1)), arg_get(2));
                    return;
                }
                log::debug!("Processing own-password command for {}", cn);
                // change own password
                God::change_pass(self, cn, cn, arg_get(1));
                return;
            }
            Some("pent") => {
                log::debug!("Processing pent command for {}", cn);
                self.do_check_pent_count(cn);
                return;
            }
            Some("poh") if f_pol => {
                log::debug!("Processing poh command for {}", cn);
                God::set_flag(self, cn, arg_get(1), CharacterFlags::Poh.bits());
                return;
            }
            Some("pol") if f_pol => {
                log::debug!("Processing pol command for {}", cn);
                God::set_flag(self, cn, arg_get(1), CharacterFlags::PohLeader.bits());
                return;
            }
            Some("prof") if f_g => {
                log::debug!("Processing prof command for {}", cn);
                God::set_flag(self, cn, arg_get(1), CharacterFlags::Profile.bits());
                return;
            }
            Some("purple") => {
                if !f_g && !f_m {
                    log::debug!("Processing become_purple command for {}", cn);
                    self.do_become_purple(cn);
                    return;
                }

                if f_g {
                    log::debug!("Processing set_purple command for {}", cn);
                    God::set_purple(self, cn, parse_usize(arg_get(1)));
                    return;
                }
            }
            Some("perase") if f_g => {
                log::debug!("Processing perase command for {}", cn);
                God::erase(self, cn, parse_usize(arg_get(1)), 1);
                return;
            }
            Some("rank") => {
                log::debug!("Processing rank command for {}", cn);
                self.do_view_exp_to_rank(cn);
                return;
            }
            Some("raise") if f_giu => {
                log::debug!("Processing raise command for {}", cn);
                God::raise_char(self, cn, arg_get(1), arg_get(2));
                return;
            }
            Some("recall") if f_giu => {
                log::debug!("Processing recall command for {}", cn);
                God::goto(self, cn, cn, "512", "512");
                return;
            }
            Some("respawn") if f_giu => {
                log::debug!("Processing respawn command for {}", cn);
                self.do_respawn(cn, parse_usize(arg_get(1)));
                return;
            }
            Some("network") if f_gius => {
                log::debug!("Processing network command for {}", cn);
                let target = args_get(0).trim();
                if f_gi && target.is_empty() {
                    God::show_network_info_all(self, cn);
                } else {
                    God::show_network_info(self, cn, target);
                }
                return;
            }
            Some("shout") => {
                log::debug!("Processing shout command for {}", cn);
                self.do_shout(cn, args_get(0));
                return;
            }
            Some("safe") if f_g => {
                log::debug!("Processing safe command for {}", cn);
                God::set_flag(self, cn, arg_get(1), CharacterFlags::Safe.bits());
                return;
            }
            Some("save") if f_g => {
                log::debug!("Processing save command for {}", cn);
                God::save(self, cn, parse_usize(arg_get(1)));
                return;
            }
            Some("seen") => {
                log::debug!("Processing seen command for {}", cn);
                self.do_seen(cn, arg_get(1));
                return;
            }
            Some("send") => {
                log::debug!("Processing send command for {}", cn);
                God::goto(self, cn, parse_usize(arg_get(1)), arg_get(2), arg_get(3));
                return;
            }
            Some("shutup") if f_gius => {
                log::debug!("Processing shutup command for {}", cn);
                God::shutup(self, cn, parse_usize(arg_get(1)));
                return;
            }
            Some("skill") if f_g => {
                log::debug!("Processing skill command for {}", cn);
                God::skill(
                    self,
                    cn,
                    parse_usize(arg_get(1)),
                    driver::skill_lookup(arg_get(2)),
                    parse_i32(arg_get(3)),
                );
                return;
            }
            Some("skua") => {
                log::debug!("Processing skua command for {}", cn);
                self.do_become_skua(cn);
                return;
            }
            Some("slap") if f_giu => {
                log::debug!("Processing slap command for {}", cn);
                God::slap(self, cn, parse_usize(arg_get(1)));
                return;
            }
            Some("sort") => {
                log::debug!("Processing sort command for {}", cn);
                self.do_sort(cn, arg_get(1));
                return;
            }
            Some("soulstone") if f_g => {
                log::debug!("Processing soulstone command for {}", cn);
                self.do_make_soulstone(cn, parse_i32(arg_get(1)));
                return;
            }
            Some("speedy") if f_g => {
                log::debug!("Processing speedy command for {}", cn);
                God::set_gflag(self, cn, GF_SPEEDY);
                return;
            }
            Some("spellignore") if !f_m => {
                log::debug!("Processing spellignore command for {}", cn);
                self.do_spellignore(cn);
                return;
            }
            Some("sprite") if f_giu => {
                log::debug!("Processing sprite command for {}", cn);
                God::spritechange(self, cn, parse_usize(arg_get(1)), parse_i32(arg_get(2)));
                return;
            }
            Some("stell") if f_giu => {
                log::debug!("Processing stell command for {}", cn);
                self.do_stell(cn, args_get(0));
                return;
            }
            Some("stat") if f_g => {
                log::debug!("Processing stat command for {}", cn);
                self.do_stat(cn);
                return;
            }
            Some("staff") if f_g => {
                log::debug!("Processing staff command for {}", cn);
                God::set_flag(self, cn, arg_get(1), CharacterFlags::Staff.bits());
                return;
            }
            Some("steal") if f_gg => {
                log::debug!("Processing steal command for {}", cn);
                self.do_steal_player(cn, arg_get(1), arg_get(2));
                return;
            }
            Some("summon") if f_g => {
                log::debug!("Processing summon command for {}", cn);
                God::summon(self, cn, arg_get(1), arg_get(2), arg_get(3));
                return;
            }
            Some("tell") => {
                log::debug!("Processing tell command for {}", cn);
                self.do_tell(cn, arg_get(1), args_get(1));
                return;
            }
            Some("tavern") if f_g && !f_m => {
                log::debug!("Processing tavern command for {}", cn);
                God::tavern(self, cn);
                return;
            }
            Some("temple") if f_giu => {
                log::debug!("Processing temple command for {}", cn);
                God::goto(self, cn, cn, "800", "800");
                return;
            }
            Some("thrall") if f_giu => {
                log::debug!("Processing thrall command for {}", cn);
                God::thrall(self, cn, arg_get(1), arg_get(2));
                return;
            }
            Some("time") => {
                log::debug!("Processing time command for {}", cn);
                helpers::show_time(self, cn);
                return;
            }
            Some("tinfo") if f_g => {
                log::debug!("Processing tinfo command for {}", cn);
                God::tinfo(self, cn, parse_usize(arg_get(1)));
                return;
            }
            Some("top") if f_g => {
                log::debug!("Processing top command for {}", cn);
                God::top(self, cn);
                return;
            }
            Some("unique") if f_g => {
                log::debug!("Processing unique command for {}", cn);
                God::unique(self, cn);
                return;
            }
            Some("usurp") if f_giu => {
                log::debug!("Processing usurp command for {}", cn);
                God::usurp(self, cn, parse_usize(arg_get(1)));
                return;
            }
            Some("who") => {
                log::debug!("Processing who command for {}", cn);
                if f_gius {
                    God::who(self, cn);
                } else {
                    God::user_who(self, cn);
                }
                return;
            }
            Some("wave") if !f_sh => {
                log::debug!("Processing wave command for {}", cn);
                self.characters[cn].misc_action = core::constants::DR_WAVE as u16;
                return;
            }
            Some("withdraw") if !f_m => {
                log::debug!("Processing withdraw command for {}", cn);
                self.do_withdraw(cn, parse_i32(arg_get(1)), parse_i32(arg_get(2)));
                return;
            }
            Some("write") if f_giu => {
                log::debug!("Processing write command for {}", cn);
                self.do_create_note(cn, args_get(0));
                return;
            }

            _ => {}
        }

        // Unknown command
        self.do_character_log(cn, FontColor::Red, &format!("Unknown command #{}\n", cmd));
    }
}

#[cfg(test)]
mod tests {
    use super::{match_command, ALL_COMMANDS};

    #[test]
    fn match_command_empty_is_none() {
        assert_eq!(match_command(""), None);
        assert_eq!(match_command("   "), None);
    }

    #[test]
    fn match_command_common_commands() {
        assert_eq!(match_command("wh"), Some("who"));
        assert_eq!(match_command("ra"), Some("rank"));
        assert_eq!(match_command("gt"), Some("gtell"));
    }

    #[test]
    fn match_command_exact_match() {
        assert_eq!(match_command("afk"), Some("afk"));
        assert_eq!(match_command("withdraw"), Some("withdraw"));
    }

    #[test]
    fn match_command_case_insensitive() {
        assert_eq!(match_command("AFK"), Some("afk"));
        assert_eq!(match_command("WiThDrAw"), Some("withdraw"));
    }

    #[test]
    fn match_command_trims_input() {
        assert_eq!(match_command("  afk  "), Some("afk"));
    }

    #[test]
    fn match_command_aliases_are_supported_when_present() {
        // These are intentionally in ALL_COMMANDS because do_command supports them explicitly.
        assert_eq!(match_command("imm"), Some("immortal"));
        assert_eq!(match_command("lookd"), Some("lookdepot"));
        assert_eq!(match_command("looke"), Some("lookequip"));
        assert_eq!(match_command("looki"), Some("lookinv"));
    }

    #[test]
    fn match_command_typo_tolerance() {
        // One mismatch allowed for len 5..=7.
        assert_eq!(match_command("follaw"), Some("follow"));

        // Two mismatches allowed for len >= 8.
        assert_eq!(match_command("withdrqw"), Some("withdraw"));
    }

    #[test]
    fn match_command_rejects_totally_unrelated_inputs() {
        assert_eq!(match_command("zzzzzz"), None);
        assert_eq!(match_command("thisisnotacommand"), None);
    }

    #[test]
    fn match_command_returns_none_when_input_longer_than_any_command() {
        // Make sure we don't accidentally return the first entry when no candidate can match.
        let longest = ALL_COMMANDS.iter().map(|c| c.len()).max().unwrap_or(0);
        let input = "x".repeat(longest + 1);
        assert_eq!(match_command(&input), None);
    }
}
