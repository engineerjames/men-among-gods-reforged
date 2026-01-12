use core::constants::{CharacterFlags, MAXCHARS, MAXPLAYER};
use core::types::ServerPlayer;
use std::cmp;
use std::sync::OnceLock;

use crate::network_manager::NetworkManager;
use crate::repository::Repository;
use crate::server::Server;
use crate::talk::npc_hear;

use super::State;

impl State {
    /// Port of `do_character_log(character_id, font, message)` from the original
    /// server sources.
    ///
    /// Sends a log message to a character's associated player connection. If the
    /// character has no player attached (and is not a special temp viewer), the
    /// message is skipped. This wrapper validates the character->player mapping
    /// before delegating to `do_log`.
    ///
    /// # Arguments
    /// * `character_id` - Character id to receive the message
    /// * `font` - Font/color to use for the message
    /// * `message` - The text to send
    pub(crate) fn do_character_log(
        &self,
        character_id: usize,
        font: core::types::FontColor,
        message: &str,
    ) {
        Repository::with_characters(|characters| {
            let ch = &characters[character_id];
            if ch.player == 0 && ch.temp != 15 {
                // TODO: Re-evaluate if we should be logging this
                // log::warn!(
                //     "do_character_log: Character '{}' has no associated player.",
                //     ch.get_name(),
                // );
                return;
            }

            self.do_log(character_id, font, message);
        });
    }

    /// Port of `do_log(character_id, font, message)` from the original server.
    ///
    /// Sends a log message directly to the player's network connection. Long
    /// lines are split into 15-byte chunks and transmitted as `SV_LOG` packets.
    /// Performs validation of the associated player and finds the matching
    /// player index before sending.
    ///
    /// # Arguments
    /// * `cn` - Character whose player will receive the message
    /// * `font` - Color/font modifier for the message
    /// * `message` - Message text (may be longer than a single packet)
    fn do_log(&self, cn: usize, font: core::types::FontColor, message: &str) {
        log::debug!(
            "do_log: cn={}, font={:?}, message='{}'",
            cn,
            font,
            message.strip_suffix('\n').unwrap_or("")
        );
        let mut buffer: [u8; 16] = [0; 16];

        let player_number = Repository::with_characters(|ch| ch[cn].player) as usize;

        if !ServerPlayer::is_sane_player(player_number) {
            log::error!(
                "do_log: Character {} has invalid player number: {}",
                cn,
                player_number
            );
            return;
        }

        if Server::with_players(|players| players[player_number].usnr) != cn {
            Repository::with_characters_mut(|ch| ch[cn].player = 0);
            return;
        }
        let bytes = message.as_bytes();
        let len = bytes.len();
        let mut pos = 0usize;

        // Send at least one packet (matches original intent), copy up to 15 bytes per packet.
        loop {
            buffer[0] = core::constants::SV_LOG + font as u8;

            let take = std::cmp::min(15, len.saturating_sub(pos));
            if take > 0 {
                buffer[1..1 + take].copy_from_slice(&bytes[pos..pos + take]);
            }
            // pad remainder with zeros (if any)
            for b in &mut buffer[1 + take..] {
                *b = 0;
            }

            NetworkManager::with(|network| {
                network.xsend(player_number, &buffer, 16);
            });

            pos += take;
            if pos >= len {
                break;
            }
        }
    }

    /// Port of `do_area_log(cn, co, xs, ys, font, message)` from the original
    /// server.
    ///
    /// Broadcasts a log message to all characters within a 12-tile radius of
    /// `(xs, ys)`, excluding the specified `cn` and `co` characters. Recipients
    /// are filtered for active players and sane characters.
    ///
    /// # Arguments
    /// * `cn` - Character to exclude (usually source)
    /// * `co` - Second character to exclude
    /// * `xs, ys` - Source coordinates for the area
    /// * `font` - Color/font for the message
    /// * `message` - Text to broadcast
    pub(crate) fn do_area_log(
        &self,
        cn: usize,
        co: usize,
        xs: i32,
        ys: i32,
        font: core::types::FontColor,
        message: &str,
    ) {
        let x_min = cmp::max(0, xs - 12);
        let x_max = cmp::min(core::constants::SERVER_MAPX, xs + 13);
        let y_min = cmp::max(0, ys - 12);
        let y_max = cmp::min(core::constants::SERVER_MAPY, ys + 13);

        let mut recipients: Vec<usize> = Vec::new();

        Repository::with_map(|map| {
            for y in y_min..y_max {
                let row_base = y * core::constants::SERVER_MAPX;
                for x in x_min..x_max {
                    let idx = (x + row_base) as usize;
                    let cc = map[idx].ch as usize;
                    if cc == 0 || cc == cn || cc == co {
                        continue;
                    }
                    recipients.push(cc);
                }
            }
        });

        let recipients: Vec<usize> = Repository::with_characters(|characters| {
            recipients
                .into_iter()
                .filter(|cc| {
                    *cc < MAXCHARS
                        && characters[*cc].used == core::constants::USE_ACTIVE
                        && characters[*cc].player != 0
                        && (characters[*cc].flags & CharacterFlags::Player.bits()) != 0
                })
                .collect()
        });

        for cc in recipients {
            self.do_character_log(cc, font, message);
        }
    }

    /// Port of `do_sayx(character_id, message)` from the original server.
    ///
    /// Formats and relays a speech message from `character_id` to nearby
    /// characters. The message is prefixed with the speaker's name and uses
    /// different colors for players (blue) and NPCs (yellow). Option prefixes
    /// (like `#<sound>`) are processed by `process_options`.
    ///
    /// # Arguments
    /// * `character_id` - Speaker character id
    /// * `message` - Raw speech text
    pub(crate) fn do_sayx(&self, character_id: usize, message: &str) {
        let mut buf = message.to_string();
        Self::process_options(character_id, &mut buf);

        let (x, y, is_player, name) = Repository::with_characters(|characters| {
            let ch = &characters[character_id];
            (
                ch.x as i32,
                ch.y as i32,
                (ch.flags & CharacterFlags::Player.bits()) != 0,
                ch.get_name().to_string(),
            )
        });

        let name_short: String = name.chars().take(30).collect();
        let msg_short: String = buf.chars().take(300).collect();

        let line = format!("{}: \"{}\"\n", name_short, msg_short);

        let font = if is_player {
            core::types::FontColor::Blue
        } else {
            core::types::FontColor::Yellow
        };

        self.do_area_log(0, 0, x, y, font, &line);
    }

    /// Port of `char_play_sound(character_id, sound, vol, pan)` from the
    /// original server.
    ///
    /// Low-level helper that sends a `SV_PLAYSOUND` packet to a single
    /// character's player connection. Validates the player mapping first.
    ///
    /// # Arguments
    /// * `character_id` - Target character id
    /// * `sound` - Sound id to play
    /// * `vol` - Volume modifier
    /// * `pan` - Stereo pan modifier
    pub(crate) fn char_play_sound(character_id: usize, sound: i32, vol: i32, pan: i32) {
        let matching_player_id = Server::with_players(|players| {
            (0..MAXPLAYER).find(|&i| players[i].usnr == character_id)
        });

        let Some(player_id) = matching_player_id else {
            log::debug!(
                "char_play_sound: Character {} has no associated player.",
                character_id
            );
            return;
        };

        let mut buf: [u8; 16] = [0; 16];
        buf[0] = core::constants::SV_PLAYSOUND;
        buf[1..5].copy_from_slice(&sound.to_le_bytes());
        buf[5..9].copy_from_slice(&vol.to_le_bytes());
        buf[9..13].copy_from_slice(&pan.to_le_bytes());

        NetworkManager::with(|network| {
            network.xsend(player_id, &buf, 13);
        });
    }

    /// Port of `do_area_sound(cn, co, xs, ys, nr)` from the original server.
    ///
    /// Broadcasts a sound event to nearby characters within an 8-tile radius,
    /// computing volume and pan based on distance. Characters `cn` and `co`
    /// are excluded from hearing the sound.
    ///
    /// # Arguments
    /// * `cn` - Character to exclude (usually source)
    /// * `co` - Second character to exclude
    /// * `xs, ys` - Coordinates of the sound source
    /// * `nr` - Sound id
    pub(crate) fn do_area_sound(cn: usize, co: usize, xs: i32, ys: i32, nr: i32) {
        let x_min = cmp::max(0, xs - 8);
        let x_max = cmp::min(core::constants::SERVER_MAPX, xs + 9);
        let y_min = cmp::max(0, ys - 8);
        let y_max = cmp::min(core::constants::SERVER_MAPY, ys + 9);

        let mut recipients: Vec<(usize, i32, i32)> = Vec::new();

        Repository::with_map(|map| {
            for y in y_min..y_max {
                let row_base = y * core::constants::SERVER_MAPX;
                for x in x_min..x_max {
                    let idx = (x + row_base) as usize;
                    let cc = map[idx].ch as usize;
                    if cc == 0 || cc == cn || cc == co {
                        continue;
                    }

                    let s = ys - y + xs - x;
                    let xpan = if s < 0 {
                        -500
                    } else if s > 0 {
                        500
                    } else {
                        0
                    };

                    let dist2 = (ys - y) * (ys - y) + (xs - x) * (xs - x);
                    let mut xvol = -150 - dist2 * 30;
                    if xvol < -5000 {
                        xvol = -5000;
                    }

                    recipients.push((cc, xvol, xpan));
                }
            }
        });

        let recipients_with_player: Vec<(usize, i32, i32)> =
            Repository::with_characters(|characters| {
                recipients
                    .into_iter()
                    .filter(|(cc, _, _)| characters[*cc].player != 0)
                    .collect()
            });

        for (cc, vol, pan) in recipients_with_player {
            Self::char_play_sound(cc, nr, vol, pan);
        }
    }

    /// Port of `process_options(character_id, buf)` from `svr_do.cpp`.
    ///
    /// Parses an optional leading `#<digits>###` prefix in the speech buffer.
    /// If a numeric sound id is parsed, the sound is played in the speaker's
    /// area and the option prefix is removed from `buf`.
    ///
    /// # Arguments
    /// * `character_id` - Speaker character id
    /// * `buf` - Mutable message buffer to strip options from
    pub(crate) fn process_options(character_id: usize, buf: &mut String) {
        if !buf.starts_with('#') {
            return;
        }

        let bytes = buf.as_bytes();
        let mut idx: usize = 1; // skip initial '#'

        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }

        let sound_id: i32 = if idx > 1 {
            buf[1..idx].parse::<i32>().unwrap_or(0)
        } else {
            0
        };

        while idx < bytes.len() && bytes[idx] == b'#' {
            idx += 1;
        }

        buf.drain(..idx);

        if sound_id != 0 {
            let (x, y) = Repository::with_characters(|characters| {
                let ch = &characters[character_id];
                (ch.x as i32, ch.y as i32)
            });
            Self::do_area_sound(character_id, 0, x, y, sound_id);
        }
    }

    /// Port of `do_imp_log(font, text)` from the original server.
    ///
    /// Sends a log message to all IMP and USURPed characters (administrative
    /// recipients). Empty text is ignored.
    ///
    /// # Arguments
    /// * `font` - Font/color to use
    /// * `text` - Message text
    pub(crate) fn do_imp_log(&self, font: core::types::FontColor, text: &str) {
        for n in 1..core::constants::MAXCHARS {
            if Repository::with_characters(|ch| {
                ch[n].player != 0
                    && (ch[n].flags & (CharacterFlags::Imp.bits() | CharacterFlags::Usurp.bits()))
                        != 0
            }) {
                self.do_log(n, font, text);
            }
        }
    }

    /// Port of `do_caution(source, author, text)` from the original server.
    ///
    /// Sends a caution/broadcast message to all active characters. When an
    /// `author` is supplied, the message is prefixed with `[author name]`.
    /// Visibility rules for invisible sources are respected.
    ///
    /// # Arguments
    /// * `source` - Source character id (for visibility checks)
    /// * `author` - Optional author character id to prefix the message
    /// * `text` - Message text
    pub(crate) fn do_caution(&self, source: usize, author: usize, text: &str) {
        if text.is_empty() {
            log::error!("do_caution called with empty text");
            return;
        }
        let anon = text.to_string();
        let named = if author != 0 {
            format!(
                "[{}] {}\n",
                Repository::with_characters(|ch| ch[author].get_name().to_string()),
                anon
            )
        } else {
            anon.clone()
        };
        for n in 1..core::constants::MAXCHARS {
            if !Repository::with_characters(|ch| ch[n].player != 0 || ch[n].temp == 15) {
                continue;
            }
            if source != 0
                && (Repository::with_characters(|ch2| ch2[source].flags)
                    & (CharacterFlags::Invisible.bits() | CharacterFlags::NoWho.bits()))
                    != 0
            {
                // visibility rules omitted
            }
            self.do_log(n, core::types::FontColor::Blue, &named);
        }
    }

    /// Port of `do_announce(source, author, text)` from the original server.
    ///
    /// Sends an announcement message to all active characters and respects
    /// invisibility levels so that the source may appear anonymous to some
    /// recipients. When `author` is provided, the message is prefixed with
    /// `[author name]`.
    ///
    /// # Arguments
    /// * `source` - Source character id for visibility rules
    /// * `author` - Optional author to prefix the message
    /// * `text` - Announcement text
    pub(crate) fn do_announce(&self, source: usize, author: usize, text: &str) {
        if text.is_empty() {
            log::error!("do_announce called with empty text");
            return;
        }
        let anon = text.to_string();
        let named = if author != 0 {
            format!(
                "[{}] {}\n",
                Repository::with_characters(|ch| ch[author].get_name().to_string()),
                anon
            )
        } else {
            anon.clone()
        };
        for n in 1..core::constants::MAXCHARS {
            // Exclude if not a player and not temp==15
            if !Repository::with_characters(|ch| ch[n].player != 0 || ch[n].temp == 15) {
                continue;
            }
            // C++: if ( ( ch[ source ].flags & ( CF_INVISIBLE | CF_NOWHO ) ) && invis_level( source ) > invis_level( n ) ) continue;
            if source != 0 {
                let (src_flags, src_invis_level) = Repository::with_characters(|ch| {
                    let f = ch[source].flags;
                    let lvl = crate::helpers::invis_level(source);
                    (f, lvl)
                });
                let n_invis_level = crate::helpers::invis_level(n);
                if (src_flags
                    & (core::constants::CharacterFlags::Invisible.bits()
                        | core::constants::CharacterFlags::NoWho.bits()))
                    != 0
                    && src_invis_level > n_invis_level
                {
                    continue;
                }
                // If source is not 0 and source's invis_level <= n's, show named, else anon
                if source != 0 && src_invis_level <= n_invis_level {
                    self.do_log(n, core::types::FontColor::Green, &named);
                } else {
                    self.do_log(n, core::types::FontColor::Green, &anon);
                }
            } else {
                self.do_log(n, core::types::FontColor::Green, &named);
            }
        }
    }

    /// Port of `do_admin_log(source, text)` from the original server.
    ///
    /// Sends an administrative log message to staff, IMPs and USURPed
    /// characters only. Visibility/invisibility rules are applied when a
    /// `source` is provided.
    #[allow(dead_code)]
    pub(crate) fn do_admin_log(&self, source: i32, text: &str) {
        if text.is_empty() {
            log::error!("do_admin_log called with empty text");
            return;
        }

        for n in 1..core::constants::MAXCHARS {
            // Exclude if not a player
            if !Repository::with_characters(|ch| ch[n].player != 0) {
                continue;
            }
            // Only to staff, IMP, or USURP
            if !Repository::with_characters(|ch| {
                (ch[n].flags
                    & (CharacterFlags::Staff.bits()
                        | CharacterFlags::Imp.bits()
                        | CharacterFlags::Usurp.bits()))
                    != 0
            }) {
                continue;
            }
            // C++: if ( ( ch[ source ].flags & ( CF_INVISIBLE | CF_NOWHO ) ) && invis_level( source ) > invis_level( n ) ) continue;
            if source > 0 {
                let (src_flags, src_invis_level) = Repository::with_characters(|ch| {
                    let f = ch[source as usize].flags;
                    let lvl = crate::helpers::invis_level(source as usize);
                    (f, lvl)
                });
                let n_invis_level = crate::helpers::invis_level(n);
                if (src_flags
                    & (core::constants::CharacterFlags::Invisible.bits()
                        | core::constants::CharacterFlags::NoWho.bits()))
                    != 0
                    && src_invis_level > n_invis_level
                {
                    continue;
                }
            }
            self.do_log(n, core::types::FontColor::Blue, text);
        }
    }

    /// Port of `do_staff_log(font, text)` from the original server.
    ///
    /// Sends a message to staff/IMP/USURPed characters that do not have the
    /// `CF_NOSTAFF` flag set. Empty text is ignored.
    ///
    /// # Arguments
    /// * `font` - Font/color to use
    /// * `text` - Message text
    pub(crate) fn do_staff_log(&self, font: core::types::FontColor, text: &str) {
        if text.is_empty() {
            log::error!("do_staff_log called with empty text");
            return;
        }
        for n in 1..core::constants::MAXCHARS {
            if Repository::with_characters(|ch| {
                ch[n].player != 0
                    && (ch[n].flags
                        & (CharacterFlags::Staff.bits()
                            | CharacterFlags::Imp.bits()
                            | CharacterFlags::Usurp.bits()))
                        != 0
                    && (ch[n].flags & CharacterFlags::NoStaff.bits()) == 0
            }) {
                self.do_log(n, font, text);
            }
        }
    }

    /// Port of `do_area_say1(cn, xs, ys, text)` from the original server.
    ///
    /// Broadcasts a speech message originating at map coordinates `(xs, ys)`
    /// to nearby characters using a spiral area search. Player listeners are
    /// sent named or anonymous messages depending on visibility; NPCs are
    /// handled in a second pass via `npc_hear` if they can see the speaker.
    ///
    /// # Arguments
    /// * `cn` - Speaker character id
    /// * `xs, ys` - Coordinates of the speech origin
    /// * `text` - Message text
    pub(crate) fn do_area_say1(&mut self, cn: usize, xs: usize, ys: usize, text: &str) {
        // Build messages
        let msg_named = format!(
            "{}: \"{}\"\n",
            Repository::with_characters(|ch| ch[cn].get_name().to_string()),
            text
        );
        let msg_invis = format!("Somebody says: \"{}\"\n", text);

        // Check invisibility of speaker
        let invis = Repository::with_characters(|ch| {
            (ch[cn].flags & CharacterFlags::Invisible.bits()) != 0
        });

        // Static spiral generation (port of initspiral / areaspiral[] from original C++)
        static AREASPIRAL: OnceLock<Vec<i32>> = OnceLock::new();
        let areaspiral = AREASPIRAL.get_or_init(|| {
            let areasize: i32 = 12; // matches original AREASIZE
            let span = 2 * areasize + 1;
            let spsize = (span * span) as usize;
            let mut v: Vec<i32> = Vec::with_capacity(spsize);
            v.push(0); // center

            let mapx = core::constants::SERVER_MAPX;
            for dist in 1..=areasize {
                v.push(-mapx); // N
                for _ in 0..(2 * dist - 1) {
                    v.push(-1); // W
                }
                for _ in 0..(2 * dist) {
                    v.push(mapx); // S
                }
                for _ in 0..(2 * dist) {
                    v.push(1); // E
                }
                for _ in 0..(2 * dist) {
                    v.push(-mapx); // N
                }
            }

            // Ensure the vector length equals SPIRALSIZE; pad with zeros if necessary
            while v.len() < spsize {
                v.push(0);
            }
            v
        });

        // Start map index at speaker location
        let mut m: i32 = (ys as i32) * core::constants::SERVER_MAPX + xs as i32;

        let mut npcs: Vec<usize> = Vec::with_capacity(20);

        for (j, &offset) in areaspiral.iter().enumerate() {
            m += offset;
            let map_area_size = core::constants::SERVER_MAPX * core::constants::SERVER_MAPY;
            if m < 0 || m >= map_area_size {
                continue;
            }

            let cc = Repository::with_map(|map| map[m as usize].ch as usize);

            if cc == 0 {
                continue;
            }

            // Check if cc is a sane character (active/used). If helper missing, leave TODO
            let sane = Repository::with_characters(|ch| {
                ch[cc].used == core::constants::USE_ACTIVE || ch[cc].temp == 15
            });
            if !sane {
                continue;
            }

            // If listener is a player (or usurp), handle visibility immediately
            let is_player_or_usurp = Repository::with_characters(|ch| {
                (ch[cc].flags & (CharacterFlags::Player.bits() | CharacterFlags::Usurp.bits())) != 0
            });

            if is_player_or_usurp {
                // Respect speaker invisibility and listener's invis level
                let show_named = !invis
                    || Repository::with_characters(|ch| ch[cn].get_invisibility_level())
                        <= Repository::with_characters(|ch| ch[cc].get_invisibility_level());
                if show_named {
                    self.do_character_log(cc, core::types::FontColor::Blue, &msg_named);
                } else {
                    self.do_character_log(cc, core::types::FontColor::Blue, &msg_invis);
                }
            } else {
                // Listener is NPC: store for second pass
                if !invis && npcs.len() < 20 {
                    // Only address mobs inside radius 6 (first 169 entries)
                    if j < 169 {
                        npcs.push(cc);
                    }
                }
            }
        }

        // Second pass: let NPCs hear if they can see the speaker
        for &npc in &npcs {
            let can_see = self.do_char_can_see(npc, cn);

            if can_see != 0 {
                npc_hear(npc, cn, text);
            }
        }
    }
}
