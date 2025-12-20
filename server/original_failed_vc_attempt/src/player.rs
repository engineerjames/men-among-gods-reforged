//! Player module - manages player connections and state

use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::net::TcpStream;

use crate::constants::*;
use crate::types::*;

/// Player connection and state structure
pub struct Player {
    pub sock: Option<TcpStream>,
    pub addr: u32,
    pub version: i32,
    pub race: i32,

    /// all incoming packets have 16 bytes max
    pub inbuf: [u8; 256],
    pub in_len: usize,

    /// tick buffer for outgoing data before compression
    pub tbuf: Vec<u8>,
    /// output buffer for compressed data
    pub obuf: Vec<u8>,

    /// write pointer into obuf
    pub iptr: usize,
    /// read pointer from obuf
    pub optr: usize,
    /// write pointer into tbuf
    pub tptr: usize,

    pub challenge: u32,
    pub state: u32,
    pub lasttick: u32,
    pub lasttick2: u32,
    pub usnr: usize, // character number this player controls
    pub pass1: u32,
    pub pass2: u32,
    pub ltick: u32,
    pub rtick: u32,

    pub prio: i32,

    pub cpl: CPlayer,
    pub cmap: Vec<CMap>,
    pub smap: Vec<CMap>,

    // copy of map for comparision
    pub xmap: Vec<Map>,

    // copy of visibility map for comparision
    pub vx: i32,
    pub vy: i32,
    pub visi: [i8; 40 * 40],

    pub input: [u8; 128],

    // for compression - we'll use flate2 crate
    pub zs: Option<ZlibEncoder<Vec<u8>>>,

    pub ticker_started: i32,

    pub unique: u64,

    pub passwd: [u8; 16],

    /// IDs of changed fields
    pub changed_field: Vec<i32>,
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

impl Player {
    pub fn new() -> Self {
        let cmap_size = TILEX * TILEY;
        let mut cmap = vec![CMap::default(); cmap_size];
        let mut smap = vec![CMap::default(); cmap_size];

        // Initialize sprites to SPR_EMPTY as in original code
        for m in 0..cmap_size {
            cmap[m].ba_sprite = SPR_EMPTY as i16;
            smap[m].ba_sprite = SPR_EMPTY as i16;
        }

        Self {
            sock: None,
            addr: 0,
            version: 0,
            race: 0,
            inbuf: [0; 256],
            in_len: 0,
            tbuf: vec![0; 16 * TBUFSIZE],
            obuf: vec![0; OBUFSIZE],
            iptr: 0,
            optr: 0,
            tptr: 0,
            challenge: 0,
            state: 0,
            lasttick: 0,
            lasttick2: 0,
            usnr: 0,
            pass1: 0,
            pass2: 0,
            ltick: 0,
            rtick: 0,
            prio: 0,
            cpl: CPlayer::default(),
            cmap,
            smap,
            xmap: vec![Map::default(); cmap_size],
            vx: 0,
            vy: 0,
            visi: [0; 40 * 40],
            input: [0; 128],
            zs: None,
            ticker_started: 0,
            unique: 0,
            passwd: [0; 16],
            changed_field: vec![0; cmap_size],
        }
    }

    /// Initialize a new player connection
    pub fn initialize(&mut self, addr: u32, ticker: i32) {
        self.addr = addr;
        self.inbuf = [0; 256];
        self.in_len = 0;
        self.iptr = 0;
        self.optr = 0;
        self.tptr = 0;
        self.challenge = 0;
        self.state = ST_CONNECT;
        self.usnr = 0;
        self.pass1 = 0;
        self.pass2 = 0;
        self.lasttick = ticker as u32;
        self.lasttick2 = ticker as u32;
        self.prio = 0;
        self.ticker_started = 0;

        // Reset cpl
        self.cpl = CPlayer::default();

        // Reset maps
        let cmap_size = TILEX * TILEY;
        for m in 0..cmap_size {
            self.cmap[m] = CMap::default();
            self.cmap[m].ba_sprite = SPR_EMPTY as i16;
            self.smap[m] = CMap::default();
            self.smap[m].ba_sprite = SPR_EMPTY as i16;
            self.xmap[m] = Map::default();
        }

        self.passwd = [0; 16];

        // Initialize zlib compressor
        self.zs = Some(ZlibEncoder::new(Vec::new(), Compression::best()));
    }

    /// Check if this player slot is connected
    pub fn is_connected(&self) -> bool {
        self.sock.is_some()
    }

    /// Close the connection and clean up
    pub fn disconnect(&mut self) {
        if let Some(stream) = self.sock.take() {
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
        self.ltick = 0;
        self.rtick = 0;
        self.zs = None;
    }
}

/// Player array management
pub struct PlayerManager {
    pub players: Vec<Player>,
}

impl PlayerManager {
    pub fn new() -> Self {
        let mut players = Vec::with_capacity(MAXPLAYER);
        for _ in 0..MAXPLAYER {
            players.push(Player::new());
        }
        Self { players }
    }

    /// Find an empty player slot
    /// Returns None if MAXPLAYER reached
    pub fn find_empty_slot(&self) -> Option<usize> {
        for n in 1..MAXPLAYER {
            if !self.players[n].is_connected() {
                return Some(n);
            }
        }
        None
    }

    /// Get a mutable reference to a player by index
    pub fn get_mut(&mut self, index: usize) -> Option<&mut Player> {
        if index > 0 && index < MAXPLAYER {
            Some(&mut self.players[index])
        } else {
            None
        }
    }

    /// Get a reference to a player by index
    pub fn get(&self, index: usize) -> Option<&Player> {
        if index > 0 && index < MAXPLAYER {
            Some(&self.players[index])
        } else {
            None
        }
    }
}

impl Default for PlayerManager {
    fn default() -> Self {
        Self::new()
    }
}
