//! Per-node effect table for the mercenary talent tree.
//!
//! Metadata (ids, names, prereqs) lives in
//! [`core::talent_trees::mercenary`].  This file maps each node id to
//! the runtime mutation applied when the talent is learned.

use core::skills::Attribute;
use core::talent_trees::{TalentId, mercenary::ids};

use super::TalentEffect;

/// `TalentId` -> `TalentEffect` lookup table for the mercenary tree.
///
/// Kept in declaration order matching `core::talent_trees::mercenary::MERCENARY_TREE`
/// for readability.  Lookup is linear; with 23 nodes a hash map is
/// unnecessary.
///
/// **Note (placeholder effects):** every node currently grants an
/// `AttributePercent` bonus.  These are intentional placeholders until
/// the proper per-node behaviour (Distract, Parasite, Disarm, etc.) is
/// implemented.
pub static MERCENARY_TALENT_EFFECTS: &[(TalentId, TalentEffect)] = &[
    (
        ids::DISTRACT,
        TalentEffect::AttributePercent {
            attr: Attribute::Strength,
            percent: 10,
        },
    ),
    (
        ids::PARASITE,
        TalentEffect::AttributePercent {
            attr: Attribute::Willpower,
            percent: 10,
        },
    ),
    (
        ids::DODGE_BOOST_1,
        TalentEffect::AttributePercent {
            attr: Attribute::Agility,
            percent: 10,
        },
    ),
    (
        ids::SPELL_BOOST_1,
        TalentEffect::AttributePercent {
            attr: Attribute::Willpower,
            percent: 10,
        },
    ),
    (
        ids::DODGE_BOOST_2,
        TalentEffect::AttributePercent {
            attr: Attribute::Agility,
            percent: 15,
        },
    ),
    (
        ids::SPELL_BOOST_2,
        TalentEffect::AttributePercent {
            attr: Attribute::Willpower,
            percent: 15,
        },
    ),
    (
        ids::ATTACK_SPEED_BOOST_1,
        TalentEffect::AttributePercent {
            attr: Attribute::Agility,
            percent: 10,
        },
    ),
    (
        ids::DAMAGE_BOOST_1,
        TalentEffect::AttributePercent {
            attr: Attribute::Strength,
            percent: 10,
        },
    ),
    (
        ids::DISARM,
        TalentEffect::AttributePercent {
            attr: Attribute::Intuition,
            percent: 10,
        },
    ),
    (
        ids::DELIVER_DEATH,
        TalentEffect::AttributePercent {
            attr: Attribute::Strength,
            percent: 15,
        },
    ),
    (
        ids::ATTACK_SPEED_BOOST_2,
        TalentEffect::AttributePercent {
            attr: Attribute::Agility,
            percent: 15,
        },
    ),
    (
        ids::DAMAGE_BOOST_2,
        TalentEffect::AttributePercent {
            attr: Attribute::Strength,
            percent: 15,
        },
    ),
    (
        ids::PROTECTIVE_SPELLS_BOOST_1,
        TalentEffect::AttributePercent {
            attr: Attribute::Willpower,
            percent: 10,
        },
    ),
    (
        ids::IMMUN_RESIST_BOOST_1,
        TalentEffect::AttributePercent {
            attr: Attribute::Braveness,
            percent: 10,
        },
    ),
    (
        ids::PROTECTIVE_SPELLS_BOOST_2,
        TalentEffect::AttributePercent {
            attr: Attribute::Willpower,
            percent: 15,
        },
    ),
    (
        ids::IMMUN_RESIST_BOOST_2,
        TalentEffect::AttributePercent {
            attr: Attribute::Braveness,
            percent: 15,
        },
    ),
    (
        ids::BLADE_DANCE,
        TalentEffect::AttributePercent {
            attr: Attribute::Agility,
            percent: 20,
        },
    ),
    (
        ids::CONTAGION,
        TalentEffect::AttributePercent {
            attr: Attribute::Willpower,
            percent: 20,
        },
    ),
    (
        ids::STRENGTH_BOOST_1,
        TalentEffect::AttributePercent {
            attr: Attribute::Strength,
            percent: 10,
        },
    ),
    (
        ids::INTELLIGENCE_BOOST_1,
        TalentEffect::AttributePercent {
            attr: Attribute::Intuition,
            percent: 10,
        },
    ),
    (
        ids::STRENGTH_BOOST_2,
        TalentEffect::AttributePercent {
            attr: Attribute::Strength,
            percent: 10,
        },
    ),
    (
        ids::INTELLIGENCE_BOOST_2,
        TalentEffect::AttributePercent {
            attr: Attribute::Intuition,
            percent: 10,
        },
    ),
    (
        ids::ALL_SKILLS_BOOST_1,
        TalentEffect::AttributePercent {
            attr: Attribute::Braveness,
            percent: 25,
        },
    ),
];
