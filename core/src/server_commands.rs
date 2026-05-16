use crate::string_operations::c_string_to_str;

/// Opcode values for incoming server commands.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ServerCommandType {
    Empty = 0,
    SetCharName1 = 3,
    SetCharName2 = 4,
    SetCharName3 = 5,
    SetCharMode = 6,
    SetCharAttrib = 7,
    SetCharSkill = 8,
    SetCharHp = 12,
    SetCharEndur = 13,
    SetCharMana = 14,
    SetCharAHP = 20,
    SetCharPts = 21,
    SetCharGold = 22,
    SetCharItem = 23,
    SetCharWorn = 24,
    SetCharObj = 25,
    Tick = 27,
    Look1 = 29,
    ScrollRight = 30,
    ScrollLeft = 31,
    ScrollUp = 32,
    ScrollDown = 33,
    LoginOk = 34,
    ScrollRightUp = 35,
    ScrollRightDown = 36,
    ScrollLeftUp = 37,
    ScrollLeftDown = 38,
    Look2 = 39,
    Look3 = 40,
    Look4 = 41,
    SetTarget = 42,
    SetMap2 = 43,
    SetOrigin = 44,
    SetMap3 = 45,
    SetCharSpell = 46,
    PlaySound = 47,
    Exit = 48,
    Msg = 49,
    Look5 = 50,
    Look6 = 51,
    Log0 = 52,
    Log1 = 53,
    Log2 = 54,
    Log3 = 55,
    Load = 56,
    Cap = 57,
    Mod1 = 58,
    Mod2 = 59,
    Mod3 = 60,
    Mod4 = 61,
    Mod5 = 62,
    Mod6 = 63,
    Mod7 = 64,
    Mod8 = 65,
    SetMap4 = 66,
    SetMap5 = 67,
    SetMap6 = 68,
    SetCharAEnd = 69,
    SetCharAMana = 70,
    SetCharDir = 71,
    Ignore = 73,
    Pong = 74,
    /// Full snapshot of the character's 25-byte packed talent state.
    ///
    /// Wire format: opcode (1 byte) + 25 bytes copied directly from
    /// the server's packed `Character::future1` talent state.
    ///
    /// Total length: 26 bytes.  See `client::network::tick_stream::sv_cmd_len`.
    SetCharTalents = 75,
    /// Per-player weather / ambient effect state.
    ///
    /// Wire format: opcode (1) + kind (1) + intensity (1) + duration_ticks
    /// (u16 LE) + tint_r (1) + tint_g (1) + tint_b (1) + tint_a (1) + flags
    /// (1) = **10 bytes total**. See [`crate::weather::WeatherKind`].
    SetWeather = 76,
    /// Per-player quest log snapshot.
    ///
    /// Wire format: opcode (1) + count (1) + 16 × entry (48 bytes each)
    /// + active_template_id (u16, 2) + active_step_idx (u8, 1)
    /// + active_npc_x (u16, 2) + active_npc_y (u16, 2) = **777 bytes total**.
    ///
    /// Each entry is laid out as: `[u16 npc_template_id, u16 npc_x, u16 npc_y,
    /// u16 item_template_id, [u8; 16] npc_name, [u8; 24] item_name]` with the
    /// name buffers NUL-padded. Entries beyond `count` are zero padding.
    /// `active_template_id == 0` means "no active quest".
    SetQuestLog = 77,
    SetMap = 128,
}

/// Computes the total byte length of a variable-length `SV_SETMAP` command
/// given its flags byte and delta offset.
///
/// # Arguments
/// * `bytes` - The raw command bytes (starting at the opcode).
/// * `off` - The delta offset from the opcode (0 = absolute index follows).
/// * `lastn` - Mutable tracker for the last absolute tile index.
///
/// # Returns
/// * `Ok(length)` — the number of bytes this command occupies.
/// * `Err(msg)` on truncated input.
fn sv_setmap_len(bytes: &[u8], off: u8, lastn: &mut i32) -> Result<usize, String> {
    if bytes.len() < 2 {
        return Err("SV_SETMAP truncated (need at least 2 bytes)".to_owned());
    }

    let mut p: usize;
    let n: i32;
    if off != 0 {
        n = *lastn + i32::from(off);
        p = 2;
    } else {
        if bytes.len() < 4 {
            return Err("SV_SETMAP truncated (need 4 bytes for index)".to_owned());
        }
        n = i32::from(u16::from_le_bytes([bytes[2], bytes[3]]));
        p = 4;
    }

    *lastn = n;

    let flags = bytes[1];
    if flags == 0 {
        return Err("SV_SETMAP has zero flags".to_owned());
    }

    if flags & 1 != 0 {
        p += 2;
    }
    if flags & 2 != 0 {
        p += 4;
    }
    if flags & 4 != 0 {
        p += 4;
    }
    if flags & 8 != 0 {
        p += 2;
    }
    if flags & 16 != 0 {
        p += 1;
    }
    if flags & 32 != 0 {
        p += 4;
    }
    if flags & 64 != 0 {
        p += 5;
    }
    if flags & 128 != 0 {
        p += 1;
    }

    Ok(p)
}

/// Returns the byte length of a `SV_SETMAP3`-style lighting command with the
/// given tile count.
///
/// The new light packet format is: `[opcode, idx_lo, idx_hi, base_light, nibble_pairs...]`
/// — a 4-byte header followed by `cnt / 2` nibble-pair bytes.
///
/// Opcode-->cnt mapping (matching server-side `cl_light_*` functions):
/// * `SV_SETMAP4` (`cl_light_one`, 1 tile)  --> `sv_setmap3_len(0)` = 4
/// * `SV_SETMAP5` (`cl_light_three`, 3 tiles) --> `sv_setmap3_len(2)` = 5
/// * `SV_SETMAP6` (`cl_light_seven`, 7 tiles) --> `sv_setmap3_len(6)` = 7
/// * `SV_SETMAP3` (`cl_light_26`, 26 tiles) --> `sv_setmap3_len(26)` = 17
fn sv_setmap3_len(cnt: usize) -> usize {
    4 + (cnt / 2)
}

impl ServerCommandType {
    pub fn get_expected_length(bytes: &[u8], last_setmap_n: &mut i32) -> Result<usize, String> {
        if bytes.is_empty() {
            return Err("sv_cmd_len called with empty buffer".to_owned());
        }

        let op = bytes[0];

        if (op & ServerCommandType::SetMap as u8) != 0 {
            let off = op & !(ServerCommandType::SetMap as u8);
            return sv_setmap_len(bytes, off, last_setmap_n);
        }

        let parsed_op = ServerCommandType::from(op);

        let len = match parsed_op {
            ServerCommandType::SetCharMode => 2,
            ServerCommandType::SetCharAttrib => 8,
            ServerCommandType::SetCharSkill => 8,
            ServerCommandType::SetCharHp => 13,
            ServerCommandType::SetCharEndur => 13,
            ServerCommandType::SetCharMana => 13,
            ServerCommandType::SetCharAHP => 3,
            ServerCommandType::SetCharAEnd => 3,
            ServerCommandType::SetCharAMana => 3,
            ServerCommandType::SetCharDir => 2,
            ServerCommandType::SetCharTalents => 26,
            ServerCommandType::SetWeather => 10,
            ServerCommandType::SetQuestLog => 777,
            ServerCommandType::SetCharPts => 13,
            ServerCommandType::SetCharGold => 13,
            ServerCommandType::SetCharItem => 9,
            ServerCommandType::SetCharWorn => 9,
            ServerCommandType::SetCharSpell => 11,
            ServerCommandType::SetCharObj => 5,
            ServerCommandType::SetMap3 => sv_setmap3_len(26),
            ServerCommandType::SetMap4 => sv_setmap3_len(0),
            ServerCommandType::SetMap5 => sv_setmap3_len(2),
            ServerCommandType::SetMap6 => sv_setmap3_len(6),
            ServerCommandType::SetOrigin => 5,
            ServerCommandType::Tick => 2,
            ServerCommandType::ScrollRight => 1,
            ServerCommandType::ScrollLeft => 1,
            ServerCommandType::ScrollDown => 1,
            ServerCommandType::ScrollUp => 1,
            ServerCommandType::ScrollRightDown => 1,
            ServerCommandType::ScrollRightUp => 1,
            ServerCommandType::ScrollLeftDown => 1,
            ServerCommandType::ScrollLeftUp => 1,
            ServerCommandType::SetTarget => 13,
            ServerCommandType::PlaySound => 13,
            ServerCommandType::Load => 5,
            ServerCommandType::Exit => {
                if bytes.len() >= 16 {
                    16
                } else {
                    5
                }
            }
            ServerCommandType::Ignore => {
                if bytes.len() < 5 {
                    return Err("SV_IGNORE truncated (need 5 bytes for size)".to_owned());
                }
                u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize
            }
            _ => 16,
        };

        Ok(len)
    }
}

impl From<u8> for ServerCommandType {
    fn from(value: u8) -> Self {
        match value {
            0 => ServerCommandType::Empty,
            3 => ServerCommandType::SetCharName1,
            4 => ServerCommandType::SetCharName2,
            5 => ServerCommandType::SetCharName3,
            6 => ServerCommandType::SetCharMode,
            7 => ServerCommandType::SetCharAttrib,
            8 => ServerCommandType::SetCharSkill,
            12 => ServerCommandType::SetCharHp,
            13 => ServerCommandType::SetCharEndur,
            14 => ServerCommandType::SetCharMana,
            20 => ServerCommandType::SetCharAHP,
            21 => ServerCommandType::SetCharPts,
            22 => ServerCommandType::SetCharGold,
            23 => ServerCommandType::SetCharItem,
            24 => ServerCommandType::SetCharWorn,
            25 => ServerCommandType::SetCharObj,
            27 => ServerCommandType::Tick,
            29 => ServerCommandType::Look1,
            30 => ServerCommandType::ScrollRight,
            31 => ServerCommandType::ScrollLeft,
            32 => ServerCommandType::ScrollUp,
            33 => ServerCommandType::ScrollDown,
            34 => ServerCommandType::LoginOk,
            35 => ServerCommandType::ScrollRightUp,
            36 => ServerCommandType::ScrollRightDown,
            37 => ServerCommandType::ScrollLeftUp,
            38 => ServerCommandType::ScrollLeftDown,
            39 => ServerCommandType::Look2,
            40 => ServerCommandType::Look3,
            41 => ServerCommandType::Look4,
            42 => ServerCommandType::SetTarget,
            43 => ServerCommandType::SetMap2,
            44 => ServerCommandType::SetOrigin,
            45 => ServerCommandType::SetMap3,
            46 => ServerCommandType::SetCharSpell,
            47 => ServerCommandType::PlaySound,
            48 => ServerCommandType::Exit,
            49 => ServerCommandType::Msg,
            50 => ServerCommandType::Look5,
            51 => ServerCommandType::Look6,
            52 => ServerCommandType::Log0,
            53 => ServerCommandType::Log1,
            54 => ServerCommandType::Log2,
            55 => ServerCommandType::Log3,
            56 => ServerCommandType::Load,
            57 => ServerCommandType::Cap,
            58 => ServerCommandType::Mod1,
            59 => ServerCommandType::Mod2,
            60 => ServerCommandType::Mod3,
            61 => ServerCommandType::Mod4,
            62 => ServerCommandType::Mod5,
            63 => ServerCommandType::Mod6,
            64 => ServerCommandType::Mod7,
            65 => ServerCommandType::Mod8,
            66 => ServerCommandType::SetMap4,
            67 => ServerCommandType::SetMap5,
            68 => ServerCommandType::SetMap6,
            69 => ServerCommandType::SetCharAEnd,
            70 => ServerCommandType::SetCharAMana,
            71 => ServerCommandType::SetCharDir,
            73 => ServerCommandType::Ignore,
            74 => ServerCommandType::Pong,
            75 => ServerCommandType::SetCharTalents,
            76 => ServerCommandType::SetWeather,
            77 => ServerCommandType::SetQuestLog,
            128 => ServerCommandType::SetMap,
            _ => {
                log::error!("Unknown server command opcode: {value}");
                ServerCommandType::Empty
            }
        }
    }
}

/// Maximum NPC name length carried in a [`QuestLogEntry`] (NUL-padded).
pub const QUEST_LOG_NPC_NAME_LEN: usize = 16;

/// Maximum item name length carried in a [`QuestLogEntry`] (NUL-padded).
pub const QUEST_LOG_ITEM_NAME_LEN: usize = 24;

/// On-wire size of a single quest log entry inside `SetQuestLog`.
///
/// Layout: `npc_template_id (u16) + npc_x (u16) + npc_y (u16) +
/// item_template_id (u16) + npc_name ([u8; 16]) + item_name ([u8; 24])`
/// = 48 bytes.
pub const QUEST_LOG_ENTRY_LEN: usize =
    2 + 2 + 2 + 2 + QUEST_LOG_NPC_NAME_LEN + QUEST_LOG_ITEM_NAME_LEN;

/// Maximum number of entries carried in a single `SetQuestLog` packet.
pub const QUEST_LOG_MAX_ENTRIES: usize = 16;

/// Total `SetQuestLog` packet size in bytes.
///
/// Layout: opcode (1) + count (1) + 16 × entry (48) + active_template_id
/// (u16) + active_step_idx (u8) + active_npc_x (u16) + active_npc_y (u16).
pub const QUEST_LOG_PACKET_LEN: usize =
    2 + QUEST_LOG_MAX_ENTRIES * QUEST_LOG_ENTRY_LEN + 2 + 1 + 2 + 2;

/// One quest entry inside a [`ServerCommandData::SetQuestLog`] payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestLogEntry {
    /// Template ID of the NPC quest giver.
    pub npc_template_id: u16,
    /// World tile X of the NPC quest giver.
    pub npc_x: u16,
    /// World tile Y of the NPC quest giver.
    pub npc_y: u16,
    /// Template ID of the item the NPC wants (0 when unknown).
    pub item_template_id: u16,
    /// Display name of the NPC quest giver (already trimmed of NUL padding).
    pub npc_name: String,
    /// Display name of the wanted item (already trimmed of NUL padding).
    pub item_name: String,
}

/// Parsed payload variants for each [`ServerCommandType`].
#[derive(Debug)]
pub enum ServerCommandData {
    Empty,
    Pong {
        seq: u32,
        #[allow(dead_code)]
        client_time_ms: u32,
    },
    SetMap {
        off: u8,
        absolute_tile_index: Option<u16>,
        flags: u8,
        ba_sprite: Option<u16>,
        flags1: Option<u32>,
        flags2: Option<u32>,
        it_sprite: Option<u16>,
        it_status: Option<u8>,
        ch_sprite: Option<u16>,
        ch_status: Option<u8>,
        ch_stat_off: Option<u8>,
        ch_nr: Option<u16>,
        ch_id: Option<u16>,
        ch_speed: Option<u8>,
        ch_proz: Option<u8>,
    },
    SetMap3 {
        start_index: u16,
        base_light: u8,
        packed: Vec<u8>,
    },
    SetCharName1 {
        chunk: String,
    },
    SetCharName2 {
        chunk: String,
    },
    SetCharName3 {
        chunk: String,
        #[allow(dead_code)]
        race: u32,
    },
    SetCharMode {
        mode: u8,
    },
    SetCharAttrib {
        index: u8,
        values: [u8; 6],
    },
    SetCharSkill {
        index: u8,
        values: [u8; 6],
    },
    SetCharHp {
        values: [u16; 6],
    },
    SetCharEndur {
        values: [i16; 6],
    },
    SetCharMana {
        values: [i16; 6],
    },
    SetCharAHP {
        value: u16,
    },
    SetCharAEnd {
        value: u16,
    },
    SetCharAMana {
        value: u16,
    },
    SetCharDir {
        dir: u8,
    },
    /// Full snapshot of the character's 25-byte packed talent state.
    ///
    /// `values[0]` is the unspent points pool; `values[1..24]` are the
    /// per-layer bit fields (8 nodes per byte).
    SetCharTalents {
        values: [u8; 25],
    },
    SetCharPts {
        points: u32,
        points_total: u32,
        kindred: u32,
    },
    SetCharGold {
        gold: u32,
        armor: u32,
        weapon: u32,
    },
    SetCharItem {
        index: u32,
        item: i16,
        item_p: i16,
    },
    SetCharWorn {
        index: u32,
        worn: i16,
        worn_p: i16,
    },
    SetCharSpell {
        index: u32,
        spell: i16,
        active: i16,
        /// Template number of the skill that created this effect (matches `SK_*` constants).
        skill_nr: i16,
    },
    SetCharObj {
        citem: i16,
        citem_p: i16,
    },
    Tick {
        ctick: u8,
    },
    SetOrigin {
        x: i16,
        y: i16,
    },
    Log {
        font: u8,
        chunk: String,
    },
    Look1 {
        worn0: u16,
        worn2: u16,
        worn3: u16,
        worn5: u16,
        worn6: u16,
        worn7: u16,
        worn8: u16,
        autoflag: u8,
    },
    Look2 {
        worn9: u16,
        sprite: u16,
        points: u32,
        hp: u32,
        worn10: u16,
    },
    Look3 {
        end: u16,
        a_hp: u16,
        a_end: u16,
        nr: u16,
        id: u16,
        mana: u16,
        a_mana: u16,
    },
    Look4 {
        worn1: u16,
        worn4: u16,
        extended: u8,
        pl_price: u32,
        worn11: u16,
        worn12: u16,
        worn13: u16,
    },
    Look5 {
        name: String,
    },
    Look6 {
        start: u8,
        entries: Vec<Look6Entry>,
    },
    SetTarget {
        attack_cn: u16,
        goto_x: u16,
        goto_y: u16,
        misc_action: u16,
        misc_target1: u16,
        misc_target2: u16,
    },
    PlaySound {
        nr: u32,
        vol: i32,
        pan: i32,
    },
    /// Per-player weather / ambient effect state.
    ///
    /// `kind` is a [`crate::weather::WeatherKind`] discriminant; `intensity`
    /// (0..=255) scales particle density and shake amplitude; `duration_ticks`
    /// of `0` means "persistent until replaced"; `tint` is RGBA where alpha
    /// `0` means "use the kind's default tint"; `flags` is a bitmask
    /// (`WEATHER_FLAG_*` in [`crate::weather`]).
    SetWeather {
        kind: u8,
        intensity: u8,
        duration_ticks: u16,
        tint: [u8; 4],
        flags: u8,
    },
    /// Per-player quest log snapshot.
    ///
    /// `entries` lists up to 16 active quest givers, including the NPC name,
    /// the template ID of the wanted item and the item's name (already
    /// resolved server-side for display). `active_template_id` is the NPC the
    /// player has currently focused (0 = none). `active_step_idx` is the
    /// current step index for the active quest; `active_npc_x` /
    /// `active_npc_y` echo the active NPC's tile so the minimap can pin a
    /// "return to giver" step without a separate lookup.
    SetQuestLog {
        entries: Vec<QuestLogEntry>,
        active_template_id: u16,
        active_step_idx: u8,
        active_npc_x: u16,
        active_npc_y: u16,
    },
    Load {
        load: u32,
    },
    Ignore {
        _size: u32,
    },
    LoginOk {
        server_version: u32,
    },
    Mod1 {
        text: String,
    },
    Mod2 {
        text: String,
    },
    Mod3 {
        text: String,
    },
    Mod4 {
        text: String,
    },
    Mod5 {
        text: String,
    },
    Mod6 {
        text: String,
    },
    Mod7 {
        text: String,
    },
    Mod8 {
        text: String,
    },
    Exit {
        reason: u32,
    },
}

#[derive(Debug)]
pub struct Look6Entry {
    pub index: u8,
    pub item: u16,
    pub price: u32,
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn read_i16(bytes: &[u8], offset: usize) -> Option<i16> {
    Some(i16::from_le_bytes(
        bytes.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn read_i32(bytes: &[u8], offset: usize) -> Option<i32> {
    Some(i32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn from_bytes(bytes: &[u8]) -> Option<(ServerCommandType, ServerCommandData)> {
    if bytes.is_empty() {
        return None;
    }

    if (bytes[0] & 128) != 0 {
        let off = bytes[0] & 127;
        let flags = *bytes.get(1)?;

        let mut p = if off == 0 { 4 } else { 2 };
        let absolute_tile_index = if off == 0 {
            Some(read_u16(bytes, 2)?)
        } else {
            None
        };

        let mut ba_sprite = None;
        let mut flags1 = None;
        let mut flags2 = None;
        let mut it_sprite = None;
        let mut it_status = None;
        let mut ch_sprite = None;
        let mut ch_status = None;
        let mut ch_stat_off = None;
        let mut ch_nr = None;
        let mut ch_id = None;
        let mut ch_speed = None;
        let mut ch_proz = None;

        if (flags & 1) != 0 {
            ba_sprite = Some(read_u16(bytes, p)?);
            p += 2;
        }
        if (flags & 2) != 0 {
            flags1 = Some(read_u32(bytes, p)?);
            p += 4;
        }
        if (flags & 4) != 0 {
            flags2 = Some(read_u32(bytes, p)?);
            p += 4;
        }
        if (flags & 8) != 0 {
            it_sprite = Some(read_u16(bytes, p)?);
            p += 2;
        }
        if (flags & 16) != 0 {
            it_status = Some(*bytes.get(p)?);
            p += 1;
        }
        if (flags & 32) != 0 {
            ch_sprite = Some(read_u16(bytes, p)?);
            p += 2;
            ch_status = Some(*bytes.get(p)?);
            p += 1;
            ch_stat_off = Some(*bytes.get(p)?);
            p += 1;
        }
        if (flags & 64) != 0 {
            ch_nr = Some(read_u16(bytes, p)?);
            p += 2;
            ch_id = Some(read_u16(bytes, p)?);
            p += 2;
            ch_speed = Some(*bytes.get(p)?);
            p += 1;
        }
        if (flags & 128) != 0 {
            ch_proz = Some(*bytes.get(p)?);
        }

        return Some((
            ServerCommandType::SetMap,
            ServerCommandData::SetMap {
                off,
                absolute_tile_index,
                flags,
                ba_sprite,
                flags1,
                flags2,
                it_sprite,
                it_status,
                ch_sprite,
                ch_status,
                ch_stat_off,
                ch_nr,
                ch_id,
                ch_speed,
                ch_proz,
            },
        ));
    }

    match bytes[0] {
        0 => Some((ServerCommandType::Empty, ServerCommandData::Empty)),
        3 => Some((
            ServerCommandType::SetCharName1,
            ServerCommandData::SetCharName1 {
                chunk: c_string_to_str(bytes.get(1..16)?).to_owned(),
            },
        )),
        4 => Some((
            ServerCommandType::SetCharName2,
            ServerCommandData::SetCharName2 {
                chunk: c_string_to_str(bytes.get(1..16)?).to_owned(),
            },
        )),
        5 => Some((
            ServerCommandType::SetCharName3,
            ServerCommandData::SetCharName3 {
                chunk: c_string_to_str(bytes.get(1..11)?).to_owned(),
                race: read_u32(bytes, 11)?,
            },
        )),
        6 => Some((
            ServerCommandType::SetCharMode,
            ServerCommandData::SetCharMode {
                mode: *bytes.get(1)?,
            },
        )),
        7 => Some((
            ServerCommandType::SetCharAttrib,
            ServerCommandData::SetCharAttrib {
                index: *bytes.get(1)?,
                values: bytes.get(2..8)?.try_into().ok()?,
            },
        )),
        8 => Some((
            ServerCommandType::SetCharSkill,
            ServerCommandData::SetCharSkill {
                index: *bytes.get(1)?,
                values: bytes.get(2..8)?.try_into().ok()?,
            },
        )),
        12 => Some((
            ServerCommandType::SetCharHp,
            ServerCommandData::SetCharHp {
                values: [
                    read_u16(bytes, 1)?,
                    read_u16(bytes, 3)?,
                    read_u16(bytes, 5)?,
                    read_u16(bytes, 7)?,
                    read_u16(bytes, 9)?,
                    read_u16(bytes, 11)?,
                ],
            },
        )),
        13 => Some((
            ServerCommandType::SetCharEndur,
            ServerCommandData::SetCharEndur {
                values: [
                    read_i16(bytes, 1)?,
                    read_i16(bytes, 3)?,
                    read_i16(bytes, 5)?,
                    read_i16(bytes, 7)?,
                    read_i16(bytes, 9)?,
                    read_i16(bytes, 11)?,
                ],
            },
        )),
        14 => Some((
            ServerCommandType::SetCharMana,
            ServerCommandData::SetCharMana {
                values: [
                    read_i16(bytes, 1)?,
                    read_i16(bytes, 3)?,
                    read_i16(bytes, 5)?,
                    read_i16(bytes, 7)?,
                    read_i16(bytes, 9)?,
                    read_i16(bytes, 11)?,
                ],
            },
        )),
        20 => Some((
            ServerCommandType::SetCharAHP,
            ServerCommandData::SetCharAHP {
                value: read_u16(bytes, 1)?,
            },
        )),
        21 => Some((
            ServerCommandType::SetCharPts,
            ServerCommandData::SetCharPts {
                points: read_u32(bytes, 1)?,
                points_total: read_u32(bytes, 5)?,
                kindred: read_u32(bytes, 9)?,
            },
        )),
        22 => Some((
            ServerCommandType::SetCharGold,
            ServerCommandData::SetCharGold {
                gold: read_u32(bytes, 1)?,
                armor: read_u32(bytes, 5)?,
                weapon: read_u32(bytes, 9)?,
            },
        )),
        23 => Some((
            ServerCommandType::SetCharItem,
            ServerCommandData::SetCharItem {
                index: read_u32(bytes, 1)?,
                item: read_i16(bytes, 5)?,
                item_p: read_i16(bytes, 7)?,
            },
        )),
        24 => Some((
            ServerCommandType::SetCharWorn,
            ServerCommandData::SetCharWorn {
                index: read_u32(bytes, 1)?,
                worn: read_i16(bytes, 5)?,
                worn_p: read_i16(bytes, 7)?,
            },
        )),
        25 => Some((
            ServerCommandType::SetCharObj,
            ServerCommandData::SetCharObj {
                citem: read_i16(bytes, 1)?,
                citem_p: read_i16(bytes, 3)?,
            },
        )),
        27 => Some((
            ServerCommandType::Tick,
            ServerCommandData::Tick {
                ctick: *bytes.get(1)?,
            },
        )),
        29 => Some((
            ServerCommandType::Look1,
            ServerCommandData::Look1 {
                worn0: read_u16(bytes, 1)?,
                worn2: read_u16(bytes, 3)?,
                worn3: read_u16(bytes, 5)?,
                worn5: read_u16(bytes, 7)?,
                worn6: read_u16(bytes, 9)?,
                worn7: read_u16(bytes, 11)?,
                worn8: read_u16(bytes, 13)?,
                autoflag: *bytes.get(15)?,
            },
        )),
        30 => Some((ServerCommandType::ScrollRight, ServerCommandData::Empty)),
        31 => Some((ServerCommandType::ScrollLeft, ServerCommandData::Empty)),
        32 => Some((ServerCommandType::ScrollUp, ServerCommandData::Empty)),
        33 => Some((ServerCommandType::ScrollDown, ServerCommandData::Empty)),
        34 => Some((
            ServerCommandType::LoginOk,
            ServerCommandData::LoginOk {
                server_version: u32::from_le_bytes(bytes.get(1..5)?.try_into().ok()?),
            },
        )),
        35 => Some((ServerCommandType::ScrollRightUp, ServerCommandData::Empty)),
        36 => Some((ServerCommandType::ScrollRightDown, ServerCommandData::Empty)),
        37 => Some((ServerCommandType::ScrollLeftUp, ServerCommandData::Empty)),
        38 => Some((ServerCommandType::ScrollLeftDown, ServerCommandData::Empty)),
        39 => Some((
            ServerCommandType::Look2,
            ServerCommandData::Look2 {
                worn9: read_u16(bytes, 1)?,
                sprite: read_u16(bytes, 3)?,
                points: read_u32(bytes, 5)?,
                hp: read_u32(bytes, 9)?,
                worn10: read_u16(bytes, 13)?,
            },
        )),
        40 => Some((
            ServerCommandType::Look3,
            ServerCommandData::Look3 {
                end: read_u16(bytes, 1)?,
                a_hp: read_u16(bytes, 3)?,
                a_end: read_u16(bytes, 5)?,
                nr: read_u16(bytes, 7)?,
                id: read_u16(bytes, 9)?,
                mana: read_u16(bytes, 11)?,
                a_mana: read_u16(bytes, 13)?,
            },
        )),
        41 => Some((
            ServerCommandType::Look4,
            ServerCommandData::Look4 {
                worn1: read_u16(bytes, 1)?,
                worn4: read_u16(bytes, 3)?,
                extended: *bytes.get(5)?,
                pl_price: read_u32(bytes, 6)?,
                worn11: read_u16(bytes, 10)?,
                worn12: read_u16(bytes, 12)?,
                worn13: read_u16(bytes, 14)?,
            },
        )),
        42 => Some((
            ServerCommandType::SetTarget,
            ServerCommandData::SetTarget {
                attack_cn: read_u16(bytes, 1)?,
                goto_x: read_u16(bytes, 3)?,
                goto_y: read_u16(bytes, 5)?,
                misc_action: read_u16(bytes, 7)?,
                misc_target1: read_u16(bytes, 9)?,
                misc_target2: read_u16(bytes, 11)?,
            },
        )),
        43 => Some((ServerCommandType::SetMap2, ServerCommandData::Empty)),
        44 => Some((
            ServerCommandType::SetOrigin,
            ServerCommandData::SetOrigin {
                x: read_i16(bytes, 1)?,
                y: read_i16(bytes, 3)?,
            },
        )),
        45 => {
            // Packet layout: [cmd, idx_lo, idx_hi, light, nibble_pairs...]
            let start_index = read_u16(bytes, 1)?;
            let base_light = *bytes.get(3)? & 0x0f;
            Some((
                ServerCommandType::SetMap3,
                ServerCommandData::SetMap3 {
                    start_index,
                    base_light,
                    packed: bytes.get(4..)?.to_vec(),
                },
            ))
        }
        46 => Some((
            ServerCommandType::SetCharSpell,
            ServerCommandData::SetCharSpell {
                index: read_u32(bytes, 1)?,
                spell: read_i16(bytes, 5)?,
                active: read_i16(bytes, 7)?,
                skill_nr: read_i16(bytes, 9)?,
            },
        )),
        47 => Some((
            ServerCommandType::PlaySound,
            ServerCommandData::PlaySound {
                nr: read_u32(bytes, 1)?,
                vol: read_i32(bytes, 5)?,
                pan: read_i32(bytes, 9)?,
            },
        )),
        48 => Some((
            ServerCommandType::Exit,
            ServerCommandData::Exit {
                reason: if bytes.len() >= 5 {
                    u32::from_le_bytes(bytes.get(1..5)?.try_into().ok()?)
                } else {
                    u32::from(*bytes.get(1)?)
                },
            },
        )),
        49 => Some((ServerCommandType::Msg, ServerCommandData::Empty)),
        50 => Some((
            ServerCommandType::Look5,
            ServerCommandData::Look5 {
                name: c_string_to_str(bytes.get(1..16)?).to_owned(),
            },
        )),
        51 => {
            let start = *bytes.get(1)?;
            let mut entries = Vec::new();
            let max = std::cmp::min(62u8, start.saturating_add(2));
            for (i, idx) in (start..max).enumerate() {
                let base = 2 + i * 6;
                let item = read_u16(bytes, base)?;
                let price = read_u32(bytes, base + 2)?;
                entries.push(Look6Entry {
                    index: idx,
                    item,
                    price,
                });
            }
            Some((
                ServerCommandType::Look6,
                ServerCommandData::Look6 { start, entries },
            ))
        }
        52 => Some((
            ServerCommandType::Log0,
            ServerCommandData::Log {
                font: 0,
                chunk: c_string_to_str(bytes.get(1..16)?).to_owned(),
            },
        )),
        53 => Some((
            ServerCommandType::Log1,
            ServerCommandData::Log {
                font: 1,
                chunk: c_string_to_str(bytes.get(1..16)?).to_owned(),
            },
        )),
        54 => Some((
            ServerCommandType::Log2,
            ServerCommandData::Log {
                font: 2,
                chunk: c_string_to_str(bytes.get(1..16)?).to_owned(),
            },
        )),
        55 => Some((
            ServerCommandType::Log3,
            ServerCommandData::Log {
                font: 3,
                chunk: c_string_to_str(bytes.get(1..16)?).to_owned(),
            },
        )),
        56 => Some((
            ServerCommandType::Load,
            ServerCommandData::Load {
                load: read_u32(bytes, 1)?,
            },
        )),
        57 => Some((ServerCommandType::Cap, ServerCommandData::Empty)),
        58 => Some((
            ServerCommandType::Mod1,
            ServerCommandData::Mod1 {
                text: c_string_to_str(bytes.get(1..16)?).to_owned(),
            },
        )),
        59 => Some((
            ServerCommandType::Mod2,
            ServerCommandData::Mod2 {
                text: c_string_to_str(bytes.get(1..16)?).to_owned(),
            },
        )),
        60 => Some((
            ServerCommandType::Mod3,
            ServerCommandData::Mod3 {
                text: c_string_to_str(bytes.get(1..16)?).to_owned(),
            },
        )),
        61 => Some((
            ServerCommandType::Mod4,
            ServerCommandData::Mod4 {
                text: c_string_to_str(bytes.get(1..16)?).to_owned(),
            },
        )),
        62 => Some((
            ServerCommandType::Mod5,
            ServerCommandData::Mod5 {
                text: c_string_to_str(bytes.get(1..16)?).to_owned(),
            },
        )),
        63 => Some((
            ServerCommandType::Mod6,
            ServerCommandData::Mod6 {
                text: c_string_to_str(bytes.get(1..16)?).to_owned(),
            },
        )),
        64 => Some((
            ServerCommandType::Mod7,
            ServerCommandData::Mod7 {
                text: c_string_to_str(bytes.get(1..16)?).to_owned(),
            },
        )),
        65 => Some((
            ServerCommandType::Mod8,
            ServerCommandData::Mod8 {
                text: c_string_to_str(bytes.get(1..16)?).to_owned(),
            },
        )),
        66 => {
            // Packet layout: [cmd, idx_lo, idx_hi, light, nibble_pairs...]
            Some((
                ServerCommandType::SetMap4,
                ServerCommandData::SetMap3 {
                    start_index: read_u16(bytes, 1)?,
                    base_light: *bytes.get(3)? & 0x0f,
                    packed: bytes.get(4..)?.to_vec(),
                },
            ))
        }
        67 => {
            // Packet layout: [cmd, idx_lo, idx_hi, light, nibble_pairs...]
            Some((
                ServerCommandType::SetMap5,
                ServerCommandData::SetMap3 {
                    start_index: read_u16(bytes, 1)?,
                    base_light: *bytes.get(3)? & 0x0f,
                    packed: bytes.get(4..)?.to_vec(),
                },
            ))
        }
        68 => {
            // Packet layout: [cmd, idx_lo, idx_hi, light, nibble_pairs...]
            Some((
                ServerCommandType::SetMap6,
                ServerCommandData::SetMap3 {
                    start_index: read_u16(bytes, 1)?,
                    base_light: *bytes.get(3)? & 0x0f,
                    packed: bytes.get(4..)?.to_vec(),
                },
            ))
        }
        69 => Some((
            ServerCommandType::SetCharAEnd,
            ServerCommandData::SetCharAEnd {
                value: read_u16(bytes, 1)?,
            },
        )),
        70 => Some((
            ServerCommandType::SetCharAMana,
            ServerCommandData::SetCharAMana {
                value: read_u16(bytes, 1)?,
            },
        )),
        71 => Some((
            ServerCommandType::SetCharDir,
            ServerCommandData::SetCharDir {
                dir: *bytes.get(1)?,
            },
        )),
        73 => Some((
            ServerCommandType::Ignore,
            ServerCommandData::Ignore {
                _size: read_u32(bytes, 1)?,
            },
        )),
        74 => Some((
            ServerCommandType::Pong,
            ServerCommandData::Pong {
                seq: read_u32(bytes, 1)?,
                client_time_ms: read_u32(bytes, 5)?,
            },
        )),
        75 => Some((
            ServerCommandType::SetCharTalents,
            ServerCommandData::SetCharTalents {
                values: bytes.get(1..26)?.try_into().ok()?,
            },
        )),
        76 => Some((
            ServerCommandType::SetWeather,
            ServerCommandData::SetWeather {
                kind: *bytes.get(1)?,
                intensity: *bytes.get(2)?,
                duration_ticks: read_u16(bytes, 3)?,
                tint: [
                    *bytes.get(5)?,
                    *bytes.get(6)?,
                    *bytes.get(7)?,
                    *bytes.get(8)?,
                ],
                flags: *bytes.get(9)?,
            },
        )),
        77 => {
            let count = (*bytes.get(1)?).min(QUEST_LOG_MAX_ENTRIES as u8) as usize;
            let mut entries = Vec::with_capacity(count);
            for i in 0..count {
                let off = 2 + i * QUEST_LOG_ENTRY_LEN;
                let npc_template_id = read_u16(bytes, off)?;
                let npc_x = read_u16(bytes, off + 2)?;
                let npc_y = read_u16(bytes, off + 4)?;
                let item_template_id = read_u16(bytes, off + 6)?;
                let npc_name_slice = bytes.get(off + 8..off + 8 + QUEST_LOG_NPC_NAME_LEN)?;
                let item_name_slice = bytes.get(
                    off + 8 + QUEST_LOG_NPC_NAME_LEN
                        ..off + 8 + QUEST_LOG_NPC_NAME_LEN + QUEST_LOG_ITEM_NAME_LEN,
                )?;
                entries.push(QuestLogEntry {
                    npc_template_id,
                    npc_x,
                    npc_y,
                    item_template_id,
                    npc_name: c_string_to_str(npc_name_slice).to_owned(),
                    item_name: c_string_to_str(item_name_slice).to_owned(),
                });
            }
            // Trailing fields sit immediately after the fixed 16-entry slot
            // table.
            let trailer_off = 2 + QUEST_LOG_MAX_ENTRIES * QUEST_LOG_ENTRY_LEN;
            let active_template_id = read_u16(bytes, trailer_off)?;
            let active_step_idx = *bytes.get(trailer_off + 2)?;
            let active_npc_x = read_u16(bytes, trailer_off + 3)?;
            let active_npc_y = read_u16(bytes, trailer_off + 5)?;
            Some((
                ServerCommandType::SetQuestLog,
                ServerCommandData::SetQuestLog {
                    entries,
                    active_template_id,
                    active_step_idx,
                    active_npc_x,
                    active_npc_y,
                },
            ))
        }
        _ => None,
    }
}

#[derive(Debug)]
pub struct ServerCommand {
    pub header: ServerCommandType,
    pub structured_data: ServerCommandData,
    pub _payload: Vec<u8>,
}

impl ServerCommand {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.is_empty() {
            return None;
        }
        let header = from_bytes(bytes)?;
        let _payload = bytes[1..].to_vec();
        Some(ServerCommand {
            header: header.0,
            structured_data: header.1,
            _payload,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_packet(opcode: u8, payload: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0u8; 16];
        bytes[0] = opcode;
        for (i, &b) in payload.iter().enumerate() {
            if i + 1 < 16 {
                bytes[i + 1] = b;
            }
        }
        bytes
    }

    #[test]
    fn parse_empty_opcode() {
        let pkt = make_packet(0, &[]);
        let cmd = ServerCommand::from_bytes(&pkt).unwrap();
        assert!(matches!(cmd.structured_data, ServerCommandData::Empty));
    }

    #[test]
    fn parse_tick() {
        let mut pkt = vec![0u8; 16];
        pkt[0] = 27; // Tick opcode
        pkt[1] = 42; // ctick value
        let cmd = ServerCommand::from_bytes(&pkt).unwrap();
        match cmd.structured_data {
            ServerCommandData::Tick { ctick } => assert_eq!(ctick, 42),
            _ => panic!("Expected Tick variant"),
        }
    }

    #[test]
    fn parse_set_char_mode() {
        let pkt = make_packet(6, &[3]); // Mode=3
        let cmd = ServerCommand::from_bytes(&pkt).unwrap();
        match cmd.structured_data {
            ServerCommandData::SetCharMode { mode } => assert_eq!(mode, 3),
            _ => panic!("Expected SetCharMode variant"),
        }
    }

    #[test]
    fn parse_scroll_right() {
        let pkt = make_packet(30, &[]);
        let cmd = ServerCommand::from_bytes(&pkt).unwrap();
        assert!(matches!(cmd.header, ServerCommandType::ScrollRight));
    }

    #[test]
    fn parse_set_origin() {
        let mut pkt = vec![0u8; 16];
        pkt[0] = 44; // SetOrigin
        let x: i16 = 100;
        let y: i16 = 200;
        pkt[1..3].copy_from_slice(&x.to_le_bytes());
        pkt[3..5].copy_from_slice(&y.to_le_bytes());
        let cmd = ServerCommand::from_bytes(&pkt).unwrap();
        match cmd.structured_data {
            ServerCommandData::SetOrigin { x: ox, y: oy } => {
                assert_eq!(ox, 100);
                assert_eq!(oy, 200);
            }
            _ => panic!("Expected SetOrigin variant"),
        }
    }

    #[test]
    fn parse_empty_bytes_returns_none() {
        assert!(ServerCommand::from_bytes(&[]).is_none());
    }

    #[test]
    fn parse_login_ok() {
        let pkt = make_packet(34, &[]);
        let cmd = ServerCommand::from_bytes(&pkt).unwrap();
        assert!(matches!(cmd.header, ServerCommandType::LoginOk));
    }

    /// Helpers for light-packet parsing tests.
    fn make_light_pkt(opcode: u8, tile_index: u16, base_light: u8, nibbles: &[u8]) -> Vec<u8> {
        let mut pkt = vec![opcode];
        pkt.extend_from_slice(&tile_index.to_le_bytes()); // bytes 1-2: index
        pkt.push(base_light & 0x0f); // byte 3: light
        pkt.extend_from_slice(nibbles); // optional packed nibbles
        // Pad to at least 16 bytes so other parsers don't need special-casing
        while pkt.len() < 16 {
            pkt.push(0);
        }
        pkt
    }

    fn assert_setmap3(cmd: ServerCommand, expected_index: u16, expected_light: u8) {
        match cmd.structured_data {
            ServerCommandData::SetMap3 {
                start_index,
                base_light,
                ..
            } => {
                assert_eq!(start_index, expected_index);
                assert_eq!(base_light, expected_light);
            }
            _ => panic!("Expected SetMap3 variant"),
        }
    }

    // -- SV_SETMAP3 (opcode 45) --

    #[test]
    fn parse_setmap3_low_index() {
        let pkt = make_light_pkt(45, 100, 7, &[]);
        let cmd = ServerCommand::from_bytes(&pkt).unwrap();
        assert_setmap3(cmd, 100, 7);
    }

    #[test]
    fn parse_setmap3_index_above_old_2047_limit() {
        let pkt = make_light_pkt(45, 2048, 8, &[]);
        let cmd = ServerCommand::from_bytes(&pkt).unwrap();
        assert_setmap3(cmd, 2048, 8);
    }

    #[test]
    fn parse_setmap3_max_viewport_index() {
        // 80 * 80 - 1 = 6399
        let pkt = make_light_pkt(45, 6399, 15, &[]);
        let cmd = ServerCommand::from_bytes(&pkt).unwrap();
        assert_setmap3(cmd, 6399, 15);
    }

    // -- SV_SETMAP4 (opcode 66) --

    #[test]
    fn parse_setmap4_index_above_old_2047_limit() {
        let pkt = make_light_pkt(66, 4096, 12, &[]);
        let cmd = ServerCommand::from_bytes(&pkt).unwrap();
        assert_setmap3(cmd, 4096, 12);
    }

    // -- SV_SETMAP5 (opcode 67) --

    #[test]
    fn parse_setmap5_index_above_old_2047_limit() {
        // One trailing nibble byte covers tiles n+1 and n+2
        let pkt = make_light_pkt(67, 3000, 5, &[0xAB]);
        let cmd = ServerCommand::from_bytes(&pkt).unwrap();
        assert_setmap3(cmd, 3000, 5);
    }

    // -- SV_SETMAP6 (opcode 68) --

    #[test]
    fn parse_setmap6_index_above_old_2047_limit() {
        let pkt = make_light_pkt(68, 6399, 0, &[]);
        let cmd = ServerCommand::from_bytes(&pkt).unwrap();
        assert_setmap3(cmd, 6399, 0);
    }

    /// Verify the new light-packet header lengths match what the server encodes.
    /// Server: [opcode, idx_lo, idx_hi, base_light, nibble_pairs...]
    #[test]
    fn sv_setmap3_len_matches_server_buffers() {
        // SV_SETMAP4 = cl_light_one (1 tile): 4-byte packet, no nibble pairs
        assert_eq!(sv_setmap3_len(0), 4);
        // SV_SETMAP5 = cl_light_three (3 tiles): 4 header + 1 nibble byte = 5
        assert_eq!(sv_setmap3_len(2), 5);
        // SV_SETMAP6 = cl_light_seven (7 tiles): 4 header + 3 nibble bytes = 7
        assert_eq!(sv_setmap3_len(6), 7);
        // SV_SETMAP3 = cl_light_26 (26 tiles): 4 header + 13 nibble bytes = 17
        assert_eq!(sv_setmap3_len(26), 17);
    }

    // -- SV_WEATHER (opcode 76) --

    #[test]
    fn parse_set_weather_roundtrip() {
        let pkt: [u8; 10] = [76, 4, 200, 0x10, 0x00, 220, 60, 30, 90, 0b1000_0001];
        let cmd = ServerCommand::from_bytes(&pkt).unwrap();
        assert_eq!(cmd.header, ServerCommandType::SetWeather);
        match cmd.structured_data {
            ServerCommandData::SetWeather {
                kind,
                intensity,
                duration_ticks,
                tint,
                flags,
            } => {
                assert_eq!(kind, 4);
                assert_eq!(intensity, 200);
                assert_eq!(duration_ticks, 0x0010);
                assert_eq!(tint, [220, 60, 30, 90]);
                assert_eq!(flags, 0b1000_0001);
            }
            _ => panic!("Expected SetWeather variant"),
        }
    }

    #[test]
    fn set_weather_expected_length_is_ten() {
        let pkt = make_packet(76, &[0; 9]);
        let mut last_n = 0i32;
        let len = ServerCommandType::get_expected_length(&pkt, &mut last_n).unwrap();
        assert_eq!(len, 10);
    }

    #[test]
    fn set_weather_opcode_decodes_from_u8() {
        assert_eq!(ServerCommandType::from(76), ServerCommandType::SetWeather);
    }

    // -- SV_SETQUESTLOG (opcode 77) --

    /// Encode a quest-log packet identically to the server's helper.
    fn encode_quest_log(
        entries: &[QuestLogEntry],
        active_template_id: u16,
        active_step_idx: u8,
        active_npc_x: u16,
        active_npc_y: u16,
    ) -> [u8; QUEST_LOG_PACKET_LEN] {
        let mut buf = [0u8; QUEST_LOG_PACKET_LEN];
        buf[0] = ServerCommandType::SetQuestLog as u8;
        let count = entries.len().min(QUEST_LOG_MAX_ENTRIES) as u8;
        buf[1] = count;
        for (i, e) in entries.iter().take(QUEST_LOG_MAX_ENTRIES).enumerate() {
            let off = 2 + i * QUEST_LOG_ENTRY_LEN;
            buf[off..off + 2].copy_from_slice(&e.npc_template_id.to_le_bytes());
            buf[off + 2..off + 4].copy_from_slice(&e.npc_x.to_le_bytes());
            buf[off + 4..off + 6].copy_from_slice(&e.npc_y.to_le_bytes());
            buf[off + 6..off + 8].copy_from_slice(&e.item_template_id.to_le_bytes());
            let npc_bytes = e.npc_name.as_bytes();
            let n = npc_bytes.len().min(QUEST_LOG_NPC_NAME_LEN - 1);
            buf[off + 8..off + 8 + n].copy_from_slice(&npc_bytes[..n]);
            let item_bytes = e.item_name.as_bytes();
            let m = item_bytes.len().min(QUEST_LOG_ITEM_NAME_LEN - 1);
            let item_off = off + 8 + QUEST_LOG_NPC_NAME_LEN;
            buf[item_off..item_off + m].copy_from_slice(&item_bytes[..m]);
        }
        let trailer_off = 2 + QUEST_LOG_MAX_ENTRIES * QUEST_LOG_ENTRY_LEN;
        buf[trailer_off..trailer_off + 2].copy_from_slice(&active_template_id.to_le_bytes());
        buf[trailer_off + 2] = active_step_idx;
        buf[trailer_off + 3..trailer_off + 5].copy_from_slice(&active_npc_x.to_le_bytes());
        buf[trailer_off + 5..trailer_off + 7].copy_from_slice(&active_npc_y.to_le_bytes());
        buf
    }

    fn make_entry(npc_template_id: u16, x: u16, y: u16, item_template_id: u16) -> QuestLogEntry {
        QuestLogEntry {
            npc_template_id,
            npc_x: x,
            npc_y: y,
            item_template_id,
            npc_name: format!("npc{npc_template_id}"),
            item_name: format!("item{item_template_id}"),
        }
    }

    #[test]
    fn set_quest_log_opcode_decodes_from_u8() {
        assert_eq!(ServerCommandType::from(77), ServerCommandType::SetQuestLog);
    }

    #[test]
    fn set_quest_log_expected_length_matches_constant() {
        let buf = encode_quest_log(&[], 0, 0, 0, 0);
        let mut last_n = 0i32;
        let len = ServerCommandType::get_expected_length(&buf, &mut last_n).unwrap();
        assert_eq!(len, QUEST_LOG_PACKET_LEN as i32);
    }

    #[test]
    fn set_quest_log_roundtrip_empty() {
        let buf = encode_quest_log(&[], 0, 0, 0, 0);
        let cmd = ServerCommand::from_bytes(&buf).unwrap();
        assert_eq!(cmd.header, ServerCommandType::SetQuestLog);
        match cmd.structured_data {
            ServerCommandData::SetQuestLog {
                entries,
                active_template_id,
                active_step_idx,
                active_npc_x,
                active_npc_y,
            } => {
                assert!(entries.is_empty());
                assert_eq!(active_template_id, 0);
                assert_eq!(active_step_idx, 0);
                assert_eq!(active_npc_x, 0);
                assert_eq!(active_npc_y, 0);
            }
            _ => panic!("expected SetQuestLog variant"),
        }
    }

    #[test]
    fn set_quest_log_roundtrip_multiple_entries() {
        let entries = vec![
            make_entry(101, 1234, 5678, 7),
            make_entry(202, 11, 22, 8),
            make_entry(303, 4000, 4001, 9),
        ];
        let buf = encode_quest_log(&entries, 202, 3, 11, 22);
        let cmd = ServerCommand::from_bytes(&buf).unwrap();
        match cmd.structured_data {
            ServerCommandData::SetQuestLog {
                entries: out,
                active_template_id,
                active_step_idx,
                active_npc_x,
                active_npc_y,
            } => {
                assert_eq!(out, entries);
                assert_eq!(active_template_id, 202);
                assert_eq!(active_step_idx, 3);
                assert_eq!(active_npc_x, 11);
                assert_eq!(active_npc_y, 22);
            }
            _ => panic!("expected SetQuestLog variant"),
        }
    }

    #[test]
    fn set_quest_log_truncates_to_sixteen_entries() {
        let mut entries = Vec::new();
        for i in 0..20u16 {
            entries.push(make_entry(i + 1, i, i, 0));
        }
        let buf = encode_quest_log(&entries, 0, 0, 0, 0);
        let cmd = ServerCommand::from_bytes(&buf).unwrap();
        match cmd.structured_data {
            ServerCommandData::SetQuestLog { entries: out, .. } => {
                assert_eq!(out.len(), QUEST_LOG_MAX_ENTRIES);
                assert_eq!(out[0].npc_template_id, 1);
                assert_eq!(out[15].npc_template_id, 16);
            }
            _ => panic!("expected SetQuestLog variant"),
        }
    }
}
