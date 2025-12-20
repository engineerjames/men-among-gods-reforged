use crate::constants;

/// Global server state structure
#[derive(Debug, Default)]
#[repr(C)]
pub struct Global {
    pub mdtime: i32,
    pub mdday: i32,
    pub mdyear: i32,
    pub dlight: i32,

    pub players_created: i32,
    pub npcs_created: i32,
    pub players_died: i32,
    pub npcs_died: i32,

    pub character_cnt: i32,
    pub item_cnt: i32,
    pub effect_cnt: i32,

    pub expire_cnt: i32,
    pub expire_run: i32,

    pub gc_cnt: i32,
    pub gc_run: i32,

    pub lost_cnt: i32,
    pub lost_run: i32,

    pub reset_char: i32,
    pub reset_item: i32,

    pub ticker: i32,

    pub total_online_time: i64,
    pub online_per_hour: [i64; 24],

    pub flags: i32,

    pub uptime: i64,
    pub uptime_per_hour: [i64; 24],

    pub awake: i32,
    pub body: i32,

    pub players_online: i32,
    pub queuesize: i32,

    pub recv: i64,
    pub send: i64,

    pub transfer_reset_time: i32,
    pub load_avg: i32,

    pub load: i64,

    pub max_online: i32,
    pub max_online_per_hour: [i32; 24],

    pub fullmoon: i8,
    pub newmoon: i8,

    pub unique: u64,

    pub cap: i32,
}

impl Global {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < std::mem::size_of::<Global>() {
            return None;
        }

        let mut offset: usize = 0;

        Some(Self {
            mdtime: read_i32!(bytes, offset),
            mdday: read_i32!(bytes, offset),
            mdyear: read_i32!(bytes, offset),
            dlight: read_i32!(bytes, offset),
            players_created: read_i32!(bytes, offset),
            npcs_created: read_i32!(bytes, offset),
            players_died: read_i32!(bytes, offset),
            npcs_died: read_i32!(bytes, offset),
            character_cnt: read_i32!(bytes, offset),
            item_cnt: read_i32!(bytes, offset),
            effect_cnt: read_i32!(bytes, offset),
            expire_cnt: read_i32!(bytes, offset),
            expire_run: read_i32!(bytes, offset),
            gc_cnt: read_i32!(bytes, offset),
            gc_run: read_i32!(bytes, offset),
            lost_cnt: read_i32!(bytes, offset),
            lost_run: read_i32!(bytes, offset),
            reset_char: read_i32!(bytes, offset),
            reset_item: read_i32!(bytes, offset),
            ticker: read_i32!(bytes, offset),
            total_online_time: read_i64!(bytes, offset),
            online_per_hour: {
                let mut arr = [0i64; 24];
                for i in 0..24 {
                    arr[i] = read_i64!(bytes, offset);
                }
                arr
            },
            flags: read_i32!(bytes, offset),
            uptime: read_i64!(bytes, offset),
            uptime_per_hour: {
                let mut arr = [0i64; 24];
                for i in 0..24 {
                    arr[i] = read_i64!(bytes, offset);
                }
                arr
            },
            awake: read_i32!(bytes, offset),
            body: read_i32!(bytes, offset),
            players_online: read_i32!(bytes, offset),
            queuesize: read_i32!(bytes, offset),
            recv: read_i64!(bytes, offset),
            send: read_i64!(bytes, offset),
            transfer_reset_time: read_i32!(bytes, offset),
            load_avg: read_i32!(bytes, offset),
            load: read_i64!(bytes, offset),
            max_online: read_i32!(bytes, offset),
            max_online_per_hour: {
                let mut arr = [0i32; 24];
                for i in 0..24 {
                    arr[i] = read_i32!(bytes, offset);
                }
                arr
            },
            fullmoon: read_i8!(bytes, offset),
            newmoon: read_i8!(bytes, offset),
            unique: read_u64!(bytes, offset),
            #[allow(unused_assignments)]
            cap: read_i32!(bytes, offset),
        })
    }

    pub fn is_dirty(&self) -> bool {
        (self.flags & constants::GF_DIRTY) != 0
    }

    pub fn set_dirty(&mut self, value: bool) {
        if value {
            self.flags |= constants::GF_DIRTY;
        } else {
            self.flags &= !constants::GF_DIRTY;
        }
    }
}
