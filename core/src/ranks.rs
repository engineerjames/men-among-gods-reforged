/// Total number of distinct ranks in the game.
pub const TOTAL_RANKS: usize = 24;

/// Rank indices used for progression, display, and talent-point milestones.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Rank {
    /// Private, rank 0.
    Private = 0,
    /// Private First Class, rank 1.
    PrivateFirstClass = 1,
    /// Lance Corporal, rank 2.
    LanceCorporal = 2,
    /// Corporal, rank 3.
    Corporal = 3,
    /// Sergeant, rank 4.
    Sergeant = 4,
    /// Staff Sergeant, rank 5.
    StaffSergeant = 5,
    /// Master Sergeant, rank 6.
    MasterSergeant = 6,
    /// First Sergeant, rank 7.
    FirstSergeant = 7,
    /// Sergeant Major, rank 8.
    SergeantMajor = 8,
    /// Second Lieutenant, rank 9.
    SecondLieutenant = 9,
    /// First Lieutenant, rank 10.
    FirstLieutenant = 10,
    /// Captain, rank 11.
    Captain = 11,
    /// Major, rank 12.
    Major = 12,
    /// Lieutenant Colonel, rank 13.
    LieutenantColonel = 13,
    /// Colonel, rank 14.
    Colonel = 14,
    /// Brigadier General, rank 15.
    BrigadierGeneral = 15,
    /// Major General, rank 16.
    MajorGeneral = 16,
    /// Lieutenant General, rank 17.
    LieutenantGeneral = 17,
    /// General, rank 18.
    General = 18,
    /// Field Marshal, rank 19.
    FieldMarshal = 19,
    /// Knight, rank 20.
    Knight = 20,
    /// Baron, rank 21.
    Baron = 21,
    /// Earl, rank 22.
    Earl = 22,
    /// Warlord, rank 23.
    Warlord = 23,
}

impl Rank {
    /// Returns the rank represented by an index, clamped to the nearest valid rank.
    ///
    /// # Arguments
    ///
    /// * `rank_idx` - Zero-based rank index.
    ///
    /// # Returns
    ///
    /// * The matching rank, or [`Rank::Warlord`] for out-of-range values.
    pub const fn from_index(rank_idx: usize) -> Self {
        match rank_idx {
            0 => Self::Private,
            1 => Self::PrivateFirstClass,
            2 => Self::LanceCorporal,
            3 => Self::Corporal,
            4 => Self::Sergeant,
            5 => Self::StaffSergeant,
            6 => Self::MasterSergeant,
            7 => Self::FirstSergeant,
            8 => Self::SergeantMajor,
            9 => Self::SecondLieutenant,
            10 => Self::FirstLieutenant,
            11 => Self::Captain,
            12 => Self::Major,
            13 => Self::LieutenantColonel,
            14 => Self::Colonel,
            15 => Self::BrigadierGeneral,
            16 => Self::MajorGeneral,
            17 => Self::LieutenantGeneral,
            18 => Self::General,
            19 => Self::FieldMarshal,
            20 => Self::Knight,
            21 => Self::Baron,
            22 => Self::Earl,
            _ => Self::Warlord,
        }
    }

    /// Returns this rank's zero-based index.
    ///
    /// # Returns
    ///
    /// * Rank index in `0..TOTAL_RANKS`.
    pub const fn index(self) -> usize {
        self as usize
    }

    /// Returns whether reaching this rank awards a talent point.
    ///
    /// # Returns
    ///
    /// * `true` for talent-point milestone ranks.
    /// * `false` otherwise.
    pub const fn awards_talent_point(self) -> bool {
        matches!(
            self,
            Self::PrivateFirstClass
                | Self::Corporal
                | Self::StaffSergeant
                | Self::FirstSergeant
                | Self::SecondLieutenant
                | Self::Captain
                | Self::LieutenantColonel
                | Self::BrigadierGeneral
                | Self::LieutenantGeneral
                | Self::FieldMarshal
                | Self::Baron
                | Self::Warlord
        )
    }
}

/// Full rank names matching `WHO_RANK_NAME` indices.
const RANK_NAMES: [&str; TOTAL_RANKS] = [
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
///
/// # Arguments
///
/// * `points` - Total experience points.
///
/// # Returns
///
/// * The full rank display name (e.g. `"Private"`, `"Warlord"`).
pub fn rank_name(points: u32) -> &'static str {
    // NOTE: `points2rank` already clamps via the returned range, but we still clamp
    // here defensively to ensure indexing safety if thresholds change.
    let idx = points2rank(points).clamp(0, TOTAL_RANKS as u32 - 1) as usize;
    RANK_NAMES[idx]
}

/// Returns a reference to the full array of rank display names.
///
/// # Returns
///
/// * A static reference to all 24 rank names.
pub fn ranks() -> &'static [&'static str; TOTAL_RANKS] {
    &RANK_NAMES
}

/// Returns the shortened rank abbreviation for the given total points.
///
/// # Arguments
///
/// * `points` - Total experience points.
///
/// # Returns
///
/// * A compact rank label (e.g. `" Pvt "`, `"WARLD"`).
pub fn rank_name_shortened(points: u32) -> &'static str {
    let idx = points2rank(points).clamp(0, TOTAL_RANKS as u32 - 1) as usize;
    WHO_RANK_NAME[idx]
}

/// Short rank names used in compact `who` displays.
const WHO_RANK_NAME: [&str; TOTAL_RANKS] = [
    " Pvt ", " PFC ", " LCp ", " Cpl ", " Sgt ", " SSg ", " MSg ", " 1Sg ", " SgM ", "2Lieu",
    "1Lieu", "Captn", "Major", "LtCol", "Colnl", "BrGen", "MaGen", "LtGen", "Genrl", "FDMAR",
    "KNIGT", "BARON", " EARL", "WARLD",
];

/// Returns the human-readable rank name for the given rank index.
///
/// Clamps out-of-range indices to the nearest valid rank.
///
/// # Arguments
///
/// * `rank_idx` - Rank index (0-based).
///
/// # Returns
///
/// * Rank name corresponding to the given index, or nearest valid rank if out of range.
pub fn rank_name_by_index(rank_idx: usize) -> &'static str {
    let idx = rank_idx.clamp(0, TOTAL_RANKS - 1);
    RANK_NAMES[idx]
}

/// Counts talent points awarded when advancing between rank indices.
///
/// The old rank is exclusive and the new rank is inclusive, so advancing from
/// rank 0 to rank 3 counts ranks 1, 2, and 3.
///
/// # Arguments
///
/// * `old_rank_idx` - Previous zero-based rank index.
/// * `new_rank_idx` - New zero-based rank index.
///
/// # Returns
///
/// * Number of talent-point milestones crossed.
pub fn talent_points_awarded_between(old_rank_idx: usize, new_rank_idx: usize) -> u8 {
    if new_rank_idx <= old_rank_idx {
        return 0;
    }

    let start = old_rank_idx.saturating_add(1);
    let end = new_rank_idx.min(TOTAL_RANKS - 1);
    (start..=end)
        .filter(|&idx| Rank::from_index(idx).awards_talent_point())
        .count() as u8
}

/// Maps total points to a rank index.
///
/// Implements the server's `points2rank` thresholds to convert experience
/// points into a discrete rank used for comparison and display.
///
/// # Arguments
///
/// * `value` - Total experience points.
///
/// # Returns
///
/// * A rank index in `0..=23`.
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
    use super::{
        RANK_NAMES, RANK_THRESHOLDS, Rank, TOTAL_RANKS, points2rank, rank_name, rank_name_by_index,
        rank_name_shortened, rank_progress, ranks, talent_points_awarded_between,
    };

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

    #[test]
    fn ranks_returns_all_rank_names() {
        let all = ranks();
        assert_eq!(all.len(), TOTAL_RANKS);
        assert_eq!(all[0], "Private");
        assert_eq!(all[TOTAL_RANKS - 1], "Warlord");
    }

    #[test]
    fn rank_name_shortened_matches_known_values() {
        assert_eq!(rank_name_shortened(0), " Pvt ");
        assert_eq!(rank_name_shortened(u32::MAX), "WARLD");
    }

    #[test]
    fn rank_name_by_index_clamps_out_of_range() {
        assert_eq!(rank_name_by_index(0), "Private");
        assert_eq!(rank_name_by_index(23), "Warlord");
        assert_eq!(rank_name_by_index(999), "Warlord");
    }

    #[test]
    fn rank_from_index_clamps_to_warlord() {
        assert_eq!(Rank::from_index(0), Rank::Private);
        assert_eq!(Rank::from_index(1), Rank::PrivateFirstClass);
        assert_eq!(Rank::from_index(23), Rank::Warlord);
        assert_eq!(Rank::from_index(999), Rank::Warlord);
    }

    #[test]
    fn talent_point_award_ranks_match_design() {
        let awarded: Vec<usize> = (0..TOTAL_RANKS)
            .filter(|&idx| Rank::from_index(idx).awards_talent_point())
            .collect();

        assert_eq!(
            awarded,
            vec![
                Rank::PrivateFirstClass.index(),
                Rank::Corporal.index(),
                Rank::StaffSergeant.index(),
                Rank::FirstSergeant.index(),
                Rank::SecondLieutenant.index(),
                Rank::Captain.index(),
                Rank::LieutenantColonel.index(),
                Rank::BrigadierGeneral.index(),
                Rank::LieutenantGeneral.index(),
                Rank::FieldMarshal.index(),
                Rank::Baron.index(),
                Rank::Warlord.index(),
            ]
        );
    }

    #[test]
    fn talent_points_awarded_between_counts_only_crossed_milestones() {
        assert_eq!(talent_points_awarded_between(0, 0), 0);
        assert_eq!(talent_points_awarded_between(0, 1), 1);
        assert_eq!(talent_points_awarded_between(1, 2), 0);
        assert_eq!(talent_points_awarded_between(2, 3), 1);
        assert_eq!(talent_points_awarded_between(0, 3), 2);
        assert_eq!(talent_points_awarded_between(0, 23), 12);
        assert_eq!(talent_points_awarded_between(23, 999), 0);
    }
}
