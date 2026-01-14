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
    #[allow(dead_code)]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(1 + self.payload.len());
        bytes.push(self.header as u8);
        bytes.extend_from_slice(&self.payload);
        bytes
    }
    #[allow(dead_code)]
    pub fn new_challenge(server_challenge: u32, client_version: u32, race: i32) -> Self {
        let mut payload = Vec::with_capacity(12);

        payload.extend_from_slice(&server_challenge.to_le_bytes());
        payload.extend_from_slice(&client_version.to_le_bytes());
        payload.extend_from_slice(&race.to_le_bytes());

        ClientCommand {
            header: ClientCommandType::Challenge,
            payload,
        }
    }

    #[allow(dead_code)]
    pub fn new_unique(unique_value_1: i32, unique_value_2: i32) -> Self {
        let mut payload = Vec::with_capacity(8);
        payload.extend_from_slice(&unique_value_1.to_le_bytes());
        payload.extend_from_slice(&unique_value_2.to_le_bytes());

        ClientCommand {
            header: ClientCommandType::CmdUnique,
            payload,
        }
    }

    #[allow(dead_code)]
    pub fn new_existing_login(user_id: u32, pass1: u32, pass2: u32) -> Self {
        let mut payload = Vec::with_capacity(12);

        payload.extend_from_slice(&user_id.to_le_bytes());
        payload.extend_from_slice(&pass1.to_le_bytes());
        payload.extend_from_slice(&pass2.to_le_bytes());

        ClientCommand {
            header: ClientCommandType::Login,
            payload,
        }
    }

    #[allow(dead_code)]
    pub fn new_newplayer_login() -> Self {
        ClientCommand {
            header: ClientCommandType::NewLogin,
            payload: Vec::new(),
        }
    }

    // TODO: Add more command constructors as needed.
}
