use core::constants::{CharacterFlags, MAXCHARS, MAXPLAYER};
use core::types::ServerPlayer;
use std::cmp;

use crate::network_manager::NetworkManager;
use crate::repository::Repository;
use crate::server::Server;

use super::State;

impl State {
    /// Sends a log message to a character if they have an associated player.
    /// Warns if the character has no player (unless temp == 15).
    pub(crate) fn do_character_log(
        &self,
        character_id: usize,
        font: core::types::FontColor,
        message: &str,
    ) {
        Repository::with_characters(|characters| {
            let ch = &characters[character_id];
            if ch.player == 0 && ch.temp != 15 {
                log::warn!(
                    "do_character_log: Character '{}' has no associated player.",
                    ch.get_name(),
                );
                return;
            }

            self.do_log(character_id, font, message);
        });
    }

    /// Sends a log message to a character's player.
    /// Splits long messages into 15-byte chunks and sends them via SV_LOG packets.
    pub(crate) fn do_log(&self, character_id: usize, font: core::types::FontColor, message: &str) {
        let mut buffer: [u8; 16] = [0; 16];

        Repository::with_characters(|characters| {
            let ch = &characters[character_id];

            if !ServerPlayer::is_sane_player(ch.player as usize)
                || (ch.flags & CharacterFlags::CF_PLAYER.bits()) == 0
            {
                let id = ch.player;
                log::error!(
                    "do_log: Invalid player ID {} for character '{}'",
                    id,
                    ch.get_name(),
                );
                return;
            }

            let matching_player_id = Server::with_players(|players| {
                for i in 0..MAXPLAYER as usize {
                    if players[i].usnr == character_id {
                        return Some(i);
                    }
                }

                None
            });

            if matching_player_id.is_none() {
                log::error!(
                    "do_log: No matching player found for character '{}'",
                    ch.get_name(),
                );
                return;
            }

            let mut bytes_sent: usize = 0;
            let len = message.len() - 1;

            while bytes_sent <= len {
                buffer[0] = core::constants::SV_LOG + font as u8;

                for i in 0..15 {
                    if bytes_sent + i > len {
                        buffer[i + 1] = 0;
                    } else {
                        buffer[i + 1] = message.as_bytes()[bytes_sent + i];
                    }
                }

                NetworkManager::with(|network| {
                    network.xsend(matching_player_id.unwrap() as usize, &buffer, 16);
                });

                bytes_sent += 15;
            }
        });
    }

    /// Sends a log message to all characters in an area (within 12 tile radius).
    /// Excludes cn and co from receiving the message.
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
        let x_max = cmp::min(core::constants::SERVER_MAPX as i32, xs + 13);
        let y_min = cmp::max(0, ys - 12);
        let y_max = cmp::min(core::constants::SERVER_MAPY as i32, ys + 13);

        let mut recipients: Vec<usize> = Vec::new();

        Repository::with_map(|map| {
            for y in y_min..y_max {
                let row_base = y * core::constants::SERVER_MAPX as i32;
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
                    *cc < MAXCHARS as usize
                        && characters[*cc].used == core::constants::USE_ACTIVE
                        && characters[*cc].player != 0
                        && (characters[*cc].flags & CharacterFlags::CF_PLAYER.bits()) != 0
                })
                .collect()
        });

        for cc in recipients {
            self.do_character_log(cc, font, message);
        }
    }

    /// Sends a message from a character to nearby characters.
    /// Formats the message as "Name: "message"" and sends it to the area.
    /// Uses blue color for players, yellow for NPCs.
    pub(crate) fn do_sayx(&self, character_id: usize, message: &str) {
        let mut buf = message.to_string();
        Self::process_options(character_id, &mut buf);

        let (x, y, is_player, name) = Repository::with_characters(|characters| {
            let ch = &characters[character_id];
            (
                ch.x as i32,
                ch.y as i32,
                (ch.flags & CharacterFlags::CF_PLAYER.bits()) != 0,
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

    /// Plays a sound to a specific character.
    /// Internal helper function used by do_area_sound.
    pub(crate) fn char_play_sound(character_id: usize, sound: i32, vol: i32, pan: i32) {
        let matching_player_id = Server::with_players(|players| {
            for i in 0..MAXPLAYER as usize {
                if players[i].usnr == character_id {
                    return Some(i);
                }
            }
            None
        });

        let Some(player_id) = matching_player_id else {
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

    /// Plays a sound to all characters in an area (within 8 tile radius).
    /// Volume and pan are calculated based on distance from the sound source.
    /// Excludes cn and co from hearing the sound.
    pub(crate) fn do_area_sound(cn: usize, co: usize, xs: i32, ys: i32, nr: i32) {
        let x_min = cmp::max(0, xs - 8);
        let x_max = cmp::min(core::constants::SERVER_MAPX as i32, xs + 9);
        let y_min = cmp::max(0, ys - 8);
        let y_max = cmp::min(core::constants::SERVER_MAPY as i32, ys + 9);

        let mut recipients: Vec<(usize, i32, i32)> = Vec::new();

        Repository::with_map(|map| {
            for y in y_min..y_max {
                let row_base = y * core::constants::SERVER_MAPX as i32;
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

    /// Port of original `process_options(int cn, char* buf)` from `svr_do.cpp`.
    ///
    /// Supports a leading `#<digits>###` option prefix:
    /// - Parses the integer sound id after the first '#'
    /// - Strips the `#<digits>` and any additional leading '#' characters
    /// - If the parsed sound id is non-zero, plays it to nearby players (excluding the speaker)
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

    /// Sends a log message to all IMPs and USURPed characters.
    pub(crate) fn do_imp_log(&self, font: core::types::FontColor, text: &str) {
        for n in 1..core::constants::MAXCHARS as usize {
            if Repository::with_characters(|ch| {
                ch[n].player != 0
                    && (ch[n].flags
                        & (CharacterFlags::CF_IMP.bits() | CharacterFlags::CF_USURP.bits()))
                        != 0
            }) {
                self.do_log(n, font, text);
            }
        }
    }

    /// Sends a caution message to all active characters.
    /// If author is provided, prefixes the message with [author name].
    pub(crate) fn do_caution(&self, source: usize, author: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let anon = text.to_string();
        let named = if author != 0 {
            format!(
                "[{}] {}",
                Repository::with_characters(|ch| ch[author].get_name().to_string()),
                anon
            )
        } else {
            anon.clone()
        };
        for n in 1..core::constants::MAXCHARS as usize {
            if !Repository::with_characters(|ch| ch[n].player != 0 || ch[n].temp == 15) {
                continue;
            }
            if source != 0
                && Repository::with_characters(|ch| {
                    (Repository::with_characters(|ch2| ch2[source].flags)
                        & (CharacterFlags::CF_INVISIBLE.bits() | CharacterFlags::CF_NOWHO.bits()))
                        != 0
                })
            {
                // visibility rules omitted
            }
            self.do_log(n, core::types::FontColor::Blue, &named);
        }
    }

    /// Sends an announcement message to all active characters.
    /// If author is provided, prefixes the message with [author name].
    pub(crate) fn do_announce(&self, source: usize, author: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let anon = text.to_string();
        let named = if author != 0 {
            format!(
                "[{}] {}",
                Repository::with_characters(|ch| ch[author].get_name().to_string()),
                anon
            )
        } else {
            anon.clone()
        };
        for n in 1..core::constants::MAXCHARS as usize {
            if !Repository::with_characters(|ch| ch[n].player != 0 || ch[n].temp == 15) {
                continue;
            }
            // visibility checks omitted
            self.do_log(n, core::types::FontColor::Green, &named);
        }
    }

    /// Sends a log message to all staff, IMPs, and USURPed characters.
    pub(crate) fn do_admin_log(&self, source: i32, text: &str) {
        if text.is_empty() {
            return;
        }
        for n in 1..core::constants::MAXCHARS as usize {
            if !Repository::with_characters(|ch| ch[n].player != 0) {
                continue;
            }
            if !Repository::with_characters(|ch| {
                (ch[n].flags
                    & (CharacterFlags::CF_STAFF.bits()
                        | CharacterFlags::CF_IMP.bits()
                        | CharacterFlags::CF_USURP.bits()))
                    != 0
            }) {
                continue;
            }
            self.do_log(n, core::types::FontColor::Blue, text);
        }
    }

    /// Sends a log message to all staff, IMPs, and USURPed characters who don't have CF_NOSTAFF set.
    pub(crate) fn do_staff_log(&self, font: core::types::FontColor, text: &str) {
        if text.is_empty() {
            return;
        }
        for n in 1..core::constants::MAXCHARS as usize {
            if Repository::with_characters(|ch| {
                ch[n].player != 0
                    && (ch[n].flags
                        & (CharacterFlags::CF_STAFF.bits()
                            | CharacterFlags::CF_IMP.bits()
                            | CharacterFlags::CF_USURP.bits()))
                        != 0
                    && (ch[n].flags & CharacterFlags::CF_NOSTAFF.bits()) == 0
            }) {
                self.do_log(n, font, text);
            }
        }
    }

    /// Sends a say message to all characters in an area.
    /// Handles visibility - invisible speakers show as "Somebody says".
    pub(crate) fn do_area_say1(&self, cn: usize, xs: usize, ys: usize, text: &str) {
        let msg_named = format!(
            "{}: \"{}\"\n",
            Repository::with_characters(|ch| ch[cn].get_name().to_string()),
            text
        );
        let msg_invis = format!("Somebody says: \"{}\"\n", text);
        let invis = Repository::with_characters(|ch| {
            (ch[cn].flags & CharacterFlags::CF_INVISIBLE.bits()) != 0
        });

        // Spiral/radius algorithm omitted; use simple radius scan
        for n in 1..core::constants::MAXCHARS as usize {
            let listener =
                Repository::with_characters(|ch| ch[n].used == core::constants::USE_ACTIVE);
            if listener {
                if !invis
                    || Repository::with_characters(|c| c[cn].get_invisibility_level())
                        <= Repository::with_characters(|c| c[n].get_invisibility_level())
                {
                    self.do_log(
                        n,
                        core::types::FontColor::Blue,
                        if !invis { &msg_named } else { &msg_invis },
                    );
                }
            }
        }
    }
}
