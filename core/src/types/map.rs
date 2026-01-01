/// Map tile structure
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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

impl Map {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < std::mem::size_of::<Map>() {
            return None;
        }

        let mut offset: usize = 0;

        Some(Self {
            sprite: read_u16!(bytes, offset),
            fsprite: read_u16!(bytes, offset),
            ch: read_u32!(bytes, offset),
            to_ch: read_u32!(bytes, offset),
            it: read_u32!(bytes, offset),
            dlight: read_u16!(bytes, offset),
            light: read_i16!(bytes, offset),
            #[allow(unused_assignments)]
            flags: read_u64!(bytes, offset),
        })
    }

    pub fn add_light(&mut self, amount: i32) {
        let new_light = self.light.saturating_add(amount as i16);
        self.light = new_light.clamp(0, i16::MAX); // TODO: What is the actual max light value?
    }

    pub fn is_sane_coordinates(x: usize, y: usize) -> bool {
        x < crate::constants::SERVER_MAPX as usize && y < crate::constants::SERVER_MAPY as usize
    }
}
