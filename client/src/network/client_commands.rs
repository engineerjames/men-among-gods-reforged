#[derive(Copy, Clone)]
#[allow(dead_code)]
#[repr(u8)]
pub enum ClientCommandType {
    Empty = 0,
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
    CmdCTick = 255,
}

#[allow(dead_code)]
pub struct ClientCommand {
    header: ClientCommandType,
    payload: Vec<u8>,
}

impl ClientCommand {
    fn new(header: ClientCommandType, payload: Vec<u8>) -> Self {
        Self { header, payload }
    }

    #[allow(dead_code)]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(1 + self.payload.len());
        bytes.push(self.header as u8);
        bytes.extend_from_slice(&self.payload);

        // if < 16 bytes, pad with zeros
        while bytes.len() < 16 {
            bytes.push(0);
        }

        bytes
    }

    /// Matches `inter.c::cmd`: u16 at +1, u32 at +3.
    #[allow(dead_code)]
    fn cmd_xy_i16_i32(cmd: ClientCommandType, x: i16, y: i32) -> Self {
        let mut payload = Vec::with_capacity(6);
        payload.extend_from_slice(&x.to_le_bytes());
        payload.extend_from_slice(&y.to_le_bytes());
        Self::new(cmd, payload)
    }

    /// Matches `inter.c::cmd1` / `inter.c::cmd1s`: u32 at +1.
    #[allow(dead_code)]
    fn cmd_u32(cmd: ClientCommandType, x: u32) -> Self {
        let mut payload = Vec::with_capacity(4);
        payload.extend_from_slice(&x.to_le_bytes());
        Self::new(cmd, payload)
    }

    /// Matches `inter.c::cmd3`: u32 at +1, +5, +9.
    #[allow(dead_code)]
    fn cmd_u32_u32_u32(cmd: ClientCommandType, x: u32, y: u32, z: u32) -> Self {
        let mut payload = Vec::with_capacity(12);
        payload.extend_from_slice(&x.to_le_bytes());
        payload.extend_from_slice(&y.to_le_bytes());
        payload.extend_from_slice(&z.to_le_bytes());
        Self::new(cmd, payload)
    }

    #[allow(dead_code)]
    pub fn new_challenge(server_challenge: u32, client_version: u32, race: i32) -> Self {
        let mut payload = Vec::with_capacity(12);

        payload.extend_from_slice(&server_challenge.to_le_bytes());
        payload.extend_from_slice(&client_version.to_le_bytes());
        payload.extend_from_slice(&race.to_le_bytes());

        Self::new(ClientCommandType::Challenge, payload)
    }

    #[allow(dead_code)]
    pub fn new_unique(unique_value_1: i32, unique_value_2: i32) -> Self {
        let mut payload = Vec::with_capacity(8);
        payload.extend_from_slice(&unique_value_1.to_le_bytes());
        payload.extend_from_slice(&unique_value_2.to_le_bytes());

        Self::new(ClientCommandType::CmdUnique, payload)
    }

    #[allow(dead_code)]
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

    /// Mirrors `socket.c` password packet: 15 raw bytes copied to payload.
    #[allow(dead_code)]
    pub fn new_password(password: &[u8]) -> Self {
        let mut payload = vec![0u8; 15];
        let n = password.len().min(15);
        payload[..n].copy_from_slice(&password[..n]);
        Self::new(ClientCommandType::Passwd, payload)
    }

    /// Mirrors `socket.c::so_perf_report` packet layout:
    /// u16 ticksize @ +1, u16 skip @ +3, u16 idle @ +5, f32 pskip @ +7.
    #[allow(dead_code)]
    pub fn new_perf_report(ticksize: u16, skip: u16, idle: u16, pskip: f32) -> Self {
        let mut payload = vec![0u8; 10];
        payload[0..2].copy_from_slice(&ticksize.to_le_bytes());
        payload[2..4].copy_from_slice(&skip.to_le_bytes());
        payload[4..6].copy_from_slice(&idle.to_le_bytes());
        payload[6..10].copy_from_slice(&pskip.to_le_bytes());
        Self::new(ClientCommandType::PerfReport, payload)
    }

    /// Mirrors `engine.c::send_opt`: 1 byte group, 1 byte offset, 13 bytes data.
    #[allow(dead_code)]
    pub fn new_setuser(group: u8, offset: u8, data: &[u8]) -> Self {
        let mut payload = vec![0u8; 15];
        payload[0] = group;
        payload[1] = offset;
        let n = data.len().min(13);
        payload[2..2 + n].copy_from_slice(&data[..n]);
        Self::new(ClientCommandType::CmdSetUser, payload)
    }

    /// Mirrors `main.c::say`: one of `CmdInput1..CmdInput8`, 15 raw bytes.
    #[allow(dead_code)]
    pub fn new_input_chunk(kind: ClientCommandType, chunk: &[u8]) -> Self {
        debug_assert!(matches!(
            kind,
            ClientCommandType::CmdInput1
                | ClientCommandType::CmdInput2
                | ClientCommandType::CmdInput3
                | ClientCommandType::CmdInput4
                | ClientCommandType::CmdInput5
                | ClientCommandType::CmdInput6
                | ClientCommandType::CmdInput7
                | ClientCommandType::CmdInput8
        ));

        let mut payload = vec![0u8; 15];
        let n = chunk.len().min(15);
        payload[..n].copy_from_slice(&chunk[..n]);
        Self::new(kind, payload)
    }

    /// Convenience helper: split up to 120 bytes across the 8 input packets.
    #[allow(dead_code)]
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

    /// Mirrors original client's `CL_CMD_CTICK` (see `orig/engine.c` + `orig/inter.c::cmd1s`).
    ///
    /// The server reads a 4-byte little-endian tick counter at payload offset 1.
    pub fn new_tick(rtick: u32) -> Self {
        Self::cmd_u32(ClientCommandType::CmdCTick, rtick)
    }

    /// `CL_CMD_MOVE` (`inter.c::cmd`).
    #[allow(dead_code)]
    pub fn new_move(x: i16, y: i32) -> Self {
        Self::cmd_xy_i16_i32(ClientCommandType::CmdMove, x, y)
    }

    /// `CL_CMD_PICKUP` (`inter.c::cmd`).
    #[allow(dead_code)]
    pub fn new_pickup(x: i16, y: i32) -> Self {
        Self::cmd_xy_i16_i32(ClientCommandType::CmdPickup, x, y)
    }

    /// `CL_CMD_DROP` (`inter.c::cmd`).
    #[allow(dead_code)]
    pub fn new_drop(x: i16, y: i32) -> Self {
        Self::cmd_xy_i16_i32(ClientCommandType::CmdDrop, x, y)
    }

    /// `CL_CMD_TURN` (`inter.c::cmd`).
    #[allow(dead_code)]
    pub fn new_turn(x: i16, y: i32) -> Self {
        Self::cmd_xy_i16_i32(ClientCommandType::CmdTurn, x, y)
    }

    /// `CL_CMD_USE` (`inter.c::cmd`).
    #[allow(dead_code)]
    pub fn new_use(x: i16, y: i32) -> Self {
        Self::cmd_xy_i16_i32(ClientCommandType::CmdUse, x, y)
    }

    /// `CL_CMD_LOOK_ITEM` (`inter.c::cmd`).
    #[allow(dead_code)]
    pub fn new_look_item(x: i16, y: i32) -> Self {
        Self::cmd_xy_i16_i32(ClientCommandType::CmdLookItem, x, y)
    }

    /// `CL_CMD_MODE` (`inter.c::cmd`).
    #[allow(dead_code)]
    pub fn new_mode(mode: i16) -> Self {
        Self::cmd_xy_i16_i32(ClientCommandType::CmdMode, mode, 0)
    }

    /// `CL_CMD_RESET` (`main.c` ESC handler uses `cmd(CL_CMD_RESET,0,0)`).
    #[allow(dead_code)]
    pub fn new_reset() -> Self {
        Self::cmd_xy_i16_i32(ClientCommandType::CmdReset, 0, 0)
    }

    /// `CL_CMD_SHOP` (`inter.c::cmd`).
    #[allow(dead_code)]
    pub fn new_shop(shop_nr: i16, action: i32) -> Self {
        Self::cmd_xy_i16_i32(ClientCommandType::CmdShop, shop_nr, action)
    }

    /// `CL_CMD_STAT` (`inter.c` uses `cmd(CL_CMD_STAT, m, stat_raised[n])`).
    #[allow(dead_code)]
    pub fn new_stat(which: i16, value: i32) -> Self {
        Self::cmd_xy_i16_i32(ClientCommandType::CmdStat, which, value)
    }

    /// `CL_CMD_ATTACK` (`inter.c::cmd1`).
    #[allow(dead_code)]
    pub fn new_attack(target: u32) -> Self {
        Self::cmd_u32(ClientCommandType::CmdAttack, target)
    }

    /// `CL_CMD_GIVE` (`inter.c::cmd1`).
    #[allow(dead_code)]
    pub fn new_give(target: u32) -> Self {
        Self::cmd_u32(ClientCommandType::CmdGive, target)
    }

    /// `CL_CMD_LOOK` (`inter.c::cmd1`).
    #[allow(dead_code)]
    pub fn new_look(target: u32) -> Self {
        Self::cmd_u32(ClientCommandType::CmdLook, target)
    }

    /// `CL_CMD_EXIT` (`engine.c::cmd_exit` uses `cmd1(CL_CMD_EXIT,0)`).
    #[allow(dead_code)]
    pub fn new_exit() -> Self {
        Self::cmd_u32(ClientCommandType::CmdExit, 0)
    }

    /// `CL_CMD_AUTOLOOK` (`engine.c` uses `cmd1s(CL_CMD_AUTOLOOK, lookat)`).
    #[allow(dead_code)]
    pub fn new_autolook(lookat: u32) -> Self {
        Self::cmd_u32(ClientCommandType::CmdAutoLook, lookat)
    }

    /// `CL_CMD_INV` (`inter.c::cmd3`).
    #[allow(dead_code)]
    pub fn new_inv(a: u32, b: u32, selected_char: u32) -> Self {
        Self::cmd_u32_u32_u32(ClientCommandType::CmdInv, a, b, selected_char)
    }

    /// `CL_CMD_INV_LOOK` (`inter.c::cmd3`).
    #[allow(dead_code)]
    pub fn new_inv_look(a: u32, b: u32, c: u32) -> Self {
        Self::cmd_u32_u32_u32(ClientCommandType::CmdInvLook, a, b, c)
    }

    /// `CL_CMD_SKILL` (`inter.c::cmd3`).
    #[allow(dead_code)]
    pub fn new_skill(skill: u32, selected_char: u32, attrib0: u32) -> Self {
        Self::cmd_u32_u32_u32(ClientCommandType::CmdSkill, skill, selected_char, attrib0)
    }
}
