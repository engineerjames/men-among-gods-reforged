/// Client-side map tile
#[derive(Debug, Clone, Copy, Default)]
#[repr(C, packed)]
pub struct CMap {
    // for background
    pub ba_sprite: i16, // background image
    pub light: u8,
    pub flags: u32,
    pub flags2: u32,

    // for character
    pub ch_sprite: i16, // basic sprite of character
    pub ch_status2: u8,
    pub ch_status: u8, // what the character is doing, animation-wise
    pub ch_speed: u8,  // speed of animation
    pub ch_nr: u16,
    pub ch_id: u16,
    pub ch_proz: u8, // health in percent

    // for item
    pub it_sprite: i16, // basic sprite of item
    pub it_status: u8,  // for items with animation (burning torches etc)
}
