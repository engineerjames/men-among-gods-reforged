// cmap from original C headers
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CMapTile {
    pub x: u16,
    pub y: u16,
    pub ba_sprite: i16,
    pub light: u8,
    pub flags: u32,
    pub flags2: u32,

    pub ch_sprite: u16,
    pub ch_status: u8,
    pub ch_stat_off: u8,
    pub ch_speed: u8,
    pub ch_nr: u16,
    pub ch_id: u16,
    pub ch_proz: u8,
    pub it_sprite: i16,
    pub it_status: u8,
    pub back: i32,
    pub obj1: i32,
    pub obj2: i32,

    pub obj_xoff: i32,
    pub obj_yoff: i32,
    pub ovl_xoff: i32,
    pub ovl_yoff: i32,

    pub idle_ani: i32,
}

const _: () = {
    assert!(std::mem::size_of::<CMapTile>() == 64);
};
