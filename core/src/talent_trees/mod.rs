//! Shared talent-tree metadata used by both the server and the client.
//!
//! The persistence layout (packed into `Character::future1`) and the
//! per-class node tables live here so the client can render a tree
//! without depending on server-only effect dispatch.  The server crate
//! layers on top of these definitions an effect table that is invoked
//! whenever a node is learned.
//!
//! Persistence layout (`future1: [u8; 25]`):
//!
//! * `future1[0]` — unspent talent points (0..=255).
//! * `future1[1..24]` — one byte per talent layer; each of the 8 bits
//!   in a byte represents a single node in that layer.

use crate::skills::{self, Attribute, Skill, SkillIndex};

use crate::traits::{
    Class, KIN_ARCHHARAKIM, KIN_ARCHTEMPLAR, KIN_HARAKIM, KIN_MERCENARY, KIN_SEYAN_DU,
    KIN_SORCERER, KIN_TEMPLAR, KIN_WARRIOR,
};

pub mod harakim;
pub mod mercenary;
pub mod seyan_du;
pub mod templar;

/// Index of the unspent-points byte in the packed talent-tree array.
pub const TALENT_POINTS_INDEX: usize = 0;

/// First byte index that represents a talent layer (inclusive).
pub const TALENT_LAYER_START: usize = 1;

/// One past the last valid talent-layer byte index (exclusive).
pub const TALENT_LAYER_END: usize = 24;

/// Maximum number of layer bytes available for talent storage
/// (`TALENT_LAYER_END - TALENT_LAYER_START`).
pub const TALENT_LAYER_COUNT: usize = TALENT_LAYER_END - TALENT_LAYER_START;

/// A reference to a node by its packed `(layer, mask)` slot.
///
/// Used in the `prereqs` slice on [`TalentNodeMeta`] to identify the
/// prerequisite layer for a node. Talent progression allows one pick per
/// layer, so any learned talent in the highest prerequisite layer satisfies
/// the gate for the next layer.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TalentRef {
    /// Layer index in `future1`, in `TALENT_LAYER_START..TALENT_LAYER_END`.
    pub layer: u8,
    /// Single-bit mask within the layer byte.
    pub mask: u8,
}

impl TalentRef {
    /// Creates a talent slot reference from wire payload bytes.
    ///
    /// # Arguments
    ///
    /// * `layer` - Talent layer byte index.
    /// * `mask` - Single-bit node mask within the layer.
    ///
    /// # Returns
    ///
    /// * `Ok(slot)` when `layer` is in range and `mask` has one bit set.
    /// * `Err(reason)` when either byte is invalid.
    pub fn from_wire(layer: u8, mask: u8) -> Result<Self, String> {
        let slot = Self { layer, mask };
        if !slot.has_valid_layer() {
            return Err("Invalid talent layer".to_string());
        }
        if !slot.has_valid_mask() {
            return Err("Talent mask must have exactly one bit set".to_string());
        }
        Ok(slot)
    }

    /// Returns whether this slot's layer is valid for talent storage.
    ///
    /// # Returns
    ///
    /// * `true` if `layer` is in `TALENT_LAYER_START..TALENT_LAYER_END`.
    /// * `false` otherwise.
    pub fn has_valid_layer(self) -> bool {
        (TALENT_LAYER_START..TALENT_LAYER_END).contains(&(self.layer as usize))
    }

    /// Returns whether this slot's mask identifies exactly one bit.
    ///
    /// # Returns
    ///
    /// * `true` if `mask` has exactly one bit set.
    /// * `false` otherwise.
    pub fn has_valid_mask(self) -> bool {
        self.mask.count_ones() == 1
    }
}

/// The set of mutations a learned talent can apply.
#[allow(dead_code)]
#[derive(Copy, Clone, Debug)]
pub enum TalentEffect {
    /// Add `amount` to a skill's base value.
    SkillFlat { skill: Skill, amount: u8 },
    /// Add `percent`% of the current base to a skill's base value.
    SkillPercent { skill: Skill, percent: i32 },
    /// Add `amount` to an attribute's base value.
    AttributeFlat { attr: Attribute, amount: u8 },
    /// Add `percent`% of the current base to an attribute's base value.
    AttributePercent { attr: Attribute, percent: i32 },
    /// Add `percent`% dodge chance to the character's total.
    DodgeChancePercent { percent: i32 },
    /// Grant a previously-unknown skill (set base value to 1).
    GrantSkill { skill: Skill },
}

/// Class-agnostic, effect-agnostic description of one talent node.
///
/// All fields are `'static` so per-class tables can be `const`/`static`
/// values stored in read-only memory.
#[derive(Copy, Clone, Debug)]
pub struct TalentNode {
    /// Packed talent slot used for persistence and wire identity.
    pub slot: TalentRef,
    /// Display name shown on the talent button.
    pub name: &'static str,
    /// Tooltip / long-form description.
    pub description: &'static str,
    /// Cost in talent points to learn this node.  Currently always `1`.
    pub cost: u8,
    /// Prior-layer nodes that gate this node. An empty slice means the node
    /// is a root. When multiple entries are present, they represent alternate
    /// picks in the same prerequisite layer, not a requirement to learn all of
    /// them.
    pub prereqs: &'static [TalentRef],
    /// Runtime effect applied by this node.
    pub effect: TalentEffect,
}

/// Accumulated talent stat bonuses for one character recompute.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TalentStatBonuses {
    /// Attribute bonuses indexed by [`Attribute`] discriminant.
    pub attrib: [i32; 5],
    /// Skill bonuses indexed by canonical [`Skill`] discriminant.
    pub skill: [i32; 50],
    /// Dodge chance bonuses from talents, in percent.
    pub dodge: i32,
}

impl Default for TalentStatBonuses {
    fn default() -> Self {
        Self {
            attrib: [0; 5],
            skill: [0; 50],
            dodge: 0,
        }
    }
}

/// A complete per-class talent tree.
#[derive(Copy, Clone, Debug)]
pub struct TalentTree {
    /// The class this tree belongs to.
    pub class: Class,
    /// Flat list of every node in the tree.
    pub nodes: &'static [TalentNode],
}

/// Look up the talent tree for a given class.
///
/// Returns `None` for classes that do not have a tree defined yet.
///
/// # Arguments
///
/// * `class` - The character's class.
///
/// # Returns
///
/// * `Some(tree)` if the class has a registered tree.
/// * `None` otherwise.
pub fn tree_for(class: Class) -> Option<&'static TalentTree> {
    match class {
        Class::Harakim => Some(&harakim::HARAKIM_TREE),
        Class::ArchHarakim => Some(&harakim::HARAKIM_TREE),
        Class::Mercenary => Some(&mercenary::MERCENARY_TREE),
        Class::Warrior => Some(&mercenary::MERCENARY_TREE),
        Class::Sorcerer => Some(&mercenary::MERCENARY_TREE),
        Class::SeyanDu => Some(&seyan_du::SEYAN_DU_TREE),
        Class::Templar => Some(&templar::TEMPLAR_TREE),
        Class::ArchTemplar => Some(&templar::TEMPLAR_TREE),
    }
}

/// Find a node within a tree by its packed slot.
///
/// Linear search; trees are tiny (≤ 184 nodes by storage limit).
///
/// # Arguments
///
/// * `tree` - Tree to search.
/// * `slot` - Node slot to match.
///
/// # Returns
///
/// * `Some(&node)` if a node with `slot` is present in `tree`.
/// * `None` otherwise.
pub fn find_node(tree: &'static TalentTree, slot: TalentRef) -> Option<&'static TalentNode> {
    tree.nodes.iter().find(|n| n.slot == slot)
}

/// Resolve the `Class` represented by the kindred bitfield on a
/// `Character`.
///
/// The bitfield can carry several flags (sex, monster, etc.); only the
/// class bits are inspected, in priority order.
///
/// # Arguments
///
/// * `kindred` - Raw `Character::kindred` value (`i32`).
///
/// # Returns
///
/// * `Some(class)` for the first matching class bit.
/// * `None` if no class bit is set.
pub fn class_for_kindred(kindred: i32) -> Option<Class> {
    let k = kindred as u32;
    if k & KIN_MERCENARY != 0 {
        Some(Class::Mercenary)
    } else if k & KIN_TEMPLAR != 0 {
        Some(Class::Templar)
    } else if k & KIN_HARAKIM != 0 {
        Some(Class::Harakim)
    } else if k & KIN_SEYAN_DU != 0 {
        Some(Class::SeyanDu)
    } else if k & KIN_ARCHTEMPLAR != 0 {
        Some(Class::ArchTemplar)
    } else if k & KIN_ARCHHARAKIM != 0 {
        Some(Class::ArchHarakim)
    } else if k & KIN_SORCERER != 0 {
        Some(Class::Sorcerer)
    } else if k & KIN_WARRIOR != 0 {
        Some(Class::Warrior)
    } else {
        None
    }
}

/// Read the number of unspent talent points from the packed array.
///
/// # Arguments
///
/// * `talents` - Packed talent-tree state.
///
/// # Returns
///
/// * The value of `talents[TALENT_POINTS_INDEX]`.
pub fn available_talent_points(talents: &[u8; 25]) -> u8 {
    talents[TALENT_POINTS_INDEX]
}

/// Count how many talent points have been spent across every layer.
///
/// # Arguments
///
/// * `talents` - Packed talent-tree state.
///
/// # Returns
///
/// * Sum of set bits across `talents[TALENT_LAYER_START..TALENT_LAYER_END]`.
pub fn total_points_spent(talents: &[u8; 25]) -> u32 {
    talents[TALENT_LAYER_START..TALENT_LAYER_END]
        .iter()
        .map(|b| b.count_ones())
        .sum()
}

/// Check whether a specific talent slot is currently unlocked.
///
/// # Arguments
///
/// * `talents` - Packed talent-tree state.
/// * `talent_mask` - Single-bit mask of the node being queried.
/// * `talent_layer` - Layer/rank index in
///   `TALENT_LAYER_START..TALENT_LAYER_END`.
///
/// # Returns
///
/// * `true` if `talent_layer` is valid, `talent_mask` is non-zero, and
///   all of its bits are set in `talents[talent_layer]`.
/// * `false` otherwise.
pub fn is_talent_spent(talents: &[u8; 25], talent_mask: u8, talent_layer: usize) -> bool {
    if !(TALENT_LAYER_START..TALENT_LAYER_END).contains(&talent_layer) {
        return false;
    }
    talent_mask != 0 && talents[talent_layer] & talent_mask == talent_mask
}

/// Check whether a specific talent slot is currently unlocked.
///
/// # Arguments
///
/// * `talents` - Packed talent-tree state.
/// * `slot` - Talent slot being queried.
///
/// # Returns
///
/// * `true` if the slot is valid and learned.
/// * `false` otherwise.
pub fn is_talent_slot_spent(talents: &[u8; 25], slot: TalentRef) -> bool {
    is_talent_spent(talents, slot.mask, slot.layer as usize)
}

/// Check whether any talent is already learned in a layer.
///
/// # Arguments
///
/// * `talents` - Packed talent-tree state.
/// * `talent_layer` - Layer/rank index in
///   `TALENT_LAYER_START..TALENT_LAYER_END`.
///
/// # Returns
///
/// * `true` if `talent_layer` is valid and any bit is set in that layer.
/// * `false` otherwise.
pub fn is_talent_layer_spent(talents: &[u8; 25], talent_layer: usize) -> bool {
    (TALENT_LAYER_START..TALENT_LAYER_END).contains(&talent_layer) && talents[talent_layer] != 0
}

/// Check whether a node's prerequisite layer is satisfied.
///
/// The talent tree grants one pick per layer. A node with no prereqs is a
/// root. A non-root node is available once any talent has been learned in the
/// highest prerequisite layer listed on the node.
///
/// # Arguments
///
/// * `talents` - Packed talent-tree state.
/// * `node` - Node whose prerequisite gate should be evaluated.
///
/// # Returns
///
/// * `true` if the node is a root or its prerequisite layer contains a learned
///   talent.
/// * `false` otherwise.
pub fn talent_prereqs_met(talents: &[u8; 25], node: &TalentNode) -> bool {
    let Some(required_layer) = node.prereqs.iter().map(|p| p.layer as usize).max() else {
        return true;
    };
    is_talent_layer_spent(talents, required_layer)
}

/// Spend a single talent point on one node in a specific slot.
///
/// # Arguments
///
/// * `talents` - Packed talent-tree state.
/// * `slot` - Slot of the node being unlocked.
///
/// # Returns
///
/// * `Ok(())` if the point was spent.
/// * `Err` describing the rejection reason.
pub fn apply_talent_point(talents: &mut [u8; 25], slot: TalentRef) -> Result<(), String> {
    if !slot.has_valid_layer() {
        return Err("Invalid talent layer".to_string());
    }

    if !slot.has_valid_mask() {
        return Err("Talent mask must have exactly one bit set".to_string());
    }

    let talent_layer = slot.layer as usize;
    if talents[talent_layer] & slot.mask != 0 {
        return Err("Talent already learned".to_string());
    }

    if is_talent_layer_spent(talents, talent_layer) {
        return Err("A talent is already learned in this layer".to_string());
    }

    if talents[TALENT_POINTS_INDEX] < 1 {
        return Err("Not enough points to spend".to_string());
    }

    talents[TALENT_POINTS_INDEX] -= 1;
    talents[talent_layer] |= slot.mask;

    Ok(())
}

/// Refund every spent talent point back into the unspent-points pool.
///
/// All layer bytes are cleared and the count of previously-set bits is
/// added back into `talents[0]`, saturating at `u8::MAX`.
///
/// # Arguments
///
/// * `talents` - Packed talent-tree state to reset in place.
pub fn reset_talent_points(talents: &mut [u8; 25]) {
    let mut refunded_points: u32 = 0;
    for byte in talents
        .iter_mut()
        .take(TALENT_LAYER_END)
        .skip(TALENT_LAYER_START)
    {
        refunded_points += byte.count_ones();
        *byte = 0;
    }
    let refund = u8::try_from(refunded_points).unwrap_or(u8::MAX);
    talents[TALENT_POINTS_INDEX] = talents[TALENT_POINTS_INDEX].saturating_add(refund);
}

/// Grant additional unspent talent points to the player's pool.
///
/// Saturates at `u8::MAX`.
///
/// # Arguments
///
/// * `talents` - Packed talent-tree state to update in place.
/// * `amount` - Number of points to add to `talents[0]`.
pub fn grant_talent_points(talents: &mut [u8; 25], amount: u8) {
    talents[TALENT_POINTS_INDEX] = talents[TALENT_POINTS_INDEX].saturating_add(amount);
}

/// Calculate all stat bonuses granted by currently learned talents.
///
/// # Arguments
///
/// * `kindred` - Character kindred bits used to resolve the class tree.
/// * `talents` - Packed talent-tree state from the character record.
/// * `attrib` - Current attribute rows for base-value lookup.
/// * `skill` - Current skill rows for base-value lookup.
///
/// # Returns
///
/// * Accumulated attribute and skill bonuses from learned stat talents.
pub fn talent_stat_bonuses(
    kindred: i32,
    talents: &[u8; 25],
    attrib: &[[u8; SkillIndex::MaxIndex as usize]; 5],
    skill: &[[u8; SkillIndex::MaxIndex as usize]; 50],
) -> TalentStatBonuses {
    let Some(class) = class_for_kindred(kindred) else {
        return TalentStatBonuses::default();
    };
    let Some(tree) = tree_for(class) else {
        return TalentStatBonuses::default();
    };

    let mut bonuses = TalentStatBonuses::default();

    for node in tree.nodes {
        if !is_talent_slot_spent(talents, node.slot) {
            continue;
        }
        accumulate_stat_bonus(node.effect, attrib, skill, &mut bonuses);
    }

    bonuses
}

/// Calculate dodge chance bonuses granted by currently learned talents.
///
/// Only learned talent nodes whose effect is [`TalentEffect::DodgeChancePercent`]
/// contribute to the returned value. Characters without a registered talent
/// tree, or without any learned dodge talents, receive no bonus.
///
/// # Arguments
///
/// * `kindred` - Character kindred bits used to resolve the class tree.
/// * `talents` - Packed talent-tree state from the character record.
///
/// # Returns
///
/// * Accumulated dodge chance bonus from learned talents, in percent.
pub fn talent_dodge_bonuses(kindred: i32, talents: &[u8; 25]) -> i32 {
    let Some(class) = class_for_kindred(kindred) else {
        log::warn!(
            "Unknown class for kindred bits {kindred:#010x}; no talent bonuses will be applied"
        );
        return 0;
    };
    let Some(tree) = tree_for(class) else {
        log::warn!(
            "No talent tree registered for class {:?}; no talent bonuses will be applied",
            class
        );
        return 0;
    };

    let mut bonus_percent = 0;

    for node in tree.nodes {
        if !is_talent_slot_spent(talents, node.slot) {
            continue;
        }
        if let TalentEffect::DodgeChancePercent { percent } = node.effect {
            bonus_percent += percent;
        }
    }

    bonus_percent
}

/// Add one effect's derived stat contribution into `bonuses`.
///
/// # Arguments
///
/// * `effect` - Effect to translate into derived bonuses.
/// * `attrib` - Attribute rows for base-value lookup.
/// * `skill_rows` - Skill rows for base-value lookup.
/// * `bonuses` - Accumulator to mutate.
fn accumulate_stat_bonus(
    effect: TalentEffect,
    attrib: &[[u8; SkillIndex::MaxIndex as usize]; 5],
    skill_rows: &[[u8; SkillIndex::MaxIndex as usize]; 50],
    bonuses: &mut TalentStatBonuses,
) {
    match effect {
        TalentEffect::SkillFlat { skill, amount } => {
            let skill_idx = skills::canonicalize_weapon_skill(skill as usize);
            bonuses.skill[skill_idx] += amount as i32;
        }
        TalentEffect::SkillPercent { skill, percent } => {
            let skill_idx = skills::canonicalize_weapon_skill(skill as usize);
            let base = skill_rows[skill_idx][SkillIndex::BaseValue as usize];
            bonuses.skill[skill_idx] += percent_bonus(base, percent);
        }
        TalentEffect::AttributeFlat { attr, amount } => {
            bonuses.attrib[attr as usize] += amount as i32;
        }
        TalentEffect::AttributePercent { attr, percent } => {
            let base = attrib[attr as usize][SkillIndex::BaseValue as usize];
            bonuses.attrib[attr as usize] += percent_bonus(base, percent);
        }
        TalentEffect::DodgeChancePercent { percent } => {
            bonuses.dodge += percent;
        }
        TalentEffect::GrantSkill { .. } => {}
    }
}

/// Calculate a rounded non-negative percent bonus from `base`.
///
/// # Arguments
///
/// * `base` - Base stat value.
/// * `percent` - Percentage bonus to apply.
///
/// # Returns
///
/// * Rounded bonus amount, clamped into `0..=u8::MAX`.
fn percent_bonus(base: u8, percent: i32) -> i32 {
    ((base as f32 * (percent as f32 / 100.0)).round() as i32).clamp(0, u8::MAX as i32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Returns every tree currently registered with [`tree_for`].
    fn all_trees() -> Vec<&'static TalentTree> {
        vec![
            &harakim::HARAKIM_TREE,
            &mercenary::MERCENARY_TREE,
            &seyan_du::SEYAN_DU_TREE,
            &templar::TEMPLAR_TREE,
        ]
    }

    fn named_node(tree: &'static TalentTree, name: &str) -> &'static TalentNode {
        tree.nodes
            .iter()
            .find(|node| node.name == name)
            .unwrap_or_else(|| panic!("missing talent node '{name}'"))
    }

    // ---- structural validation (runs over every registered tree) ------

    #[test]
    fn every_node_has_single_bit_mask() {
        for tree in all_trees() {
            for node in tree.nodes {
                assert_eq!(
                    node.slot.mask.count_ones(),
                    1,
                    "tree {:?} node '{}' has non-single-bit mask 0x{:02x}",
                    tree.class,
                    node.name,
                    node.slot.mask
                );
            }
        }
    }

    #[test]
    fn every_node_layer_in_range() {
        for tree in all_trees() {
            for node in tree.nodes {
                let layer = node.slot.layer as usize;
                assert!(
                    (TALENT_LAYER_START..TALENT_LAYER_END).contains(&layer),
                    "tree {:?} node '{}' has out-of-range layer {}",
                    tree.class,
                    node.name,
                    layer
                );
            }
        }
    }

    #[test]
    fn no_duplicate_layer_mask_pairs() {
        for tree in all_trees() {
            let mut seen = HashSet::new();
            for node in tree.nodes {
                let key = node.slot;
                if let Some(prev) = tree
                    .nodes
                    .iter()
                    .find(|n| !std::ptr::eq(*n, node) && n.slot == key)
                {
                    if seen.contains(&key) {
                        // already reported once
                        continue;
                    }
                    panic!(
                        "tree {:?} has duplicate (layer={}, mask=0x{:02x}) at nodes '{}' and '{}'",
                        tree.class, node.slot.layer, node.slot.mask, node.name, prev.name
                    );
                }
                seen.insert(key);
            }
        }
    }

    #[test]
    fn no_duplicate_node_names() {
        for tree in all_trees() {
            let mut seen = HashSet::new();
            for node in tree.nodes {
                assert!(
                    seen.insert(node.name),
                    "tree {:?} has duplicate node name '{}'",
                    tree.class,
                    node.name,
                );
            }
        }
    }

    #[test]
    fn every_prereq_resolves() {
        for tree in all_trees() {
            for node in tree.nodes {
                for prereq in node.prereqs {
                    let resolved = tree.nodes.iter().any(|n| n.slot == *prereq);
                    assert!(
                        resolved,
                        "tree {:?} node '{}' has dangling prereq (layer={}, mask=0x{:02x})",
                        tree.class, node.name, prereq.layer, prereq.mask
                    );
                }
            }
        }
    }

    #[test]
    fn prereqs_are_in_strictly_lower_layers() {
        for tree in all_trees() {
            for node in tree.nodes {
                for prereq in node.prereqs {
                    assert!(
                        prereq.layer < node.slot.layer,
                        "tree {:?} node '{}' (layer {}) has non-strictly-lower prereq layer {}",
                        tree.class,
                        node.name,
                        node.slot.layer,
                        prereq.layer
                    );
                }
            }
        }
    }

    #[test]
    fn node_does_not_list_itself_as_prereq() {
        for tree in all_trees() {
            for node in tree.nodes {
                for prereq in node.prereqs {
                    assert!(
                        *prereq != node.slot,
                        "tree {:?} node '{}' lists itself as prereq",
                        tree.class,
                        node.name,
                    );
                }
            }
        }
    }

    #[test]
    fn no_duplicate_prereqs_within_node() {
        for tree in all_trees() {
            for node in tree.nodes {
                let mut seen = HashSet::new();
                for prereq in node.prereqs {
                    assert!(
                        seen.insert((prereq.layer, prereq.mask)),
                        "tree {:?} node '{}' has duplicate prereq (layer={}, mask=0x{:02x})",
                        tree.class,
                        node.name,
                        prereq.layer,
                        prereq.mask
                    );
                }
            }
        }
    }

    #[test]
    fn cost_is_at_least_one() {
        for tree in all_trees() {
            for node in tree.nodes {
                assert!(
                    node.cost >= 1,
                    "tree {:?} node '{}' has cost {} (must be >= 1)",
                    tree.class,
                    node.name,
                    node.cost,
                );
            }
        }
    }

    #[test]
    fn tree_class_matches_module() {
        assert_eq!(harakim::HARAKIM_TREE.class, Class::Harakim);
        assert_eq!(mercenary::MERCENARY_TREE.class, Class::Mercenary);
        assert_eq!(seyan_du::SEYAN_DU_TREE.class, Class::SeyanDu);
        assert_eq!(templar::TEMPLAR_TREE.class, Class::Templar);
    }

    #[test]
    fn name_and_description_non_empty() {
        for tree in all_trees() {
            for node in tree.nodes {
                assert!(
                    !node.name.is_empty(),
                    "tree {:?} has node with empty name",
                    tree.class
                );
                assert!(
                    !node.description.is_empty(),
                    "tree {:?} node '{}' has empty description",
                    tree.class,
                    node.name,
                );
            }
        }
    }

    #[test]
    fn tree_fits_in_storage() {
        let max = 8 * TALENT_LAYER_COUNT;
        for tree in all_trees() {
            assert!(
                tree.nodes.len() <= max,
                "tree {:?} has {} nodes; storage holds at most {}",
                tree.class,
                tree.nodes.len(),
                max,
            );
        }
    }

    // ---- pure-function helpers ----------------------------------------

    #[test]
    fn available_talent_points_reads_slot_zero() {
        let mut t = [0u8; 25];
        t[0] = 42;
        assert_eq!(available_talent_points(&t), 42);
    }

    #[test]
    fn total_points_spent_counts_bits_across_layers() {
        let mut t = [0u8; 25];
        t[0] = 99; // ignored
        t[1] = 0b0000_0101;
        t[5] = 0b1111_0000;
        t[23] = 0b0000_0001;
        assert_eq!(total_points_spent(&t), 2 + 4 + 1);
    }

    #[test]
    fn is_talent_spent_reports_set_bits() {
        let mut t = [0u8; 25];
        t[3] = 0b0001_0000;
        assert!(is_talent_spent(&t, 0b0001_0000, 3));
        assert!(!is_talent_spent(&t, 0b0010_0000, 3));
    }

    #[test]
    fn is_talent_spent_returns_false_for_invalid_layer() {
        let mut t = [0u8; 25];
        t[1] = 0xFF;
        assert!(!is_talent_spent(&t, 1, 0));
        assert!(!is_talent_spent(&t, 1, TALENT_LAYER_END));
    }

    #[test]
    fn is_talent_spent_returns_false_for_zero_mask() {
        let mut t = [0u8; 25];
        t[1] = 0xFF;
        assert!(!is_talent_spent(&t, 0, 1));
    }

    #[test]
    fn is_talent_layer_spent_reports_any_bit_in_valid_layer() {
        let mut t = [0u8; 25];
        assert!(!is_talent_layer_spent(&t, 1));
        t[1] = 0b0000_0010;
        assert!(is_talent_layer_spent(&t, 1));
    }

    #[test]
    fn is_talent_layer_spent_returns_false_for_invalid_layer() {
        let mut t = [0u8; 25];
        t[0] = 0xFF;
        t[TALENT_LAYER_END] = 0xFF;
        assert!(!is_talent_layer_spent(&t, 0));
        assert!(!is_talent_layer_spent(&t, TALENT_LAYER_END));
    }

    #[test]
    fn talent_prereqs_met_allows_one_pick_from_previous_layer() {
        let tree = tree_for(Class::Mercenary).unwrap();
        let dodge = named_node(tree, "Dodge Boost I");
        let mut t = [0u8; 25];
        t[1] = 0b0000_0010;
        assert!(talent_prereqs_met(&t, dodge));
    }

    #[test]
    fn talent_prereqs_met_rejects_empty_prereq_layer() {
        let tree = tree_for(Class::Mercenary).unwrap();
        let dodge = named_node(tree, "Dodge Boost I");
        let t = [0u8; 25];
        assert!(!talent_prereqs_met(&t, dodge));
    }

    #[test]
    fn class_for_kindred_resolves_each_class_bit() {
        assert_eq!(
            class_for_kindred(KIN_MERCENARY as i32),
            Some(Class::Mercenary)
        );
        assert_eq!(class_for_kindred(KIN_TEMPLAR as i32), Some(Class::Templar));
        assert_eq!(class_for_kindred(KIN_HARAKIM as i32), Some(Class::Harakim));
        assert_eq!(class_for_kindred(KIN_SEYAN_DU as i32), Some(Class::SeyanDu));
        assert_eq!(
            class_for_kindred(KIN_ARCHTEMPLAR as i32),
            Some(Class::ArchTemplar)
        );
        assert_eq!(
            class_for_kindred(KIN_ARCHHARAKIM as i32),
            Some(Class::ArchHarakim)
        );
        assert_eq!(
            class_for_kindred(KIN_SORCERER as i32),
            Some(Class::Sorcerer)
        );
        assert_eq!(class_for_kindred(KIN_WARRIOR as i32), Some(Class::Warrior));
    }

    #[test]
    fn class_for_kindred_returns_none_when_no_class_bit_set() {
        assert_eq!(class_for_kindred(0), None);
        // Sex flag alone is not a class.
        assert_eq!(class_for_kindred(crate::traits::KIN_MALE as i32), None);
    }

    #[test]
    fn class_for_kindred_picks_first_matching_bit_when_multiple() {
        let combined = (KIN_MERCENARY | KIN_TEMPLAR) as i32;
        assert_eq!(class_for_kindred(combined), Some(Class::Mercenary));
    }

    #[test]
    fn tree_for_returns_registered_base_class_trees() {
        assert!(tree_for(Class::Mercenary).is_some());
        assert!(tree_for(Class::Templar).is_some());
        assert!(tree_for(Class::Harakim).is_some());
        assert!(tree_for(Class::SeyanDu).is_some());
        assert!(tree_for(Class::Sorcerer).is_some());
        assert!(tree_for(Class::Warrior).is_some());
        assert!(tree_for(Class::ArchTemplar).is_some());
        assert!(tree_for(Class::ArchHarakim).is_some());
    }

    #[test]
    fn find_node_locates_existing_slot_and_misses_unknown() {
        let tree = tree_for(Class::Mercenary).unwrap();
        let first = tree.nodes.first().expect("mercenary tree non-empty");
        assert!(find_node(tree, first.slot).is_some());
        assert!(
            find_node(
                tree,
                TalentRef {
                    layer: 23,
                    mask: 0b1000_0000,
                }
            )
            .is_none()
        );
    }

    #[test]
    fn talent_ref_from_wire_validates_layer_and_mask() {
        assert_eq!(
            TalentRef::from_wire(1, 0b0000_0010).unwrap(),
            TalentRef {
                layer: 1,
                mask: 0b0000_0010,
            }
        );
        assert!(TalentRef::from_wire(0, 1).is_err());
        assert!(TalentRef::from_wire(TALENT_LAYER_END as u8, 1).is_err());
        assert!(TalentRef::from_wire(1, 0).is_err());
        assert!(TalentRef::from_wire(1, 0b0000_0011).is_err());
    }

    #[test]
    fn talent_dodge_bonuses_returns_zero_for_unknown_class() {
        let talents = [0u8; 25];

        assert_eq!(talent_dodge_bonuses(0, &talents), 0);
        assert_eq!(
            talent_dodge_bonuses(crate::traits::KIN_MALE as i32, &talents),
            0
        );
    }

    #[test]
    fn talent_dodge_bonuses_returns_zero_for_class_without_registered_tree() {
        let talents = [0u8; 25];

        assert_eq!(talent_dodge_bonuses(KIN_WARRIOR as i32, &talents), 0);
        assert_eq!(talent_dodge_bonuses(KIN_SORCERER as i32, &talents), 0);
    }

    #[test]
    fn talent_dodge_bonuses_returns_zero_when_no_dodge_talent_is_learned() {
        let tree = tree_for(Class::Mercenary).unwrap();
        let distract = named_node(tree, "Distract").slot;
        let mut talents = [0u8; 25];
        talents[distract.layer as usize] |= distract.mask;

        assert_eq!(talent_dodge_bonuses(KIN_MERCENARY as i32, &talents), 0);
    }

    #[test]
    fn talent_dodge_bonuses_returns_first_dodge_boost_bonus() {
        let tree = tree_for(Class::Mercenary).unwrap();
        let dodge_boost_1 = named_node(tree, "Dodge Boost I").slot;
        let mut talents = [0u8; 25];
        talents[dodge_boost_1.layer as usize] |= dodge_boost_1.mask;

        assert_eq!(talent_dodge_bonuses(KIN_MERCENARY as i32, &talents), 5);
    }

    #[test]
    fn talent_dodge_bonuses_accumulates_multiple_dodge_boosts() {
        let tree = tree_for(Class::Mercenary).unwrap();
        let dodge_boost_1 = named_node(tree, "Dodge Boost I").slot;
        let dodge_boost_2 = named_node(tree, "Dodge Boost II").slot;
        let mut talents = [0u8; 25];
        talents[dodge_boost_1.layer as usize] |= dodge_boost_1.mask;
        talents[dodge_boost_2.layer as usize] |= dodge_boost_2.mask;

        assert_eq!(talent_dodge_bonuses(KIN_MERCENARY as i32, &talents), 10);
    }
}
