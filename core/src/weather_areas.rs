//! Per-area default weather table.
//!
//! When no admin override is active, the server's per-second tick driver
//! looks up the player's current map coordinates here and sends the matching
//! [`WeatherKind`] (or [`WeatherKind::None`] if no entry matches).
//!
//! The lookup is by area **name** so this table stays decoupled from the
//! exact ordering of [`crate::area::AREAS`]. Names that don't match any
//! defined area are silently ignored so the table can be edited freely.
//!
//! Adding entries here is a one-line change; everything is `const`.

use crate::area::AREAS;
use crate::weather::WeatherKind;

/// Default weather for an area, looked up at runtime by the server.
pub struct AreaWeather {
    /// Matches [`crate::area::Area::name`].
    pub area_name: &'static str,
    /// Weather kind to broadcast to players inside this area.
    pub kind: WeatherKind,
    /// Particle/effect intensity (0..=255).
    pub intensity: u8,
    /// Optional tint override; `[0,0,0,0]` keeps the kind's client default.
    pub tint: [u8; 4],
    /// Wire-protocol flags forwarded to the client (e.g. additive blending).
    pub flags: u8,
}

/// Static table of per-area ambient weather.
///
/// Order matters only when multiple entries match the same point; the first
/// matching entry wins (mirrors `get_area_m` semantics).
pub const AREA_WEATHER: &[AreaWeather] = &[
    AreaWeather {
        area_name: "Strange Forest",
        kind: WeatherKind::Fireflies,
        intensity: 64,
        tint: [0, 0, 0, 0],
        flags: 0,
    },
    AreaWeather {
        area_name: "Pentagram Quest",
        kind: WeatherKind::Fire,
        intensity: 180,
        // Strong red glow.
        tint: [200, 50, 30, 80],
        flags: 0,
    },
    AreaWeather {
        area_name: "Ice Pentagram Quest",
        kind: WeatherKind::Snow,
        intensity: 160,
        tint: [180, 200, 230, 50],
        flags: 0,
    },
];

/// Returns the [`AreaWeather`] entry whose area contains `(x, y)`, or `None`.
///
/// Iterates through [`AREA_WEATHER`] and, for each entry, checks whether any
/// area in [`AREAS`] with that name contains the point.
///
/// # Arguments
///
/// * `x` - Horizontal world tile coordinate.
/// * `y` - Vertical world tile coordinate.
///
/// # Returns
///
/// * `Some(&AreaWeather)` if the point lies in a configured area.
/// * `None` otherwise.
pub fn area_weather_for(x: i32, y: i32) -> Option<&'static AreaWeather> {
    for entry in AREA_WEATHER.iter() {
        for area in AREAS.iter() {
            if area.name == entry.area_name && area.contains(x, y) {
                return Some(entry);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_reference_existing_areas() {
        for entry in AREA_WEATHER.iter() {
            assert!(
                AREAS.iter().any(|a| a.name == entry.area_name),
                "AREA_WEATHER entry '{}' has no matching Area in AREAS",
                entry.area_name
            );
        }
    }

    #[test]
    fn lookup_returns_none_outside_any_area() {
        // Far edge of the world map is unlikely to be in any seeded area.
        assert!(area_weather_for(0, 0).is_none());
    }

    #[test]
    fn lookup_finds_strange_forest_fireflies() {
        // Strange Forest is roughly x=480..634, y=234..405.
        let entry = area_weather_for(550, 320).expect("Strange Forest lookup");
        assert_eq!(entry.kind, WeatherKind::Fireflies);
        assert_eq!(entry.intensity, 64);
    }
}
