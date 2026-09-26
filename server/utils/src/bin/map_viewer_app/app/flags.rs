//! Map flag definitions, naming, and visualization colors.

use eframe::egui;
use mag_core::constants as c;

/// Shared map flag definitions, aligned with `core/src/constants.rs`.
///
/// # Returns
///
/// * `(mask, name)` pairs; `MF_GFX_*` multi-bit fields appear alongside their unit bit.
pub(super) fn map_flag_defs() -> &'static [(u64, &'static str)] {
    // `as u64` (not `u64::from`) keeps this array `const`.
    const DEFS: &[(u64, &str)] = &[
        (c::MF_MOVEBLOCK as u64, "MF_MOVEBLOCK"),
        (c::MF_SIGHTBLOCK as u64, "MF_SIGHTBLOCK"),
        (c::MF_INDOORS as u64, "MF_INDOORS"),
        (c::MF_UWATER as u64, "MF_UWATER"),
        (c::MF_NOLAG as u64, "MF_NOLAG"),
        (c::MF_NOMONST as u64, "MF_NOMONST"),
        (c::MF_BANK as u64, "MF_BANK"),
        (c::MF_TAVERN as u64, "MF_TAVERN"),
        (c::MF_NOMAGIC as u64, "MF_NOMAGIC"),
        (c::MF_DEATHTRAP as u64, "MF_DEATHTRAP"),
        (c::MF_ARENA as u64, "MF_ARENA"),
        (c::MF_NOEXPIRE as u64, "MF_NOEXPIRE"),
        (c::MF_NOFIGHT, "MF_NOFIGHT"),
        (c::MF_GFX_INJURED, "MF_GFX_INJURED"),
        (c::MF_GFX_INJURED1, "MF_GFX_INJURED1"),
        (c::MF_GFX_INJURED2, "MF_GFX_INJURED2"),
        (c::MF_GFX_TOMB, "MF_GFX_TOMB"),
        (c::MF_GFX_TOMB1, "MF_GFX_TOMB1"),
        (c::MF_GFX_DEATH, "MF_GFX_DEATH"),
        (c::MF_GFX_DEATH1, "MF_GFX_DEATH1"),
        (c::MF_GFX_EMAGIC, "MF_GFX_EMAGIC"),
        (c::MF_GFX_EMAGIC1, "MF_GFX_EMAGIC1"),
        (c::MF_GFX_GMAGIC, "MF_GFX_GMAGIC"),
        (c::MF_GFX_GMAGIC1, "MF_GFX_GMAGIC1"),
        (c::MF_GFX_CMAGIC, "MF_GFX_CMAGIC"),
        (c::MF_GFX_CMAGIC1, "MF_GFX_CMAGIC1"),
    ];
    DEFS
}

/// Persistent gameplay flag bits; the `MF_GFX_*` bits above 32 are transient visual effects.
pub(super) const GAMEPLAY_MAP_FLAG_MASK: u64 = 0xFFFF_FFFF;

/// Default alpha of the map flag tint overlay.
pub(super) const DEFAULT_FLAG_VIZ_OPACITY: f32 = 0.45;

/// Maximum number of flag combinations listed in the side-panel legend.
pub(super) const MAX_FLAG_VIZ_LEGEND_ENTRIES: usize = 32;

/// Names of every flag definition overlapping `mask`.
///
/// # Arguments
///
/// * `mask` - Map flag bits to describe.
///
/// # Returns
///
/// * Flag names in definition order; empty when no definition overlaps.
pub(super) fn flag_names(mask: u64) -> Vec<&'static str> {
    map_flag_defs()
        .iter()
        .filter(|(def, _)| mask & def != 0)
        .map(|(_, name)| *name)
        .collect()
}

/// Checkbox that toggles `mask` within `bits`; shown checked when any bit of `mask` is set.
///
/// # Arguments
///
/// * `ui` - Target UI.
/// * `bits` - Flag word being edited.
/// * `mask` - Bits controlled by this checkbox.
/// * `name` - Checkbox label.
///
/// # Returns
///
/// * `true` when `bits` changed.
pub(super) fn flag_checkbox(ui: &mut egui::Ui, bits: &mut u64, mask: u64, name: &str) -> bool {
    let mut on = (*bits & mask) != 0;
    if !ui.checkbox(&mut on, name).changed() {
        return false;
    }
    if on {
        *bits |= mask;
    } else {
        *bits &= !mask;
    }
    true
}

/// Deterministic, well-spread opaque color for a map flag combination.
///
/// Equal combinations always get the same color, so adjacent tiles with matching
/// flags read as one uniform region.
///
/// # Arguments
///
/// * `flags` - The (already filtered) flag combination.
///
/// # Returns
///
/// * An opaque color derived from a hash of `flags`.
pub(super) fn flag_combo_color(flags: u64) -> egui::Color32 {
    // splitmix64 finalizer: nearby bit patterns map to unrelated hues.
    let mut z = flags.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;

    let hue = (z & 0xFFFF) as f32 / 65536.0;
    let sat = 0.6 + ((z >> 16) & 0xFF) as f32 / 255.0 * 0.35;
    let val = 0.8 + ((z >> 24) & 0xFF) as f32 / 255.0 * 0.2;
    egui::Color32::from(egui::ecolor::Hsva::new(hue, sat, val, 1.0))
}

#[cfg(test)]
mod tests {
    use super::{flag_combo_color, flag_names};
    use mag_core::constants::{MF_GFX_TOMB, MF_GFX_TOMB1, MF_INDOORS, MF_MOVEBLOCK};

    #[test]
    fn flag_names_lists_every_overlapping_definition() {
        assert!(flag_names(0).is_empty());
        assert_eq!(
            flag_names(u64::from(MF_MOVEBLOCK | MF_INDOORS)),
            vec!["MF_MOVEBLOCK", "MF_INDOORS"]
        );
        assert_eq!(
            flag_names(MF_GFX_TOMB1),
            vec!["MF_GFX_TOMB", "MF_GFX_TOMB1"]
        );
        assert_eq!(flag_names(MF_GFX_TOMB & !MF_GFX_TOMB1), vec!["MF_GFX_TOMB"]);
    }

    #[test]
    fn flag_combo_color_is_deterministic_and_distinguishes_combos() {
        let a = u64::from(MF_MOVEBLOCK);
        let b = u64::from(MF_INDOORS);
        assert_eq!(flag_combo_color(a), flag_combo_color(a));
        assert_ne!(flag_combo_color(a), flag_combo_color(b));
        assert_ne!(flag_combo_color(a), flag_combo_color(a | b));
        assert_eq!(flag_combo_color(a).a(), 255);
    }
}
