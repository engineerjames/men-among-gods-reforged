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
