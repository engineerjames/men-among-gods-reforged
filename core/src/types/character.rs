//! Character structure - represents both players and NPCs

use crate::{
    constants::{CharacterFlags, USE_EMPTY},
    string_operations::c_string_to_str,
};
use bincode::{Decode, Encode};

/// Character structure - represents both players and NPCs
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Character {
    pub used: u8, // 1

    // general
    pub name: [u8; 40],         // 41
    pub reference: [u8; 40],    // 81
    pub description: [u8; 200], // 281

    pub kindred: i32, // 285

    pub player: i32, // 289
    pub pass1: u32,
    pub pass2: u32, // 297

    pub sprite: u16, // 299, sprite base value, 1024 dist
    pub sound: u16,  // 301, sound base value, 64 dist

    pub flags: u64, // 309

    pub alignment: i16, // 311

    pub temple_x: u16, // 313, position of temple for recall and dying
    pub temple_y: u16, // 315

    pub tavern_x: u16, // 317, position of last temple for re-login
    pub tavern_y: u16, // 319

    pub temp: u16, // 321, created from template n

    // character stats
    // [0]=bare value, 0=unknown
    // [1]=preset modifier, is race/npc dependend
    // [2]=race specific maximum
    // [3]=race specific difficulty to raise (0=not raisable, 1=easy ... 10=hard)
    // [4]=dynamic modifier, depends on equipment and spells (this one is currently not used)
    // [5]=total value
    pub attrib: [[u8; 6]; 5], // 351

    pub hp: [u16; 6],   // 363
    pub end: [u16; 6],  // 375
    pub mana: [u16; 6], // 387

    pub skill: [[u8; 6]; 50], // 687

    pub weapon_bonus: u8,
    pub armor_bonus: u8,

    // temporary attributes
    pub a_hp: i32,
    pub a_end: i32,
    pub a_mana: i32,

    pub light: u8, // strength of lightsource
    pub mode: u8,  // 0 = slow, 1 = medium, 2 = fast
    pub speed: i16,

    pub points: i32,
    pub points_tot: i32,

    // summary of weapons + armor
    pub armor: i16,
    pub weapon: i16,

    // map stuff
    pub x: i16,
    pub y: i16, // current position x,y NOTE: x=-1, y=-1 = void
    pub tox: i16,
    pub toy: i16, // target coordinated, where the char will be next turn
    pub frx: i16,
    pub fry: i16,     // where the char was last turn
    pub status: i16,  // what the character is doing, animation-wise
    pub status2: i16, // for plr_misc(): what is misc?
    pub dir: u8,      // direction character is facing

    // posessions
    pub gold: i32,

    // items carried
    pub item: [u32; 40],

    // items worn
    pub worn: [u32; 20],

    // spells active on character
    pub spell: [u32; 20],

    // item currently in hand (mouse cursor)
    pub citem: u32,

    // In reality this should be time_t
    pub creation_date: u32,

    // In reality this should be time_t
    pub login_date: u32,

    pub addr: u32,

    // misc
    pub current_online_time: u32,
    pub total_online_time: u32,
    pub comp_volume: u32,
    pub raw_volume: u32,
    pub idle: u32,

    // generic driver data
    pub attack_cn: u16,     // target for attacks, will attack if set (prio 4)
    pub skill_nr: u16,      // skill to use/spell to cast, will cast if set (prio 2)
    pub skill_target1: u16, // target for skills/spells
    pub skill_target2: u16, // target for skills/spells
    pub goto_x: u16,        // will goto x,y if set (prio 3)
    pub goto_y: u16,
    pub use_nr: u16, // will use worn item nr if set (prio 1)

    pub misc_action: u16,  // drop, pickup, use, whatever (prio 5)
    pub misc_target1: u16, // item for misc_action
    pub misc_target2: u16, // location for misc_action

    pub cerrno: u16, // error/success indicator for last action (svr_act level)

    pub escape_timer: u16,  // can try again to escape in X ticks
    pub enemy: [u16; 4],    // currently being fought against by these
    pub current_enemy: u16, // currently fighting against X

    pub retry: u16, // retry current action X times

    pub stunned: u16, // is stunned for X ticks

    // misc stuff added later:
    pub speed_mod: i8,   // race dependand speed modification
    pub last_action: i8, // last action was success/failure (driver_generic level)
    pub unused: i8,
    pub depot_sold: i8, // items from depot where sold to pay for the rent

    pub gethit_dam: i8,   // damage for attacker when hitting this char
    pub gethit_bonus: i8, // race specific bonus for above

    pub light_bonus: u8, // char emits light all the time

    pub passwd: [u8; 16],

    pub lastattack: i8,    // neater display: remembers the last attack animation
    pub future1: [i8; 25], // space for future expansion

    pub sprite_override: i16,

    pub future2: [i16; 49],

    pub depot: [u32; 62],

    pub depot_cost: i32,

    pub luck: i32,

    pub unreach: i32,
    pub unreachx: i32,
    pub unreachy: i32,

    pub monster_class: i32, // monster class

    pub future3: [i32; 12],

    // In reality this should be time_t
    pub logout_date: u32,

    // driver data
    pub data: [i32; 100],
    pub text: [[u8; 160]; 10],
}

impl Default for Character {
    fn default() -> Self {
        Self {
            used: 0,
            name: [0; 40],
            reference: [0; 40],
            description: [0; 200],
            kindred: 0,
            player: 0,
            pass1: 0,
            pass2: 0,
            sprite: 0,
            sound: 0,
            flags: 0,
            alignment: 0,
            temple_x: 0,
            temple_y: 0,
            tavern_x: 0,
            tavern_y: 0,
            temp: 0,
            attrib: [[0; 6]; 5],
            hp: [0; 6],
            end: [0; 6],
            mana: [0; 6],
            skill: [[0; 6]; 50],
            weapon_bonus: 0,
            armor_bonus: 0,
            a_hp: 0,
            a_end: 0,
            a_mana: 0,
            light: 0,
            mode: 0,
            speed: 0,
            points: 0,
            points_tot: 0,
            armor: 0,
            weapon: 0,
            x: 0,
            y: 0,
            tox: 0,
            toy: 0,
            frx: 0,
            fry: 0,
            status: 0,
            status2: 0,
            dir: 0,
            gold: 0,
            item: [0; 40],
            worn: [0; 20],
            spell: [0; 20],
            citem: 0,
            creation_date: 0,
            login_date: 0,
            addr: 0,
            current_online_time: 0,
            total_online_time: 0,
            comp_volume: 0,
            raw_volume: 0,
            idle: 0,
            attack_cn: 0,
            skill_nr: 0,
            skill_target1: 0,
            skill_target2: 0,
            goto_x: 0,
            goto_y: 0,
            use_nr: 0,
            misc_action: 0,
            misc_target1: 0,
            misc_target2: 0,
            cerrno: 0,
            escape_timer: 0,
            enemy: [0; 4],
            current_enemy: 0,
            retry: 0,
            stunned: 0,
            speed_mod: 0,
            last_action: 0,
            unused: 0,
            depot_sold: 0,
            gethit_dam: 0,
            gethit_bonus: 0,
            light_bonus: 0,
            passwd: [0; 16],
            lastattack: 0,
            future1: [0; 25],
            sprite_override: 0,
            future2: [0; 49],
            depot: [0; 62],
            depot_cost: 0,
            luck: 0,
            unreach: 0,
            unreachx: 0,
            unreachy: 0,
            monster_class: 0,
            future3: [0; 12],
            logout_date: 0,
            data: [0; 100],
            text: [[0; 160]; 10],
        }
    }
}

impl Character {
    /// Get name as a string slice
    pub fn get_name(&self) -> &str {
        c_string_to_str(&self.name)
    }

    /// Check if character is a player
    pub fn is_player(&self) -> bool {
        (self.flags & CharacterFlags::Player.bits()) != 0
    }

    /// Check if character has profile flag set
    pub fn has_prof(&self) -> bool {
        (self.flags & CharacterFlags::Profile.bits()) != 0
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::encode_to_vec(self, bincode::config::standard())
            .expect("Character::to_bytes failed")
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let (value, consumed): (Self, usize) =
            bincode::decode_from_slice(bytes, bincode::config::standard()).ok()?;
        if consumed == bytes.len() {
            Some(value)
        } else {
            None
        }
    }

    pub fn is_close_to_temple(&self) -> bool {
        let dx = (self.x as i32 - self.temple_x as i32).abs();
        let dy = (self.y as i32 - self.temple_y as i32).abs();
        (dx + dy) <= 10
    }

    pub fn in_no_lag_scroll_area(
        &self,
        map_tiles: &[crate::types::Map;
             crate::constants::SERVER_MAPX as usize
                 * crate::constants::SERVER_MAPY as usize],
    ) -> bool {
        let map_index =
            (self.y as usize) * (crate::constants::SERVER_MAPX as usize) + (self.x as usize);

        map_tiles[map_index].flags & crate::constants::MF_NOLAG as u64 != 0
    }

    pub fn is_sane_character(char_id: usize) -> bool {
        char_id > 0 && char_id < crate::constants::MAXCHARS
    }

    pub fn is_living_character(&self, char_id: usize) -> bool {
        Self::is_sane_character(char_id) && self.used != crate::constants::USE_EMPTY
    }

    pub fn get_next_inventory_slot(&self) -> Option<usize> {
        let inventory = self.item;
        for (i, &item_id) in inventory.iter().enumerate() {
            if item_id == USE_EMPTY as u32 {
                return Some(i);
            }
        }
        None
    }

    pub fn set_do_update_flags(&mut self) {
        self.flags |= CharacterFlags::Update.bits() | CharacterFlags::SaveMe.bits();
    }

    pub fn is_monster(&self) -> bool {
        (self.kindred as u32 & crate::constants::KIN_MONSTER) != 0
    }

    pub fn is_usurp_or_thrall(&self) -> bool {
        (self.flags & (CharacterFlags::Usurp.bits() | CharacterFlags::Thrall.bits())) != 0
    }

    pub fn is_building(&self) -> bool {
        (self.flags & CharacterFlags::BuildMode.bits()) != 0
    }

    pub fn get_kindred_as_string(&self) -> String {
        let kindred = self.kindred as u32;
        if kindred & crate::constants::KIN_TEMPLAR != 0 {
            "Templar".to_string()
        } else if kindred & crate::constants::KIN_HARAKIM != 0 {
            "Harakim".to_string()
        } else if kindred & crate::constants::KIN_MERCENARY != 0 {
            "Monster".to_string()
        } else if kindred & crate::constants::KIN_SEYAN_DU != 0 {
            "Seyan'Du".to_string()
        } else {
            "Monster".to_string()
        }
    }

    pub fn get_gender_as_string(&self) -> String {
        let kindred = self.kindred as u32;
        if kindred & crate::constants::KIN_FEMALE != 0 {
            "Female".to_string()
        } else if kindred & crate::constants::KIN_MALE != 0 {
            "Male".to_string()
        } else {
            "It".to_string()
        }
    }

    pub fn get_default_description(&self) -> String {
        format!(
            "{} is a {}. {} looks somewhat nondescript.",
            self.get_name(),
            self.get_kindred_as_string(),
            self.get_gender_as_string()
        )
    }

    pub fn get_reference(&self) -> &str {
        c_string_to_str(&self.reference)
    }

    pub fn is_sane_npc(character_id: usize, character: &Character) -> bool {
        character_id > 0 && character_id < crate::constants::MAXCHARS && !character.is_player()
    }

    pub fn get_invisibility_level(&self) -> i32 {
        if self.flags & CharacterFlags::GreaterInv.bits() != 0 {
            return 15;
        }

        if self.flags & CharacterFlags::God.bits() != 0 {
            return 10;
        }

        if self.flags & (CharacterFlags::Imp | CharacterFlags::Usurp).bits() != 0 {
            return 5;
        }

        if self.flags & CharacterFlags::Staff.bits() != 0 {
            return 2;
        }

        1
    }

    pub fn set_name(&mut self, new_name: &str) {
        let bytes = new_name.as_bytes();
        let limit = if bytes.len() < self.name.len() {
            bytes.len()
        } else {
            log::warn!(
                "Truncating character name '{}' to fit in {} bytes",
                new_name,
                self.name.len()
            );
            self.name.len()
        };
        self.name[..limit].copy_from_slice(&bytes[..limit]);
    }

    pub fn set_reference(&mut self, new_reference: &str) {
        let bytes = new_reference.as_bytes();
        let limit = if bytes.len() < self.reference.len() {
            bytes.len()
        } else {
            log::warn!(
                "Truncating character reference '{}' to fit in {} bytes",
                new_reference,
                self.reference.len()
            );
            self.reference.len()
        };
        self.reference[..limit].copy_from_slice(&bytes[..limit]);
    }

    pub fn set_description(&mut self, new_description: &str) {
        let bytes = new_description.as_bytes();
        let limit = if bytes.len() < self.description.len() {
            bytes.len()
        } else {
            log::warn!(
                "Truncating character description '{}' to fit in {} bytes",
                new_description,
                self.description.len()
            );
            self.description.len()
        };
        self.description[..limit].copy_from_slice(&bytes[..limit]);
    }

    pub fn group_active(&self) -> bool {
        if (self.flags
            & (CharacterFlags::Player | CharacterFlags::Usurp | CharacterFlags::NoSleep).bits())
            != 0
            && self.used == crate::constants::USE_ACTIVE
        {
            return true;
        }

        if self.data[92] != 0 {
            return true;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_character_to_bytes_size() {
        let character = Character::default();
        let bytes = character.to_bytes();
        assert!(
            !bytes.is_empty(),
            "Serialized Character should not be empty"
        );
    }

    #[test]
    fn test_character_roundtrip() {
        let mut original = Character::default();
        original.used = 1;
        original.name =
            *b"TestHero\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";
        original.reference = *b"a brave warrior\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";
        original.kindred = 5;
        original.player = 1;
        original.sprite = 1000;
        original.sound = 500;
        original.flags = 0x123456789ABCDEF0;
        original.alignment = 100;
        original.temple_x = 50;
        original.temple_y = 60;
        original.hp = [100, 120, 140, 10, 0, 100];
        original.attrib[0] = [10, 5, 50, 3, 2, 17];
        original.skill[0] = [20, 10, 100, 5, 3, 33];
        original.x = 100;
        original.y = 200;
        original.gold = 5000;
        original.item[0] = 123;
        original.worn[0] = 456;
        original.depot[0] = 789;
        original.data[0] = 999;

        let bytes = original.to_bytes();
        let deserialized = Character::from_bytes(&bytes).expect("Failed to deserialize Character");
        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_character_from_bytes_insufficient_data() {
        let mut bytes = Character::default().to_bytes();
        bytes.pop();
        assert!(
            Character::from_bytes(&bytes).is_none(),
            "Should fail with insufficient data"
        );
    }

    #[test]
    fn test_character_get_name() {
        let mut character = Character::default();
        character.name =
            *b"Hero123\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";
        assert_eq!(character.get_name(), "Hero123");
    }

    #[test]
    fn test_character_is_player() {
        let mut character = Character::default();
        assert!(!character.is_player());

        character.flags = CharacterFlags::Player.bits();
        assert!(character.is_player());
    }

    #[test]
    fn test_character_has_prof() {
        let mut character = Character::default();
        assert!(!character.has_prof());

        character.flags = CharacterFlags::Profile.bits();
        assert!(character.has_prof());
    }

    #[test]
    fn test_character_set_name() {
        let mut character = Character::default();
        character.set_name("NewHero");
        assert_eq!(character.get_name(), "NewHero");

        // Test that very long names get truncated
        let long_name = "ThisIsAVeryLongNameThatExceedsTheMaximumAllowedLength";
        character.set_name(long_name);
        // Name fills the entire 40-byte buffer
        assert_eq!(character.get_name().len(), 40);
        // But it should start with the beginning of the long name
        assert!(character
            .get_name()
            .starts_with("ThisIsAVeryLongNameThatExceedsTheMaximu"));
    }

    #[test]
    fn test_is_close_to_temple() {
        let mut character = Character::default();
        character.temple_x = 100;
        character.temple_y = 100;

        // At temple
        character.x = 100;
        character.y = 100;
        assert!(character.is_close_to_temple());

        // Within distance
        character.x = 105;
        character.y = 105;
        assert!(character.is_close_to_temple());

        // At edge of distance (10 tiles total manhattan distance)
        character.x = 110;
        character.y = 100;
        assert!(character.is_close_to_temple());

        // Just outside distance
        character.x = 111;
        character.y = 100;
        assert!(!character.is_close_to_temple());

        // Far away
        character.x = 200;
        character.y = 200;
        assert!(!character.is_close_to_temple());
    }

    #[test]
    fn test_is_sane_character() {
        assert!(!Character::is_sane_character(0));
        assert!(Character::is_sane_character(1));
        assert!(Character::is_sane_character(100));
        assert!(Character::is_sane_character(crate::constants::MAXCHARS - 1));
        assert!(!Character::is_sane_character(crate::constants::MAXCHARS));
        assert!(!Character::is_sane_character(
            crate::constants::MAXCHARS + 1
        ));
    }

    #[test]
    fn test_is_living_character() {
        let mut character = Character::default();
        character.used = USE_EMPTY;

        // Dead character
        assert!(!character.is_living_character(1));

        // Living character
        character.used = 1;
        assert!(character.is_living_character(1));

        // Invalid character ID
        assert!(!character.is_living_character(0));
        assert!(!character.is_living_character(crate::constants::MAXCHARS));
    }

    #[test]
    fn test_get_next_inventory_slot() {
        let mut character = Character::default();

        // Empty inventory
        for i in 0..40 {
            character.item[i] = USE_EMPTY as u32;
        }
        assert_eq!(character.get_next_inventory_slot(), Some(0));

        // First slot taken
        character.item[0] = 123;
        assert_eq!(character.get_next_inventory_slot(), Some(1));

        // Multiple slots taken
        character.item[1] = 456;
        character.item[2] = 789;
        assert_eq!(character.get_next_inventory_slot(), Some(3));

        // Full inventory
        for i in 0..40 {
            character.item[i] = i as u32 + 1;
        }
        assert_eq!(character.get_next_inventory_slot(), None);
    }

    #[test]
    fn test_set_do_update_flags() {
        let mut character = Character::default();
        character.flags = 0;

        character.set_do_update_flags();

        assert_ne!(character.flags & CharacterFlags::Update.bits(), 0);
        assert_ne!(character.flags & CharacterFlags::SaveMe.bits(), 0);
    }

    #[test]
    fn test_is_monster() {
        let mut character = Character::default();
        character.kindred = 0;
        assert!(!character.is_monster());

        character.kindred = crate::constants::KIN_MONSTER as i32;
        assert!(character.is_monster());
    }

    #[test]
    fn test_is_usurp_or_thrall() {
        let mut character = Character::default();
        character.flags = 0;
        assert!(!character.is_usurp_or_thrall());

        character.flags = CharacterFlags::Usurp.bits();
        assert!(character.is_usurp_or_thrall());

        character.flags = CharacterFlags::Thrall.bits();
        assert!(character.is_usurp_or_thrall());

        character.flags = CharacterFlags::Usurp.bits() | CharacterFlags::Thrall.bits();
        assert!(character.is_usurp_or_thrall());
    }

    #[test]
    fn test_is_building() {
        let mut character = Character::default();
        character.flags = 0;
        assert!(!character.is_building());

        character.flags = CharacterFlags::BuildMode.bits();
        assert!(character.is_building());
    }

    #[test]
    fn test_get_kindred_as_string() {
        let mut character = Character::default();

        character.kindred = crate::constants::KIN_TEMPLAR as i32;
        assert_eq!(character.get_kindred_as_string(), "Templar");

        character.kindred = crate::constants::KIN_HARAKIM as i32;
        assert_eq!(character.get_kindred_as_string(), "Harakim");

        character.kindred = crate::constants::KIN_MERCENARY as i32;
        assert_eq!(character.get_kindred_as_string(), "Monster");

        character.kindred = crate::constants::KIN_SEYAN_DU as i32;
        assert_eq!(character.get_kindred_as_string(), "Seyan'Du");

        character.kindred = 0;
        assert_eq!(character.get_kindred_as_string(), "Monster");
    }

    #[test]
    fn test_get_gender_as_string() {
        let mut character = Character::default();

        character.kindred = crate::constants::KIN_FEMALE as i32;
        assert_eq!(character.get_gender_as_string(), "Female");

        character.kindred = crate::constants::KIN_MALE as i32;
        assert_eq!(character.get_gender_as_string(), "Male");

        character.kindred = 0;
        assert_eq!(character.get_gender_as_string(), "It");
    }

    #[test]
    fn test_get_default_description() {
        let mut character = Character::default();
        character.set_name("TestHero");
        character.kindred =
            crate::constants::KIN_TEMPLAR as i32 | crate::constants::KIN_MALE as i32;

        let desc = character.get_default_description();
        assert!(desc.contains("TestHero"));
        assert!(desc.contains("Templar"));
        assert!(desc.contains("Male"));
    }

    #[test]
    fn test_get_reference() {
        let mut character = Character::default();
        character.reference = *b"a brave warrior\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";
        assert_eq!(character.get_reference(), "a brave warrior");
    }

    #[test]
    fn test_is_sane_npc() {
        let mut character = Character::default();
        character.flags = 0; // Not a player, so it's an NPC

        assert!(Character::is_sane_npc(1, &character));
        assert!(!Character::is_sane_npc(0, &character));
        assert!(!Character::is_sane_npc(
            crate::constants::MAXCHARS,
            &character
        ));

        // Player characters should not be sane NPCs
        character.flags = CharacterFlags::Player.bits();
        assert!(!Character::is_sane_npc(1, &character));
    }

    #[test]
    fn test_get_invisibility_level() {
        let mut character = Character::default();
        character.flags = 0;
        assert_eq!(character.get_invisibility_level(), 1);

        character.flags = CharacterFlags::Staff.bits();
        assert_eq!(character.get_invisibility_level(), 2);

        character.flags = CharacterFlags::Imp.bits();
        assert_eq!(character.get_invisibility_level(), 5);

        character.flags = CharacterFlags::Usurp.bits();
        assert_eq!(character.get_invisibility_level(), 5);

        character.flags = CharacterFlags::God.bits();
        assert_eq!(character.get_invisibility_level(), 10);

        character.flags = CharacterFlags::GreaterInv.bits();
        assert_eq!(character.get_invisibility_level(), 15);
    }

    #[test]
    fn test_set_reference() {
        let mut character = Character::default();
        character.set_reference("a skilled mage");
        assert_eq!(character.get_reference(), "a skilled mage");

        // Test truncation
        let long_ref =
            "a very long reference that exceeds the maximum allowed length for the reference field";
        character.set_reference(long_ref);
        assert_eq!(character.get_reference().len(), 40);
    }
}
