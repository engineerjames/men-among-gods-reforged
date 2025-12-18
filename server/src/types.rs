/*************************************************************************

This file is part of 'Mercenaries of Astonia v2'
Copyright (c) 1997-2001 Daniel Brockhaus (joker@astonia.com)
All rights reserved.

Rust port maintains original logic and comments.

**************************************************************************/

//! Data types module - contains all game data structures ported from the original C++ headers

use crate::constants::*;

// =============================================================================
// Global State (from data.h)
// =============================================================================

/// Global server state structure
#[derive(Debug, Default)]
#[repr(C)]
pub struct Global {
    pub mdtime: i32,
    pub mdday: i32,
    pub mdyear: i32,
    pub dlight: i32,

    pub players_created: i32,
    pub npcs_created: i32,
    pub players_died: i32,
    pub npcs_died: i32,

    pub character_cnt: i32,
    pub item_cnt: i32,
    pub effect_cnt: i32,

    pub expire_cnt: i32,
    pub expire_run: i32,

    pub gc_cnt: i32,
    pub gc_run: i32,

    pub lost_cnt: i32,
    pub lost_run: i32,

    pub reset_char: i32,
    pub reset_item: i32,

    pub ticker: i32,

    pub total_online_time: i64,
    pub online_per_hour: [i64; 24],

    pub flags: i32,

    pub uptime: i64,
    pub uptime_per_hour: [i64; 24],

    pub awake: i32,
    pub body: i32,

    pub players_online: i32,
    pub queuesize: i32,

    pub recv: i64,
    pub send: i64,

    pub transfer_reset_time: i32,
    pub load_avg: i32,

    pub load: i64,

    pub max_online: i32,
    pub max_online_per_hour: [i32; 24],

    pub fullmoon: i8,
    pub newmoon: i8,

    pub unique: u64,

    pub cap: i32,
}

// =============================================================================
// Map Structure (from data.h)
// =============================================================================

/// Map tile structure
#[derive(Debug, Clone, Copy, Default)]
#[repr(C, packed)]
pub struct Map {
    /// background image
    pub sprite: u16,
    /// foreground sprite
    pub fsprite: u16,

    // for fast access to objects & characters
    pub ch: u32,
    pub to_ch: u32,
    pub it: u32,

    /// percentage of dlight
    pub dlight: u16,
    /// strength of light (objects only, daylight is computed independendly)
    pub light: i16,

    /// s.a.
    pub flags: u64,
}

// =============================================================================
// Character Structure (from Character.h)
// =============================================================================

/// Character structure - represents both players and NPCs
#[derive(Clone)]
#[repr(C, packed)]
pub struct Character {
    pub used: u8,  // 1

    // general
    pub name: [u8; 40],        // 41
    pub reference: [u8; 40],   // 81
    pub description: [u8; 200], // 281

    pub kindred: i32,  // 285

    pub player: i32,       // 289
    pub pass1: u32,
    pub pass2: u32,  // 297

    pub sprite: u16,  // 299, sprite base value, 1024 dist
    pub sound: u16,   // 301, sound base value, 64 dist

    pub flags: u64,  // 309

    pub alignment: i16,  // 311

    pub temple_x: u16,  // 313, position of temple for recall and dying
    pub temple_y: u16,  // 315

    pub tavern_x: u16,  // 317, position of last temple for re-login
    pub tavern_y: u16,  // 319

    pub temp: u16,  // 321, created from template n

    // character stats
    // [0]=bare value, 0=unknown
    // [1]=preset modifier, is race/npc dependend
    // [2]=race specific maximum
    // [3]=race specific difficulty to raise (0=not raisable, 1=easy ... 10=hard)
    // [4]=dynamic modifier, depends on equipment and spells (this one is currently not used)
    // [5]=total value
    pub attrib: [[u8; 6]; 5],  // 351

    pub hp: [u16; 6],    // 363
    pub end: [u16; 6],   // 375
    pub mana: [u16; 6],  // 387

    pub skill: [[u8; 6]; 50],  // 687

    pub weapon_bonus: u8,
    pub armor_bonus: u8,

    // temporary attributes
    pub a_hp: i32,
    pub a_end: i32,
    pub a_mana: i32,

    pub light: u8,  // strength of lightsource
    pub mode: u8,   // 0 = slow, 1 = medium, 2 = fast
    pub speed: i16,

    pub points: i32,
    pub points_tot: i32,

    // summary of weapons + armor
    pub armor: i16,
    pub weapon: i16,

    // map stuff
    pub x: i16,
    pub y: i16,       // current position x,y NOTE: x=-1, y=-1 = void
    pub tox: i16,
    pub toy: i16,     // target coordinated, where the char will be next turn
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
    pub attack_cn: u16,      // target for attacks, will attack if set (prio 4)
    pub skill_nr: u16,       // skill to use/spell to cast, will cast if set (prio 2)
    pub skill_target1: u16,  // target for skills/spells
    pub skill_target2: u16,  // target for skills/spells
    pub goto_x: u16,         // will goto x,y if set (prio 3)
    pub goto_y: u16,
    pub use_nr: u16,  // will use worn item nr if set (prio 1)

    pub misc_action: u16,   // drop, pickup, use, whatever (prio 5)
    pub misc_target1: u16,  // item for misc_action
    pub misc_target2: u16,  // location for misc_action

    pub cerrno: u16,  // error/success indicator for last action (svr_act level)

    pub escape_timer: u16,    // can try again to escape in X ticks
    pub enemy: [u16; 4],      // currently being fought against by these
    pub current_enemy: u16,   // currently fighting against X

    pub retry: u16,  // retry current action X times

    pub stunned: u16,  // is stunned for X ticks

    // misc stuff added later:
    pub speed_mod: i8,    // race dependand speed modification
    pub last_action: i8,  // last action was success/failure (driver_generic level)
    pub unused: i8,
    pub depot_sold: i8,  // items from depot where sold to pay for the rent

    pub gethit_dam: i8,    // damage for attacker when hitting this char
    pub gethit_bonus: i8,  // race specific bonus for above

    pub light_bonus: u8,  // char emits light all the time

    pub passwd: [u8; 16],

    pub lastattack: i8,     // neater display: remembers the last attack animation
    pub future1: [i8; 25],  // space for future expansion

    pub sprite_override: i16,

    pub future2: [i16; 49],

    pub depot: [u32; 62],

    pub depot_cost: i32,

    pub luck: i32,

    pub unreach: i32,
    pub unreachx: i32,
    pub unreachy: i32,

    pub monster_class: i32,  // monster class

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
        let end = self.name.iter().position(|&c| c == 0).unwrap_or(self.name.len());
        std::str::from_utf8(&self.name[..end]).unwrap_or("*unknown*")
    }
    
    /// Check if character is a player
    pub fn is_player(&self) -> bool {
        (self.flags & CharacterFlags::CF_PLAYER.bits()) != 0
    }
    
    /// Check if character has profile flag set
    pub fn has_prof(&self) -> bool {
        (self.flags & CharacterFlags::CF_PROF.bits()) != 0
    }
}

// =============================================================================
// Item Structure (from data.h)
// =============================================================================

/// Item structure
#[derive(Clone)]
#[repr(C, packed)]
pub struct Item {
    pub used: u8,               // 1
    pub name: [u8; 40],         // 41
    pub reference: [u8; 40],    // 81, a pair of boots
    pub description: [u8; 200], // 281, A pair of studded leather boots.

    pub flags: u64,  // 289, s.a.

    pub value: u32,       // 293, value to a merchant
    pub placement: u16,   // 295, see constants above

    pub temp: u16,  // 297, created from template temp

    pub damage_state: u8,  // 298, has reached damage level X of 5, 0=OK, 4=almost destroyed, 5=destroyed

    // states for non-active [0] and active[1]
    pub max_age: [u32; 2],      // 306, maximum age per state
    pub current_age: [u32; 2],  // 314, current age in current state

    pub max_damage: u32,      // 318, maximum damage per state
    pub current_damage: u32,  // 322, current damage in current state

    // modifiers - modifiers apply only when the item is being
    // worn (wearable objects) or when spell is cast. After duration expires,
    // the effects are removed.
    //
    // modifiers - modifier [0] applies when the item is being
    // worn (wearable objects) or is added to the powers (spells) for permanent spells
    // modifier [1] applies when it is active
    // modifier [2] is not a modifier but the minimum value that attibute/skill must have to wear or use
    // the item
    pub attrib: [[i8; 3]; 5],  // 337

    pub hp: [i16; 3],    // 343
    pub end: [i16; 3],   // 349
    pub mana: [i16; 3],  // 355

    pub skill: [[i8; 3]; 50],  // 505

    pub armor: [i8; 2],   // 506
    pub weapon: [i8; 2],  // 507

    pub light: [i16; 2],  // 511

    pub duration: u32,  // 515
    pub cost: u32,      // 519
    pub power: u32,     // 523
    pub active: u32,    // 527

    // map stuff
    pub x: u16,
    pub y: u16,              // 531, current position NOTE: x=0, y=0 = void
    pub carried: u16,        // 533, carried by character carried
    pub sprite_override: u16, // 535, used for potions/spells which change the character sprite

    pub sprite: [i16; 2],  // 543
    pub status: [u8; 2],   // 545

    pub gethit_dam: [i8; 2],  // 547, damage for hitting this item

    pub min_rank: i8,      // minimum rank to wear the item
    pub future: [i8; 3],
    pub future3: [i32; 9],  // 587

    pub t_bought: i32,  // 591
    pub t_sold: i32,    // 595

    pub driver: u8,        // 596, special routines for LOOKSPECIAL and USESPECIAL
    pub data: [u32; 10],   // 634, driver data
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
        let end = self.name.iter().position(|&c| c == 0).unwrap_or(self.name.len());
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
}

// =============================================================================
// Effect Structure (from data.h)
// =============================================================================

/// Effect structure
#[derive(Debug, Clone, Copy, Default)]
#[repr(C, packed)]
pub struct Effect {
    pub used: u8,
    pub flags: u8,

    pub effect_type: u8,  // what type of effect (FX_)

    pub duration: u32,  // time effect will stay

    pub data: [u32; 10],  // some data
}

// =============================================================================
// See Map Structure (from data.h)
// =============================================================================

/// Visibility map for a character
#[derive(Clone)]
pub struct SeeMap {
    pub x: i32,
    pub y: i32,
    pub vis: [i8; 40 * 40],
}

impl Default for SeeMap {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            vis: [0; 40 * 40],
        }
    }
}

// =============================================================================
// Client Map Structure (from client.h)
// =============================================================================

/// Client-side map tile
#[derive(Debug, Clone, Copy, Default)]
#[repr(C, packed)]
pub struct CMap {
    // for background
    pub ba_sprite: i16,  // background image
    pub light: u8,
    pub flags: u32,
    pub flags2: u32,

    // for character
    pub ch_sprite: i16,   // basic sprite of character
    pub ch_status2: u8,
    pub ch_status: u8,    // what the character is doing, animation-wise
    pub ch_speed: u8,     // speed of animation
    pub ch_nr: u16,
    pub ch_id: u16,
    pub ch_proz: u8,      // health in percent

    // for item
    pub it_sprite: i16,  // basic sprite of item
    pub it_status: u8,   // for items with animation (burning torches etc)
}

// =============================================================================
// Client Player Structure (from client.h)
// =============================================================================

/// Client-side player data
#[derive(Clone)]
pub struct CPlayer {
    // informative stuff
    pub name: [u8; 40],

    pub mode: i32,  // 0 = slow, 1 = medium, 2 = fast

    // character stats
    // [0]=bare value, 0=unknown
    // [1]=preset modifier, is race/npc dependend
    // [2]=race specific maximum
    // [3]=race specific difficulty to raise (0=not raisable, 1=easy ... 10=hard)
    // [4]=dynamic modifier, depends on equipment and spells
    // [5]=total value
    pub attrib: [[u8; 6]; 5],

    pub hp: [u16; 6],
    pub end: [u16; 6],
    pub mana: [u16; 6],

    pub skill: [[u8; 6]; 100],

    // temporary attributes
    pub a_hp: i32,
    pub a_end: i32,
    pub a_mana: i32,

    pub points: i32,
    pub points_tot: i32,
    pub kindred: i32,

    // posessions
    pub gold: i32,

    // items carried
    pub item: [i32; 40],
    pub item_p: [i32; 40],

    // items worn
    pub worn: [i32; 20],
    pub worn_p: [i32; 20],

    pub spell: [i32; 20],
    pub active: [i8; 20],

    pub weapon: i32,
    pub armor: i32,

    pub citem: i32,
    pub citem_p: i32,

    pub attack_cn: i32,
    pub goto_x: i32,
    pub goto_y: i32,
    pub misc_action: i32,
    pub misc_target1: i32,
    pub misc_target2: i32,
    pub dir: i32,

    // server only:
    pub x: i32,
    pub y: i32,
}

impl Default for CPlayer {
    fn default() -> Self {
        Self {
            name: [0; 40],
            mode: 0,
            attrib: [[0; 6]; 5],
            hp: [0; 6],
            end: [0; 6],
            mana: [0; 6],
            skill: [[0; 6]; 100],
            a_hp: 0,
            a_end: 0,
            a_mana: 0,
            points: 0,
            points_tot: 0,
            kindred: 0,
            gold: 0,
            item: [0; 40],
            item_p: [0; 40],
            worn: [0; 20],
            worn_p: [0; 20],
            spell: [0; 20],
            active: [0; 20],
            weapon: 0,
            armor: 0,
            citem: 0,
            citem_p: 0,
            attack_cn: 0,
            goto_x: 0,
            goto_y: 0,
            misc_action: 0,
            misc_target1: 0,
            misc_target2: 0,
            dir: 0,
            x: 0,
            y: 0,
        }
    }
}

// =============================================================================
// Macros as Functions (from macros.h)
// =============================================================================

/// Sanity checks on map locations x
#[inline]
pub fn sanex(x: i32) -> bool {
    x >= 0 && x < SERVER_MAPX
}

/// Sanity checks on map locations y
#[inline]
pub fn saney(y: i32) -> bool {
    y >= 0 && y < SERVER_MAPY
}

/// Sanity checks on map locations x and y
#[inline]
pub fn sanexy(x: i32, y: i32) -> bool {
    sanex(x) && saney(y)
}

/// Convert (x,y) coordinates to absolute position
#[inline]
pub fn xy2m(x: i32, y: i32) -> usize {
    (x + y * SERVER_MAPX) as usize
}

/// Sanity checks on item numbers
#[inline]
pub fn is_sane_item(index: usize) -> bool {
    index > 0 && index < MAXITEM
}

/// Sanity checks on character numbers
#[inline]
pub fn is_sane_char(cn: usize) -> bool {
    cn > 0 && cn < MAXCHARS
}

/// Sanity check on skill number
#[inline]
pub fn sane_skill(s: usize) -> bool {
    s < MAXSKILL
}

/// Sanity checks on item templates
#[inline]
pub fn is_sane_itemplate(tn: usize) -> bool {
    tn > 0 && tn < MAXTITEM
}

/// Sanity checks on character templates
#[inline]
pub fn is_sane_ctemplate(tn: usize) -> bool {
    tn > 0 && tn < MAXTCHARS
}

/// Check if this is a sane player character number
#[inline]
pub fn is_sane_player(cn: usize, ch: &[Character]) -> bool {
    is_sane_char(cn) && (ch[cn].flags & CharacterFlags::CF_PLAYER.bits()) != 0
}
