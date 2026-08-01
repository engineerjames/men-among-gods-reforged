//! Shared weather / ambient effect kinds used by `SV_WEATHER` (`SetWeather`).
//!
//! The server picks a [`WeatherKind`] for each player (either an admin
//! override or an area-driven default) and pushes it via the `SV_WEATHER`
//! opcode in [`crate::server_commands`]. The client renders particles and
//! a tint overlay based on the kind.

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    /// Bitmask carried in [`SetWeather.flags`](crate::server_commands::ServerCommandData::SetWeather).
    pub struct WeatherFlags: u8 {
        /// Marks the active weather as an admin override; the server's
        /// area-tick driver will not replace overridden weather until it expires.
        const Override = 0b1000_0000;
        /// Hints the client to use additive blending for particles
        /// (e.g. fire, embers).
        const Additive = 0b0000_0001;
    }
}

/// All weather / ambient effect kinds the server can request.
///
/// Numeric values are part of the wire protocol — do not renumber.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum WeatherKind {
    /// No weather effect; clears any active overlay.
    None = 0,
    /// Angled rain particles + faint blue tint.
    Rain = 1,
    /// Slow-falling white dot particles.
    Snow = 2,
    /// Sparse pulsing yellow-green dots.
    Fireflies = 3,
    /// Rising orange/red particles (additive); good for the Pentagram quest.
    Fire = 4,
    /// Persistent red tint, no particles.
    BloodMoon = 5,
    /// Gray/white tint plus drifting puffs.
    Fog = 6,
    /// Periodic full-screen white flash; dim cool tint between strikes.
    Lightning = 7,
    /// Slower-rising fire-style particles.
    Embers = 8,
    /// Drifting falling leaves / petals.
    Leaves = 9,
    /// Subtle yellow tint plus shimmer particles (v1 fallback).
    HeatHaze = 10,
    /// Cycling green→purple gradient strip near the top of the screen.
    Aurora = 11,
    /// Camera shake; no particles.
    Earthquake = 12,
}

impl Default for WeatherKind {
    /// Returns [`WeatherKind::None`].
    fn default() -> Self {
        WeatherKind::None
    }
}

impl WeatherKind {
    /// Returns the wire-protocol byte representation.
    ///
    /// # Returns
    ///
    /// * The discriminant byte for this kind.
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

impl From<u8> for WeatherKind {
    /// Decodes a wire byte into a [`WeatherKind`].
    ///
    /// Unknown values map to [`WeatherKind::None`] so the client can
    /// safely ignore future protocol additions without crashing.
    ///
    /// # Arguments
    ///
    /// * `value` - The byte value to decode.
    ///
    /// # Returns
    ///
    /// * The matching [`WeatherKind`], or [`WeatherKind::None`] if unknown.
    fn from(value: u8) -> Self {
        match value {
            0 => WeatherKind::None,
            1 => WeatherKind::Rain,
            2 => WeatherKind::Snow,
            3 => WeatherKind::Fireflies,
            4 => WeatherKind::Fire,
            5 => WeatherKind::BloodMoon,
            6 => WeatherKind::Fog,
            7 => WeatherKind::Lightning,
            8 => WeatherKind::Embers,
            9 => WeatherKind::Leaves,
            10 => WeatherKind::HeatHaze,
            11 => WeatherKind::Aurora,
            12 => WeatherKind::Earthquake,
            _ => WeatherKind::None,
        }
    }
}

/// Returns the [`WeatherKind`] matching a case-insensitive name, or `None`
/// if the name is not a recognized kind.
///
/// Used by admin god commands such as `weather rain 200 30`.
///
/// # Arguments
///
/// * `name` - The textual name of the kind (e.g. `"rain"`, `"BloodMoon"`).
///
/// # Returns
///
/// * `Some(kind)` if the name matches a known kind.
/// * `None` otherwise.
pub fn parse_weather_name(name: &str) -> Option<WeatherKind> {
    let lower = name.to_ascii_lowercase();
    Some(match lower.as_str() {
        "none" | "clear" | "off" => WeatherKind::None,
        "rain" => WeatherKind::Rain,
        "snow" => WeatherKind::Snow,
        "fireflies" | "firefly" => WeatherKind::Fireflies,
        "fire" => WeatherKind::Fire,
        "bloodmoon" | "blood_moon" | "blood-moon" => WeatherKind::BloodMoon,
        "fog" | "mist" => WeatherKind::Fog,
        "lightning" | "thunder" | "storm" => WeatherKind::Lightning,
        "embers" | "ember" => WeatherKind::Embers,
        "leaves" | "leaf" | "petals" => WeatherKind::Leaves,
        "heathaze" | "heat_haze" | "heat-haze" | "haze" => WeatherKind::HeatHaze,
        "aurora" => WeatherKind::Aurora,
        "earthquake" | "quake" | "shake" => WeatherKind::Earthquake,
        _ => return None,
    })
}

/// Returns the chat line shown to a player when area-driven weather starts
/// `kind`, or `None` for [`WeatherKind::None`] (nothing to announce).
///
/// # Arguments
///
/// * `kind` - The weather kind that just became active.
///
/// # Returns
///
/// * `Some(message)` for every kind except `None`.
pub fn weather_start_message(kind: WeatherKind) -> Option<&'static str> {
    Some(match kind {
        WeatherKind::None => return None,
        WeatherKind::Rain => "Dark clouds gather and it begins to rain.",
        WeatherKind::Snow => "Snow begins to fall.",
        WeatherKind::Fireflies => "Fireflies drift out of the undergrowth.",
        WeatherKind::Fire => "The air grows hot as flames spring up around you.",
        WeatherKind::BloodMoon => "The moon turns blood red.",
        WeatherKind::Fog => "A thick fog rolls in.",
        WeatherKind::Lightning => "Thunder rumbles as a storm rolls in.",
        WeatherKind::Embers => "Embers drift through the air.",
        WeatherKind::Leaves => "A breeze scatters leaves through the air.",
        WeatherKind::HeatHaze => "Heat shimmers in the air around you.",
        WeatherKind::Aurora => "Strange lights ripple across the sky.",
        WeatherKind::Earthquake => "The ground begins to shake!",
    })
}

/// Returns the chat line shown to a player when area-driven weather `kind`
/// ends, or `None` for [`WeatherKind::None`] (nothing to announce).
///
/// # Arguments
///
/// * `kind` - The weather kind that just ended.
///
/// # Returns
///
/// * `Some(message)` for every kind except `None`.
pub fn weather_stop_message(kind: WeatherKind) -> Option<&'static str> {
    Some(match kind {
        WeatherKind::None => return None,
        WeatherKind::Rain => "The rain stops falling.",
        WeatherKind::Snow => "The snow stops falling.",
        WeatherKind::Fireflies => "The fireflies fade away.",
        WeatherKind::Fire => "The flames die down.",
        WeatherKind::BloodMoon => "The moon returns to its normal color.",
        WeatherKind::Fog => "The fog lifts.",
        WeatherKind::Lightning => "The storm passes.",
        WeatherKind::Embers => "The embers fade away.",
        WeatherKind::Leaves => "The breeze dies down.",
        WeatherKind::HeatHaze => "The heat haze fades.",
        WeatherKind::Aurora => "The lights in the sky fade away.",
        WeatherKind::Earthquake => "The shaking subsides.",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_none() {
        assert_eq!(WeatherKind::default(), WeatherKind::None);
    }

    #[test]
    fn roundtrip_all_known_kinds() {
        let all = [
            WeatherKind::None,
            WeatherKind::Rain,
            WeatherKind::Snow,
            WeatherKind::Fireflies,
            WeatherKind::Fire,
            WeatherKind::BloodMoon,
            WeatherKind::Fog,
            WeatherKind::Lightning,
            WeatherKind::Embers,
            WeatherKind::Leaves,
            WeatherKind::HeatHaze,
            WeatherKind::Aurora,
            WeatherKind::Earthquake,
        ];
        for k in all {
            assert_eq!(WeatherKind::from(k.as_u8()), k);
        }
    }

    #[test]
    fn start_and_stop_messages_cover_every_non_none_kind() {
        let all = [
            WeatherKind::Rain,
            WeatherKind::Snow,
            WeatherKind::Fireflies,
            WeatherKind::Fire,
            WeatherKind::BloodMoon,
            WeatherKind::Fog,
            WeatherKind::Lightning,
            WeatherKind::Embers,
            WeatherKind::Leaves,
            WeatherKind::HeatHaze,
            WeatherKind::Aurora,
            WeatherKind::Earthquake,
        ];
        for k in all {
            assert!(
                weather_start_message(k).is_some(),
                "missing start message for {k:?}"
            );
            assert!(
                weather_stop_message(k).is_some(),
                "missing stop message for {k:?}"
            );
        }
        assert!(weather_start_message(WeatherKind::None).is_none());
        assert!(weather_stop_message(WeatherKind::None).is_none());
    }

    #[test]
    fn unknown_byte_decodes_to_none() {
        assert_eq!(WeatherKind::from(200), WeatherKind::None);
        assert_eq!(WeatherKind::from(13), WeatherKind::None);
    }

    #[test]
    fn parse_name_known_aliases() {
        assert_eq!(parse_weather_name("Rain"), Some(WeatherKind::Rain));
        assert_eq!(
            parse_weather_name("BLOOD_MOON"),
            Some(WeatherKind::BloodMoon)
        );
        assert_eq!(parse_weather_name("quake"), Some(WeatherKind::Earthquake));
        assert_eq!(parse_weather_name("clear"), Some(WeatherKind::None));
    }

    #[test]
    fn parse_name_unknown_returns_none() {
        assert!(parse_weather_name("hurricane").is_none());
        assert!(parse_weather_name("").is_none());
    }
}
