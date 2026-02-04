#[derive(Copy, Clone, Debug)]
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
    CmdCTick = 255,
}

#[derive(Debug)]
pub struct ClientCommand {
    header: ClientCommandType,
    payload: Vec<u8>,
}

impl ClientCommand {
    /// Creates a command with a specific header and raw payload bytes.
    fn new(header: ClientCommandType, payload: Vec<u8>) -> Self {
        Self { header, payload }
    }

    /// Serializes the command into the on-wire 16-byte packet format.
    ///
    /// The protocol pads any short command with trailing zeros up to 16 bytes.
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
    fn cmd_xy_i16_i32(cmd: ClientCommandType, x: i16, y: i32) -> Self {
        let mut payload = Vec::with_capacity(6);
        payload.extend_from_slice(&x.to_le_bytes());
        payload.extend_from_slice(&y.to_le_bytes());
        Self::new(cmd, payload)
    }

    /// Matches `inter.c::cmd1` / `inter.c::cmd1s`: u32 at +1.
    fn cmd_u32(cmd: ClientCommandType, x: u32) -> Self {
        let mut payload = Vec::with_capacity(4);
        payload.extend_from_slice(&x.to_le_bytes());
        Self::new(cmd, payload)
    }

    /// Matches `inter.c::cmd3`: u32 at +1, +5, +9.
    fn cmd_u32_u32_u32(cmd: ClientCommandType, x: u32, y: u32, z: u32) -> Self {
        let mut payload = Vec::with_capacity(12);
        payload.extend_from_slice(&x.to_le_bytes());
        payload.extend_from_slice(&y.to_le_bytes());
        payload.extend_from_slice(&z.to_le_bytes());
        Self::new(cmd, payload)
    }

    /// Packet helper: u32 at +1, +5.
    fn cmd_u32_u32(cmd: ClientCommandType, x: u32, y: u32) -> Self {
        let mut payload = Vec::with_capacity(8);
        payload.extend_from_slice(&x.to_le_bytes());
        payload.extend_from_slice(&y.to_le_bytes());
        Self::new(cmd, payload)
    }

    /// Builds the challenge response packet sent after the server's `SV_CHALLENGE`.
    pub fn new_challenge(server_challenge: u32, client_version: u32, race: i32) -> Self {
        let mut payload = Vec::with_capacity(12);

        payload.extend_from_slice(&server_challenge.to_le_bytes());
        payload.extend_from_slice(&client_version.to_le_bytes());
        payload.extend_from_slice(&race.to_le_bytes());

        log::info!(
            "Building challenge packet: server_challenge={}, client_version={}, race={}",
            server_challenge,
            client_version,
            race
        );
        Self::new(ClientCommandType::Challenge, payload)
    }

    /// Builds the unique packet (used by the legacy client during login).
    pub fn new_unique(unique_value_1: i32, unique_value_2: i32) -> Self {
        let mut payload = Vec::with_capacity(8);
        payload.extend_from_slice(&unique_value_1.to_le_bytes());
        payload.extend_from_slice(&unique_value_2.to_le_bytes());

        log::info!(
            "Building unique packet: unique_value_1={}, unique_value_2={}",
            unique_value_1,
            unique_value_2
        );
        Self::new(ClientCommandType::CmdUnique, payload)
    }

    /// Builds the existing-login packet (`CL_LOGIN`) using stored credentials.
    pub fn new_existing_login(user_id: u32, pass1: u32, pass2: u32) -> Self {
        let mut payload = Vec::with_capacity(12);

        payload.extend_from_slice(&user_id.to_le_bytes());
        payload.extend_from_slice(&pass1.to_le_bytes());
        payload.extend_from_slice(&pass2.to_le_bytes());

        log::info!(
            "Building existing-login packet: user_id={}, pass1={}, pass2={}",
            user_id,
            pass1,
            pass2
        );
        Self::new(ClientCommandType::Login, payload)
    }

    /// Builds the new-player login packet (`CL_NEWLOGIN`).
    pub fn new_newplayer_login() -> Self {
        log::info!("Building new-player login packet");
        Self::new(ClientCommandType::NewLogin, Vec::new())
    }

    /// Mirrors `socket.c` password packet: 15 raw bytes copied to payload.
    pub fn new_password(password: &[u8]) -> Self {
        let mut payload = vec![0u8; 15];
        let n = password.len().min(15);
        payload[..n].copy_from_slice(&password[..n]);

        log::info!("Building password packet: password={:?}", &password[..n]);
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

        log::info!(
            "Building perf_report packet: ticksize={}, skip={}, idle={}, pskip={}",
            ticksize,
            skip,
            idle,
            pskip
        );
        Self::new(ClientCommandType::PerfReport, payload)
    }

    /// Mirrors `engine.c::send_opt`: 1 byte group, 1 byte offset, 13 bytes data.
    pub fn new_setuser(group: u8, offset: u8, data: &[u8]) -> Self {
        let mut payload = vec![0u8; 15];
        payload[0] = group;
        payload[1] = offset;
        let n = data.len().min(13);
        payload[2..2 + n].copy_from_slice(&data[..n]);
        log::info!(
            "Building setuser packet: group={}, offset={}, data={:?}",
            group,
            offset,
            &data[..n]
        );
        Self::new(ClientCommandType::CmdSetUser, payload)
    }

    /// Mirrors `main.c::say`: one of `CmdInput1..CmdInput8`, 15 raw bytes.
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

        log::info!(
            "Building input chunk packet: kind={:?}, chunk={:?}",
            kind,
            &chunk[..n]
        );
        Self::new(kind, payload)
    }

    /// Convenience helper: split up to 120 bytes across the 8 input packets.
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
        log::info!("Building say packet: {:?}", out);
        out
    }

    /// Mirrors original client's `CL_CMD_CTICK` (see `orig/engine.c` + `orig/inter.c::cmd1s`).
    ///
    /// The server reads a 4-byte little-endian tick counter at payload offset 1.
    pub fn new_tick(rtick: u32) -> Self {
        log::debug!("Building tick packet: rtick={}", rtick);
        Self::cmd_u32(ClientCommandType::CmdCTick, rtick)
    }

    /// Sends a ping to the server for RTT measurement.
    ///
    /// The server replies with `SV_PONG` echoing the sequence and timestamp.
    pub fn new_ping(seq: u32, client_time_ms: u32) -> Self {
        log::debug!("Building ping packet: seq={}, client_time_ms={}", seq, client_time_ms);
        Self::cmd_u32_u32(ClientCommandType::Ping, seq, client_time_ms)
    }

    /// `CL_CMD_MOVE` (`inter.c::cmd`).
    pub fn new_move(x: i16, y: i32) -> Self {
        log::info!("Building move packet: x={}, y={}", x, y);
        Self::cmd_xy_i16_i32(ClientCommandType::CmdMove, x, y)
    }

    /// `CL_CMD_PICKUP` (`inter.c::cmd`).
    pub fn new_pickup(x: i16, y: i32) -> Self {
        log::info!("Building pickup packet: x={}, y={}", x, y);
        Self::cmd_xy_i16_i32(ClientCommandType::CmdPickup, x, y)
    }

    /// `CL_CMD_DROP` (`inter.c::cmd`).
    pub fn new_drop(x: i16, y: i32) -> Self {
        log::info!("Building drop packet: x={}, y={}", x, y);
        Self::cmd_xy_i16_i32(ClientCommandType::CmdDrop, x, y)
    }

    /// `CL_CMD_TURN` (`inter.c::cmd`).
    pub fn new_turn(x: i16, y: i32) -> Self {
        log::info!("Building turn packet: x={}, y={}", x, y);
        Self::cmd_xy_i16_i32(ClientCommandType::CmdTurn, x, y)
    }

    /// `CL_CMD_USE` (`inter.c::cmd`).
    pub fn new_use(x: i16, y: i32) -> Self {
        log::info!("Building use packet: x={}, y={}", x, y);
        Self::cmd_xy_i16_i32(ClientCommandType::CmdUse, x, y)
    }

    /// `CL_CMD_LOOK_ITEM` (`inter.c::cmd`).
    pub fn new_look_item(x: i16, y: i32) -> Self {
        log::info!("Building look_item packet: x={}, y={}", x, y);
        Self::cmd_xy_i16_i32(ClientCommandType::CmdLookItem, x, y)
    }

    /// `CL_CMD_MODE` (`inter.c::cmd`).
    pub fn new_mode(mode: i16) -> Self {
        log::info!("Building mode packet: mode={}", mode);
        Self::cmd_xy_i16_i32(ClientCommandType::CmdMode, mode, 0)
    }

    /// `CL_CMD_RESET` (`main.c` ESC handler uses `cmd(CL_CMD_RESET,0,0)`).
    pub fn new_reset() -> Self {
        log::info!("Building reset packet");
        Self::cmd_xy_i16_i32(ClientCommandType::CmdReset, 0, 0)
    }

    /// `CL_CMD_SHOP` (`inter.c::cmd`).
    pub fn new_shop(shop_nr: i16, action: i32) -> Self {
        log::info!(
            "Building shop packet: shop_nr={}, action={}",
            shop_nr,
            action
        );
        Self::cmd_xy_i16_i32(ClientCommandType::CmdShop, shop_nr, action)
    }

    /// `CL_CMD_STAT` (`inter.c` uses `cmd(CL_CMD_STAT, m, stat_raised[n])`).
    pub fn new_stat(which: i16, value: i32) -> Self {
        log::info!("Building stat packet: which={}, value={}", which, value);
        Self::cmd_xy_i16_i32(ClientCommandType::CmdStat, which, value)
    }

    /// `CL_CMD_ATTACK` (`inter.c::cmd1`).
    pub fn new_attack(target: u32) -> Self {
        log::info!("Building attack packet: target={}", target);
        Self::cmd_u32(ClientCommandType::CmdAttack, target)
    }

    /// `CL_CMD_GIVE` (`inter.c::cmd1`).
    pub fn new_give(target: u32) -> Self {
        log::info!("Building give packet: target={}", target);
        Self::cmd_u32(ClientCommandType::CmdGive, target)
    }

    /// `CL_CMD_LOOK` (`inter.c::cmd1`).
    pub fn new_look(target: u32) -> Self {
        log::info!("Building look packet: target={}", target);
        Self::cmd_u32(ClientCommandType::CmdLook, target)
    }

    /// `CL_CMD_EXIT` (`engine.c::cmd_exit` uses `cmd1(CL_CMD_EXIT,0)`).
    pub fn new_exit() -> Self {
        log::info!("Building exit packet");
        Self::cmd_u32(ClientCommandType::CmdExit, 0)
    }

    /// `CL_CMD_AUTOLOOK` (`engine.c` uses `cmd1s(CL_CMD_AUTOLOOK, lookat)`).
    pub fn new_autolook(lookat: u32) -> Self {
        log::debug!("Building autolook packet: lookat={}", lookat);
        Self::cmd_u32(ClientCommandType::CmdAutoLook, lookat)
    }

    /// `CL_CMD_INV` (`inter.c::cmd3`).
    pub fn new_inv(a: u32, b: u32, selected_char: u32) -> Self {
        log::info!(
            "Building inv packet: a={}, b={}, selected_char={}",
            a,
            b,
            selected_char
        );
        Self::cmd_u32_u32_u32(ClientCommandType::CmdInv, a, b, selected_char)
    }

    /// `CL_CMD_INV_LOOK` (`inter.c::cmd3`).
    pub fn new_inv_look(a: u32, b: u32, c: u32) -> Self {
        log::info!("Building inv_look packet: a={}, b={}, c={}", a, b, c);
        Self::cmd_u32_u32_u32(ClientCommandType::CmdInvLook, a, b, c)
    }

    /// `CL_CMD_SKILL` (`inter.c::cmd3`).
    pub fn new_skill(skill: u32, selected_char: u32, attrib0: u32) -> Self {
        log::info!(
            "Building skill packet: skill={}, selected_char={}, attrib0={}",
            skill,
            selected_char,
            attrib0
        );
        Self::cmd_u32_u32_u32(ClientCommandType::CmdSkill, skill, selected_char, attrib0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Ensures client commands are always padded to 16 bytes on the wire.
    fn to_bytes_pads_to_16_bytes() {
        let cmd = ClientCommand::new_newplayer_login();
        let bytes = cmd.to_bytes();
        assert_eq!(bytes.len(), 16);
        assert_eq!(bytes[0], ClientCommandType::NewLogin as u8);
        assert!(bytes[1..].iter().all(|&b| b == 0));
    }

    #[test]
    /// Ensures password packets truncate to the protocol's 15-byte payload.
    fn password_truncates_to_15_payload_bytes() {
        let password: Vec<u8> = (0u8..20u8).collect();
        let cmd = ClientCommand::new_password(&password);
        let bytes = cmd.to_bytes();
        assert_eq!(bytes.len(), 16);
        assert_eq!(bytes[0], ClientCommandType::Passwd as u8);
        assert_eq!(&bytes[1..], &password[..15]);
    }

    #[test]
    /// Ensures chat text splits into exactly 8 x 15-byte input packets.
    fn say_packets_split_into_8_chunks() {
        let text: Vec<u8> = (0u8..120u8).collect();
        let packets = ClientCommand::new_say_packets(&text);
        assert_eq!(packets.len(), 8);
        for (i, cmd) in packets.iter().enumerate() {
            let bytes = cmd.to_bytes();
            assert_eq!(bytes.len(), 16);
            assert_eq!(bytes[1..16], text[i * 15..i * 15 + 15]);
        }
    }
}
