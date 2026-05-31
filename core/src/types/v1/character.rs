//! Frozen v1 `Character` layout.
//!
//! Mirrors the pre-MAX_SKILLS-bump shape (50 skill slots) so the snapshot
//! migrator can deserialize v1 `world_seed.wsnap` files and convert them
//! to the live (`v2`) shape.
//!
//! Do NOT modify this struct's layout; if it needs to change, introduce a
//! new versioned module instead.

use crate::skills::SkillIndex;
use bincode::{Decode, Encode};

/// Original v1 fixed slot count for the per-character skill matrix.
pub const V1_MAX_SKILLS: usize = 50;

/// Snapshot of the `Character` layout as it was at snapshot schema v1.
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

    pub skill: [[u8; SkillIndex::MaxIndex as usize]; V1_MAX_SKILLS],

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
    fn from(v1: Character) -> Self {
        let mut out = super::super::Character {
            used: v1.used,
            name: v1.name,
            reference: v1.reference,
            description: v1.description,
            kindred: v1.kindred,
            player: v1.player,
            pass1: v1.pass1,
            pass2: v1.pass2,
            sprite: v1.sprite,
            sound: v1.sound,
            flags: v1.flags,
            alignment: v1.alignment,
            temple_x: v1.temple_x,
            temple_y: v1.temple_y,
            tavern_x: v1.tavern_x,
            tavern_y: v1.tavern_y,
            temp: v1.temp,
            attrib: v1.attrib,
            hp: v1.hp,
            end: v1.end,
            mana: v1.mana,
            skill: [[0; SkillIndex::MaxIndex as usize]; crate::skills::MAX_SKILLS],
            weapon_bonus: v1.weapon_bonus,
            armor_bonus: v1.armor_bonus,
            a_hp: v1.a_hp,
            a_end: v1.a_end,
            a_mana: v1.a_mana,
            light: v1.light,
            mode: v1.mode,
            speed: v1.speed,
            points: v1.points,
            points_tot: v1.points_tot,
            armor: v1.armor,
            weapon: v1.weapon,
            x: v1.x,
            y: v1.y,
            tox: v1.tox,
            toy: v1.toy,
            frx: v1.frx,
            fry: v1.fry,
            status: v1.status,
            status2: v1.status2,
            dir: v1.dir,
            gold: v1.gold,
            item: v1.item,
            worn: v1.worn,
            spell: v1.spell,
            citem: v1.citem,
            creation_date: v1.creation_date,
            login_date: v1.login_date,
            addr: v1.addr,
            current_online_time: v1.current_online_time,
            total_online_time: v1.total_online_time,
            comp_volume: v1.comp_volume,
            raw_volume: v1.raw_volume,
            idle: v1.idle,
            attack_cn: v1.attack_cn,
            skill_nr: v1.skill_nr,
            skill_target1: v1.skill_target1,
            skill_target2: v1.skill_target2,
            goto_x: v1.goto_x,
            goto_y: v1.goto_y,
            use_nr: v1.use_nr,
            misc_action: v1.misc_action,
            misc_target1: v1.misc_target1,
            misc_target2: v1.misc_target2,
            cerrno: v1.cerrno,
            escape_timer: v1.escape_timer,
            enemy: v1.enemy,
            current_enemy: v1.current_enemy,
            retry: v1.retry,
            stunned: v1.stunned,
            speed_mod: v1.speed_mod,
            last_action: v1.last_action,
            unused: v1.unused,
            depot_sold: v1.depot_sold,
            gethit_dam: v1.gethit_dam,
            gethit_bonus: v1.gethit_bonus,
            light_bonus: v1.light_bonus,
            passwd: v1.passwd,
            lastattack: v1.lastattack,
            future1: v1.future1,
            sprite_override: v1.sprite_override,
            future2: v1.future2,
            depot: v1.depot,
            depot_cost: v1.depot_cost,
            luck: v1.luck,
            unreach: v1.unreach,
            unreachx: v1.unreachx,
            unreachy: v1.unreachy,
            monster_class: v1.monster_class,
            future3: v1.future3,
            logout_date: v1.logout_date,
            data: v1.data,
            text: v1.text,
        };
        // Copy first V1_MAX_SKILLS rows of the skill matrix; remainder
        // stays zero-initialized (newly added skill slots).
        for n in 0..V1_MAX_SKILLS {
            out.skill[n] = v1.skill[n];
        }
        out
    }
}
