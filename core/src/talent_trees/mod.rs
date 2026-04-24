//! Shared talent-tree metadata used by both the server and the client.
//!
//! The persistence layout (packed into `Character::future1`) and the
//! per-class node tables live here so the client can render a tree
//! without depending on server-only effect dispatch.  The server crate
//! layers on top of these definitions an effect table that is invoked
//! whenever a node is learned.
//!
//! Persistence layout (`future1: [i8; 25]`, reinterpreted as `[u8; 25]`):
//!
//! * `future1[0]` — unspent talent points (0..=255).
//! * `future1[1..24]` — one byte per talent layer; each of the 8 bits
//!   in a byte represents a single node in that layer.

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

/// Stable, class-scoped node identifier used by the wire protocol.
///
/// Independent of position in the tree so the UI/balance team can move
/// a node between layers without invalidating existing save data.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TalentId(pub u16);

/// A reference to a node by its packed `(layer, mask)` slot.
///
/// Used in the `prereqs` slice on [`TalentNodeMeta`] to identify the
/// prerequisite layer for a node. Talent progression allows one pick per
/// layer, so any learned talent in the highest prerequisite layer satisfies
/// the gate for the next layer.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TalentRef {
    /// Layer index in `future1`, in `TALENT_LAYER_START..TALENT_LAYER_END`.
    pub layer: u8,
    /// Single-bit mask within the layer byte.
    pub mask: u8,
}

/// Class-agnostic, effect-agnostic description of one talent node.
///
/// All fields are `'static` so per-class tables can be `const`/`static`
/// values stored in read-only memory.
#[derive(Copy, Clone, Debug)]
pub struct TalentNodeMeta {
    /// Stable wire-protocol identifier.
    pub id: TalentId,
    /// Layer this node lives in, in
    /// `TALENT_LAYER_START..TALENT_LAYER_END`.
    pub layer: u8,
    /// Single-bit mask within `future1[layer]` (1, 2, 4, ..., 128).
    pub mask: u8,
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
}

/// A complete per-class talent tree.
#[derive(Copy, Clone, Debug)]
pub struct TalentTreeMeta {
    /// The class this tree belongs to.
    pub class: Class,
    /// Flat list of every node in the tree.
    pub nodes: &'static [TalentNodeMeta],
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
pub fn tree_for(class: Class) -> Option<&'static TalentTreeMeta> {
    match class {
        Class::Harakim => Some(&harakim::HARAKIM_TREE),
        Class::Mercenary => Some(&mercenary::MERCENARY_TREE),
        Class::SeyanDu => Some(&seyan_du::SEYAN_DU_TREE),
        Class::Templar => Some(&templar::TEMPLAR_TREE),
        _ => None,
    }
}

/// Find a node within a tree by its stable id.
///
/// Linear search; trees are tiny (≤ 184 nodes by storage limit).
///
/// # Arguments
///
/// * `tree` - Tree to search.
/// * `id` - Node identifier to match.
///
/// # Returns
///
/// * `Some(&node)` if a node with `id` is present in `tree`.
/// * `None` otherwise.
pub fn find_node(tree: &'static TalentTreeMeta, id: TalentId) -> Option<&'static TalentNodeMeta> {
    tree.nodes.iter().find(|n| n.id == id)
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
pub fn talent_prereqs_met(talents: &[u8; 25], node: &TalentNodeMeta) -> bool {
    let Some(required_layer) = node.prereqs.iter().map(|p| p.layer as usize).max() else {
        return true;
    };
    is_talent_layer_spent(talents, required_layer)
}

/// Reinterpret a `&mut [i8; 25]` (the raw `Character::future1`
/// representation) as `&mut [u8; 25]` for bit-packed talent storage.
///
/// `i8` and `u8` have identical layout, so the cast is safe.
///
/// # Arguments
///
/// * `future1` - The character's raw 25-byte scratch slot.
///
/// # Returns
///
/// * The same memory viewed as an unsigned byte array.
pub fn talents_mut_from_future1(future1: &mut [i8; 25]) -> &mut [u8; 25] {
    // SAFETY: `i8` and `u8` have identical size/alignment; this
    // transmute is a well-defined view cast.
    unsafe { &mut *(future1.as_mut_ptr() as *mut [u8; 25]) }
}

/// Const counterpart of [`talents_mut_from_future1`].
///
/// # Arguments
///
/// * `future1` - The character's raw 25-byte scratch slot.
///
/// # Returns
///
/// * The same memory viewed as an unsigned byte array.
pub fn talents_from_future1(future1: &[i8; 25]) -> &[u8; 25] {
    // SAFETY: `i8` and `u8` have identical size/alignment; this
    // transmute is a well-defined view cast.
    unsafe { &*(future1.as_ptr() as *const [u8; 25]) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Returns every tree currently registered with [`tree_for`].
    fn all_trees() -> Vec<&'static TalentTreeMeta> {
        vec![
            &harakim::HARAKIM_TREE,
            &mercenary::MERCENARY_TREE,
            &seyan_du::SEYAN_DU_TREE,
            &templar::TEMPLAR_TREE,
        ]
    }

    // ---- structural validation (runs over every registered tree) ------

    #[test]
    fn every_node_has_single_bit_mask() {
        for tree in all_trees() {
            for node in tree.nodes {
                assert_eq!(
                    node.mask.count_ones(),
                    1,
                    "tree {:?} node '{}' has non-single-bit mask 0x{:02x}",
                    tree.class,
                    node.name,
                    node.mask
                );
            }
        }
    }

    #[test]
    fn every_node_layer_in_range() {
        for tree in all_trees() {
            for node in tree.nodes {
                let layer = node.layer as usize;
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
                let key = (node.layer, node.mask);
                if let Some(prev) = tree
                    .nodes
                    .iter()
                    .find(|n| !std::ptr::eq(*n, node) && (n.layer, n.mask) == key)
                {
                    if seen.contains(&key) {
                        // already reported once
                        continue;
                    }
                    panic!(
                        "tree {:?} has duplicate (layer={}, mask=0x{:02x}) at nodes '{}' and '{}'",
                        tree.class, node.layer, node.mask, node.name, prev.name
                    );
                }
                seen.insert(key);
            }
        }
    }

    #[test]
    fn no_duplicate_talent_ids() {
        for tree in all_trees() {
            let mut seen = HashSet::new();
            for node in tree.nodes {
                assert!(
                    seen.insert(node.id),
                    "tree {:?} has duplicate TalentId({}) at node '{}'",
                    tree.class,
                    node.id.0,
                    node.name,
                );
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
                    let resolved = tree
                        .nodes
                        .iter()
                        .any(|n| n.layer == prereq.layer && n.mask == prereq.mask);
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
                        prereq.layer < node.layer,
                        "tree {:?} node '{}' (layer {}) has non-strictly-lower prereq layer {}",
                        tree.class,
                        node.name,
                        node.layer,
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
                        !(prereq.layer == node.layer && prereq.mask == node.mask),
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
        let dodge = find_node(tree, mercenary::ids::DODGE_BOOST_1).unwrap();
        let mut t = [0u8; 25];
        t[1] = 0b0000_0010;
        assert!(talent_prereqs_met(&t, dodge));
    }

    #[test]
    fn talent_prereqs_met_rejects_empty_prereq_layer() {
        let tree = tree_for(Class::Mercenary).unwrap();
        let dodge = find_node(tree, mercenary::ids::DODGE_BOOST_1).unwrap();
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
        assert!(tree_for(Class::Sorcerer).is_none());
        assert!(tree_for(Class::Warrior).is_none());
        assert!(tree_for(Class::ArchTemplar).is_none());
        assert!(tree_for(Class::ArchHarakim).is_none());
    }

    #[test]
    fn find_node_locates_existing_id_and_misses_unknown() {
        let tree = tree_for(Class::Mercenary).unwrap();
        let first = tree.nodes.first().expect("mercenary tree non-empty");
        assert!(find_node(tree, first.id).is_some());
        assert!(find_node(tree, TalentId(0xFFFF)).is_none());
    }

    #[test]
    fn talents_view_casts_are_byte_identical() {
        let mut buf: [i8; 25] = [0; 25];
        for (i, b) in buf.iter_mut().enumerate() {
            *b = (i as i8).wrapping_mul(17);
        }
        let original = buf;

        // mut view: write through u8, observe through i8
        {
            let view = talents_mut_from_future1(&mut buf);
            view[5] = 0xAB;
        }
        assert_eq!(buf[5] as u8, 0xAB);

        // const view: read through u8, compare against original i8 reps
        let view = talents_from_future1(&buf);
        for (i, b) in view.iter().enumerate() {
            let expected = if i == 5 { 0xAB } else { original[i] as u8 };
            assert_eq!(*b, expected, "byte {i} mismatch");
        }
    }
}
