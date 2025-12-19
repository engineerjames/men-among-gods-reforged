/// Map tile structure
#[derive(Debug, Clone, Copy, Default)]
#[repr(C, packed)]
pub struct Map {
    /// background image
    pub sprite: u16,
    /// foreground sprite
    pub fsprite: u16,

    // for fast access to objects & characters
    pub ch: u32,
    pub to_ch: u32,
    pub it: u32,

    /// percentage of dlight
    pub dlight: u16,
    /// strength of light (objects only, daylight is computed independendly)
    pub light: i16,

    /// s.a.
    pub flags: u64,
}
