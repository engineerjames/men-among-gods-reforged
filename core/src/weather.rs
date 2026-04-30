//! Shared weather / ambient effect kinds used by `SV_WEATHER` (`SetWeather`).
//!
//! The server picks a [`WeatherKind`] for each player (either an admin
//! override or an area-driven default) and pushes it via the `SV_WEATHER`
//! opcode in [`crate::server_commands`]. The client renders particles and
//! a tint overlay based on the kind.

/// Bit in [`SetWeather.flags`](crate::server_commands::ServerCommandData::SetWeather)
/// that marks the active weather as an admin override; the server's
/// area-tick driver will not replace overridden weather until it expires.
pub const WEATHER_FLAG_OVERRIDE: u8 = 0b1000_0000;

/// Bit in [`SetWeather.flags`](crate::server_commands::ServerCommandData::SetWeather)
/// hinting that the client should use additive blending for particles
/// (e.g. fire, embers).
pub const WEATHER_FLAG_ADDITIVE: u8 = 0b0000_0001;

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
