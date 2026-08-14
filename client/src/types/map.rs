/// Number of fixed-point units per screen pixel used by the sub-tile movement
/// offsets (`obj_xoff_sub` / `obj_yoff_sub`).
///
/// Movement interpolation is computed in this finer unit and only rounded down
/// to whole pixels once, when the final screen position of a sprite is derived.
/// Rounding each character's offset independently would make two characters
/// that are a constant sub-pixel distance apart alternate between two pixel
/// positions every frame.
pub const SUBPIXEL_UNIT: i32 = 256;

/// A single tile in the visible map grid, matching the original C `cmap`
/// struct (64 bytes).
///
/// Stores world coordinates, sprite IDs for background / item / character
/// layers, animation state, lighting, and rendering offsets.
// cmap from original C headers
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
    pub ch_aspeed: u8,
    pub ch_nr: u16,
    pub ch_id: u16,
    pub ch_proz: u8,
    pub it_sprite: u16,
    pub it_status: u8,
    pub back: i32,
    pub obj1: i32,
    pub obj2: i32,

    /// Horizontal sub-tile movement offset in [`SUBPIXEL_UNIT`] units.
    pub obj_xoff_sub: i32,
    /// Vertical sub-tile movement offset in [`SUBPIXEL_UNIT`] units.
    pub obj_yoff_sub: i32,
    pub ovl_xoff: i32,
    pub ovl_yoff: i32,

    pub idle_ani: i32,
}
