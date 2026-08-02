//! Per-area weather profiles: candidate kinds, trigger chance, duration.
//!
//! The server's `area_weather_system_tick` (see `server/src/state/weather.rs`)
//! periodically rolls each [`AreaWeatherProfile`] for a chance to start a new
//! weather effect, and the per-player `weather_tick` broadcasts whatever is
//! currently active in a player's area (or clears it back to
//! [`WeatherKind::None`] if nothing is active or the player left the area).
//!
//! Weather profiles are keyed by their own [`WeatherArea`] rectangles rather
//! than [`crate::area::Area`] names, so a profile can cover a custom region
//! independent of the named areas (e.g. one larger rectangle spanning
//! several of Aston's many small sub-areas), and can stitch together
//! multiple disjoint rectangles via `regions`.
//!
//! Adding entries here is a one-line change; everything is `const`.

use crate::weather::{WeatherFlags, WeatherKind};

/// An axis-aligned rectangular region used to define where a weather
/// pattern can trigger, independent of any named [`crate::area::Area`].
pub struct WeatherArea {
    /// Left edge (inclusive).
    pub x1: i32,
    /// Top edge (inclusive).
    pub y1: i32,
    /// Right edge (inclusive).
    pub x2: i32,
    /// Bottom edge (inclusive).
    pub y2: i32,
}

impl WeatherArea {
    /// Builds a new rectangle from its bounds.
    ///
    /// # Arguments
    ///
    /// * `x1` - Left edge (inclusive).
    /// * `y1` - Top edge (inclusive).
    /// * `x2` - Right edge (inclusive).
    /// * `y2` - Bottom edge (inclusive).
    ///
    /// # Returns
    ///
    /// * The constructed [`WeatherArea`].
    pub const fn new(x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    /// Returns `true` if the point (`x`, `y`) lies within this rectangle
    /// (inclusive).
    ///
    /// # Arguments
    ///
    /// * `x` - Horizontal map coordinate.
    /// * `y` - Vertical map coordinate.
    ///
    /// # Returns
    ///
    /// * `true` if the point is inside the bounding box.
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x1 && y >= self.y1 && x <= self.x2 && y <= self.y2
    }
}

/// One possible weather outcome for an [`AreaWeatherProfile`], picked by
/// weighted random when the profile's trigger check succeeds.
pub struct WeatherCandidate {
    /// Weather kind to broadcast if this candidate is picked.
    pub kind: WeatherKind,
    /// Relative weight used by [`pick_candidate`] (higher = more likely).
    pub weight: u8,
    /// Particle/effect intensity (0..=255).
    pub intensity: u8,
    /// Optional tint override; `None` keeps the kind's client default.
    pub tint: Option<[u8; 4]>,
    /// Wire-protocol flags forwarded to the client (e.g. additive blending).
    pub flags: WeatherFlags,
}

/// Probabilistic weather profile for a named area.
pub struct AreaWeatherProfile {
    /// Human-readable label used only for logging/debugging.
    pub label: &'static str,
    /// One or more rectangles that make up this profile's coverage; a point
    /// matches the profile if it falls inside any of them.
    pub regions: &'static [WeatherArea],
    /// Weighted list of possible weather kinds for this area.
    pub candidates: &'static [WeatherCandidate],
    /// Chance out of 1000 that a new weather effect starts each time the
    /// area is re-evaluated (see `AREA_EVAL_PERIOD_TICKS` server-side).
    pub trigger_chance_per_eval: u32,
    /// Minimum duration (ticks) once a weather effect triggers.
    pub min_duration_ticks: u32,
    /// Maximum duration (ticks) once a weather effect triggers.
    pub max_duration_ticks: u32,
    /// When `true`, the client keeps the effect fully visible even while
    /// standing on an indoor-flagged tile (see `WeatherFlags::IgnoreIndoorFade`).
    pub ignore_indoor_fade: bool,
}

/// Default trigger chance shared by areas without special tuning (40%).
const DEFAULT_TRIGGER_CHANCE: u32 = 400;

/// Default minimum duration shared by areas without special tuning (5 min at 36 TPS).
const DEFAULT_MIN_DURATION_TICKS: u32 = 36 * 60 * 5;

/// Default maximum duration shared by areas without special tuning (20 min at 36 TPS).
const DEFAULT_MAX_DURATION_TICKS: u32 = 36 * 60 * 20;

/// Static table of per-area weather profiles.
///
/// Order matters only when multiple entries match the same point; the first
/// matching entry wins (mirrors `get_area_m` semantics).
pub const AREA_WEATHER_PROFILES: &[AreaWeatherProfile] = &[
    AreaWeatherProfile {
        label: "Aston",
        regions: &[WeatherArea::new(481, 407, 633, 628)],
        candidates: &[WeatherCandidate {
            kind: WeatherKind::Rain,
            weight: 1,
            intensity: 128,
            tint: Some([100, 100, 120, 50]),
            flags: WeatherFlags::empty(),
        }],
        trigger_chance_per_eval: DEFAULT_TRIGGER_CHANCE,
        min_duration_ticks: DEFAULT_MIN_DURATION_TICKS,
        max_duration_ticks: DEFAULT_MAX_DURATION_TICKS,
        ignore_indoor_fade: false,
    },
    AreaWeatherProfile {
        label: "Strange Forest",
        regions: &[WeatherArea::new(480, 234, 634, 405)],
        candidates: &[WeatherCandidate {
            kind: WeatherKind::Fireflies,
            weight: 1,
            intensity: 64,
            tint: None,
            flags: WeatherFlags::empty(),
        }],
        trigger_chance_per_eval: DEFAULT_TRIGGER_CHANCE,
        min_duration_ticks: DEFAULT_MIN_DURATION_TICKS,
        max_duration_ticks: DEFAULT_MAX_DURATION_TICKS,
        ignore_indoor_fade: false,
    },
    AreaWeatherProfile {
        label: "Pentagram Quest",
        regions: &[WeatherArea::new(210, 300, 410, 600)],
        candidates: &[WeatherCandidate {
            kind: WeatherKind::Fire,
            weight: 1,
            intensity: 180,
            // Strong red glow.
            tint: Some([200, 50, 30, 80]),
            flags: WeatherFlags::Additive,
        }],
        trigger_chance_per_eval: DEFAULT_TRIGGER_CHANCE,
        min_duration_ticks: DEFAULT_MIN_DURATION_TICKS,
        max_duration_ticks: DEFAULT_MAX_DURATION_TICKS,
        ignore_indoor_fade: true,
    },
    AreaWeatherProfile {
        label: "Ice Pentagram Quest",
        regions: &[WeatherArea::new(330, 246, 408, 299)],
        candidates: &[WeatherCandidate {
            kind: WeatherKind::Snow,
            weight: 1,
            intensity: 160,
            tint: Some([180, 200, 230, 50]),
            flags: WeatherFlags::empty(),
        }],
        trigger_chance_per_eval: DEFAULT_TRIGGER_CHANCE,
        min_duration_ticks: DEFAULT_MIN_DURATION_TICKS,
        max_duration_ticks: DEFAULT_MAX_DURATION_TICKS,
        ignore_indoor_fade: true,
    },
];

/// Picks a [`WeatherCandidate`] from `profile` using an explicit weighted
/// `roll`, without touching any RNG itself (callers supply
/// `random_mod(total_weight(profile))`), so selection stays unit-testable.
///
/// # Arguments
///
/// * `profile` - Profile whose `candidates` to pick from.
/// * `roll` - A value expected to be in `0..total_weight(profile)`;
///   out-of-range rolls clamp to the last candidate.
///
/// # Returns
///
/// * The selected candidate. Panics if `profile.candidates` is empty (all
///   configured profiles must have at least one candidate).
pub fn pick_candidate(profile: &AreaWeatherProfile, roll: u32) -> &WeatherCandidate {
    let mut acc: u32 = 0;
    let last = profile
        .candidates
        .last()
        .expect("AreaWeatherProfile must have at least one candidate");
    for candidate in profile.candidates {
        acc += u32::from(candidate.weight);
        if roll < acc {
            return candidate;
        }
    }
    last
}

/// Returns the total weight of `profile`'s candidates, for rolling
/// `random_mod(total_weight(profile))` before calling [`pick_candidate`].
///
/// # Arguments
///
/// * `profile` - Profile to sum weights for.
///
/// # Returns
///
/// * Sum of all candidate weights.
pub fn total_weight(profile: &AreaWeatherProfile) -> u32 {
    profile.candidates.iter().map(|c| u32::from(c.weight)).sum()
}

/// Returns the [`AreaWeatherProfile`] whose area contains `(x, y)`, or `None`.
///
/// # Arguments
///
/// * `x` - Horizontal world tile coordinate.
/// * `y` - Vertical world tile coordinate.
///
/// # Returns
///
/// * `Some(&AreaWeatherProfile)` if the point lies in a configured area.
/// * `None` otherwise.
pub fn area_weather_profile_for(x: i32, y: i32) -> Option<&'static AreaWeatherProfile> {
    AREA_WEATHER_PROFILES
        .iter()
        .find(|entry| entry.regions.iter().any(|region| region.contains(x, y)))
}

/// Returns the index into [`AREA_WEATHER_PROFILES`] whose area contains
/// `(x, y)`, or `None`.
///
/// Used by the server to index its parallel per-area runtime state `Vec`.
///
/// # Arguments
///
/// * `x` - Horizontal world tile coordinate.
/// * `y` - Vertical world tile coordinate.
///
/// # Returns
///
/// * `Some(index)` if the point lies in a configured area.
/// * `None` otherwise.
pub fn area_weather_profile_index_for(x: i32, y: i32) -> Option<usize> {
    AREA_WEATHER_PROFILES
        .iter()
        .position(|entry| entry.regions.iter().any(|region| region.contains(x, y)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weather_area_contains_respects_inclusive_bounds() {
        let region = WeatherArea::new(10, 10, 20, 20);
        assert!(region.contains(10, 10));
        assert!(region.contains(20, 20));
        assert!(!region.contains(9, 10));
        assert!(!region.contains(10, 21));
    }

    #[test]
    fn lookup_returns_none_outside_any_area() {
        // Far edge of the world map is unlikely to be in any seeded area.
        assert!(area_weather_profile_for(0, 0).is_none());
        assert!(area_weather_profile_index_for(0, 0).is_none());
    }

    #[test]
    fn lookup_finds_strange_forest_fireflies() {
        // Strange Forest is roughly x=480..634, y=234..405.
        let entry = area_weather_profile_for(550, 320).expect("Strange Forest lookup");
        assert_eq!(entry.candidates[0].kind, WeatherKind::Fireflies);
        assert_eq!(entry.candidates[0].intensity, 64);

        let idx = area_weather_profile_index_for(550, 320).expect("index lookup");
        assert_eq!(AREA_WEATHER_PROFILES[idx].label, "Strange Forest");
    }

    #[test]
    fn multiple_regions_are_stitched_into_one_profile() {
        const REGIONS: &[WeatherArea] = &[
            WeatherArea::new(0, 0, 10, 10),
            WeatherArea::new(100, 100, 110, 110),
        ];
        const CANDIDATES: &[WeatherCandidate] = &[WeatherCandidate {
            kind: WeatherKind::Rain,
            weight: 1,
            intensity: 0,
            tint: None,
            flags: WeatherFlags::empty(),
        }];
        let profile = AreaWeatherProfile {
            label: "test",
            regions: REGIONS,
            candidates: CANDIDATES,
            trigger_chance_per_eval: 0,
            min_duration_ticks: 0,
            max_duration_ticks: 0,
            ignore_indoor_fade: false,
        };
        assert!(profile.regions.iter().any(|r| r.contains(5, 5)));
        assert!(profile.regions.iter().any(|r| r.contains(105, 105)));
        assert!(!profile.regions.iter().any(|r| r.contains(50, 50)));
    }

    #[test]
    fn pick_candidate_respects_weight_boundaries() {
        const CANDIDATES: &[WeatherCandidate] = &[
            WeatherCandidate {
                kind: WeatherKind::Rain,
                weight: 2,
                intensity: 0,
                tint: None,
                flags: WeatherFlags::empty(),
            },
            WeatherCandidate {
                kind: WeatherKind::Snow,
                weight: 1,
                intensity: 0,
                tint: None,
                flags: WeatherFlags::empty(),
            },
        ];
        let profile = AreaWeatherProfile {
            label: "test",
            regions: &[],
            candidates: CANDIDATES,
            trigger_chance_per_eval: 0,
            min_duration_ticks: 0,
            max_duration_ticks: 0,
            ignore_indoor_fade: false,
        };
        assert_eq!(total_weight(&profile), 3);
        assert_eq!(pick_candidate(&profile, 0).kind, WeatherKind::Rain);
        assert_eq!(pick_candidate(&profile, 1).kind, WeatherKind::Rain);
        assert_eq!(pick_candidate(&profile, 2).kind, WeatherKind::Snow);
        // Out-of-range roll clamps to the last candidate.
        assert_eq!(pick_candidate(&profile, 99).kind, WeatherKind::Snow);
    }
}
