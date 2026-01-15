// cmap from original C headers
#[repr(C)]
struct Map {
    x: u16,
    y: u16,
    ba_sprite: i16,
    light: u8,
    flags: u32,
    flags2: u32,

    ch_sprite: u16,
    ch_status: u8,
    ch_stat_off: u8,
    ch_speed: u8,
    ch_nr: u16,
    ch_id: u16,
    ch_proz: u8,
    it_sprite: i16,
    it_status: u8,
    back: i32,
    obj1: i32,
    obj2: i32,

    obj_xoff: i32,
    obj_yoff: i32,
    ovl_xoff: i32,
    ovl_yoff: i32,

    idle_ani: i32,
}

const _: () = {
    assert!(std::mem::size_of::<Map>() == 64);
};
