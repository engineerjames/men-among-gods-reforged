//! Frozen v3 `Character` layout.
//!
//! Mirrors the pre-stat-cap-widening shape (75 skill slots, `u8` attribute
//! and skill values) so the snapshot migrator can deserialize schema v2
//! `world_seed.wsnap` files and convert them to the live (`v2`) shape.
//!
//! Do NOT modify this struct's layout; if it needs to change, introduce a
//! new versioned module instead.

use crate::skills::{MAX_SKILLS, SkillIndex};
use bincode::{Decode, Encode};

/// Snapshot of the `Character` layout as it was at snapshot schema v2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Character {
    pub used: u8,

    pub name: [u8; 40],
    pub reference: [u8; 40],
    pub description: [u8; 200],

    pub kindred: i32,

    pub player: i32,
    pub pass1: u32,
    pub pass2: u32,

    pub sprite: u16,
    pub sound: u16,

    pub flags: u64,

    pub alignment: i16,

    pub temple_x: u16,
    pub temple_y: u16,

    pub tavern_x: u16,
    pub tavern_y: u16,

    pub temp: u16,

    pub attrib: [[u8; SkillIndex::MaxIndex as usize]; 5],

    pub hp: [u16; SkillIndex::MaxIndex as usize],
    pub end: [u16; SkillIndex::MaxIndex as usize],
    pub mana: [u16; SkillIndex::MaxIndex as usize],

    pub skill: [[u8; SkillIndex::MaxIndex as usize]; MAX_SKILLS],

    pub weapon_bonus: u8,
    pub armor_bonus: u8,

    pub a_hp: i32,
    pub a_end: i32,
    pub a_mana: i32,

    pub light: u8,
    pub mode: u8,
    pub speed: i16,

    pub points: i32,
    pub points_tot: i32,

    pub armor: i16,
    pub weapon: i16,

    pub x: i16,
    pub y: i16,
    pub tox: i16,
    pub toy: i16,
    pub frx: i16,
    pub fry: i16,
    pub status: i16,
    pub status2: i16,
    pub dir: u8,

    pub gold: i32,

    pub item: [u32; 40],

    pub worn: [u32; 20],

    pub spell: [u32; 20],

    pub citem: u32,

    pub creation_date: u32,

    pub login_date: u32,

    pub addr: u32,

    pub current_online_time: u32,
    pub total_online_time: u32,
    pub comp_volume: u32,
    pub raw_volume: u32,
    pub idle: u32,

    pub attack_cn: u16,
    pub skill_nr: u16,
    pub skill_target1: u16,
    pub skill_target2: u16,
    pub goto_x: u16,
    pub goto_y: u16,
    pub use_nr: u16,

    pub misc_action: u16,
    pub misc_target1: u16,
    pub misc_target2: u16,

    pub cerrno: u16,

    pub escape_timer: u16,
    pub enemy: [u16; 4],
    pub current_enemy: u16,

    pub retry: u16,

    pub stunned: u16,

    pub speed_mod: i8,
    pub last_action: i8,
    pub unused: i8,
    pub depot_sold: i8,

    pub gethit_dam: i8,
    pub gethit_bonus: i8,

    pub light_bonus: u8,

    pub passwd: [u8; 16],

    pub lastattack: i8,
    pub future1: [u8; 25],

    pub sprite_override: i16,

    pub future2: [i16; 49],

    pub depot: [u32; 62],

    pub depot_cost: i32,

    pub luck: i32,

    pub unreach: i32,
    pub unreachx: i32,
    pub unreachy: i32,

    pub monster_class: i32,

    pub future3: [i32; 12],

    pub logout_date: u32,

    pub data: [i32; 100],

    pub text: [[u8; 160]; 10],
}

impl From<Character> for super::super::Character {
    fn from(v2: Character) -> Self {
        let mut out = super::super::Character {
            used: v2.used,
            name: v2.name,
            reference: v2.reference,
            description: v2.description,
            kindred: v2.kindred,
            player: v2.player,
            pass1: v2.pass1,
            pass2: v2.pass2,
            sprite: v2.sprite,
            sound: v2.sound,
            flags: v2.flags,
            alignment: v2.alignment,
            temple_x: v2.temple_x,
            temple_y: v2.temple_y,
            tavern_x: v2.tavern_x,
            tavern_y: v2.tavern_y,
            temp: v2.temp,
            attrib: v2.attrib.map(|row| row.map(u16::from)),
            hp: v2.hp,
            end: v2.end,
            mana: v2.mana,
            skill: [[0; SkillIndex::MaxIndex as usize]; MAX_SKILLS],
            weapon_bonus: v2.weapon_bonus,
            armor_bonus: v2.armor_bonus,
            a_hp: v2.a_hp,
            a_end: v2.a_end,
            a_mana: v2.a_mana,
            light: v2.light,
            mode: v2.mode,
            speed: v2.speed,
            points: v2.points,
            points_tot: v2.points_tot,
            armor: v2.armor,
            weapon: v2.weapon,
            x: v2.x,
            y: v2.y,
            tox: v2.tox,
            toy: v2.toy,
            frx: v2.frx,
            fry: v2.fry,
            status: v2.status,
            status2: v2.status2,
            dir: v2.dir,
            gold: v2.gold,
            item: v2.item,
            worn: v2.worn,
            spell: v2.spell,
            citem: v2.citem,
            creation_date: v2.creation_date,
            login_date: v2.login_date,
            addr: v2.addr,
            current_online_time: v2.current_online_time,
            total_online_time: v2.total_online_time,
            comp_volume: v2.comp_volume,
            raw_volume: v2.raw_volume,
            idle: v2.idle,
            attack_cn: v2.attack_cn,
            skill_nr: v2.skill_nr,
            skill_target1: v2.skill_target1,
            skill_target2: v2.skill_target2,
            goto_x: v2.goto_x,
            goto_y: v2.goto_y,
            use_nr: v2.use_nr,
            misc_action: v2.misc_action,
            misc_target1: v2.misc_target1,
            misc_target2: v2.misc_target2,
            cerrno: v2.cerrno,
            escape_timer: v2.escape_timer,
            enemy: v2.enemy,
            current_enemy: v2.current_enemy,
            retry: v2.retry,
            stunned: v2.stunned,
            speed_mod: v2.speed_mod,
            last_action: v2.last_action,
            unused: v2.unused,
            depot_sold: v2.depot_sold,
            gethit_dam: v2.gethit_dam,
            gethit_bonus: v2.gethit_bonus,
            light_bonus: v2.light_bonus,
            passwd: v2.passwd,
            lastattack: v2.lastattack,
            future1: v2.future1,
            sprite_override: v2.sprite_override,
            future2: v2.future2,
            depot: v2.depot,
            depot_cost: v2.depot_cost,
            luck: v2.luck,
            unreach: v2.unreach,
            unreachx: v2.unreachx,
            unreachy: v2.unreachy,
            monster_class: v2.monster_class,
            future3: v2.future3,
            logout_date: v2.logout_date,
            data: v2.data,
            text: v2.text,
        };
        // Skill matrix is already 75 rows wide at schema v2; promote each
        // `u8` value to the live `u16` representation.
        for n in 0..MAX_SKILLS {
            out.skill[n] = v2.skill[n].map(u16::from);
        }
        out
    }
}
