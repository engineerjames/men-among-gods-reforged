//! Grave auto-loot configuration shared by the client and the server.
//!
//! The client persists an [`AutolootConfig`] per character, edits it through
//! the "Autoloot Settings" panel, and uploads it to the server with a
//! `CmdAutolootConfig` packet right after login and whenever it changes.  The
//! server keeps the most recent config per connection and consults
//! [`AutolootConfig::wants_item`] when a `CmdAutoloot` request arrives for an
//! adjacent grave.

use bitflags::bitflags;
use serde::{Deserialize, Serialize};

use crate::constants::{
    AUTOLOOT_MAGICAL_WEAPON_TEMPLATE_IDS, AUTOLOOT_POTION_TEMPLATE_IDS,
    AUTOLOOT_QUEST_ITEM_TEMPLATE_IDS, AUTOLOOT_SCROLL_TEMPLATE_IDS, GREENLING_EYE_TEMPLATE_IDS,
    IT_GENERIC_SOULSTONE, IT_GHOST_KING_SOUL, ItemFlags, PL_NECK, PL_RING,
    RATLING_EYE_TEMPLATE_IDS,
};
use crate::types::Item;

bitflags! {
    /// Item categories the auto-looter is allowed to take from a grave.
    ///
    /// Serialised as a plain `u16` bitmask both on disk and on the wire.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct AutolootCategories: u16 {
        /// Gold carried by the corpse (grave slot 61).
        const GOLD = 1 << 0;
        /// Soulstones of any rank.
        const SOULSTONES = 1 << 1;
        /// Weapons that are magical, unique, soulstone-enhanced or carry stat bonuses.
        const MAGICAL_WEAPONS = 1 << 2;
        /// Armor pieces that are magical, unique, soulstone-enhanced or carry stat bonuses.
        const MAGICAL_ARMOR = 1 << 3;
        /// Potions (see [`AUTOLOOT_POTION_TEMPLATE_IDS`]).
        const POTIONS = 1 << 4;
        /// Skill, teleport and spell scrolls (see [`AUTOLOOT_SCROLL_TEMPLATE_IDS`]).
        const SCROLLS = 1 << 5;
        /// Rings and amulets.
        const JEWELRY = 1 << 6;
        /// Ratling eyes at or above [`AutolootConfig::ratling_min_rank`].
        const RATLING_EYES = 1 << 7;
        /// Greenling eyes at or above [`AutolootConfig::greenling_min_rank`].
        const GREENLING_EYES = 1 << 8;
        /// Items some quest NPC asks for (see [`AUTOLOOT_QUEST_ITEM_TEMPLATE_IDS`]).
        const QUEST_ITEMS = 1 << 9;
    }
}

impl Serialize for AutolootCategories {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u16(self.bits())
    }
}

impl<'de> Deserialize<'de> for AutolootCategories {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        u16::deserialize(deserializer).map(Self::from_bits_truncate)
    }
}

impl AutolootCategories {
    /// All categories in UI display order.
    pub const ALL: [AutolootCategories; 10] = [
        Self::GOLD,
        Self::SOULSTONES,
        Self::MAGICAL_WEAPONS,
        Self::MAGICAL_ARMOR,
        Self::POTIONS,
        Self::SCROLLS,
        Self::JEWELRY,
        Self::RATLING_EYES,
        Self::GREENLING_EYES,
        Self::QUEST_ITEMS,
    ];

    /// Human-readable label for a single category.
    ///
    /// # Returns
    ///
    /// * The display label, or `"Unknown"` for combined/unknown bit sets.
    pub const fn label(self) -> &'static str {
        match self {
            Self::GOLD => "Gold",
            Self::SOULSTONES => "Soulstones",
            Self::MAGICAL_WEAPONS => "Magical Weapons",
            Self::MAGICAL_ARMOR => "Magical Armor",
            Self::POTIONS => "Potions",
            Self::SCROLLS => "Scrolls",
            Self::JEWELRY => "Jewelry",
            Self::RATLING_EYES => "Ratling Eyes",
            Self::GREENLING_EYES => "Greenling Eyes",
            Self::QUEST_ITEMS => "Quest Items",
            _ => "Unknown",
        }
    }
}

/// Social rank of a ratling or greenling, as carried by its eye.
///
/// The discriminant doubles as the index into
/// [`RATLING_EYE_TEMPLATE_IDS`] / [`GREENLING_EYE_TEMPLATE_IDS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum EyeRank {
    /// Plain ratling / greenling.
    #[default]
    Base = 0,
    Fighter = 1,
    Warrior = 2,
    Knight = 3,
    Baron = 4,
    Count = 5,
    Duke = 6,
    Prince = 7,
    King = 8,
}

impl EyeRank {
    /// All ranks from lowest to highest.
    pub const ALL: [EyeRank; 9] = [
        Self::Base,
        Self::Fighter,
        Self::Warrior,
        Self::Knight,
        Self::Baron,
        Self::Count,
        Self::Duke,
        Self::Prince,
        Self::King,
    ];

    /// Decodes a rank from its wire byte, clamping out-of-range values to
    /// [`EyeRank::King`].
    ///
    /// # Arguments
    ///
    /// * `value` - Raw discriminant.
    ///
    /// # Returns
    ///
    /// * The matching rank.
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Base,
            1 => Self::Fighter,
            2 => Self::Warrior,
            3 => Self::Knight,
            4 => Self::Baron,
            5 => Self::Count,
            6 => Self::Duke,
            7 => Self::Prince,
            _ => Self::King,
        }
    }

    /// Rank title used after the creature name, e.g. `"Duke"`.
    ///
    /// # Returns
    ///
    /// * The title, or `""` for [`EyeRank::Base`].
    pub const fn title(self) -> &'static str {
        match self {
            Self::Base => "",
            Self::Fighter => "Fighter",
            Self::Warrior => "Warrior",
            Self::Knight => "Knight",
            Self::Baron => "Baron",
            Self::Count => "Count",
            Self::Duke => "Duke",
            Self::Prince => "Prince",
            Self::King => "King",
        }
    }

    /// Full display label for a creature at this rank, e.g. `"Ratling Duke"`.
    ///
    /// # Arguments
    ///
    /// * `creature` - Creature name (`"Ratling"` or `"Greenling"`).
    ///
    /// # Returns
    ///
    /// * The combined label.
    pub fn label(self, creature: &str) -> String {
        if self == Self::Base {
            creature.to_owned()
        } else {
            format!("{creature} {}", self.title())
        }
    }
}

/// Byte length of the `CmdAutolootConfig` payload (after the opcode).
pub const AUTOLOOT_CONFIG_WIRE_LEN: usize = 4;

/// A player's grave auto-loot preferences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutolootConfig {
    /// Enabled item categories.
    #[serde(default = "AutolootConfig::default_categories")]
    pub categories: AutolootCategories,
    /// Lowest ratling rank whose eye is taken when `RATLING_EYES` is enabled.
    #[serde(default)]
    pub ratling_min_rank: EyeRank,
    /// Lowest greenling rank whose eye is taken when `GREENLING_EYES` is enabled.
    #[serde(default)]
    pub greenling_min_rank: EyeRank,
}

impl Default for AutolootConfig {
    fn default() -> Self {
        Self {
            categories: Self::default_categories(),
            ratling_min_rank: EyeRank::Base,
            greenling_min_rank: EyeRank::Base,
        }
    }
}

impl AutolootConfig {
    /// Categories enabled for a brand-new character.
    ///
    /// # Returns
    ///
    /// * Gold, soulstones, potions and scrolls.
    pub const fn default_categories() -> AutolootCategories {
        AutolootCategories::GOLD
            .union(AutolootCategories::SOULSTONES)
            .union(AutolootCategories::POTIONS)
            .union(AutolootCategories::SCROLLS)
    }

    /// Whether `category` is enabled.
    ///
    /// # Arguments
    ///
    /// * `category` - Category bit to test.
    ///
    /// # Returns
    ///
    /// * `true` when enabled.
    pub const fn has(&self, category: AutolootCategories) -> bool {
        self.categories.contains(category)
    }

    /// Enables or disables `category`.
    ///
    /// # Arguments
    ///
    /// * `category` - Category bit to change.
    /// * `enabled` - New state.
    pub fn set(&mut self, category: AutolootCategories, enabled: bool) {
        self.categories.set(category, enabled);
    }

    /// Whether gold should be taken from a grave.
    ///
    /// # Returns
    ///
    /// * `true` when the `GOLD` category is enabled.
    pub const fn wants_gold(&self) -> bool {
        self.has(AutolootCategories::GOLD)
    }

    /// Whether an item found on a corpse should be auto-looted.
    ///
    /// Classification is instance-based where the game generates items at
    /// runtime (specialised armor gets `temp = 0` plus stat bonuses) and
    /// template-based otherwise.  Soulstones are checked first so a
    /// ring-placement soulstone is never treated as jewelry.
    ///
    /// # Arguments
    ///
    /// * `item` - Item instance sitting in the corpse's inventory or worn slots.
    ///
    /// # Returns
    ///
    /// * `true` when at least one enabled category matches.
    pub fn wants_item(&self, item: &Item) -> bool {
        let temp = usize::from(item.temp);
        let flags = ItemFlags::from_bits_truncate(item.flags);

        if temp == IT_GENERIC_SOULSTONE {
            return self.has(AutolootCategories::SOULSTONES);
        }

        if self.has(AutolootCategories::POTIONS) && AUTOLOOT_POTION_TEMPLATE_IDS.contains(&temp) {
            return true;
        }
        if self.has(AutolootCategories::SCROLLS) && AUTOLOOT_SCROLL_TEMPLATE_IDS.contains(&temp) {
            return true;
        }
        if self.has(AutolootCategories::QUEST_ITEMS)
            && AUTOLOOT_QUEST_ITEM_TEMPLATE_IDS.contains(&temp)
        {
            return true;
        }
        if self.has(AutolootCategories::RATLING_EYES)
            && eye_rank(&RATLING_EYE_TEMPLATE_IDS, temp)
                .is_some_and(|rank| rank >= self.ratling_min_rank)
        {
            return true;
        }
        if self.has(AutolootCategories::GREENLING_EYES)
            && eye_rank(&GREENLING_EYE_TEMPLATE_IDS, temp)
                .is_some_and(|rank| rank >= self.greenling_min_rank)
        {
            return true;
        }

        let special = flags
            .intersects(ItemFlags::IF_MAGIC | ItemFlags::IF_UNIQUE | ItemFlags::IF_SOULSTONE)
            || has_stat_bonus(item);

        if self.has(AutolootCategories::MAGICAL_WEAPONS)
            && flags.intersects(ItemFlags::IF_WEAPON)
            && (special || AUTOLOOT_MAGICAL_WEAPON_TEMPLATE_IDS.contains(&temp))
        {
            return true;
        }
        if self.has(AutolootCategories::MAGICAL_ARMOR)
            && flags.contains(ItemFlags::IF_ARMOR)
            && special
        {
            return true;
        }
        if self.has(AutolootCategories::JEWELRY)
            && flags.contains(ItemFlags::IF_TAKE)
            && item.placement & (PL_RING | PL_NECK) != 0
            && temp != IT_GHOST_KING_SOUL
        {
            return true;
        }

        false
    }

    /// Encodes the config as the `CmdAutolootConfig` payload.
    ///
    /// Layout: `u16 categories (LE)`, `u8 ratling_min_rank`, `u8 greenling_min_rank`.
    ///
    /// # Returns
    ///
    /// * Exactly [`AUTOLOOT_CONFIG_WIRE_LEN`] bytes.
    pub fn to_wire_bytes(&self) -> [u8; AUTOLOOT_CONFIG_WIRE_LEN] {
        let cats = self.categories.bits().to_le_bytes();
        [
            cats[0],
            cats[1],
            self.ratling_min_rank as u8,
            self.greenling_min_rank as u8,
        ]
    }

    /// Decodes a config from a `CmdAutolootConfig` payload.
    ///
    /// Unknown category bits are dropped and out-of-range ranks clamp to
    /// [`EyeRank::King`].
    ///
    /// # Arguments
    ///
    /// * `bytes` - Payload bytes following the opcode; at least
    ///   [`AUTOLOOT_CONFIG_WIRE_LEN`] long.
    ///
    /// # Returns
    ///
    /// * `Some(config)` when enough bytes are present, `None` otherwise.
    pub fn from_wire_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < AUTOLOOT_CONFIG_WIRE_LEN {
            return None;
        }
        Some(Self {
            categories: AutolootCategories::from_bits_truncate(u16::from_le_bytes([
                bytes[0], bytes[1],
            ])),
            ratling_min_rank: EyeRank::from_u8(bytes[2]),
            greenling_min_rank: EyeRank::from_u8(bytes[3]),
        })
    }
}

/// Looks up the rank of an eye template within a rank-ordered template table.
///
/// # Arguments
///
/// * `table` - Eye templates ordered by rank.
/// * `temp` - Item template id to look up.
///
/// # Returns
///
/// * `Some(rank)` when `temp` is an eye from `table`.
fn eye_rank(table: &[usize; 9], temp: usize) -> Option<EyeRank> {
    table
        .iter()
        .position(|&id| id == temp)
        .map(|idx| EyeRank::from_u8(idx as u8))
}

/// Whether an item instance modifies any attribute, skill, hp, endurance or
/// mana value — the signature of runtime-specialised "of the Bear" gear and
/// rainbow belts, which have `temp == 0`.
///
/// # Arguments
///
/// * `item` - Item instance to inspect.
///
/// # Returns
///
/// * `true` when any modifier is non-zero.
fn has_stat_bonus(item: &Item) -> bool {
    item.attrib.iter().any(|a| a[0] != 0)
        || item.skill.iter().any(|s| s[0] != 0)
        || item.hp[0] != 0
        || item.end[0] != 0
        || item.mana[0] != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{
        IT_BARBARIAN_SWORD, IT_BLACK_CANDLE, IT_GREENLING_DUKE_EYE, IT_GREENLING_KNIGHT_EYE,
        IT_HEALING_POTION, IT_RATLING_DUKE_EYE, IT_RATLING_KNIGHT_EYE, IT_SCROLL_OF_HEAL,
    };

    fn item(temp: usize, flags: ItemFlags, placement: u16) -> Item {
        Item {
            temp: temp as u16,
            flags: flags.bits(),
            placement,
            ..Item::default()
        }
    }

    fn all() -> AutolootConfig {
        AutolootConfig {
            categories: AutolootCategories::all(),
            ..AutolootConfig::default()
        }
    }

    fn only(cat: AutolootCategories) -> AutolootConfig {
        AutolootConfig {
            categories: cat,
            ..AutolootConfig::default()
        }
    }

    #[test]
    fn wire_roundtrip_preserves_all_fields() {
        let cfg = AutolootConfig {
            categories: AutolootCategories::GOLD | AutolootCategories::GREENLING_EYES,
            ratling_min_rank: EyeRank::Duke,
            greenling_min_rank: EyeRank::King,
        };
        let bytes = cfg.to_wire_bytes();
        assert_eq!(AutolootConfig::from_wire_bytes(&bytes), Some(cfg));
    }

    #[test]
    fn wire_decode_rejects_short_payload_and_clamps_rank() {
        assert!(AutolootConfig::from_wire_bytes(&[1, 0, 0]).is_none());
        let cfg = AutolootConfig::from_wire_bytes(&[0xFF, 0xFF, 42, 3]).unwrap();
        assert_eq!(cfg.categories, AutolootCategories::all());
        assert_eq!(cfg.ratling_min_rank, EyeRank::King);
        assert_eq!(cfg.greenling_min_rank, EyeRank::Knight);
    }

    #[test]
    fn default_config_takes_gold_potions_scrolls_soulstones() {
        let cfg = AutolootConfig::default();
        assert!(cfg.wants_gold());
        assert!(cfg.wants_item(&item(IT_HEALING_POTION, ItemFlags::IF_TAKE, 0)));
        assert!(cfg.wants_item(&item(IT_SCROLL_OF_HEAL, ItemFlags::IF_TAKE, 0)));
        assert!(cfg.wants_item(&item(IT_GENERIC_SOULSTONE, ItemFlags::IF_TAKE, PL_RING)));
        assert!(!cfg.wants_item(&item(IT_RATLING_DUKE_EYE, ItemFlags::IF_TAKE, 0)));
    }

    #[test]
    fn empty_config_takes_nothing() {
        let cfg = only(AutolootCategories::empty());
        assert!(!cfg.wants_gold());
        assert!(!cfg.wants_item(&item(IT_HEALING_POTION, ItemFlags::IF_TAKE, 0)));
        assert!(!cfg.wants_item(&item(
            IT_BARBARIAN_SWORD,
            ItemFlags::IF_TAKE | ItemFlags::IF_WP_SWORD,
            0
        )));
    }

    #[test]
    fn eye_rank_threshold_is_inclusive() {
        let mut cfg = only(AutolootCategories::RATLING_EYES | AutolootCategories::GREENLING_EYES);
        cfg.ratling_min_rank = EyeRank::Duke;
        cfg.greenling_min_rank = EyeRank::Knight;

        assert!(cfg.wants_item(&item(IT_RATLING_DUKE_EYE, ItemFlags::IF_TAKE, 0)));
        assert!(!cfg.wants_item(&item(IT_RATLING_KNIGHT_EYE, ItemFlags::IF_TAKE, 0)));
        assert!(cfg.wants_item(&item(IT_GREENLING_DUKE_EYE, ItemFlags::IF_TAKE, 0)));
        assert!(cfg.wants_item(&item(IT_GREENLING_KNIGHT_EYE, ItemFlags::IF_TAKE, 0)));
        assert!(!cfg.wants_item(&item(IT_GREENLING_EYE_BASE, ItemFlags::IF_TAKE, 0)));
    }

    const IT_GREENLING_EYE_BASE: usize = crate::constants::IT_GREENLING_EYE;

    #[test]
    fn magical_weapons_match_flags_bonuses_and_named_list() {
        let cfg = only(AutolootCategories::MAGICAL_WEAPONS);
        assert!(cfg.wants_item(&item(
            280,
            ItemFlags::IF_TAKE | ItemFlags::IF_WP_DAGGER | ItemFlags::IF_UNIQUE,
            0
        )));
        assert!(cfg.wants_item(&item(
            IT_BARBARIAN_SWORD,
            ItemFlags::IF_TAKE | ItemFlags::IF_WP_SWORD,
            0
        )));
        let mut bonus = item(0, ItemFlags::IF_TAKE | ItemFlags::IF_WP_SWORD, 0);
        bonus.attrib[4][0] = 4;
        assert!(cfg.wants_item(&bonus));
        assert!(!cfg.wants_item(&item(31, ItemFlags::IF_TAKE | ItemFlags::IF_WP_SWORD, 0)));
        assert!(!cfg.wants_item(&item(
            94,
            ItemFlags::IF_TAKE | ItemFlags::IF_ARMOR | ItemFlags::IF_MAGIC,
            0
        )));
    }

    #[test]
    fn magical_armor_requires_special_marker() {
        let cfg = only(AutolootCategories::MAGICAL_ARMOR);
        assert!(!cfg.wants_item(&item(59, ItemFlags::IF_TAKE | ItemFlags::IF_ARMOR, 4)));
        let mut bear = item(0, ItemFlags::IF_TAKE | ItemFlags::IF_ARMOR, 4);
        bear.hp[0] = 10;
        assert!(cfg.wants_item(&bear));
        assert!(cfg.wants_item(&item(
            59,
            ItemFlags::IF_TAKE | ItemFlags::IF_ARMOR | ItemFlags::IF_SOULSTONE,
            4
        )));
    }

    #[test]
    fn jewelry_uses_placement_but_never_takes_soulstones() {
        let cfg = only(AutolootCategories::JEWELRY);
        assert!(cfg.wants_item(&item(337, ItemFlags::IF_TAKE, PL_RING)));
        assert!(cfg.wants_item(&item(105, ItemFlags::IF_TAKE, PL_NECK)));
        assert!(!cfg.wants_item(&item(IT_GENERIC_SOULSTONE, ItemFlags::IF_TAKE, PL_RING)));
        assert!(!cfg.wants_item(&item(IT_GHOST_KING_SOUL, ItemFlags::IF_TAKE, PL_RING)));
    }

    #[test]
    fn quest_items_match_template_list() {
        let cfg = only(AutolootCategories::QUEST_ITEMS);
        assert!(cfg.wants_item(&item(IT_BLACK_CANDLE, ItemFlags::IF_TAKE, 0)));
        assert!(!cfg.wants_item(&item(IT_HEALING_POTION, ItemFlags::IF_TAKE, 0)));
        assert!(all().wants_item(&item(IT_HEALING_POTION, ItemFlags::IF_TAKE, 0)));
    }

    #[test]
    fn eye_rank_labels_and_ordering() {
        assert_eq!(EyeRank::Base.label("Ratling"), "Ratling");
        assert_eq!(EyeRank::Duke.label("Greenling"), "Greenling Duke");
        assert!(EyeRank::King > EyeRank::Prince);
        assert_eq!(EyeRank::from_u8(EyeRank::Count as u8), EyeRank::Count);
        for (idx, rank) in EyeRank::ALL.iter().enumerate() {
            assert_eq!(*rank as usize, idx);
        }
    }

    #[test]
    fn category_labels_are_unique() {
        let labels: Vec<_> = AutolootCategories::ALL.iter().map(|c| c.label()).collect();
        for (i, a) in labels.iter().enumerate() {
            assert_ne!(*a, "Unknown");
            assert!(!labels[i + 1..].contains(a), "duplicate label {a}");
        }
    }

    #[test]
    fn serde_roundtrip_and_defaults() {
        let cfg = AutolootConfig {
            categories: AutolootCategories::JEWELRY,
            ratling_min_rank: EyeRank::Prince,
            greenling_min_rank: EyeRank::Base,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        assert_eq!(serde_json::from_str::<AutolootConfig>(&json).unwrap(), cfg);
        let partial: AutolootConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(partial, AutolootConfig::default());
    }
}
