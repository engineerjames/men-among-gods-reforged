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
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(std::mem::size_of::<Map>());

        // We want to maintain the little-endian byte ordering since
        // the original data files are in little-endian format.
        bytes.extend_from_slice(&self.sprite.to_le_bytes());
        bytes.extend_from_slice(&self.fsprite.to_le_bytes());
        bytes.extend_from_slice(&self.ch.to_le_bytes());
        bytes.extend_from_slice(&self.to_ch.to_le_bytes());
        bytes.extend_from_slice(&self.it.to_le_bytes());
        bytes.extend_from_slice(&self.dlight.to_le_bytes());
        bytes.extend_from_slice(&self.light.to_le_bytes());
        bytes.extend_from_slice(&self.flags.to_le_bytes());

        if bytes.len() != std::mem::size_of::<Map>() {
            log::error!(
                "Map::to_bytes: expected size {}, got {}",
                std::mem::size_of::<Map>(),
                bytes.len()
            );
        }

        bytes
    }

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
        let new_light = self.light as i32 + amount;
        self.light = new_light.clamp(0, i16::MAX as i32) as i16;
    }

    pub fn is_sane_coordinates(x: usize, y: usize) -> bool {
        x < crate::constants::SERVER_MAPX as usize && y < crate::constants::SERVER_MAPY as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_to_bytes_size() {
        let map = Map::default();
        let bytes = map.to_bytes();
        assert_eq!(
            bytes.len(),
            std::mem::size_of::<Map>(),
            "Serialized Map size should match struct size"
        );
    }

    #[test]
    fn test_map_roundtrip() {
        let original = Map {
            sprite: 100,
            fsprite: 200,
            ch: 1000,
            to_ch: 2000,
            it: 3000,
            dlight: 50,
            light: -10,
            flags: 0x123456789ABCDEF0,
        };

        let bytes = original.to_bytes();
        let deserialized = Map::from_bytes(&bytes).expect("Failed to deserialize Map");

        assert_eq!(original, deserialized, "Round-trip serialization failed");
    }

    #[test]
    fn test_map_from_bytes_insufficient_data() {
        let bytes = vec![0u8; std::mem::size_of::<Map>() - 1];
        assert!(
            Map::from_bytes(&bytes).is_none(),
            "Should fail with insufficient data"
        );
    }

    #[test]
    fn test_map_add_light() {
        let mut map = Map::default();
        map.light = 100;
        map.add_light(50);

        let light_copy = map.light;
        assert_eq!(light_copy, 150);

        // Test clamping at max
        map.light = i16::MAX - 10;
        map.add_light(20);

        let light_copy = map.light;
        assert_eq!(light_copy, i16::MAX);

        // Test clamping at min (0)
        map.light = 5;
        map.add_light(-10);
        let light_copy = map.light;
        assert_eq!(light_copy, 0);
    }
}
