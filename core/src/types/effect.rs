/// Effect structure
#[derive(Debug, Clone, Copy, Default)]
pub struct Effect {
    pub used: u8,
    pub flags: u8,

    pub effect_type: u8, // what type of effect (FX_)

    pub duration: u32, // time effect will stay

    pub data: [u32; 10], // some data
}

const WIRE_SIZE: usize = std::mem::size_of::<u8>() // used
    + std::mem::size_of::<u8>() // flags
    + std::mem::size_of::<u8>() // effect_type
    + std::mem::size_of::<u32>() // duration
    + 10 * std::mem::size_of::<u32>(); // data

impl Effect {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(WIRE_SIZE);

        bytes.push(self.used);
        bytes.push(self.flags);
        bytes.push(self.effect_type);
        bytes.extend_from_slice(&self.duration.to_le_bytes());

        for &value in &self.data {
            bytes.extend_from_slice(&value.to_le_bytes());
        }

        if bytes.len() != WIRE_SIZE {
            log::error!(
                "Effect::to_bytes: expected size {}, got {}",
                WIRE_SIZE,
                bytes.len()
            );
        }

        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < WIRE_SIZE {
            return None;
        }

        let mut offset: usize = 0;

        Some(Self {
            used: read_u8!(bytes, offset),
            flags: read_u8!(bytes, offset),
            effect_type: read_u8!(bytes, offset),
            duration: read_u32!(bytes, offset),
            data: {
                let mut arr = [0u32; 10];
                for i in 0..10 {
                    arr[i] = read_u32!(bytes, offset);
                }
                arr
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effect_to_bytes_size() {
        let effect = Effect::default();
        let bytes = effect.to_bytes();
        assert_eq!(
            bytes.len(),
            WIRE_SIZE,
            "Serialized Effect size should match struct size"
        );
    }

    #[test]
    fn test_effect_roundtrip() {
        let original = Effect {
            used: 1,
            flags: 2,
            effect_type: 3,
            duration: 1000,
            data: [10, 20, 30, 40, 50, 60, 70, 80, 90, 100],
        };

        let bytes = original.to_bytes();
        let deserialized = Effect::from_bytes(&bytes).expect("Failed to deserialize Effect");

        assert_eq!(original.used, deserialized.used);
        assert_eq!(original.flags, deserialized.flags);
        assert_eq!(original.effect_type, deserialized.effect_type);

        let original_duration = original.duration;
        let de_duration_copy = deserialized.duration;
        assert_eq!(original_duration, de_duration_copy);

        let original_data = original.data;
        let deserialized_data = deserialized.data;
        assert_eq!(original_data, deserialized_data);
    }

    #[test]
    fn test_effect_from_bytes_insufficient_data() {
        let bytes = vec![0u8; WIRE_SIZE - 1];
        assert!(
            Effect::from_bytes(&bytes).is_none(),
            "Should fail with insufficient data"
        );
    }

    #[test]
    fn test_effect_default() {
        let effect = Effect::default();
        assert_eq!(effect.used, 0);
        assert_eq!(effect.flags, 0);
        assert_eq!(effect.effect_type, 0);

        let duration_copy = effect.duration;
        assert_eq!(duration_copy, 0);

        let data_copy = effect.data;
        assert_eq!(data_copy, [0; 10]);
    }
}
