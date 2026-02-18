use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use bincode::{Decode, Encode};
use mag_core::constants::{
    MAXCHARS, MAXEFFECT, MAXITEM, MAXTCHARS, MAXTITEM, SERVER_MAPX, SERVER_MAPY,
};
use mag_core::types::{Character, Effect, Global, Item, Map};

#[derive(Debug, Encode, Decode)]
struct NormalizedDataSet<T> {
    magic: [u8; 4],
    version: u32,
    source_file: String,
    source_record_size: usize,
    records: Vec<T>,
}

#[derive(Debug, Clone, Encode, Decode)]
struct MapPlain {
    sprite: u16,
    fsprite: u16,
    ch: u32,
    to_ch: u32,
    it: u32,
    dlight: u16,
    light: i16,
    flags: u64,
}

#[derive(Debug, Clone, Encode, Decode)]
struct ItemPlain {
    used: u8,
    name: [u8; 40],
    reference: [u8; 40],
    description: [u8; 200],
    flags: u64,
    value: u32,
    placement: u16,
    temp: u16,
    damage_state: u8,
    max_age: [u32; 2],
    current_age: [u32; 2],
    max_damage: u32,
    current_damage: u32,
    attrib: [[i8; 3]; 5],
    hp: [i16; 3],
    end: [i16; 3],
    mana: [i16; 3],
    skill: [[i8; 3]; 50],
    armor: [i8; 2],
    weapon: [i8; 2],
    light: [i16; 2],
    duration: u32,
    cost: u32,
    power: u32,
    active: u32,
    x: u16,
    y: u16,
    carried: u16,
    sprite_override: u16,
    sprite: [i16; 2],
    status: [u8; 2],
    gethit_dam: [i8; 2],
    min_rank: i8,
    future: [i8; 3],
    future3: [i32; 9],
    t_bought: i32,
    t_sold: i32,
    driver: u8,
    data: [u32; 10],
}

#[derive(Debug, Clone, Encode, Decode)]
struct CharacterPlain {
    used: u8,
    name: [u8; 40],
    reference: [u8; 40],
    description: [u8; 200],
    kindred: i32,
    player: i32,
    pass1: u32,
    pass2: u32,
    sprite: u16,
    sound: u16,
    flags: u64,
    alignment: i16,
    temple_x: u16,
    temple_y: u16,
    tavern_x: u16,
    tavern_y: u16,
    temp: u16,
    attrib: [[u8; 6]; 5],
    hp: [u16; 6],
    end: [u16; 6],
    mana: [u16; 6],
    skill: [[u8; 6]; 50],
    weapon_bonus: u8,
    armor_bonus: u8,
    a_hp: i32,
    a_end: i32,
    a_mana: i32,
    light: u8,
    mode: u8,
    speed: i16,
    points: i32,
    points_tot: i32,
    armor: i16,
    weapon: i16,
    x: i16,
    y: i16,
    tox: i16,
    toy: i16,
    frx: i16,
    fry: i16,
    status: i16,
    status2: i16,
    dir: u8,
    gold: i32,
    item: [u32; 40],
    worn: [u32; 20],
    spell: [u32; 20],
    citem: u32,
    creation_date: u32,
    login_date: u32,
    addr: u32,
    current_online_time: u32,
    total_online_time: u32,
    comp_volume: u32,
    raw_volume: u32,
    idle: u32,
    attack_cn: u16,
    skill_nr: u16,
    skill_target1: u16,
    skill_target2: u16,
    goto_x: u16,
    goto_y: u16,
    use_nr: u16,
    misc_action: u16,
    misc_target1: u16,
    misc_target2: u16,
    cerrno: u16,
    escape_timer: u16,
    enemy: [u16; 4],
    current_enemy: u16,
    retry: u16,
    stunned: u16,
    speed_mod: i8,
    last_action: i8,
    unused: i8,
    depot_sold: i8,
    gethit_dam: i8,
    gethit_bonus: i8,
    light_bonus: u8,
    passwd: [u8; 16],
    lastattack: i8,
    future1: [i8; 25],
    sprite_override: i16,
    future2: [i16; 49],
    depot: [u32; 62],
    depot_cost: i32,
    luck: i32,
    unreach: i32,
    unreachx: i32,
    unreachy: i32,
    monster_class: i32,
    future3: [i32; 12],
    logout_date: u32,
    data: [i32; 100],
    text: [[u8; 160]; 10],
}

#[derive(Debug, Clone, Encode, Decode)]
struct EffectPlain {
    used: u8,
    flags: u8,
    effect_type: u8,
    duration: u32,
    data: [u32; 10],
}

#[derive(Debug, Clone, Encode, Decode)]
struct GlobalPlain {
    mdtime: i32,
    mdday: i32,
    mdyear: i32,
    dlight: i32,
    players_created: i32,
    npcs_created: i32,
    players_died: i32,
    npcs_died: i32,
    character_cnt: i32,
    item_cnt: i32,
    effect_cnt: i32,
    expire_cnt: i32,
    expire_run: i32,
    gc_cnt: i32,
    gc_run: i32,
    lost_cnt: i32,
    lost_run: i32,
    reset_char: i32,
    reset_item: i32,
    ticker: i32,
    total_online_time: i64,
    online_per_hour: [i64; 24],
    flags: i32,
    uptime: i64,
    uptime_per_hour: [i64; 24],
    awake: i32,
    body: i32,
    players_online: i32,
    queuesize: i32,
    recv: i64,
    send: i64,
    transfer_reset_time: i32,
    load_avg: i32,
    load: i64,
    max_online: i32,
    max_online_per_hour: [i32; 24],
    fullmoon: i8,
    newmoon: i8,
    unique: u64,
    cap: i32,
    original_bytes: Vec<u8>,
    trailing_bytes: Vec<u8>,
}

macro_rules! packed_field {
    ($obj:expr, $field:ident) => {{
        unsafe { std::ptr::read_unaligned(std::ptr::addr_of!($obj.$field)) }
    }};
}

impl From<Map> for MapPlain {
    fn from(value: Map) -> Self {
        Self {
            sprite: packed_field!(value, sprite),
            fsprite: packed_field!(value, fsprite),
            ch: packed_field!(value, ch),
            to_ch: packed_field!(value, to_ch),
            it: packed_field!(value, it),
            dlight: packed_field!(value, dlight),
            light: packed_field!(value, light),
            flags: packed_field!(value, flags),
        }
    }
}

impl From<Item> for ItemPlain {
    fn from(value: Item) -> Self {
        Self {
            used: packed_field!(value, used),
            name: packed_field!(value, name),
            reference: packed_field!(value, reference),
            description: packed_field!(value, description),
            flags: packed_field!(value, flags),
            value: packed_field!(value, value),
            placement: packed_field!(value, placement),
            temp: packed_field!(value, temp),
            damage_state: packed_field!(value, damage_state),
            max_age: packed_field!(value, max_age),
            current_age: packed_field!(value, current_age),
            max_damage: packed_field!(value, max_damage),
            current_damage: packed_field!(value, current_damage),
            attrib: packed_field!(value, attrib),
            hp: packed_field!(value, hp),
            end: packed_field!(value, end),
            mana: packed_field!(value, mana),
            skill: packed_field!(value, skill),
            armor: packed_field!(value, armor),
            weapon: packed_field!(value, weapon),
            light: packed_field!(value, light),
            duration: packed_field!(value, duration),
            cost: packed_field!(value, cost),
            power: packed_field!(value, power),
            active: packed_field!(value, active),
            x: packed_field!(value, x),
            y: packed_field!(value, y),
            carried: packed_field!(value, carried),
            sprite_override: packed_field!(value, sprite_override),
            sprite: packed_field!(value, sprite),
            status: packed_field!(value, status),
            gethit_dam: packed_field!(value, gethit_dam),
            min_rank: packed_field!(value, min_rank),
            future: packed_field!(value, future),
            future3: packed_field!(value, future3),
            t_bought: packed_field!(value, t_bought),
            t_sold: packed_field!(value, t_sold),
            driver: packed_field!(value, driver),
            data: packed_field!(value, data),
        }
    }
}

impl From<Character> for CharacterPlain {
    fn from(value: Character) -> Self {
        Self {
            used: packed_field!(value, used),
            name: packed_field!(value, name),
            reference: packed_field!(value, reference),
            description: packed_field!(value, description),
            kindred: packed_field!(value, kindred),
            player: packed_field!(value, player),
            pass1: packed_field!(value, pass1),
            pass2: packed_field!(value, pass2),
            sprite: packed_field!(value, sprite),
            sound: packed_field!(value, sound),
            flags: packed_field!(value, flags),
            alignment: packed_field!(value, alignment),
            temple_x: packed_field!(value, temple_x),
            temple_y: packed_field!(value, temple_y),
            tavern_x: packed_field!(value, tavern_x),
            tavern_y: packed_field!(value, tavern_y),
            temp: packed_field!(value, temp),
            attrib: packed_field!(value, attrib),
            hp: packed_field!(value, hp),
            end: packed_field!(value, end),
            mana: packed_field!(value, mana),
            skill: packed_field!(value, skill),
            weapon_bonus: packed_field!(value, weapon_bonus),
            armor_bonus: packed_field!(value, armor_bonus),
            a_hp: packed_field!(value, a_hp),
            a_end: packed_field!(value, a_end),
            a_mana: packed_field!(value, a_mana),
            light: packed_field!(value, light),
            mode: packed_field!(value, mode),
            speed: packed_field!(value, speed),
            points: packed_field!(value, points),
            points_tot: packed_field!(value, points_tot),
            armor: packed_field!(value, armor),
            weapon: packed_field!(value, weapon),
            x: packed_field!(value, x),
            y: packed_field!(value, y),
            tox: packed_field!(value, tox),
            toy: packed_field!(value, toy),
            frx: packed_field!(value, frx),
            fry: packed_field!(value, fry),
            status: packed_field!(value, status),
            status2: packed_field!(value, status2),
            dir: packed_field!(value, dir),
            gold: packed_field!(value, gold),
            item: packed_field!(value, item),
            worn: packed_field!(value, worn),
            spell: packed_field!(value, spell),
            citem: packed_field!(value, citem),
            creation_date: packed_field!(value, creation_date),
            login_date: packed_field!(value, login_date),
            addr: packed_field!(value, addr),
            current_online_time: packed_field!(value, current_online_time),
            total_online_time: packed_field!(value, total_online_time),
            comp_volume: packed_field!(value, comp_volume),
            raw_volume: packed_field!(value, raw_volume),
            idle: packed_field!(value, idle),
            attack_cn: packed_field!(value, attack_cn),
            skill_nr: packed_field!(value, skill_nr),
            skill_target1: packed_field!(value, skill_target1),
            skill_target2: packed_field!(value, skill_target2),
            goto_x: packed_field!(value, goto_x),
            goto_y: packed_field!(value, goto_y),
            use_nr: packed_field!(value, use_nr),
            misc_action: packed_field!(value, misc_action),
            misc_target1: packed_field!(value, misc_target1),
            misc_target2: packed_field!(value, misc_target2),
            cerrno: packed_field!(value, cerrno),
            escape_timer: packed_field!(value, escape_timer),
            enemy: packed_field!(value, enemy),
            current_enemy: packed_field!(value, current_enemy),
            retry: packed_field!(value, retry),
            stunned: packed_field!(value, stunned),
            speed_mod: packed_field!(value, speed_mod),
            last_action: packed_field!(value, last_action),
            unused: packed_field!(value, unused),
            depot_sold: packed_field!(value, depot_sold),
            gethit_dam: packed_field!(value, gethit_dam),
            gethit_bonus: packed_field!(value, gethit_bonus),
            light_bonus: packed_field!(value, light_bonus),
            passwd: packed_field!(value, passwd),
            lastattack: packed_field!(value, lastattack),
            future1: packed_field!(value, future1),
            sprite_override: packed_field!(value, sprite_override),
            future2: packed_field!(value, future2),
            depot: packed_field!(value, depot),
            depot_cost: packed_field!(value, depot_cost),
            luck: packed_field!(value, luck),
            unreach: packed_field!(value, unreach),
            unreachx: packed_field!(value, unreachx),
            unreachy: packed_field!(value, unreachy),
            monster_class: packed_field!(value, monster_class),
            future3: packed_field!(value, future3),
            logout_date: packed_field!(value, logout_date),
            data: packed_field!(value, data),
            text: packed_field!(value, text),
        }
    }
}

impl From<Effect> for EffectPlain {
    fn from(value: Effect) -> Self {
        Self {
            used: packed_field!(value, used),
            flags: packed_field!(value, flags),
            effect_type: packed_field!(value, effect_type),
            duration: packed_field!(value, duration),
            data: packed_field!(value, data),
        }
    }
}

impl From<Global> for GlobalPlain {
    fn from(value: Global) -> Self {
        Self {
            mdtime: value.mdtime,
            mdday: value.mdday,
            mdyear: value.mdyear,
            dlight: value.dlight,
            players_created: value.players_created,
            npcs_created: value.npcs_created,
            players_died: value.players_died,
            npcs_died: value.npcs_died,
            character_cnt: value.character_cnt,
            item_cnt: value.item_cnt,
            effect_cnt: value.effect_cnt,
            expire_cnt: value.expire_cnt,
            expire_run: value.expire_run,
            gc_cnt: value.gc_cnt,
            gc_run: value.gc_run,
            lost_cnt: value.lost_cnt,
            lost_run: value.lost_run,
            reset_char: value.reset_char,
            reset_item: value.reset_item,
            ticker: value.ticker,
            total_online_time: value.total_online_time,
            online_per_hour: value.online_per_hour,
            flags: value.flags,
            uptime: value.uptime,
            uptime_per_hour: value.uptime_per_hour,
            awake: value.awake,
            body: value.body,
            players_online: value.players_online,
            queuesize: value.queuesize,
            recv: value.recv,
            send: value.send,
            transfer_reset_time: value.transfer_reset_time,
            load_avg: value.load_avg,
            load: value.load,
            max_online: value.max_online,
            max_online_per_hour: value.max_online_per_hour,
            fullmoon: value.fullmoon,
            newmoon: value.newmoon,
            unique: value.unique,
            cap: value.cap,
            original_bytes: Vec::new(),
            trailing_bytes: Vec::new(),
        }
    }
}

impl From<MapPlain> for Map {
    fn from(value: MapPlain) -> Self {
        Self {
            sprite: value.sprite,
            fsprite: value.fsprite,
            ch: value.ch,
            to_ch: value.to_ch,
            it: value.it,
            dlight: value.dlight,
            light: value.light,
            flags: value.flags,
        }
    }
}

impl From<ItemPlain> for Item {
    fn from(value: ItemPlain) -> Self {
        Self {
            used: value.used,
            name: value.name,
            reference: value.reference,
            description: value.description,
            flags: value.flags,
            value: value.value,
            placement: value.placement,
            temp: value.temp,
            damage_state: value.damage_state,
            max_age: value.max_age,
            current_age: value.current_age,
            max_damage: value.max_damage,
            current_damage: value.current_damage,
            attrib: value.attrib,
            hp: value.hp,
            end: value.end,
            mana: value.mana,
            skill: value.skill,
            armor: value.armor,
            weapon: value.weapon,
            light: value.light,
            duration: value.duration,
            cost: value.cost,
            power: value.power,
            active: value.active,
            x: value.x,
            y: value.y,
            carried: value.carried,
            sprite_override: value.sprite_override,
            sprite: value.sprite,
            status: value.status,
            gethit_dam: value.gethit_dam,
            min_rank: value.min_rank,
            future: value.future,
            future3: value.future3,
            t_bought: value.t_bought,
            t_sold: value.t_sold,
            driver: value.driver,
            data: value.data,
        }
    }
}

impl From<CharacterPlain> for Character {
    fn from(value: CharacterPlain) -> Self {
        Self {
            used: value.used,
            name: value.name,
            reference: value.reference,
            description: value.description,
            kindred: value.kindred,
            player: value.player,
            pass1: value.pass1,
            pass2: value.pass2,
            sprite: value.sprite,
            sound: value.sound,
            flags: value.flags,
            alignment: value.alignment,
            temple_x: value.temple_x,
            temple_y: value.temple_y,
            tavern_x: value.tavern_x,
            tavern_y: value.tavern_y,
            temp: value.temp,
            attrib: value.attrib,
            hp: value.hp,
            end: value.end,
            mana: value.mana,
            skill: value.skill,
            weapon_bonus: value.weapon_bonus,
            armor_bonus: value.armor_bonus,
            a_hp: value.a_hp,
            a_end: value.a_end,
            a_mana: value.a_mana,
            light: value.light,
            mode: value.mode,
            speed: value.speed,
            points: value.points,
            points_tot: value.points_tot,
            armor: value.armor,
            weapon: value.weapon,
            x: value.x,
            y: value.y,
            tox: value.tox,
            toy: value.toy,
            frx: value.frx,
            fry: value.fry,
            status: value.status,
            status2: value.status2,
            dir: value.dir,
            gold: value.gold,
            item: value.item,
            worn: value.worn,
            spell: value.spell,
            citem: value.citem,
            creation_date: value.creation_date,
            login_date: value.login_date,
            addr: value.addr,
            current_online_time: value.current_online_time,
            total_online_time: value.total_online_time,
            comp_volume: value.comp_volume,
            raw_volume: value.raw_volume,
            idle: value.idle,
            attack_cn: value.attack_cn,
            skill_nr: value.skill_nr,
            skill_target1: value.skill_target1,
            skill_target2: value.skill_target2,
            goto_x: value.goto_x,
            goto_y: value.goto_y,
            use_nr: value.use_nr,
            misc_action: value.misc_action,
            misc_target1: value.misc_target1,
            misc_target2: value.misc_target2,
            cerrno: value.cerrno,
            escape_timer: value.escape_timer,
            enemy: value.enemy,
            current_enemy: value.current_enemy,
            retry: value.retry,
            stunned: value.stunned,
            speed_mod: value.speed_mod,
            last_action: value.last_action,
            unused: value.unused,
            depot_sold: value.depot_sold,
            gethit_dam: value.gethit_dam,
            gethit_bonus: value.gethit_bonus,
            light_bonus: value.light_bonus,
            passwd: value.passwd,
            lastattack: value.lastattack,
            future1: value.future1,
            sprite_override: value.sprite_override,
            future2: value.future2,
            depot: value.depot,
            depot_cost: value.depot_cost,
            luck: value.luck,
            unreach: value.unreach,
            unreachx: value.unreachx,
            unreachy: value.unreachy,
            monster_class: value.monster_class,
            future3: value.future3,
            logout_date: value.logout_date,
            data: value.data,
            text: value.text,
        }
    }
}

impl From<EffectPlain> for Effect {
    fn from(value: EffectPlain) -> Self {
        Self {
            used: value.used,
            flags: value.flags,
            effect_type: value.effect_type,
            duration: value.duration,
            data: value.data,
        }
    }
}

impl From<GlobalPlain> for Global {
    fn from(value: GlobalPlain) -> Self {
        Self {
            mdtime: value.mdtime,
            mdday: value.mdday,
            mdyear: value.mdyear,
            dlight: value.dlight,
            players_created: value.players_created,
            npcs_created: value.npcs_created,
            players_died: value.players_died,
            npcs_died: value.npcs_died,
            character_cnt: value.character_cnt,
            item_cnt: value.item_cnt,
            effect_cnt: value.effect_cnt,
            expire_cnt: value.expire_cnt,
            expire_run: value.expire_run,
            gc_cnt: value.gc_cnt,
            gc_run: value.gc_run,
            lost_cnt: value.lost_cnt,
            lost_run: value.lost_run,
            reset_char: value.reset_char,
            reset_item: value.reset_item,
            ticker: value.ticker,
            total_online_time: value.total_online_time,
            online_per_hour: value.online_per_hour,
            flags: value.flags,
            uptime: value.uptime,
            uptime_per_hour: value.uptime_per_hour,
            awake: value.awake,
            body: value.body,
            players_online: value.players_online,
            queuesize: value.queuesize,
            recv: value.recv,
            send: value.send,
            transfer_reset_time: value.transfer_reset_time,
            load_avg: value.load_avg,
            load: value.load,
            max_online: value.max_online,
            max_online_per_hour: value.max_online_per_hour,
            fullmoon: value.fullmoon,
            newmoon: value.newmoon,
            unique: value.unique,
            cap: value.cap,
        }
    }
}

fn default_dat_dir() -> Option<PathBuf> {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidate = crate_dir.join("../assets/.dat");
    if candidate.is_dir() {
        return Some(candidate);
    }

    None
}

fn usage() -> &'static str {
    "Usage:\n  cargo run --package server-utils --bin dat_normalizer -- [--dat-dir <path>] [--in-place] [--reverse]\n\nModes:\n  default            legacy packed .dat -> normalized bincode payload\n  --reverse          normalized bincode payload -> legacy packed .dat\n\nOptions:\n  --dat-dir <path>   Path to .dat directory (defaults to server/assets/.dat when present)\n  --in-place         Replace files in place with backup files\n  --reverse          Run reverse conversion\n  --help             Print this help"
}

fn parse_args() -> Result<(PathBuf, bool, bool)> {
    let mut dat_dir: Option<PathBuf> = None;
    let mut in_place = false;
    let mut reverse = false;

    let mut args = env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--help" || arg == "-h" {
            println!("{}", usage());
            std::process::exit(0);
        }

        if arg == "--in-place" {
            in_place = true;
            continue;
        }

        if arg == "--reverse" {
            reverse = true;
            continue;
        }

        if arg == "--dat-dir" || arg == "--data-dir" {
            let Some(path) = args.next() else {
                return Err(anyhow!("missing value for --dat-dir"));
            };
            dat_dir = Some(PathBuf::from(path));
            continue;
        }

        let path = PathBuf::from(&arg);
        if path.is_dir() {
            dat_dir = Some(path);
            continue;
        }

        return Err(anyhow!("unknown argument: {:?}", arg));
    }

    let dat_dir = dat_dir
        .or_else(default_dat_dir)
        .ok_or_else(|| anyhow!("unable to determine .dat directory; pass --dat-dir <path>"))?;

    if !dat_dir.is_dir() {
        return Err(anyhow!(
            "data directory does not exist: {}",
            dat_dir.display()
        ));
    }

    Ok((dat_dir, in_place, reverse))
}

fn output_path_for(input: &Path, in_place: bool) -> PathBuf {
    if in_place {
        input.to_path_buf()
    } else {
        PathBuf::from(format!("{}.normalized", input.display()))
    }
}

fn normalized_input_path(dat_dir: &Path, file_name: &str, in_place: bool) -> PathBuf {
    if in_place {
        dat_dir.join(file_name)
    } else {
        PathBuf::from(format!("{}.normalized", dat_dir.join(file_name).display()))
    }
}

fn restored_output_path(dat_dir: &Path, file_name: &str, in_place: bool) -> PathBuf {
    if in_place {
        dat_dir.join(file_name)
    } else {
        PathBuf::from(format!("{}.restored", dat_dir.join(file_name).display()))
    }
}

fn write_output<T: Encode>(input_path: &Path, in_place: bool, payload: &T) -> Result<()> {
    let bytes = bincode::encode_to_vec(payload, bincode::config::standard())
        .context("failed to serialize normalized output")?;

    if in_place {
        let backup_path = PathBuf::from(format!("{}.legacy", input_path.display()));
        if backup_path.exists() {
            return Err(anyhow!(
                "backup file already exists, refusing to overwrite: {}",
                backup_path.display()
            ));
        }
        fs::copy(input_path, &backup_path).with_context(|| {
            format!(
                "failed to create backup for {} at {}",
                input_path.display(),
                backup_path.display()
            )
        })?;
        fs::write(input_path, bytes)
            .with_context(|| format!("failed to write in-place file {}", input_path.display()))?;
        println!(
            "converted {} (backup: {})",
            input_path.display(),
            backup_path.display()
        );
        return Ok(());
    }

    let output_path = output_path_for(input_path, in_place);
    fs::write(&output_path, bytes)
        .with_context(|| format!("failed to write {}", output_path.display()))?;
    println!("wrote {}", output_path.display());
    Ok(())
}

fn parse_fixed_records<T>(
    path: &Path,
    expected_count: usize,
    parse_one: impl Fn(&[u8]) -> Option<T>,
) -> Result<Vec<T>> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let record_size = std::mem::size_of::<T>();

    if bytes.len() != expected_count * record_size {
        return Err(anyhow!(
            "unexpected size for {}: got {} bytes, expected {} ({} records x {})",
            path.display(),
            bytes.len(),
            expected_count * record_size,
            expected_count,
            record_size
        ));
    }

    let mut out = Vec::with_capacity(expected_count);
    for (idx, chunk) in bytes.chunks_exact(record_size).enumerate() {
        let parsed = parse_one(chunk)
            .ok_or_else(|| anyhow!("failed to parse record {} in {}", idx, path.display()))?;
        out.push(parsed);
    }

    Ok(out)
}

fn read_normalized<T: Decode<()>>(
    path: &Path,
    expected_source_file: &str,
    expected_source_record_size: usize,
    expected_record_count: usize,
) -> Result<Vec<T>> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let (payload, consumed): (NormalizedDataSet<T>, usize) =
        bincode::decode_from_slice(&bytes, bincode::config::standard())
            .context("failed to decode normalized payload")?;

    if consumed != bytes.len() {
        return Err(anyhow!(
            "normalized payload has trailing bytes in {}",
            path.display()
        ));
    }

    if payload.magic != *b"MAG2" {
        return Err(anyhow!(
            "unexpected payload magic in {}: {:?}",
            path.display(),
            payload.magic
        ));
    }

    if payload.version != 1 {
        return Err(anyhow!(
            "unsupported payload version in {}: {}",
            path.display(),
            payload.version
        ));
    }

    if payload.source_file != expected_source_file {
        return Err(anyhow!(
            "payload source_file mismatch in {}: got {}, expected {}",
            path.display(),
            payload.source_file,
            expected_source_file
        ));
    }

    if payload.source_record_size != expected_source_record_size {
        return Err(anyhow!(
            "payload source_record_size mismatch in {}: got {}, expected {}",
            path.display(),
            payload.source_record_size,
            expected_source_record_size
        ));
    }

    if payload.records.len() != expected_record_count {
        return Err(anyhow!(
            "payload record count mismatch in {}: got {}, expected {}",
            path.display(),
            payload.records.len(),
            expected_record_count
        ));
    }

    Ok(payload.records)
}

fn write_legacy_output(path: &Path, in_place: bool, bytes: &[u8]) -> Result<()> {
    if in_place {
        let backup_path = PathBuf::from(format!("{}.normalized.bak", path.display()));
        if backup_path.exists() {
            return Err(anyhow!(
                "backup file already exists, refusing to overwrite: {}",
                backup_path.display()
            ));
        }
        fs::copy(path, &backup_path).with_context(|| {
            format!(
                "failed to create backup for {} at {}",
                path.display(),
                backup_path.display()
            )
        })?;
        fs::write(path, bytes)
            .with_context(|| format!("failed to write in-place file {}", path.display()))?;
        println!(
            "restored {} (normalized backup: {})",
            path.display(),
            backup_path.display()
        );
        return Ok(());
    }

    fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))?;
    println!("wrote {}", path.display());
    Ok(())
}

fn convert_map(dat_dir: &Path, in_place: bool) -> Result<()> {
    let path = dat_dir.join("map.dat");
    let records = parse_fixed_records::<Map>(
        &path,
        (SERVER_MAPX as usize) * (SERVER_MAPY as usize),
        Map::from_bytes,
    )?
    .into_iter()
    .map(MapPlain::from)
    .collect::<Vec<_>>();

    let payload = NormalizedDataSet {
        magic: *b"MAG2",
        version: 1,
        source_file: "map.dat".to_string(),
        source_record_size: std::mem::size_of::<Map>(),
        records,
    };

    write_output(&path, in_place, &payload)
}

fn convert_items(dat_dir: &Path, in_place: bool, file_name: &str, expected: usize) -> Result<()> {
    let path = dat_dir.join(file_name);
    let records = parse_fixed_records::<Item>(&path, expected, Item::from_bytes)?
        .into_iter()
        .map(ItemPlain::from)
        .collect::<Vec<_>>();

    let payload = NormalizedDataSet {
        magic: *b"MAG2",
        version: 1,
        source_file: file_name.to_string(),
        source_record_size: std::mem::size_of::<Item>(),
        records,
    };

    write_output(&path, in_place, &payload)
}

fn convert_characters(
    dat_dir: &Path,
    in_place: bool,
    file_name: &str,
    expected: usize,
) -> Result<()> {
    let path = dat_dir.join(file_name);
    let records = parse_fixed_records::<Character>(&path, expected, Character::from_bytes)?
        .into_iter()
        .map(CharacterPlain::from)
        .collect::<Vec<_>>();

    let payload = NormalizedDataSet {
        magic: *b"MAG2",
        version: 1,
        source_file: file_name.to_string(),
        source_record_size: std::mem::size_of::<Character>(),
        records,
    };

    write_output(&path, in_place, &payload)
}

fn convert_effects(dat_dir: &Path, in_place: bool) -> Result<()> {
    let path = dat_dir.join("effect.dat");
    let records = parse_fixed_records::<Effect>(&path, MAXEFFECT, Effect::from_bytes)?
        .into_iter()
        .map(EffectPlain::from)
        .collect::<Vec<_>>();

    let payload = NormalizedDataSet {
        magic: *b"MAG2",
        version: 1,
        source_file: "effect.dat".to_string(),
        source_record_size: std::mem::size_of::<Effect>(),
        records,
    };

    write_output(&path, in_place, &payload)
}

fn convert_globals(dat_dir: &Path, in_place: bool) -> Result<()> {
    let path = dat_dir.join("global.dat");
    let bytes = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let global_size = std::mem::size_of::<Global>();
    if bytes.len() < global_size {
        return Err(anyhow!(
            "unexpected size for {}: got {} bytes, expected at least {}",
            path.display(),
            bytes.len(),
            global_size
        ));
    }

    let global = Global::from_bytes(&bytes[..global_size])
        .ok_or_else(|| anyhow!("failed to parse global.dat"))?;
    let mut plain = GlobalPlain::from(global);
    plain.original_bytes = bytes[..global_size].to_vec();
    plain.trailing_bytes = bytes[global_size..].to_vec();

    let payload = NormalizedDataSet {
        magic: *b"MAG2",
        version: 1,
        source_file: "global.dat".to_string(),
        source_record_size: std::mem::size_of::<Global>(),
        records: vec![plain],
    };

    write_output(&path, in_place, &payload)
}

fn reverse_map(dat_dir: &Path, in_place: bool) -> Result<()> {
    let source_path = normalized_input_path(dat_dir, "map.dat", in_place);
    let output_path = restored_output_path(dat_dir, "map.dat", in_place);

    let records = read_normalized::<MapPlain>(
        &source_path,
        "map.dat",
        std::mem::size_of::<Map>(),
        (SERVER_MAPX as usize) * (SERVER_MAPY as usize),
    )?;

    let mut out = Vec::with_capacity(records.len() * std::mem::size_of::<Map>());
    for record in records {
        out.extend_from_slice(&Map::from(record).to_bytes());
    }

    write_legacy_output(&output_path, in_place, &out)
}

fn reverse_items(dat_dir: &Path, in_place: bool, file_name: &str, expected: usize) -> Result<()> {
    let source_path = normalized_input_path(dat_dir, file_name, in_place);
    let output_path = restored_output_path(dat_dir, file_name, in_place);

    let records = read_normalized::<ItemPlain>(
        &source_path,
        file_name,
        std::mem::size_of::<Item>(),
        expected,
    )?;

    let mut out = Vec::with_capacity(records.len() * std::mem::size_of::<Item>());
    for record in records {
        out.extend_from_slice(&Item::from(record).to_bytes());
    }

    write_legacy_output(&output_path, in_place, &out)
}

fn reverse_characters(
    dat_dir: &Path,
    in_place: bool,
    file_name: &str,
    expected: usize,
) -> Result<()> {
    let source_path = normalized_input_path(dat_dir, file_name, in_place);
    let output_path = restored_output_path(dat_dir, file_name, in_place);

    let records = read_normalized::<CharacterPlain>(
        &source_path,
        file_name,
        std::mem::size_of::<Character>(),
        expected,
    )?;

    let mut out = Vec::with_capacity(records.len() * std::mem::size_of::<Character>());
    for record in records {
        out.extend_from_slice(&Character::from(record).to_bytes());
    }

    write_legacy_output(&output_path, in_place, &out)
}

fn reverse_effects(dat_dir: &Path, in_place: bool) -> Result<()> {
    let source_path = normalized_input_path(dat_dir, "effect.dat", in_place);
    let output_path = restored_output_path(dat_dir, "effect.dat", in_place);

    let records = read_normalized::<EffectPlain>(
        &source_path,
        "effect.dat",
        std::mem::size_of::<Effect>(),
        MAXEFFECT,
    )?;

    let mut out = Vec::with_capacity(records.len() * std::mem::size_of::<Effect>());
    for record in records {
        out.extend_from_slice(&Effect::from(record).to_bytes());
    }

    write_legacy_output(&output_path, in_place, &out)
}

fn reverse_globals(dat_dir: &Path, in_place: bool) -> Result<()> {
    let source_path = normalized_input_path(dat_dir, "global.dat", in_place);
    let output_path = restored_output_path(dat_dir, "global.dat", in_place);

    let records = read_normalized::<GlobalPlain>(
        &source_path,
        "global.dat",
        std::mem::size_of::<Global>(),
        1,
    )?;

    let global = records
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("global.dat normalized payload is empty"))?;
    let trailing_bytes = global.trailing_bytes.clone();
    let mut bytes = if global.original_bytes.is_empty() {
        Global::from(global).to_bytes()
    } else {
        global.original_bytes.clone()
    };
    bytes.extend_from_slice(&trailing_bytes);

    write_legacy_output(&output_path, in_place, &bytes)
}

fn main() -> Result<()> {
    let (dat_dir, in_place, reverse) = parse_args()?;

    println!("processing dat directory: {}", dat_dir.display());
    if reverse {
        if in_place {
            println!("mode: reverse in-place (normalized backups as *.normalized.bak)");
        } else {
            println!("mode: reverse side-by-side input (*.normalized) output (*.restored)");
        }

        reverse_map(&dat_dir, in_place)?;
        reverse_items(&dat_dir, in_place, "item.dat", MAXITEM)?;
        reverse_items(&dat_dir, in_place, "titem.dat", MAXTITEM)?;
        reverse_characters(&dat_dir, in_place, "char.dat", MAXCHARS)?;
        reverse_characters(&dat_dir, in_place, "tchar.dat", MAXTCHARS)?;
        reverse_effects(&dat_dir, in_place)?;
        reverse_globals(&dat_dir, in_place)?;
    } else {
        if in_place {
            println!("mode: normalize in-place (legacy backups as *.legacy)");
        } else {
            println!("mode: normalize side-by-side output (*.normalized)");
        }

        convert_map(&dat_dir, in_place)?;
        convert_items(&dat_dir, in_place, "item.dat", MAXITEM)?;
        convert_items(&dat_dir, in_place, "titem.dat", MAXTITEM)?;
        convert_characters(&dat_dir, in_place, "char.dat", MAXCHARS)?;
        convert_characters(&dat_dir, in_place, "tchar.dat", MAXTCHARS)?;
        convert_effects(&dat_dir, in_place)?;
        convert_globals(&dat_dir, in_place)?;
    }

    println!("done.");
    Ok(())
}
