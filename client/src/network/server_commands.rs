#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum ServerCommandType {
    Empty = 0,
    Challenge = 1,
    NewPlayer = 2,
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
    Unique = 72,
    Ignore = 73,
    SetMap = 128, // 128-255 are used !!!
}

#[derive(Debug)]
pub enum ServerCommandData {
    Empty,
    SetMap {
        /// For SV_SETMAP opcodes, the lower 7 bits encode an offset (0 means absolute index is present).
        off: u8,
        /// When `off == 0`, the packet carries an absolute tile index at bytes 2..4.
        absolute_tile_index: Option<u16>,
        /// Flag bits describing which optional fields are present.
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
        /// Start tile index (0..2047) from packed word.
        start_index: u16,
        /// Base light value (upper 4 bits of packed word).
        base_light: u8,
        /// Packed light nibbles, as sent by the server.
        packed: Vec<u8>,
    },
    Challenge {
        server_challenge: u32,
    },
    NewPlayer {
        player_id: u32,
        pass1: u32,
        pass2: u32,
        server_version: u32,
    },
    SetCharName1 {
        chunk: String,
    },
    SetCharName2 {
        chunk: String,
    },
    SetCharName3 {
        chunk: String,
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
    Load {
        load: u32,
    },
    Unique {
        unique1: u32,
        unique2: u32,
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

fn parse_fixed_text(bytes: &[u8]) -> String {
    let mut end = bytes.len();
    while end > 0 && bytes[end - 1] == 0 {
        end -= 1;
    }
    String::from_utf8_lossy(&bytes[..end]).to_string()
}

fn from_bytes(bytes: &[u8]) -> Option<(ServerCommandType, ServerCommandData)> {
    if bytes.is_empty() {
        return None;
    }

    // Any opcode with the SV_SETMAP bit set (0x80) is a SetMap packet.
    // The original client treats *all* 0x80..0xFF as SetMap, where the lower
    // 7 bits represent a delta offset from the previous tile index.
    if (bytes[0] & 128) != 0 {
        let off = bytes[0] & 127;
        let flags = *bytes.get(1)?;

        // Mirrors `sv_setmap`: when off==0 the tile index is included.
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
        1 => Some((
            ServerCommandType::Challenge,
            ServerCommandData::Challenge {
                server_challenge: u32::from_le_bytes(bytes.get(1..5)?.try_into().ok()?),
            },
        )),
        2 => Some((
            ServerCommandType::NewPlayer,
            ServerCommandData::NewPlayer {
                player_id: u32::from_le_bytes(bytes.get(1..5)?.try_into().ok()?),
                pass1: u32::from_le_bytes(bytes.get(5..9)?.try_into().ok()?),
                pass2: u32::from_le_bytes(bytes.get(9..13)?.try_into().ok()?),
                server_version: (u32::from(*bytes.get(13)?)
                    | (u32::from(*bytes.get(14)?) << 8)
                    | (u32::from(*bytes.get(15)?) << 16)),
            },
        )),
        3 => Some((
            ServerCommandType::SetCharName1,
            ServerCommandData::SetCharName1 {
                chunk: parse_fixed_text(bytes.get(1..16)?),
            },
        )),
        4 => Some((
            ServerCommandType::SetCharName2,
            ServerCommandData::SetCharName2 {
                chunk: parse_fixed_text(bytes.get(1..16)?),
            },
        )),
        5 => Some((
            ServerCommandType::SetCharName3,
            ServerCommandData::SetCharName3 {
                chunk: parse_fixed_text(bytes.get(1..11)?),
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
            let packed = read_u16(bytes, 1)?;
            let start_index = packed & 2047;
            let base_light = ((packed >> 12) & 15) as u8;
            Some((
                ServerCommandType::SetMap3,
                ServerCommandData::SetMap3 {
                    start_index,
                    base_light,
                    packed: bytes.get(3..)?.to_vec(),
                },
            ))
        }
        46 => Some((
            ServerCommandType::SetCharSpell,
            ServerCommandData::SetCharSpell {
                index: read_u32(bytes, 1)?,
                spell: read_i16(bytes, 5)?,
                active: read_i16(bytes, 7)?,
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
                reason: u32::from_le_bytes(bytes.get(1..5)?.try_into().ok()?),
            },
        )),
        49 => Some((ServerCommandType::Msg, ServerCommandData::Empty)),
        50 => Some((
            ServerCommandType::Look5,
            ServerCommandData::Look5 {
                name: parse_fixed_text(bytes.get(1..16)?),
            },
        )),
        51 => {
            let start = *bytes.get(1)?;
            let mut entries = Vec::new();
            // Mirrors `sv_look6`: sends up to 2 entries of 6 bytes each.
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
                chunk: parse_fixed_text(bytes.get(1..16)?),
            },
        )),
        53 => Some((
            ServerCommandType::Log1,
            ServerCommandData::Log {
                font: 1,
                chunk: parse_fixed_text(bytes.get(1..16)?),
            },
        )),
        54 => Some((
            ServerCommandType::Log2,
            ServerCommandData::Log {
                font: 2,
                chunk: parse_fixed_text(bytes.get(1..16)?),
            },
        )),
        55 => Some((
            ServerCommandType::Log3,
            ServerCommandData::Log {
                font: 3,
                chunk: parse_fixed_text(bytes.get(1..16)?),
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
                text: parse_fixed_text(bytes.get(1..16)?),
            },
        )),
        59 => Some((
            ServerCommandType::Mod2,
            ServerCommandData::Mod2 {
                text: parse_fixed_text(bytes.get(1..16)?),
            },
        )),
        60 => Some((
            ServerCommandType::Mod3,
            ServerCommandData::Mod3 {
                text: parse_fixed_text(bytes.get(1..16)?),
            },
        )),
        61 => Some((
            ServerCommandType::Mod4,
            ServerCommandData::Mod4 {
                text: parse_fixed_text(bytes.get(1..16)?),
            },
        )),
        62 => Some((
            ServerCommandType::Mod5,
            ServerCommandData::Mod5 {
                text: parse_fixed_text(bytes.get(1..16)?),
            },
        )),
        63 => Some((
            ServerCommandType::Mod6,
            ServerCommandData::Mod6 {
                text: parse_fixed_text(bytes.get(1..16)?),
            },
        )),
        64 => Some((
            ServerCommandType::Mod7,
            ServerCommandData::Mod7 {
                text: parse_fixed_text(bytes.get(1..16)?),
            },
        )),
        65 => Some((
            ServerCommandType::Mod8,
            ServerCommandData::Mod8 {
                text: parse_fixed_text(bytes.get(1..16)?),
            },
        )),
        66 => {
            let packed = read_u16(bytes, 1)?;
            let start_index = packed & 2047;
            let base_light = ((packed >> 12) & 15) as u8;
            Some((
                ServerCommandType::SetMap4,
                ServerCommandData::SetMap3 {
                    start_index,
                    base_light,
                    packed: bytes.get(3..)?.to_vec(),
                },
            ))
        }
        67 => {
            let packed = read_u16(bytes, 1)?;
            let start_index = packed & 2047;
            let base_light = ((packed >> 12) & 15) as u8;
            Some((
                ServerCommandType::SetMap5,
                ServerCommandData::SetMap3 {
                    start_index,
                    base_light,
                    packed: bytes.get(3..)?.to_vec(),
                },
            ))
        }
        68 => {
            let packed = read_u16(bytes, 1)?;
            let start_index = packed & 2047;
            let base_light = ((packed >> 12) & 15) as u8;
            Some((
                ServerCommandType::SetMap6,
                ServerCommandData::SetMap3 {
                    start_index,
                    base_light,
                    packed: bytes.get(3..)?.to_vec(),
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
        72 => Some((
            ServerCommandType::Unique,
            ServerCommandData::Unique {
                unique1: read_u32(bytes, 1)?,
                unique2: read_u32(bytes, 5)?,
            },
        )),
        73 => Some((
            ServerCommandType::Ignore,
            ServerCommandData::Ignore {
                _size: read_u32(bytes, 1)?,
            },
        )),
        // NOTE: Any opcode with 0x80 set is handled by the early-return SetMap branch above.
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
