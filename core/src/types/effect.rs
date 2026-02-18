use bincode::{Decode, Encode};

/// Effect structure
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Encode, Decode)]
pub struct Effect {
    pub used: u8,
    pub flags: u8,

    pub effect_type: u8, // what type of effect (FX_)

    pub duration: u32, // time effect will stay

    pub data: [u32; 10], // some data
}

impl Effect {
    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::encode_to_vec(self, bincode::config::standard()).expect("Effect::to_bytes failed")
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let (value, consumed): (Self, usize) =
            bincode::decode_from_slice(bytes, bincode::config::standard()).ok()?;
        if consumed == bytes.len() {
            Some(value)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effect_to_bytes_size() {
        let effect = Effect::default();
        let bytes = effect.to_bytes();
        assert!(!bytes.is_empty(), "Serialized Effect should not be empty");
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
        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_effect_from_bytes_insufficient_data() {
        let mut bytes = Effect::default().to_bytes();
        bytes.pop();
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
