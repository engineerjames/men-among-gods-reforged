#[derive(Copy, Clone, Debug)]
#[allow(dead_code)]
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

#[allow(dead_code)]
#[derive(Debug)]
pub enum ServerCommandData {
    Empty,
    SetMap {
        /// For SV_SETMAP opcodes, the lower 7 bits encode an offset (0 means absolute index is present).
        off: u8,
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

// struct key
// {
//   unsigned int usnr;
//   unsigned int pass1, pass2;
//   char         name[ 40 ];
//   int          race;
// };
// static_assert( sizeof( key ) == 56 );

fn from_bytes(bytes: &[u8]) -> Option<(ServerCommandType, ServerCommandData)> {
    if bytes.is_empty() {
        return None;
    }

    // Any opcode with the SV_SETMAP bit set (0x80) is a SetMap packet.
    // The original client treats *all* 0x80..0xFF as SetMap, where the lower
    // 7 bits represent a delta offset from the previous tile index.
    if (bytes[0] & 128) != 0 {
        return Some((
            ServerCommandType::SetMap,
            ServerCommandData::SetMap {
                off: bytes[0] & 127,
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
        3 => Some((ServerCommandType::SetCharName1, ServerCommandData::Empty)),
        4 => Some((ServerCommandType::SetCharName2, ServerCommandData::Empty)),
        5 => Some((ServerCommandType::SetCharName3, ServerCommandData::Empty)),
        6 => Some((ServerCommandType::SetCharMode, ServerCommandData::Empty)),
        7 => Some((ServerCommandType::SetCharAttrib, ServerCommandData::Empty)),
        8 => Some((ServerCommandType::SetCharSkill, ServerCommandData::Empty)),
        12 => Some((ServerCommandType::SetCharHp, ServerCommandData::Empty)),
        13 => Some((ServerCommandType::SetCharEndur, ServerCommandData::Empty)),
        14 => Some((ServerCommandType::SetCharMana, ServerCommandData::Empty)),
        20 => Some((ServerCommandType::SetCharAHP, ServerCommandData::Empty)),
        21 => Some((ServerCommandType::SetCharPts, ServerCommandData::Empty)),
        22 => Some((ServerCommandType::SetCharGold, ServerCommandData::Empty)),
        23 => Some((ServerCommandType::SetCharItem, ServerCommandData::Empty)),
        24 => Some((ServerCommandType::SetCharWorn, ServerCommandData::Empty)),
        25 => Some((ServerCommandType::SetCharObj, ServerCommandData::Empty)),
        27 => Some((ServerCommandType::Tick, ServerCommandData::Empty)),
        29 => Some((ServerCommandType::Look1, ServerCommandData::Empty)),
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
        39 => Some((ServerCommandType::Look2, ServerCommandData::Empty)),
        40 => Some((ServerCommandType::Look3, ServerCommandData::Empty)),
        41 => Some((ServerCommandType::Look4, ServerCommandData::Empty)),
        42 => Some((ServerCommandType::SetTarget, ServerCommandData::Empty)),
        43 => Some((ServerCommandType::SetMap2, ServerCommandData::Empty)),
        44 => Some((ServerCommandType::SetOrigin, ServerCommandData::Empty)),
        45 => Some((ServerCommandType::SetMap3, ServerCommandData::Empty)),
        46 => Some((ServerCommandType::SetCharSpell, ServerCommandData::Empty)),
        47 => Some((ServerCommandType::PlaySound, ServerCommandData::Empty)),
        48 => Some((
            ServerCommandType::Exit,
            ServerCommandData::Exit {
                reason: u32::from_le_bytes(bytes.get(1..5)?.try_into().ok()?),
            },
        )),
        49 => Some((ServerCommandType::Msg, ServerCommandData::Empty)),
        50 => Some((ServerCommandType::Look5, ServerCommandData::Empty)),
        51 => Some((ServerCommandType::Look6, ServerCommandData::Empty)),
        52 => Some((ServerCommandType::Log0, ServerCommandData::Empty)),
        53 => Some((ServerCommandType::Log1, ServerCommandData::Empty)),
        54 => Some((ServerCommandType::Log2, ServerCommandData::Empty)),
        55 => Some((ServerCommandType::Log3, ServerCommandData::Empty)),
        56 => Some((ServerCommandType::Load, ServerCommandData::Empty)),
        57 => Some((ServerCommandType::Cap, ServerCommandData::Empty)),
        58 => Some((
            // TODO: Fill in message of the day data properly
            ServerCommandType::Mod1,
            ServerCommandData::Mod1 {
                text: "".to_string(),
            },
        )),
        59 => Some((
            ServerCommandType::Mod2,
            ServerCommandData::Mod2 {
                text: "".to_string(),
            },
        )),
        60 => Some((
            ServerCommandType::Mod3,
            ServerCommandData::Mod3 {
                text: "".to_string(),
            },
        )),
        61 => Some((
            ServerCommandType::Mod4,
            ServerCommandData::Mod4 {
                text: "".to_string(),
            },
        )),
        62 => Some((
            ServerCommandType::Mod5,
            ServerCommandData::Mod5 {
                text: "".to_string(),
            },
        )),
        63 => Some((
            ServerCommandType::Mod6,
            ServerCommandData::Mod6 {
                text: "".to_string(),
            },
        )),
        64 => Some((
            ServerCommandType::Mod7,
            ServerCommandData::Mod7 {
                text: "".to_string(),
            },
        )),
        65 => Some((
            ServerCommandType::Mod8,
            ServerCommandData::Mod8 {
                text: "".to_string(),
            },
        )),
        66 => Some((ServerCommandType::SetMap4, ServerCommandData::Empty)),
        67 => Some((ServerCommandType::SetMap5, ServerCommandData::Empty)),
        68 => Some((ServerCommandType::SetMap6, ServerCommandData::Empty)),
        69 => Some((ServerCommandType::SetCharAEnd, ServerCommandData::Empty)),
        70 => Some((ServerCommandType::SetCharAMana, ServerCommandData::Empty)),
        71 => Some((ServerCommandType::SetCharDir, ServerCommandData::Empty)),
        72 => Some((ServerCommandType::Unique, ServerCommandData::Empty)),
        73 => Some((ServerCommandType::Ignore, ServerCommandData::Empty)),
        128 => Some((
            ServerCommandType::SetMap,
            ServerCommandData::SetMap { off: 0 },
        )),
        _ => None,
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct ServerCommand {
    pub header: ServerCommandType,
    pub structured_data: ServerCommandData,
    pub payload: Vec<u8>,
}

impl ServerCommand {
    #[allow(dead_code)]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.is_empty() {
            return None;
        }

        let header = from_bytes(bytes)?;
        let payload = bytes[1..].to_vec();

        Some(ServerCommand {
            header: header.0,
            structured_data: header.1,
            payload,
        })
    }
}
