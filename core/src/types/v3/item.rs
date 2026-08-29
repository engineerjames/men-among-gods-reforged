//! Frozen v3 `Item` layout.
//!
//! Mirrors the pre-stat-cap-widening shape (75 skill slots, `i8` attribute
//! and skill modifiers) so the snapshot migrator can deserialize schema v2
//! `world_seed.wsnap` files and convert them to the live (`v2`) shape.
//!
//! Do NOT modify this struct's layout.

use crate::skills::MAX_SKILLS;
use bincode::{Decode, Encode};

/// Snapshot of the `Item` layout as it was at snapshot schema v2.
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

    pub skill: [[i8; 3]; MAX_SKILLS],

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
    fn from(v2: Item) -> Self {
        let mut out = super::super::Item {
            used: v2.used,
            name: v2.name,
            reference: v2.reference,
            description: v2.description,
            flags: v2.flags,
            value: v2.value,
            placement: v2.placement,
            temp: v2.temp,
            damage_state: v2.damage_state,
            max_age: v2.max_age,
            current_age: v2.current_age,
            max_damage: v2.max_damage,
            current_damage: v2.current_damage,
            attrib: v2.attrib.map(|row| row.map(i16::from)),
            hp: v2.hp,
            end: v2.end,
            mana: v2.mana,
            skill: [[0; 3]; MAX_SKILLS],
            armor: v2.armor,
            weapon: v2.weapon,
            light: v2.light,
            duration: v2.duration,
            cost: v2.cost,
            power: v2.power,
            active: v2.active,
            x: v2.x,
            y: v2.y,
            carried: v2.carried,
            sprite_override: v2.sprite_override,
            sprite: v2.sprite,
            status: v2.status,
            gethit_dam: v2.gethit_dam,
            min_rank: v2.min_rank,
            future: v2.future,
            future3: v2.future3,
            t_bought: v2.t_bought,
            t_sold: v2.t_sold,
            driver: v2.driver,
            data: v2.data,
        };
        // Skill matrix is already 75 rows wide at schema v2; promote each
        // `i8` value to the live `i16` representation.
        for n in 0..MAX_SKILLS {
            out.skill[n] = v2.skill[n].map(i16::from);
        }
        out
    }
}
