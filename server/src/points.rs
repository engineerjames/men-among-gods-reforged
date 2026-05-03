/// Pure functions for calculating character experience points.
///
/// These functions operate on `Character` structs directly, with no dependency
/// on `Repository` or any other server-global state. They are used by both
/// the live server (via `populate::reset_char`) and the server utilities
/// (template viewer) to keep `points_tot` in sync when template stats change.
use core::{skills, types::Character};

/// Calculate the total experience points a character template is worth.
///
/// The formula mirrors the inline calculation in legacy `populate::reset_char`:
///
/// 1. **Attributes** - `attrib_needed(value, 3)` for `value` in `10..attrib[z][0]`.
/// 2. **HP** - `hp_needed(value, 3)` for `value` in `50..hp[0]`.
/// 3. **Endurance** - `end_needed(value, 2)` for `value` in `50..end[0]`.
/// 4. **Mana** - `mana_needed(value, 3)` for `value` in `50..mana[0]`.
/// 5. **Skills** - `skill_needed(value, 2)` for `value` in `1..skill[z][0]`,
///    excluding perception, stealth, and lock picking.
///
/// # Arguments
///
/// * `character` - The character or character template to evaluate.
///
/// # Returns
///
/// * The computed total experience points.
#[allow(dead_code)] // Used by server-utils via the lib target (server::points::calculate_points_tot)
pub fn calculate_points_tot(character: &Character) -> i32 {
    let mut points = 0;

    for attribute in 0..5 {
        for value in 10..character.attrib[attribute][0] as i32 {
            points += attrib_needed(value, 3);
        }
    }

    for value in 50..character.hp[0] as i32 {
        points += hp_needed(value, 3);
    }

    for value in 50..character.end[0] as i32 {
        points += end_needed(value, 2);
    }

    for value in 50..character.mana[0] as i32 {
        points += mana_needed(value, 3);
    }

    for skill in 0..50 {
        if skill == skills::SK_PERCEPT || skill == skills::SK_STEALTH || skill == skills::SK_LOCK {
            continue;
        }

        for value in 1..character.skill[skill][0] as i32 {
            points += skill_needed(value, 2);
        }
    }

    points
}

/// Return the legacy point cost for an attribute value.
///
/// # Arguments
///
/// * `value` - Current attribute value being raised from.
/// * `difficulty` - Difficulty multiplier.
///
/// # Returns
///
/// * Point cost for that increment.
pub(crate) fn attrib_needed(value: i32, difficulty: i32) -> i32 {
    value * value * value * difficulty / 20
}

/// Return the legacy point cost for an HP value.
///
/// # Arguments
///
/// * `value` - Current HP value being raised from.
/// * `difficulty` - Difficulty multiplier.
///
/// # Returns
///
/// * Point cost for that increment.
pub(crate) fn hp_needed(value: i32, difficulty: i32) -> i32 {
    value * difficulty
}

/// Return the legacy point cost for an endurance value.
///
/// # Arguments
///
/// * `value` - Current endurance value being raised from.
/// * `difficulty` - Difficulty multiplier.
///
/// # Returns
///
/// * Point cost for that increment.
pub(crate) fn end_needed(value: i32, difficulty: i32) -> i32 {
    value * difficulty / 2
}

/// Return the legacy point cost for a mana value.
///
/// # Arguments
///
/// * `value` - Current mana value being raised from.
/// * `difficulty` - Difficulty multiplier.
///
/// # Returns
///
/// * Point cost for that increment.
pub(crate) fn mana_needed(value: i32, difficulty: i32) -> i32 {
    value * difficulty
}

/// Return the legacy point cost for a skill value.
///
/// # Arguments
///
/// * `value` - Current skill value being raised from.
/// * `difficulty` - Difficulty multiplier.
///
/// # Returns
///
/// * Point cost for that increment.
pub(crate) fn skill_needed(value: i32, difficulty: i32) -> i32 {
    value.max(value * value * value * difficulty / 40)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A default character should produce 0 points.
    #[test]
    fn default_character_produces_zero_points() {
        let character = Character::default();
        assert_eq!(calculate_points_tot(&character), 0);
    }

    /// Attributes use the legacy cubic cost above baseline 10.
    #[test]
    fn attributes_use_legacy_cost_above_baseline() {
        let mut character = Character::default();
        character.attrib[0][0] = 12;

        let expected = attrib_needed(10, 3) + attrib_needed(11, 3);
        assert_eq!(calculate_points_tot(&character), expected);
    }

    /// Attributes at or below baseline 10 contribute nothing.
    #[test]
    fn attributes_at_baseline_contribute_nothing() {
        let mut character = Character::default();
        character.attrib[0][0] = 10;
        assert_eq!(calculate_points_tot(&character), 0);
    }

    /// HP at baseline 50 contributes nothing.
    #[test]
    fn hp_at_baseline_contributes_nothing() {
        let mut character = Character::default();
        character.hp[0] = 50;
        assert_eq!(calculate_points_tot(&character), 0);
    }

    /// HP above baseline uses `hp_needed(value, 3)`.
    #[test]
    fn hp_above_baseline_uses_legacy_cost() {
        let mut character = Character::default();
        character.hp[0] = 55;

        let expected: i32 = (50..55).map(|value| hp_needed(value, 3)).sum();
        assert_eq!(calculate_points_tot(&character), expected);
    }

    /// Endurance above baseline uses `end_needed(value, 2)`.
    #[test]
    fn endurance_above_baseline_uses_legacy_cost() {
        let mut character = Character::default();
        character.end[0] = 60;

        let expected: i32 = (50..60).map(|value| end_needed(value, 2)).sum();
        assert_eq!(calculate_points_tot(&character), expected);
    }

    /// Mana above baseline uses `mana_needed(value, 3)`.
    #[test]
    fn mana_above_baseline_uses_legacy_cost() {
        let mut character = Character::default();
        character.mana[0] = 70;

        let expected: i32 = (50..70).map(|value| mana_needed(value, 3)).sum();
        assert_eq!(calculate_points_tot(&character), expected);
    }

    /// Skills start costing at value 1.
    #[test]
    fn skills_accumulate_from_one() {
        let mut character = Character::default();
        character.skill[0][0] = 10;

        let expected: i32 = (1..10).map(|value| skill_needed(value, 2)).sum();
        assert_eq!(expected, 104);
        assert_eq!(calculate_points_tot(&character), expected);
    }

    /// Perception, stealth, and lock picking do not contribute to template rank.
    #[test]
    fn special_skills_are_ignored() {
        let mut character = Character::default();
        character.skill[skills::SK_PERCEPT][0] = 50;
        character.skill[skills::SK_STEALTH][0] = 50;
        character.skill[skills::SK_LOCK][0] = 50;

        assert_eq!(calculate_points_tot(&character), 0);
    }

    /// Multiple skills contribute independently.
    #[test]
    fn multiple_skills_sum_independently() {
        let mut character = Character::default();
        character.skill[0][0] = 5;
        character.skill[10][0] = 15;
        character.skill[49][0] = 20;

        let expected: i32 = (1..5i32).map(|value| skill_needed(value, 2)).sum::<i32>()
            + (1..15i32).map(|value| skill_needed(value, 2)).sum::<i32>()
            + (1..20i32).map(|value| skill_needed(value, 2)).sum::<i32>();
        assert_eq!(calculate_points_tot(&character), expected);
    }

    /// Combined attributes, stats, and skills compute correctly.
    #[test]
    fn combined_stats_compute_correctly() {
        let mut character = Character::default();
        character.attrib[0][0] = 12;
        character.attrib[1][0] = 13;
        character.hp[0] = 100;
        character.skill[5][0] = 25;

        let attribute_points: i32 = (10..12).map(|value| attrib_needed(value, 3)).sum::<i32>()
            + (10..13).map(|value| attrib_needed(value, 3)).sum::<i32>();
        let hp_points: i32 = (50..100).map(|value| hp_needed(value, 3)).sum();
        let skill_points: i32 = (1..25i32).map(|value| skill_needed(value, 2)).sum();

        assert_eq!(
            calculate_points_tot(&character),
            attribute_points + hp_points + skill_points
        );
    }

    /// HP, endurance, and mana below baseline 50 should contribute 0.
    #[test]
    fn stats_below_baseline_contribute_zero() {
        let mut character = Character::default();
        character.hp[0] = 30;
        character.end[0] = 10;
        character.mana[0] = 0;

        assert_eq!(calculate_points_tot(&character), 0);
    }
}
