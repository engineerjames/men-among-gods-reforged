//! Character structure - represents both players and NPCs

use crate::{constants::CharacterFlags, string_operations::c_string_to_str};

/// Character structure - represents both players and NPCs
#[derive(Clone, Copy)]
#[repr(C, packed)]
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
        (self.flags & CharacterFlags::CF_PLAYER.bits()) != 0
    }

    /// Check if character has profile flag set
    pub fn has_prof(&self) -> bool {
        (self.flags & CharacterFlags::CF_PROF.bits()) != 0
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(std::mem::size_of::<Character>());

        bytes.extend_from_slice(&self.used.to_le_bytes());
        bytes.extend_from_slice(&self.name);
        bytes.extend_from_slice(&self.reference);
        bytes.extend_from_slice(&self.description);
        bytes.extend_from_slice(&self.kindred.to_le_bytes());
        bytes.extend_from_slice(&self.player.to_le_bytes());
        bytes.extend_from_slice(&self.pass1.to_le_bytes());
        bytes.extend_from_slice(&self.pass2.to_le_bytes());
        bytes.extend_from_slice(&self.sprite.to_le_bytes());
        bytes.extend_from_slice(&self.sound.to_le_bytes());
        bytes.extend_from_slice(&self.flags.to_le_bytes());
        bytes.extend_from_slice(&self.alignment.to_le_bytes());
        bytes.extend_from_slice(&self.temple_x.to_le_bytes());
        bytes.extend_from_slice(&self.temple_y.to_le_bytes());
        bytes.extend_from_slice(&self.tavern_x.to_le_bytes());
        bytes.extend_from_slice(&self.tavern_y.to_le_bytes());
        bytes.extend_from_slice(&self.temp.to_le_bytes());

        for attrib in &self.attrib {
            for &value in attrib {
                bytes.push(value);
            }
        }

        let hp_copy = self.hp;
        for &value in &hp_copy {
            bytes.extend_from_slice(&value.to_le_bytes());
        }

        let end_copy = self.end;
        for &value in &end_copy {
            bytes.extend_from_slice(&value.to_le_bytes());
        }

        let mana_copy = self.mana;
        for &value in &mana_copy {
            bytes.extend_from_slice(&value.to_le_bytes());
        }

        for skill in &self.skill {
            for &value in skill {
                bytes.push(value);
            }
        }

        bytes.push(self.weapon_bonus);
        bytes.push(self.armor_bonus);
        bytes.extend_from_slice(&self.a_hp.to_le_bytes());
        bytes.extend_from_slice(&self.a_end.to_le_bytes());
        bytes.extend_from_slice(&self.a_mana.to_le_bytes());
        bytes.push(self.light);
        bytes.push(self.mode);
        bytes.extend_from_slice(&self.speed.to_le_bytes());
        bytes.extend_from_slice(&self.points.to_le_bytes());
        bytes.extend_from_slice(&self.points_tot.to_le_bytes());
        bytes.extend_from_slice(&self.armor.to_le_bytes());
        bytes.extend_from_slice(&self.weapon.to_le_bytes());
        bytes.extend_from_slice(&self.x.to_le_bytes());
        bytes.extend_from_slice(&self.y.to_le_bytes());
        bytes.extend_from_slice(&self.tox.to_le_bytes());
        bytes.extend_from_slice(&self.toy.to_le_bytes());
        bytes.extend_from_slice(&self.frx.to_le_bytes());
        bytes.extend_from_slice(&self.fry.to_le_bytes());
        bytes.extend_from_slice(&self.status.to_le_bytes());
        bytes.extend_from_slice(&self.status2.to_le_bytes());
        bytes.push(self.dir);
        bytes.extend_from_slice(&self.gold.to_le_bytes());

        let item_copy = self.item;
        for &item_id in &item_copy {
            bytes.extend_from_slice(&item_id.to_le_bytes());
        }

        let worn_copy = self.worn;
        for &worn_id in &worn_copy {
            bytes.extend_from_slice(&worn_id.to_le_bytes());
        }

        let spell_copy = self.spell;
        for &spell_id in &spell_copy {
            bytes.extend_from_slice(&spell_id.to_le_bytes());
        }
        bytes.extend_from_slice(&self.citem.to_le_bytes());
        bytes.extend_from_slice(&self.creation_date.to_le_bytes());
        bytes.extend_from_slice(&self.login_date.to_le_bytes());
        bytes.extend_from_slice(&self.addr.to_le_bytes());
        bytes.extend_from_slice(&self.current_online_time.to_le_bytes());
        bytes.extend_from_slice(&self.total_online_time.to_le_bytes());
        bytes.extend_from_slice(&self.comp_volume.to_le_bytes());
        bytes.extend_from_slice(&self.raw_volume.to_le_bytes());
        bytes.extend_from_slice(&self.idle.to_le_bytes());
        bytes.extend_from_slice(&self.attack_cn.to_le_bytes());
        bytes.extend_from_slice(&self.skill_nr.to_le_bytes());
        bytes.extend_from_slice(&self.skill_target1.to_le_bytes());
        bytes.extend_from_slice(&self.skill_target2.to_le_bytes());
        bytes.extend_from_slice(&self.goto_x.to_le_bytes());
        bytes.extend_from_slice(&self.goto_y.to_le_bytes());
        bytes.extend_from_slice(&self.use_nr.to_le_bytes());
        bytes.extend_from_slice(&self.misc_action.to_le_bytes());
        bytes.extend_from_slice(&self.misc_target1.to_le_bytes());
        bytes.extend_from_slice(&self.misc_target2.to_le_bytes());
        bytes.extend_from_slice(&self.cerrno.to_le_bytes());
        bytes.extend_from_slice(&self.escape_timer.to_le_bytes());

        let enemy_copy = self.enemy;
        for &enemy_id in &enemy_copy {
            bytes.extend_from_slice(&enemy_id.to_le_bytes());
        }
        bytes.extend_from_slice(&self.current_enemy.to_le_bytes());
        bytes.extend_from_slice(&self.retry.to_le_bytes());
        bytes.extend_from_slice(&self.stunned.to_le_bytes());
        bytes.push(self.speed_mod as u8);
        bytes.push(self.last_action as u8);
        bytes.push(self.unused as u8);
        bytes.push(self.depot_sold as u8);
        bytes.push(self.gethit_dam as u8);
        bytes.push(self.gethit_bonus as u8);
        bytes.push(self.light_bonus);
        bytes.extend_from_slice(&self.passwd);
        bytes.push(self.lastattack as u8);
        for &value in &self.future1 {
            bytes.push(value as u8);
        }
        bytes.extend_from_slice(&self.sprite_override.to_le_bytes());

        let future2_copy = self.future2;
        for &value in &future2_copy {
            bytes.extend_from_slice(&value.to_le_bytes());
        }

        let depot_copy = self.depot;
        for &item_id in &depot_copy {
            bytes.extend_from_slice(&item_id.to_le_bytes());
        }
        bytes.extend_from_slice(&self.depot_cost.to_le_bytes());
        bytes.extend_from_slice(&self.luck.to_le_bytes());
        bytes.extend_from_slice(&self.unreach.to_le_bytes());
        bytes.extend_from_slice(&self.unreachx.to_le_bytes());
        bytes.extend_from_slice(&self.unreachy.to_le_bytes());
        bytes.extend_from_slice(&self.monster_class.to_le_bytes());

        let future3_copy = self.future3;
        for &value in &future3_copy {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&self.logout_date.to_le_bytes());

        let data_copy = self.data;
        for &value in &data_copy {
            bytes.extend_from_slice(&value.to_le_bytes());
        }

        for text_entry in &self.text {
            for &char_byte in text_entry {
                bytes.push(char_byte);
            }
        }

        if bytes.len() != std::mem::size_of::<Character>() {
            log::warn!(
                "Character::to_bytes: expected size {}, got {}",
                std::mem::size_of::<Character>(),
                bytes.len()
            );
        }

        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < std::mem::size_of::<Character>() {
            return None;
        }

        let mut offset: usize = 0;

        Some(Self {
            used: read_u8!(bytes, offset),
            name: {
                let mut arr = [0u8; 40];
                for i in 0..40 {
                    arr[i] = read_u8!(bytes, offset);
                }
                arr
            },
            reference: {
                let mut arr = [0u8; 40];
                for i in 0..40 {
                    arr[i] = read_u8!(bytes, offset);
                }
                arr
            },
            description: {
                let mut arr = [0u8; 200];
                for i in 0..200 {
                    arr[i] = read_u8!(bytes, offset);
                }
                arr
            },
            kindred: read_i32!(bytes, offset),
            player: read_i32!(bytes, offset),
            pass1: read_u32!(bytes, offset),
            pass2: read_u32!(bytes, offset),
            sprite: read_u16!(bytes, offset),
            sound: read_u16!(bytes, offset),
            flags: read_u64!(bytes, offset),
            alignment: read_i16!(bytes, offset),
            temple_x: read_u16!(bytes, offset),
            temple_y: read_u16!(bytes, offset),
            tavern_x: read_u16!(bytes, offset),
            tavern_y: read_u16!(bytes, offset),
            temp: read_u16!(bytes, offset),
            attrib: {
                let mut arr = [[0u8; 6]; 5];
                for i in 0..5 {
                    for j in 0..6 {
                        arr[i][j] = read_u8!(bytes, offset);
                    }
                }
                arr
            },
            hp: {
                let mut arr = [0u16; 6];
                for i in 0..6 {
                    arr[i] = read_u16!(bytes, offset);
                }
                arr
            },
            end: {
                let mut arr = [0u16; 6];
                for i in 0..6 {
                    arr[i] = read_u16!(bytes, offset);
                }
                arr
            },
            mana: {
                let mut arr = [0u16; 6];
                for i in 0..6 {
                    arr[i] = read_u16!(bytes, offset);
                }
                arr
            },
            skill: {
                let mut arr = [[0u8; 6]; 50];
                for i in 0..50 {
                    for j in 0..6 {
                        arr[i][j] = read_u8!(bytes, offset);
                    }
                }
                arr
            },
            weapon_bonus: read_u8!(bytes, offset),
            armor_bonus: read_u8!(bytes, offset),
            a_hp: read_i32!(bytes, offset),
            a_end: read_i32!(bytes, offset),
            a_mana: read_i32!(bytes, offset),
            light: read_u8!(bytes, offset),
            mode: read_u8!(bytes, offset),
            speed: read_i16!(bytes, offset),
            points: read_i32!(bytes, offset),
            points_tot: read_i32!(bytes, offset),
            armor: read_i16!(bytes, offset),
            weapon: read_i16!(bytes, offset),
            x: read_i16!(bytes, offset),
            y: read_i16!(bytes, offset),
            tox: read_i16!(bytes, offset),
            toy: read_i16!(bytes, offset),
            frx: read_i16!(bytes, offset),
            fry: read_i16!(bytes, offset),
            status: read_i16!(bytes, offset),
            status2: read_i16!(bytes, offset),
            dir: read_u8!(bytes, offset),
            gold: read_i32!(bytes, offset),
            item: {
                let mut arr = [0u32; 40];
                for i in 0..40 {
                    arr[i] = read_u32!(bytes, offset);
                }
                arr
            },
            worn: {
                let mut arr = [0u32; 20];
                for i in 0..20 {
                    arr[i] = read_u32!(bytes, offset);
                }
                arr
            },
            spell: {
                let mut arr = [0u32; 20];
                for i in 0..20 {
                    arr[i] = read_u32!(bytes, offset);
                }
                arr
            },
            citem: read_u32!(bytes, offset),
            creation_date: read_u32!(bytes, offset),
            login_date: read_u32!(bytes, offset),
            addr: read_u32!(bytes, offset),
            current_online_time: read_u32!(bytes, offset),
            total_online_time: read_u32!(bytes, offset),
            comp_volume: read_u32!(bytes, offset),
            raw_volume: read_u32!(bytes, offset),
            idle: read_u32!(bytes, offset),
            attack_cn: read_u16!(bytes, offset),
            skill_nr: read_u16!(bytes, offset),
            skill_target1: read_u16!(bytes, offset),
            skill_target2: read_u16!(bytes, offset),
            goto_x: read_u16!(bytes, offset),
            goto_y: read_u16!(bytes, offset),
            use_nr: read_u16!(bytes, offset),
            misc_action: read_u16!(bytes, offset),
            misc_target1: read_u16!(bytes, offset),
            misc_target2: read_u16!(bytes, offset),
            cerrno: read_u16!(bytes, offset),
            escape_timer: read_u16!(bytes, offset),
            enemy: {
                let mut arr = [0u16; 4];
                for i in 0..4 {
                    arr[i] = read_u16!(bytes, offset);
                }
                arr
            },
            current_enemy: read_u16!(bytes, offset),
            retry: read_u16!(bytes, offset),
            stunned: read_u16!(bytes, offset),
            speed_mod: read_i8!(bytes, offset),
            last_action: read_i8!(bytes, offset),
            unused: read_i8!(bytes, offset),
            depot_sold: read_i8!(bytes, offset),
            gethit_dam: read_i8!(bytes, offset),
            gethit_bonus: read_i8!(bytes, offset),
            light_bonus: read_u8!(bytes, offset),
            passwd: {
                let mut arr = [0u8; 16];
                for i in 0..16 {
                    arr[i] = read_u8!(bytes, offset);
                }
                arr
            },
            lastattack: read_i8!(bytes, offset),
            future1: {
                let mut arr = [0i8; 25];
                for i in 0..25 {
                    arr[i] = read_i8!(bytes, offset);
                }
                arr
            },
            sprite_override: read_i16!(bytes, offset),
            future2: {
                let mut arr = [0i16; 49];
                for i in 0..49 {
                    arr[i] = read_i16!(bytes, offset);
                }
                arr
            },
            depot: {
                let mut arr = [0u32; 62];
                for i in 0..62 {
                    arr[i] = read_u32!(bytes, offset);
                }
                arr
            },
            depot_cost: read_i32!(bytes, offset),
            luck: read_i32!(bytes, offset),
            unreach: read_i32!(bytes, offset),
            unreachx: read_i32!(bytes, offset),
            unreachy: read_i32!(bytes, offset),
            monster_class: read_i32!(bytes, offset),
            future3: {
                let mut arr = [0i32; 12];
                for i in 0..12 {
                    arr[i] = read_i32!(bytes, offset);
                }
                arr
            },
            logout_date: read_u32!(bytes, offset),
            data: {
                let mut arr = [0i32; 100];
                for i in 0..100 {
                    arr[i] = read_i32!(bytes, offset);
                }
                arr
            },
            text: {
                let mut arr = [[0u8; 160]; 10];
                for i in 0..10 {
                    for j in 0..160 {
                        arr[i][j] = read_u8!(bytes, offset);
                    }
                }
                arr
            },
        })
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
            if item_id == 0 {
                // TODO: Should this 0 be a constant?
                return Some(i);
            }
        }
        None
    }

    pub fn set_do_update_flags(&mut self) {
        self.flags |= CharacterFlags::CF_UPDATE.bits() | CharacterFlags::CF_SAVEME.bits();
    }

    pub fn is_monster(&self) -> bool {
        (self.kindred as u32 & crate::constants::KIN_MONSTER) != 0
    }

    pub fn is_usurp_or_thrall(&self) -> bool {
        (self.flags & (CharacterFlags::CF_USURP.bits() | CharacterFlags::CF_THRALL.bits())) != 0
    }

    pub fn is_building(&self) -> bool {
        (self.flags & CharacterFlags::CF_BUILDMODE.bits()) != 0
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
        if self.flags & CharacterFlags::CF_GREATERINV.bits() != 0 {
            return 15;
        }

        if self.flags & CharacterFlags::CF_GOD.bits() != 0 {
            return 10;
        }

        if self.flags & (CharacterFlags::CF_IMP | CharacterFlags::CF_USURP).bits() != 0 {
            return 5;
        }

        if self.flags & CharacterFlags::CF_STAFF.bits() != 0 {
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
            & (CharacterFlags::CF_PLAYER | CharacterFlags::CF_USURP | CharacterFlags::CF_NOSLEEP)
                .bits())
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
        assert_eq!(
            bytes.len(),
            std::mem::size_of::<Character>(),
            "Serialized Character size should match struct size"
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

        assert_eq!(original.used, deserialized.used);
        assert_eq!(original.name, deserialized.name);
        assert_eq!(original.reference, deserialized.reference);

        let original_kindred = original.kindred;
        let deserialized_kindred = deserialized.kindred;
        assert_eq!(original_kindred, deserialized_kindred);

        let original_player = original.player;
        let deserialized_player = deserialized.player;
        assert_eq!(original_player, deserialized_player);

        let original_sprite = original.sprite;
        let deserialized_sprite = deserialized.sprite;
        assert_eq!(original_sprite, deserialized_sprite);

        let original_sound = original.sound;
        let deserialized_sound = deserialized.sound;
        assert_eq!(original_sound, deserialized_sound);

        let original_flags = original.flags;
        let deserialized_flags = deserialized.flags;
        assert_eq!(original_flags, deserialized_flags);

        let original_alignment = original.alignment;
        let deserialized_alignment = deserialized.alignment;
        assert_eq!(original_alignment, deserialized_alignment);

        let original_temple_x = original.temple_x;
        let deserialized_temple_x = deserialized.temple_x;
        assert_eq!(original_temple_x, deserialized_temple_x);

        let original_temple_y = original.temple_y;
        let deserialized_temple_y = deserialized.temple_y;
        assert_eq!(original_temple_y, deserialized_temple_y);

        let original_hp = original.hp;
        let deserialized_hp = deserialized.hp;
        assert_eq!(original_hp, deserialized_hp);

        let original_attrib = original.attrib;
        let deserialized_attrib = deserialized.attrib;
        assert_eq!(original_attrib, deserialized_attrib);

        let original_skill = original.skill;
        let deserialized_skill = deserialized.skill;
        assert_eq!(original_skill, deserialized_skill);

        let original_x = original.x;
        let deserialized_x = deserialized.x;
        assert_eq!(original_x, deserialized_x);

        let original_y = original.y;
        let deserialized_y = deserialized.y;
        assert_eq!(original_y, deserialized_y);

        let original_gold = original.gold;
        let deserialized_gold = deserialized.gold;
        assert_eq!(original_gold, deserialized_gold);

        let original_item = original.item;
        let deserialized_item = deserialized.item;
        assert_eq!(original_item, deserialized_item);

        let original_worn = original.worn;
        let deserialized_worn = deserialized.worn;
        assert_eq!(original_worn, deserialized_worn);

        let original_depot = original.depot;
        let deserialized_depot = deserialized.depot;
        assert_eq!(original_depot, deserialized_depot);

        let original_data = original.data;
        let deserialized_data = deserialized.data;
        assert_eq!(original_data, deserialized_data);
    }

    #[test]
    fn test_character_from_bytes_insufficient_data() {
        let bytes = vec![0u8; std::mem::size_of::<Character>() - 1];
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

        character.flags = CharacterFlags::CF_PLAYER.bits();
        assert!(character.is_player());
    }

    #[test]
    fn test_character_has_prof() {
        let mut character = Character::default();
        assert!(!character.has_prof());

        character.flags = CharacterFlags::CF_PROF.bits();
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
}
