// Talent tree implementation intended to be stored by the future1 field.
// 0-255; future1 has 25 elements of 1-byte each, so we'll start with 1-23
// being representative of the talent points spent at each level of the tree.
// The tree structure will be captured by the individual bits of each byte.
// future1[0] = Talent tree points to spend
// future1[1..24] = Talent tree points spent at each rank
//
// For example (each [ ] represents a talent node, and the indentation represents the tree structure)
// and an 'x' inside the [ ] represents a point spent in that talent:
//
// Player has 0 points to spend <-- future1[0] = 0
// [x] - [ ] <-- future1[1] = 0b00000010 (1 point spent in the first talent layer)
//  |     |
// [ ]   [x] <-- future1[2] = 0b00000001 (1 point spent in the second talent layer)
//  |     |
// [ ] - [x] <-- future1[3] = 0b00000001 (1 point spent in the third talent layer)
//  |.    |
// [ ]   [ ] <-- future1[4] = 0b00000000 (0 points spent in the fourth talent layer)
//
// This is probably overkill for now but gives us plenty of degrees of freedom to grow
// the talent tree system in the future without needing to change the underlying storage format.
//
// This could get a bit weird if we try to add new talent nodes to the "right"
// of the tree after players have already spent points, but we can cross that
// bridge when we come to it. For now we'll just focus on implementing the basic
// structure and point spending logic.

use core::{
    skills::{Attribute, Skill, SkillIndex},
    string_operations::c_string_to_str,
};

use crate::game_state::GameState;

pub fn apply_talent_point(
    cn: usize,
    talents: &mut [u8; 25],
    talent_mask: u8,
    talent_layer: usize,
) -> Result<(), String> {
    if talent_layer == 0 || talent_layer >= talents.len() {
        return Err("Invalid talent layer".to_string());
    }

    if talents[0] < 1 {
        return Err("Not enough points to spend".to_string());
    }

    // Dispatch function to update player state based on the talent chosen
    // This is a placeholder for future implementation
    // update_player_state(cn, talent_layer, talent_mask);

    // Spend the points
    talents[0] -= 1;
    talents[talent_layer] |= talent_mask;

    Ok(())
}

pub fn reset_talent_points(talents: &mut [u8; 25]) {
    let mut refunded_points = 0;
    for i in 1..talents.len() {
        refunded_points += talents[i].count_ones();
        talents[i] = 0; // Clear all talent points
    }
    talents[0] += refunded_points as u8; // Add refunded points back to the pool
}

// Various talent effects - some will be dynamic (like stat bonuses)
// and some will be more static (like unlocking new abilities).
fn modify_base_skill_by_percentage(
    cn: usize,
    game_state: &mut GameState,
    skill: Skill,
    percentage_bonus: i32,
) -> Result<(), String> {
    let skill_base =
        game_state.characters[cn].skill[skill as usize][SkillIndex::BaseValue as usize];

    let bonus_amount = (skill_base as f32 * (percentage_bonus as f32 / 100.0)).round() as u8;
    game_state.characters[cn].skill[skill as usize][SkillIndex::BaseValue as usize] += bonus_amount;

    log::info!(
        "Applied talent bonus: +{}% to {:?} ({} points) for character {}",
        percentage_bonus,
        skill,
        bonus_amount,
        c_string_to_str(&game_state.characters[cn].name)
    );

    Ok(())
}

fn modify_attribute_by_percentage(
    cn: usize,
    game_state: &mut GameState,
    attribute_index: Attribute,
    percentage_bonus: i32,
) -> Result<(), String> {
    let attribute_base =
        game_state.characters[cn].attrib[attribute_index as usize][SkillIndex::BaseValue as usize];

    let bonus_amount = (attribute_base as f32 * (percentage_bonus as f32 / 100.0)).round() as u8;
    game_state.characters[cn].attrib[attribute_index as usize][SkillIndex::BaseValue as usize] +=
        bonus_amount;

    log::info!(
        "Applied talent bonus: +{}% to attribute {} ({} points) for character {}",
        percentage_bonus,
        attribute_index as usize,
        bonus_amount,
        c_string_to_str(&game_state.characters[cn].name)
    );

    Ok(())
}

fn modify_skill_by_flat_amount(
    cn: usize,
    game_state: &mut GameState,
    skill: Skill,
    flat_bonus: u8,
) -> Result<(), String> {
    game_state.characters[cn].skill[skill as usize][SkillIndex::BaseValue as usize] += flat_bonus;

    log::info!(
        "Applied talent bonus: +{} to {:?} for character {}",
        flat_bonus,
        skill,
        c_string_to_str(&game_state.characters[cn].name)
    );

    Ok(())
}

fn modify_attribute_by_flat_amount(
    cn: usize,
    game_state: &mut GameState,
    attribute_index: Attribute,
    flat_bonus: u8,
) -> Result<(), String> {
    game_state.characters[cn].attrib[attribute_index as usize][SkillIndex::BaseValue as usize] +=
        flat_bonus;

    log::info!(
        "Applied talent bonus: +{} to attribute {} for character {}",
        flat_bonus,
        attribute_index as usize,
        c_string_to_str(&game_state.characters[cn].name)
    );

    Ok(())
}

fn grant_skill(cn: usize, game_state: &mut GameState, skill: Skill) -> Result<(), String> {
    let skill_base =
        game_state.characters[cn].skill[skill as usize][SkillIndex::BaseValue as usize];

    if skill_base > 0 {
        return Err(format!(
            "Character {} already has skill {:?}",
            c_string_to_str(&game_state.characters[cn].name),
            skill
        ));
    }

    // Mark skill as granted
    game_state.characters[cn].skill[skill as usize][SkillIndex::BaseValue as usize] = 1;

    log::info!(
        "Granted new skill {:?} to character {}",
        skill,
        c_string_to_str(&game_state.characters[cn].name)
    );

    Ok(())
}

fn remove_skill(cn: usize, game_state: &mut GameState, skill: Skill) -> Result<(), String> {
    let skill_base =
        game_state.characters[cn].skill[skill as usize][SkillIndex::BaseValue as usize];

    if skill_base == 0 {
        return Err(format!(
            "Character {} does not have skill {:?} to remove",
            c_string_to_str(&game_state.characters[cn].name),
            skill
        ));
    }

    // Mark skill as removed
    game_state.characters[cn].skill[skill as usize][SkillIndex::BaseValue as usize] = 0;

    log::info!(
        "Removed skill {:?} from character {}",
        skill,
        c_string_to_str(&game_state.characters[cn].name)
    );

    Ok(())
}
