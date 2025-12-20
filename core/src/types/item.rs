//! Item structure

use crate::constants::ItemFlags;

/// Item structure
#[derive(Clone)]
#[repr(C, packed)]
pub struct Item {
    pub used: u8,               // 1
    pub name: [u8; 40],         // 41
    pub reference: [u8; 40],    // 81, a pair of boots
    pub description: [u8; 200], // 281, A pair of studded leather boots.

    pub flags: u64, // 289, s.a.

    pub value: u32,     // 293, value to a merchant
    pub placement: u16, // 295, see constants above

    pub temp: u16, // 297, created from template temp

    pub damage_state: u8, // 298, has reached damage level X of 5, 0=OK, 4=almost destroyed, 5=destroyed

    // states for non-active [0] and active[1]
    pub max_age: [u32; 2],     // 306, maximum age per state
    pub current_age: [u32; 2], // 314, current age in current state

    pub max_damage: u32,     // 318, maximum damage per state
    pub current_damage: u32, // 322, current damage in current state

    // modifiers - modifiers apply only when the item is being
    // worn (wearable objects) or when spell is cast. After duration expires,
    // the effects are removed.
    //
    // modifiers - modifier [0] applies when the item is being
    // worn (wearable objects) or is added to the powers (spells) for permanent spells
    // modifier [1] applies when it is active
    // modifier [2] is not a modifier but the minimum value that attibute/skill must have to wear or use
    // the item
    pub attrib: [[i8; 3]; 5], // 337

    pub hp: [i16; 3],   // 343
    pub end: [i16; 3],  // 349
    pub mana: [i16; 3], // 355

    pub skill: [[i8; 3]; 50], // 505

    pub armor: [i8; 2],  // 506
    pub weapon: [i8; 2], // 507

    pub light: [i16; 2], // 511

    pub duration: u32, // 515
    pub cost: u32,     // 519
    pub power: u32,    // 523
    pub active: u32,   // 527

    // map stuff
    pub x: u16,
    pub y: u16,               // 531, current position NOTE: x=0, y=0 = void
    pub carried: u16,         // 533, carried by character carried
    pub sprite_override: u16, // 535, used for potions/spells which change the character sprite

    pub sprite: [i16; 2], // 543
    pub status: [u8; 2],  // 545

    pub gethit_dam: [i8; 2], // 547, damage for hitting this item

    pub min_rank: i8, // minimum rank to wear the item
    pub future: [i8; 3],
    pub future3: [i32; 9], // 587

    pub t_bought: i32, // 591
    pub t_sold: i32,   // 595

    pub driver: u8,      // 596, special routines for LOOKSPECIAL and USESPECIAL
    pub data: [u32; 10], // 634, driver data
}

impl Default for Item {
    fn default() -> Self {
        Self {
            used: 0,
            name: [0; 40],
            reference: [0; 40],
            description: [0; 200],
            flags: 0,
            value: 0,
            placement: 0,
            temp: 0,
            damage_state: 0,
            max_age: [0; 2],
            current_age: [0; 2],
            max_damage: 0,
            current_damage: 0,
            attrib: [[0; 3]; 5],
            hp: [0; 3],
            end: [0; 3],
            mana: [0; 3],
            skill: [[0; 3]; 50],
            armor: [0; 2],
            weapon: [0; 2],
            light: [0; 2],
            duration: 0,
            cost: 0,
            power: 0,
            active: 0,
            x: 0,
            y: 0,
            carried: 0,
            sprite_override: 0,
            sprite: [0; 2],
            status: [0; 2],
            gethit_dam: [0; 2],
            min_rank: 0,
            future: [0; 3],
            future3: [0; 9],
            t_bought: 0,
            t_sold: 0,
            driver: 0,
            data: [0; 10],
        }
    }
}

impl Item {
    /// Get name as a string slice
    pub fn get_name(&self) -> &str {
        let end = self
            .name
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(self.name.len());
        std::str::from_utf8(&self.name[..end]).unwrap_or("*unknown*")
    }

    /// Check if item has labyrinth destroy flag
    pub fn has_laby_destroy(&self) -> bool {
        (self.flags & ItemFlags::IF_LABYDESTROY.bits()) != 0
    }

    /// Check if item has soulstone flag
    pub fn has_soulstone(&self) -> bool {
        (self.flags & ItemFlags::IF_SOULSTONE.bits()) != 0
    }

    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < std::mem::size_of::<Item>() {
            return None;
        }

        let mut offset: usize = 0;

        Some(Self {
            used: read_u8!(data, offset),
            name: {
                let mut arr = [0u8; 40];
                for i in 0..40 {
                    arr[i] = read_u8!(data, offset);
                }
                arr
            },
            reference: {
                let mut arr = [0u8; 40];
                for i in 0..40 {
                    arr[i] = read_u8!(data, offset);
                }
                arr
            },
            description: {
                let mut arr = [0u8; 200];
                for i in 0..200 {
                    arr[i] = read_u8!(data, offset);
                }
                arr
            },
            flags: read_u64!(data, offset),
            value: read_u32!(data, offset),
            placement: read_u16!(data, offset),
            temp: read_u16!(data, offset),
            damage_state: read_u8!(data, offset),
            max_age: [read_u32!(data, offset), read_u32!(data, offset)],
            current_age: [read_u32!(data, offset), read_u32!(data, offset)],
            max_damage: read_u32!(data, offset),
            current_damage: read_u32!(data, offset),
            attrib: {
                let mut arr = [[0i8; 3]; 5];
                for i in 0..5 {
                    for j in 0..3 {
                        arr[i][j] = read_i8!(data, offset);
                    }
                }
                arr
            },
            hp: [
                read_i16!(data, offset),
                read_i16!(data, offset),
                read_i16!(data, offset),
            ],
            end: [
                read_i16!(data, offset),
                read_i16!(data, offset),
                read_i16!(data, offset),
            ],
            mana: [
                read_i16!(data, offset),
                read_i16!(data, offset),
                read_i16!(data, offset),
            ],
            skill: {
                let mut arr = [[0i8; 3]; 50];
                for i in 0..50 {
                    for j in 0..3 {
                        arr[i][j] = read_i8!(data, offset);
                    }
                }
                arr
            },
            armor: [read_i8!(data, offset), read_i8!(data, offset)],
            weapon: [read_i8!(data, offset), read_i8!(data, offset)],
            light: [read_i16!(data, offset), read_i16!(data, offset)],
            duration: read_u32!(data, offset),
            cost: read_u32!(data, offset),
            power: read_u32!(data, offset),
            active: read_u32!(data, offset),
            x: read_u16!(data, offset),
            y: read_u16!(data, offset),
            carried: read_u16!(data, offset),
            sprite_override: read_u16!(data, offset),
            sprite: [read_i16!(data, offset), read_i16!(data, offset)],
            status: [read_u8!(data, offset), read_u8!(data, offset)],
            gethit_dam: [read_i8!(data, offset), read_i8!(data, offset)],
            min_rank: read_i8!(data, offset),
            future: [
                read_i8!(data, offset),
                read_i8!(data, offset),
                read_i8!(data, offset),
            ],
            future3: {
                let mut arr = [0i32; 9];
                for i in 0..9 {
                    arr[i] = read_i32!(data, offset);
                }
                arr
            },
            t_bought: read_i32!(data, offset),
            t_sold: read_i32!(data, offset),
            driver: read_u8!(data, offset),
            data: {
                let mut arr = [0u32; 10];
                for i in 0..10 {
                    arr[i] = read_u32!(data, offset);
                }
                arr
            },
        })
    }
}
