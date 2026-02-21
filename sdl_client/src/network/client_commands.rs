#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ClientCommandType {
    _Empty = 0,
    NewLogin = 1,
    Login = 2,
    Challenge = 3,
    PerfReport = 4,
    CmdMove = 5,
    CmdPickup = 6,
    CmdAttack = 7,
    CmdMode = 8,
    CmdInv = 9,
    CmdStat = 10,
    CmdDrop = 11,
    CmdGive = 12,
    CmdLook = 13,
    CmdInput1 = 14,
    CmdInput2 = 15,
    CmdInvLook = 16,
    CmdLookItem = 17,
    CmdUse = 18,
    CmdSetUser = 19,
    CmdTurn = 20,
    CmdAutoLook = 21,
    CmdInput3 = 22,
    CmdInput4 = 23,
    CmdReset = 24,
    CmdShop = 25,
    CmdSkill = 26,
    CmdInput5 = 27,
    CmdInput6 = 28,
    CmdInput7 = 29,
    CmdInput8 = 30,
    CmdExit = 31,
    CmdUnique = 32,
    Passwd = 33,
    Ping = 34,
    ApiLogin = 35,
    CmdCTick = 255,
}

#[derive(Debug)]
pub struct ClientCommand {
    pub header: ClientCommandType,
    payload: Vec<u8>,
}

impl ClientCommand {
    fn new(header: ClientCommandType, payload: Vec<u8>) -> Self {
        Self { header, payload }
    }

    /// Serializes the command into the on-wire 16-byte packet format.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(1 + self.payload.len());
        bytes.push(self.header as u8);
        bytes.extend_from_slice(&self.payload);
        while bytes.len() < 16 {
            bytes.push(0);
        }
        bytes
    }

    fn cmd_xy_i16_i32(cmd: ClientCommandType, x: i16, y: i32) -> Self {
        let mut payload = Vec::with_capacity(6);
        payload.extend_from_slice(&x.to_le_bytes());
        payload.extend_from_slice(&y.to_le_bytes());
        Self::new(cmd, payload)
    }

    fn cmd_u32(cmd: ClientCommandType, x: u32) -> Self {
        let mut payload = Vec::with_capacity(4);
        payload.extend_from_slice(&x.to_le_bytes());
        Self::new(cmd, payload)
    }

    fn cmd_u32_u32_u32(cmd: ClientCommandType, x: u32, y: u32, z: u32) -> Self {
        let mut payload = Vec::with_capacity(12);
        payload.extend_from_slice(&x.to_le_bytes());
        payload.extend_from_slice(&y.to_le_bytes());
        payload.extend_from_slice(&z.to_le_bytes());
        Self::new(cmd, payload)
    }

    fn cmd_u32_u32(cmd: ClientCommandType, x: u32, y: u32) -> Self {
        let mut payload = Vec::with_capacity(8);
        payload.extend_from_slice(&x.to_le_bytes());
        payload.extend_from_slice(&y.to_le_bytes());
        Self::new(cmd, payload)
    }

    pub fn new_challenge(server_challenge: u32, client_version: u32, race: i32) -> Self {
        let mut payload = Vec::with_capacity(12);
        payload.extend_from_slice(&server_challenge.to_le_bytes());
        payload.extend_from_slice(&client_version.to_le_bytes());
        payload.extend_from_slice(&race.to_le_bytes());
        Self::new(ClientCommandType::Challenge, payload)
    }

    pub fn new_unique(unique_value_1: i32, unique_value_2: i32) -> Self {
        let mut payload = Vec::with_capacity(8);
        payload.extend_from_slice(&unique_value_1.to_le_bytes());
        payload.extend_from_slice(&unique_value_2.to_le_bytes());
        Self::new(ClientCommandType::CmdUnique, payload)
    }

    pub fn new_existing_login(user_id: u32, pass1: u32, pass2: u32) -> Self {
        let mut payload = Vec::with_capacity(12);
        payload.extend_from_slice(&user_id.to_le_bytes());
        payload.extend_from_slice(&pass1.to_le_bytes());
        payload.extend_from_slice(&pass2.to_le_bytes());
        Self::new(ClientCommandType::Login, payload)
    }

    pub fn new_newplayer_login() -> Self {
        Self::new(ClientCommandType::NewLogin, Vec::new())
    }

    pub fn new_api_login(ticket: u64) -> Self {
        let mut payload = Vec::with_capacity(8);
        payload.extend_from_slice(&ticket.to_le_bytes());
        Self::new(ClientCommandType::ApiLogin, payload)
    }

    pub fn new_password(password: &[u8]) -> Self {
        let mut payload = vec![0u8; 15];
        let n = password.len().min(15);
        payload[..n].copy_from_slice(&password[..n]);
        Self::new(ClientCommandType::Passwd, payload)
    }

    pub fn new_setuser(group: u8, offset: u8, data: &[u8]) -> Self {
        let mut payload = vec![0u8; 15];
        payload[0] = group;
        payload[1] = offset;
        let n = data.len().min(13);
        payload[2..2 + n].copy_from_slice(&data[..n]);
        Self::new(ClientCommandType::CmdSetUser, payload)
    }

    pub fn new_input_chunk(kind: ClientCommandType, chunk: &[u8]) -> Self {
        let mut payload = vec![0u8; 15];
        let n = chunk.len().min(15);
        payload[..n].copy_from_slice(&chunk[..n]);
        Self::new(kind, payload)
    }

    /// Splits up to 120 bytes across 8 CmdInput packets (mirrors main.c `say`).
    pub fn new_say_packets(text: &[u8]) -> Vec<Self> {
        let kinds = [
            ClientCommandType::CmdInput1,
            ClientCommandType::CmdInput2,
            ClientCommandType::CmdInput3,
            ClientCommandType::CmdInput4,
            ClientCommandType::CmdInput5,
            ClientCommandType::CmdInput6,
            ClientCommandType::CmdInput7,
            ClientCommandType::CmdInput8,
        ];

        let mut out = Vec::with_capacity(8);
        for (i, kind) in kinds.into_iter().enumerate() {
            let start = i * 15;
            if start >= text.len() {
                out.push(Self::new_input_chunk(kind, &[]));
                continue;
            }
            let end = (start + 15).min(text.len());
            out.push(Self::new_input_chunk(kind, &text[start..end]));
        }
        out
    }

    pub fn new_tick(rtick: u32) -> Self {
        Self::cmd_u32(ClientCommandType::CmdCTick, rtick)
    }

    pub fn new_ping(seq: u32, client_time_ms: u32) -> Self {
        Self::cmd_u32_u32(ClientCommandType::Ping, seq, client_time_ms)
    }

    pub fn new_move(x: i16, y: i32) -> Self {
        Self::cmd_xy_i16_i32(ClientCommandType::CmdMove, x, y)
    }

    pub fn new_pickup(x: i16, y: i32) -> Self {
        Self::cmd_xy_i16_i32(ClientCommandType::CmdPickup, x, y)
    }

    pub fn new_drop(x: i16, y: i32) -> Self {
        Self::cmd_xy_i16_i32(ClientCommandType::CmdDrop, x, y)
    }

    pub fn new_turn(x: i16, y: i32) -> Self {
        Self::cmd_xy_i16_i32(ClientCommandType::CmdTurn, x, y)
    }

    pub fn new_use(x: i16, y: i32) -> Self {
        Self::cmd_xy_i16_i32(ClientCommandType::CmdUse, x, y)
    }

    pub fn new_look_item(x: i16, y: i32) -> Self {
        Self::cmd_xy_i16_i32(ClientCommandType::CmdLookItem, x, y)
    }

    pub fn new_mode(mode: i16) -> Self {
        Self::cmd_xy_i16_i32(ClientCommandType::CmdMode, mode, 0)
    }

    pub fn new_reset() -> Self {
        Self::cmd_xy_i16_i32(ClientCommandType::CmdReset, 0, 0)
    }

    pub fn new_shop(shop_nr: i16, action: i32) -> Self {
        Self::cmd_xy_i16_i32(ClientCommandType::CmdShop, shop_nr, action)
    }

    pub fn new_stat(which: i16, value: i32) -> Self {
        Self::cmd_xy_i16_i32(ClientCommandType::CmdStat, which, value)
    }

    pub fn new_attack(target: u32) -> Self {
        Self::cmd_u32(ClientCommandType::CmdAttack, target)
    }

    pub fn new_give(target: u32) -> Self {
        Self::cmd_u32(ClientCommandType::CmdGive, target)
    }

    pub fn new_look(target: u32) -> Self {
        Self::cmd_u32(ClientCommandType::CmdLook, target)
    }

    pub fn new_exit() -> Self {
        Self::cmd_u32(ClientCommandType::CmdExit, 0)
    }

    pub fn new_autolook(lookat: u32) -> Self {
        Self::cmd_u32(ClientCommandType::CmdAutoLook, lookat)
    }

    pub fn new_inv(a: u32, b: u32, selected_char: u32) -> Self {
        Self::cmd_u32_u32_u32(ClientCommandType::CmdInv, a, b, selected_char)
    }

    pub fn new_inv_look(a: u32, b: u32, c: u32) -> Self {
        Self::cmd_u32_u32_u32(ClientCommandType::CmdInvLook, a, b, c)
    }

    pub fn new_skill(skill: u32, selected_char: u32, attrib0: u32) -> Self {
        Self::cmd_u32_u32_u32(ClientCommandType::CmdSkill, skill, selected_char, attrib0)
    }
}
