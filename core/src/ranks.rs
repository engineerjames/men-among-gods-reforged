use crate::constants::RANKS;

/// Full rank names matching `WHO_RANK_NAME` indices.
pub const RANK_NAMES: [&str; RANKS] = [
    "Private",
    "Private First Class",
    "Lance Corporal",
    "Corporal",
    "Sergeant",
    "Staff Sergeant",
    "Master Sergeant",
    "First Sergeant",
    "Sergeant Major",
    "Second Lieutenant",
    "First Lieutenant",
    "Captain",
    "Major",
    "Lieutenant Colonel",
    "Colonel",
    "Brigadier General",
    "Major General",
    "Lieutenant General",
    "General",
    "Field Marshal",
    "Knight",
    "Baron",
    "Earl",
    "Warlord",
];

/// Returns the human-readable rank name for the given total points.
pub fn rank_name(points: u32) -> &'static str {
    // NOTE: `points2rank` already clamps via the returned range, but we still clamp
    // here defensively to ensure indexing safety if thresholds change.
    let idx = points2rank(points).clamp(0, RANKS as u32 - 1) as usize;
    RANK_NAMES[idx]
}

/// Map total points to a rank index.
///
/// Implements the server's `points2rank` thresholds to convert experience
/// points into a discrete rank used for comparison and display.
///
/// # Arguments
/// * `value` - Total experience points
pub fn points2rank(value: u32) -> u32 {
    match value {
        0..50 => 0,
        50..850 => 1,
        850..4900 => 2,
        4900..17700 => 3,
        17700..48950 => 4,
        48950..113750 => 5,
        113750..233800 => 6,
        233800..438600 => 7,
        438600..766650 => 8,
        766650..1266650 => 9,
        1266650..1998700 => 10,
        1998700..3035500 => 11,
        3035500..4463550 => 12,
        4463550..6384350 => 13,
        6384350..8915600 => 14,
        8915600..12192400 => 15,
        12192400..16368450 => 16,
        16368450..21617250 => 17,
        21617250..28133300 => 18,
        28133300..36133300 => 19,
        36133300..49014500 => 20,
        49014500..63000600 => 21,
        63000600..80977100 => 22,
        _ => 23,
    }
}

/// Lower-bound experience thresholds for each of the 24 ranks.
///
/// `RANK_THRESHOLDS[i]` is the minimum total points to enter rank `i`.
pub const RANK_THRESHOLDS: [u32; 24] = [
    0, 50, 850, 4_900, 17_700, 48_950, 113_750, 233_800, 438_600, 766_650, 1_266_650, 1_998_700,
    3_035_500, 4_463_550, 6_384_350, 8_915_600, 12_192_400, 16_368_450, 21_617_250, 28_133_300,
    36_133_300, 49_014_500, 63_000_600, 80_977_100,
];

/// Computes the fractional progress toward the next rank.
///
/// Returns a value in `[0.0, 1.0]`. At the maximum rank (Warlord, index 23)
/// the function returns `1.0`.
///
/// # Arguments
///
/// * `points` - Total experience points.
///
/// # Returns
///
/// Fractional progress within the current rank.
pub fn rank_progress(points: u32) -> f64 {
    let idx = points2rank(points) as usize;
    if idx >= 23 {
        return 1.0;
    }
    let lo = RANK_THRESHOLDS[idx] as f64;
    let hi = RANK_THRESHOLDS[idx + 1] as f64;
    let span = hi - lo;
    if span <= 0.0 {
        return 1.0;
    }
    ((points as f64 - lo) / span).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::{points2rank, rank_name, rank_progress, RANK_NAMES, RANK_THRESHOLDS};

    #[test]
    fn points2rank_respects_threshold_boundaries() {
        assert_eq!(points2rank(0), 0);
        assert_eq!(points2rank(49), 0);
        assert_eq!(points2rank(50), 1);
        assert_eq!(points2rank(849), 1);
        assert_eq!(points2rank(850), 2);
        assert_eq!(points2rank(4899), 2);
        assert_eq!(points2rank(4900), 3);
    }

    #[test]
    fn points2rank_returns_last_rank_for_large_values() {
        assert_eq!(points2rank(80_977_100), 23);
        assert_eq!(points2rank(u32::MAX), 23);
    }

    #[test]
    fn rank_name_matches_expected_display_names() {
        assert_eq!(rank_name(0), "Private");
        assert_eq!(rank_name(50), "Private First Class");
        assert_eq!(rank_name(36133300), "Knight");
        assert_eq!(rank_name(u32::MAX), "Warlord");
    }

    #[test]
    fn rank_name_is_always_a_known_rank_string() {
        for points in [0_u32, 1, 49, 50, 849, 850, 4_899, 4_900, u32::MAX] {
            let name = rank_name(points);
            assert!(RANK_NAMES.contains(&name));
        }
    }

    #[test]
    fn rank_progress_zero_at_rank_start() {
        assert!((rank_progress(0) - 0.0).abs() < 1e-9);
        assert!((rank_progress(50) - 0.0).abs() < 1e-9);
        assert!((rank_progress(850) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn rank_progress_approaches_one_near_boundary() {
        // 49 out of 50 threshold for rank 0:
        let p = rank_progress(49);
        assert!(p > 0.9 && p < 1.0);
    }

    #[test]
    fn rank_progress_max_rank_returns_one() {
        assert!((rank_progress(80_977_100) - 1.0).abs() < 1e-9);
        assert!((rank_progress(u32::MAX) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn rank_thresholds_are_sorted() {
        for w in RANK_THRESHOLDS.windows(2) {
            assert!(w[0] < w[1], "thresholds not sorted: {} >= {}", w[0], w[1]);
        }
    }
}
