/// Pure functions for calculating character experience points.
///
/// These functions operate on `Character` structs directly, with no dependency
/// on `Repository` or any other server-global state.  They are used by both
/// the live server (via `populate::reset_char`) and the server utilities
/// (template viewer) to keep `points_tot` in sync when template stats change.
use core::types::Character;

/// Calculate the total experience points a character template is worth.
///
/// The formula mirrors the inline calculation in `populate::reset_char`:
///
/// 1. **Attributes** — flat sum of `attrib[z][0]` for `z` in `0..5`.
/// 2. **HP**         — `sum(m / 10 + 1)` for `m` in `50..hp[0]`.
/// 3. **Endurance**  — `sum(m / 10 + 1)` for `m` in `50..end[0]`.
/// 4. **Mana**       — `sum(m / 10 + 1)` for `m` in `50..mana[0]`.
/// 5. **Skills**     — for each of the 50 skills,
///                     `sum(m / 10 + 1)` for `m` in `0..skill[z][0]`.
///
/// Skill difficulty (`skill[z][3]`) is intentionally **not** factored in,
/// matching the existing `reset_char` behaviour.
///
/// # Arguments
///
/// * `character` - The character (or character template) to evaluate.
///
/// # Returns
///
/// * The computed total experience points.
pub fn calculate_points_tot(character: &Character) -> i32 {
    let mut pts: i32 = 0;

    // 1. Base attributes
    for z in 0..5 {
        pts += character.attrib[z][0] as i32;
    }

    // 2. HP (baseline 50)
    for m in 50..character.hp[0] as i32 {
        pts += m / 10 + 1;
    }

    // 3. Endurance (baseline 50)
    for m in 50..character.end[0] as i32 {
        pts += m / 10 + 1;
    }

    // 4. Mana (baseline 50)
    for m in 50..character.mana[0] as i32 {
        pts += m / 10 + 1;
    }

    // 5. Skills (baseline 0)
    for z in 0..50 {
        for m in 0..character.skill[z][0] as i32 {
            pts += m / 10 + 1;
        }
    }

    pts
}

// ---------------------------------------------------------------------------
//  Unit Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A default character (all zeroes) should produce 0 points.
    #[test]
    fn default_character_produces_zero_points() {
        let ch = Character::default();
        assert_eq!(calculate_points_tot(&ch), 0);
    }

    /// Attributes contribute a flat sum of their `[0]` values.
    #[test]
    fn attributes_sum_correctly() {
        let mut ch = Character::default();
        ch.attrib[0][0] = 10;
        ch.attrib[1][0] = 20;
        ch.attrib[2][0] = 30;
        ch.attrib[3][0] = 15;
        ch.attrib[4][0] = 25;
        assert_eq!(calculate_points_tot(&ch), 10 + 20 + 30 + 15 + 25);
    }

    /// HP below or equal to the baseline (50) contributes nothing.
    #[test]
    fn hp_at_baseline_contributes_nothing() {
        let mut ch = Character::default();
        ch.hp[0] = 50;
        assert_eq!(calculate_points_tot(&ch), 0);
    }

    /// HP above the baseline accumulates cost via `m / 10 + 1`.
    #[test]
    fn hp_above_baseline_accumulates_cost() {
        let mut ch = Character::default();
        ch.hp[0] = 55;
        // m=50: 50/10+1=6, m=51: 6, m=52: 6, m=53: 6, m=54: 6  => 30
        let expected: i32 = (50..55).map(|m| m / 10 + 1).sum();
        assert_eq!(expected, 30);
        assert_eq!(calculate_points_tot(&ch), 30);
    }

    /// Endurance uses the same formula as HP (baseline 50).
    #[test]
    fn endurance_above_baseline_accumulates_cost() {
        let mut ch = Character::default();
        ch.end[0] = 60;
        let expected: i32 = (50..60).map(|m| m / 10 + 1).sum();
        assert_eq!(calculate_points_tot(&ch), expected);
    }

    /// Mana uses the same formula as HP (baseline 50).
    #[test]
    fn mana_above_baseline_accumulates_cost() {
        let mut ch = Character::default();
        ch.mana[0] = 70;
        let expected: i32 = (50..70).map(|m| m / 10 + 1).sum();
        assert_eq!(calculate_points_tot(&ch), expected);
    }

    /// Skills start from 0, not 50.
    #[test]
    fn skills_accumulate_from_zero() {
        let mut ch = Character::default();
        ch.skill[0][0] = 10;
        // m=0..10: sum of (m/10+1) = 1*10 = 10
        let expected: i32 = (0..10).map(|m| m / 10 + 1).sum();
        assert_eq!(expected, 10);
        assert_eq!(calculate_points_tot(&ch), 10);
    }

    /// Multiple skills contribute independently.
    #[test]
    fn multiple_skills_sum_independently() {
        let mut ch = Character::default();
        ch.skill[0][0] = 5;
        ch.skill[10][0] = 15;
        ch.skill[49][0] = 20;

        let expected: i32 = (0..5i32).map(|m| m / 10 + 1).sum::<i32>()
            + (0..15i32).map(|m| m / 10 + 1).sum::<i32>()
            + (0..20i32).map(|m| m / 10 + 1).sum::<i32>();
        assert_eq!(calculate_points_tot(&ch), expected);
    }

    /// Combined attributes + stats + skills compute correctly.
    #[test]
    fn combined_stats_compute_correctly() {
        let mut ch = Character::default();
        ch.attrib[0][0] = 10;
        ch.attrib[1][0] = 10;
        ch.hp[0] = 100;
        ch.skill[5][0] = 25;

        let attr_pts = 20i32;
        let hp_pts: i32 = (50..100).map(|m| m / 10 + 1).sum();
        let skill_pts: i32 = (0..25i32).map(|m| m / 10 + 1).sum();

        assert_eq!(calculate_points_tot(&ch), attr_pts + hp_pts + skill_pts);
    }

    /// HP/end/mana below baseline 50 should contribute 0, not negative.
    #[test]
    fn stats_below_baseline_contribute_zero() {
        let mut ch = Character::default();
        ch.hp[0] = 30;
        ch.end[0] = 10;
        ch.mana[0] = 0;
        // Ranges 50..30, 50..10, 50..0 are all empty → 0
        assert_eq!(calculate_points_tot(&ch), 0);
    }
}
