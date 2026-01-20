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
