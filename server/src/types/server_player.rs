use core::{
    constants::MAXPLAYER,
    types::{ClientPlayer, Map},
};
use std::net::TcpStream;

use flate2::write::ZlibEncoder;

use crate::{
    core::constants::{OBUFSIZE, SPR_EMPTY, TBUFSIZE, TILEX, TILEY},
    types::cmap::CMap,
};

// Server side player data
#[allow(dead_code)]
pub struct ServerPlayer {
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
    /// One-time API login ticket used for account-managed character login.
    pub login_ticket: u64,
    pub ltick: u32,
    pub rtick: u32,

    pub prio: i32,

    pub cpl: ClientPlayer,
    pub cmap: [CMap; TILEX * TILEY],
    pub smap: [CMap; TILEX * TILEY],

    // copy of map for comparision
    pub xmap: [Map; TILEX * TILEY],

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
    pub changed_field: [i32; TILEX * TILEY],
}

impl ServerPlayer {
    pub fn new() -> Self {
        let cmap_size = TILEX * TILEY;
        let mut cmap = std::array::from_fn(|_| CMap::default());
        let mut smap = std::array::from_fn(|_| CMap::default());

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
            login_ticket: 0,
            ltick: 0,
            rtick: 0,
            prio: 0,
            cpl: ClientPlayer::default(),
            cmap,
            smap,
            xmap: std::array::from_fn(|_| Map::default()),
            vx: 0,
            vy: 0,
            visi: [0; 40 * 40],
            input: [0; 128],
            zs: None,
            ticker_started: 0,
            unique: 0,
            passwd: [0; 16],
            changed_field: [0; TILEX * TILEY],
        }
    }

    pub fn is_sane_player(player_index: usize) -> bool {
        player_index < MAXPLAYER
    }
}
