//! Frozen v1 `Item` layout (50-slot skill matrix).
//!
//! Do NOT modify this struct's layout.

use bincode::{Decode, Encode};

/// Original v1 skill matrix length for `Item`.
pub const V1_MAX_SKILLS: usize = 50;

/// Snapshot of the `Item` layout as it was at snapshot schema v1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Item {
    pub used: u8,
    pub name: [u8; 40],
    pub reference: [u8; 40],
    pub description: [u8; 200],

    pub flags: u64,

    pub value: u32,
    pub placement: u16,

    pub temp: u16,

    pub damage_state: u8,

    pub max_age: [u32; 2],
    pub current_age: [u32; 2],

    pub max_damage: u32,
    pub current_damage: u32,

    pub attrib: [[i8; 3]; 5],

    pub hp: [i16; 3],
    pub end: [i16; 3],
    pub mana: [i16; 3],

    pub skill: [[i8; 3]; V1_MAX_SKILLS],

    pub armor: [i8; 2],
    pub weapon: [i8; 2],

    pub light: [i16; 2],

    pub duration: u32,
    pub cost: u32,
    pub power: u32,
    pub active: u32,

    pub x: u16,
    pub y: u16,
    pub carried: u16,
    pub sprite_override: u16,

    pub sprite: [i16; 2],
    pub status: [u8; 2],

    pub gethit_dam: [i8; 2],

    pub min_rank: i8,
    pub future: [i8; 3],
    pub future3: [i32; 9],

    pub t_bought: i32,
    pub t_sold: i32,

    pub driver: u8,
    pub data: [u32; 10],
}

impl From<Item> for super::super::Item {
    fn from(v1: Item) -> Self {
        let mut out = super::super::Item {
            used: v1.used,
            name: v1.name,
            reference: v1.reference,
            description: v1.description,
            flags: v1.flags,
            value: v1.value,
            placement: v1.placement,
            temp: v1.temp,
            damage_state: v1.damage_state,
            max_age: v1.max_age,
            current_age: v1.current_age,
            max_damage: v1.max_damage,
            current_damage: v1.current_damage,
            attrib: v1.attrib,
            hp: v1.hp,
            end: v1.end,
            mana: v1.mana,
            skill: [[0; 3]; crate::skills::MAX_SKILLS],
            armor: v1.armor,
            weapon: v1.weapon,
            light: v1.light,
            duration: v1.duration,
            cost: v1.cost,
            power: v1.power,
            active: v1.active,
            x: v1.x,
            y: v1.y,
            carried: v1.carried,
            sprite_override: v1.sprite_override,
            sprite: v1.sprite,
            status: v1.status,
            gethit_dam: v1.gethit_dam,
            min_rank: v1.min_rank,
            future: v1.future,
            future3: v1.future3,
            t_bought: v1.t_bought,
            t_sold: v1.t_sold,
            driver: v1.driver,
            data: v1.data,
        };
        for n in 0..V1_MAX_SKILLS {
            out.skill[n] = v1.skill[n];
        }
        out
    }
}
