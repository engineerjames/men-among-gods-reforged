//! Shared KeyDB key schema and payload for the server's public live status.
//!
//! The game server publishes a compact [`LiveStatus`] snapshot to
//! [`LIVE_STATUS_KEY`] every [`LIVE_STATUS_INTERVAL_TICKS`] ticks with a TTL
//! of [`LIVE_STATUS_TTL_SECS`]. The API serves it to the public (`/world/*`)
//! without touching the much larger and much staler `game:char:*` /
//! `game:global` persistence keys. If the key is missing the server is
//! either down or not yet publishing.

use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::constants::CharacterFlags;
use crate::types::Global;

/// KeyDB key holding the bincode-encoded [`LiveStatus`].
pub const LIVE_STATUS_KEY: &str = "game:live:status";

/// Ticks between live status publications (~10 s at 36 TPS).
pub const LIVE_STATUS_INTERVAL_TICKS: u32 = 360;

/// TTL applied to [`LIVE_STATUS_KEY`]; several publish intervals so a
/// hiccup does not flap the API, but short enough to detect a dead server.
pub const LIVE_STATUS_TTL_SECS: u64 = 60;

/// In-game seconds per in-game hour.
const MD_HOUR: i32 = 3600;

/// Days per in-game month (one lunar cycle).
const MD_MONTH_DAYS: i32 = 28;

/// Lunar phase derived from the day of the month.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Encode, Decode)]
#[serde(rename_all = "snake_case")]
pub enum MoonPhase {
    /// First day of the month.
    #[default]
    New,
    /// Days 2 through 14.
    Growing,
    /// Day 15.
    Full,
    /// Days 16 through 28.
    Dwindling,
}

impl MoonPhase {
    /// Derive the phase from the raw `mdday` counter.
    ///
    /// Mirrors the original `svr_god.c`/`info.c` logic and the server's
    /// `global_tick` (`newmoon` on day 1, `fullmoon` on day 15).
    ///
    /// # Arguments
    ///
    /// * `mdday` - Day-of-year counter from [`Global::mdday`].
    ///
    /// # Returns
    ///
    /// * The lunar phase for that day.
    pub fn from_mdday(mdday: i32) -> Self {
        match mdday.rem_euclid(MD_MONTH_DAYS) + 1 {
            1 => Self::New,
            2..=14 => Self::Growing,
            15 => Self::Full,
            _ => Self::Dwindling,
        }
    }

    /// Player-facing sentence for this phase, as printed by the original server.
    ///
    /// # Returns
    ///
    /// * A static sentence such as `"The Moon is growing."`.
    pub fn description(self) -> &'static str {
        match self {
            Self::New => "New Moon tonight!",
            Self::Growing => "The Moon is growing.",
            Self::Full => "Full Moon tonight!",
            Self::Dwindling => "The moon is dwindling.",
        }
    }
}

/// English ordinal suffix (`st`, `nd`, `rd`, `th`) for a day or month number.
///
/// # Arguments
///
/// * `n` - Positive number to suffix.
///
/// # Returns
///
/// * The suffix without the number.
pub fn ordinal_suffix(n: i32) -> &'static str {
    if (11..=13).contains(&(n % 100)) {
        return "th";
    }
    match n % 10 {
        1 => "st",
        2 => "nd",
        3 => "rd",
        _ => "th",
    }
}

/// In-game calendar/clock state derived from [`Global`].
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Encode, Decode)]
pub struct WorldTime {
    /// Raw seconds elapsed in the current in-game day.
    pub mdtime: i32,
    /// Raw day-of-year counter.
    pub mdday: i32,
    /// In-game year.
    pub year: i32,
    /// Hour of the day (0-23).
    pub hour: i32,
    /// Minute of the hour (0-59).
    pub minute: i32,
    /// Day of the month (1-28).
    pub day: i32,
    /// Month of the year (1-based).
    pub month: i32,
    /// Current lunar phase.
    pub moon: MoonPhase,
    /// Ambient daylight level, 0 (night) to 255 (full day).
    pub daylight: i32,
}

impl WorldTime {
    /// Derive the calendar/clock from the live global counters.
    ///
    /// # Arguments
    ///
    /// * `global` - Server globals holding `mdtime`/`mdday`/`mdyear`/`dlight`.
    ///
    /// # Returns
    ///
    /// * A populated [`WorldTime`].
    pub fn from_global(global: &Global) -> Self {
        Self {
            mdtime: global.mdtime,
            mdday: global.mdday,
            year: global.mdyear,
            hour: global.mdtime / MD_HOUR,
            minute: (global.mdtime / 60) % 60,
            day: global.mdday.rem_euclid(MD_MONTH_DAYS) + 1,
            month: global.mdday / MD_MONTH_DAYS + 1,
            moon: MoonPhase::from_mdday(global.mdday),
            daylight: global.dlight,
        }
    }

    /// Format the in-game time the way the `#time` command does.
    ///
    /// # Returns
    ///
    /// * e.g. `"It's 0:40 on the 8th of the 11th month of the year 50."`
    pub fn sentence(&self) -> String {
        format!(
            "It's {}:{:02} on the {}{} of the {}{} month of the year {}.",
            self.hour,
            self.minute,
            self.day,
            ordinal_suffix(self.day),
            self.month,
            ordinal_suffix(self.month),
            self.year
        )
    }
}

/// Privilege/affiliation markers for a character, decoded from its flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Encode, Decode)]
pub struct CharacterRoles {
    /// `CF_GOD`.
    pub god: bool,
    /// `CF_CREATOR`.
    pub creator: bool,
    /// `CF_IMP`.
    pub imp: bool,
    /// `CF_STAFF`.
    pub staff: bool,
    /// `CF_USURP` (staff controlling an NPC body).
    pub usurp: bool,
    /// Purple (PvP-flagged) kindred.
    pub purple: bool,
    /// Member of the Purple Order of Heroes.
    pub poh: bool,
    /// Leader of the Purple Order of Heroes.
    pub poh_leader: bool,
}

impl CharacterRoles {
    /// Decode roles from a character's flag bits and kindred.
    ///
    /// # Arguments
    ///
    /// * `flags` - [`crate::types::Character::flags`] bit set.
    /// * `kindred` - [`crate::types::Character::kindred`] bit set.
    ///
    /// # Returns
    ///
    /// * The decoded roles.
    pub fn from_flags(flags: u64, kindred: i32) -> Self {
        let has = |f: CharacterFlags| flags & f.bits() != 0;
        Self {
            god: has(CharacterFlags::God),
            creator: has(CharacterFlags::Creator),
            imp: has(CharacterFlags::Imp),
            staff: has(CharacterFlags::Staff),
            usurp: has(CharacterFlags::Usurp),
            purple: (kindred as u32) & crate::traits::KIN_PURPLE != 0,
            poh: has(CharacterFlags::Poh),
            poh_leader: has(CharacterFlags::PohLeader),
        }
    }
}

/// One player on the public "who" list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
pub struct OnlinePlayer {
    /// Character name.
    pub name: String,
    /// Total experience points.
    pub points: i32,
    /// Full rank title for `points`.
    pub rank: String,
    /// Area name(s) the player is in, or `None` when hidden from normal
    /// players (gods and purple-kindred characters).
    pub area: Option<String>,
    /// Privilege/affiliation markers.
    pub roles: CharacterRoles,
}

/// Snapshot of live, non-persisted server state for public consumption.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Encode, Decode)]
pub struct LiveStatus {
    /// Unix timestamp (seconds) when the server built this snapshot.
    pub published_at: u64,
    /// Server uptime in ticks (divide by [`crate::constants::TICKS`] for seconds).
    pub uptime_ticks: i64,
    /// Server-side count of visible online players ([`Global::players_online`]).
    pub players_online: i32,
    /// All-time peak concurrent players ([`Global::max_online`]).
    pub max_online: i32,
    /// In-game clock and calendar.
    pub time: WorldTime,
    /// Players visible on the public who list, in slot order.
    pub online: Vec<OnlinePlayer>,
}

impl LiveStatus {
    /// Serialize to bincode bytes for KeyDB storage.
    ///
    /// # Returns
    ///
    /// * Encoded bytes, or the bincode error message.
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        bincode::encode_to_vec(self, bincode::config::standard()).map_err(|e| e.to_string())
    }

    /// Deserialize from bincode bytes read from KeyDB.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Encoded snapshot.
    ///
    /// # Returns
    ///
    /// * `Some` on success; `None` if the payload is malformed.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let (value, consumed): (Self, usize) =
            bincode::decode_from_slice(bytes, bincode::config::standard()).ok()?;
        (consumed == bytes.len()).then_some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moon_phase_follows_original_day_table() {
        // mdday is 0-based so day-of-month = mdday % 28 + 1.
        assert_eq!(MoonPhase::from_mdday(0), MoonPhase::New);
        assert_eq!(MoonPhase::from_mdday(28), MoonPhase::New);
        assert_eq!(MoonPhase::from_mdday(1), MoonPhase::Growing);
        assert_eq!(MoonPhase::from_mdday(13), MoonPhase::Growing);
        assert_eq!(MoonPhase::from_mdday(14), MoonPhase::Full);
        assert_eq!(MoonPhase::from_mdday(15), MoonPhase::Dwindling);
        assert_eq!(MoonPhase::from_mdday(27), MoonPhase::Dwindling);
    }

    #[test]
    fn ordinal_suffix_handles_teens() {
        assert_eq!(ordinal_suffix(1), "st");
        assert_eq!(ordinal_suffix(2), "nd");
        assert_eq!(ordinal_suffix(3), "rd");
        assert_eq!(ordinal_suffix(4), "th");
        assert_eq!(ordinal_suffix(11), "th");
        assert_eq!(ordinal_suffix(12), "th");
        assert_eq!(ordinal_suffix(13), "th");
        assert_eq!(ordinal_suffix(21), "st");
    }

    #[test]
    fn world_time_sentence_matches_show_time_format() {
        let global = Global {
            mdtime: 40 * 60,
            mdday: 10 * 28 + 7,
            mdyear: 50,
            dlight: 0,
            ..Global::default()
        };
        let time = WorldTime::from_global(&global);
        assert_eq!((time.hour, time.minute), (0, 40));
        assert_eq!((time.day, time.month, time.year), (8, 11, 50));
        assert_eq!(time.moon, MoonPhase::Growing);
        assert_eq!(
            time.sentence(),
            "It's 0:40 on the 8th of the 11th month of the year 50."
        );
    }

    #[test]
    fn roles_decode_from_flags_and_kindred() {
        let flags = (CharacterFlags::God | CharacterFlags::Staff | CharacterFlags::Poh).bits();
        let roles = CharacterRoles::from_flags(flags, crate::traits::KIN_PURPLE as i32);
        assert!(roles.god && roles.staff && roles.poh && roles.purple);
        assert!(!roles.imp && !roles.creator && !roles.usurp && !roles.poh_leader);
        assert_eq!(CharacterRoles::from_flags(0, 0), CharacterRoles::default());
    }

    #[test]
    fn live_status_roundtrip() {
        let status = LiveStatus {
            published_at: 1_700_000_000,
            uptime_ticks: 36 * 3600,
            players_online: 1,
            max_online: 47,
            time: WorldTime::from_global(&Global {
                mdtime: 12 * 3600 + 5 * 60,
                mdday: 14,
                mdyear: 3,
                ..Global::default()
            }),
            online: vec![OnlinePlayer {
                name: "Piltur".into(),
                points: 1234,
                rank: "Private".into(),
                area: Some("Aston".into()),
                roles: CharacterRoles::default(),
            }],
        };
        let bytes = status.to_bytes().unwrap();
        assert_eq!(LiveStatus::from_bytes(&bytes).unwrap(), status);
        assert!(LiveStatus::from_bytes(&bytes[..bytes.len() - 1]).is_none());
    }
}
