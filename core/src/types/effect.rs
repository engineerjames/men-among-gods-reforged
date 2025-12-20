/// Effect structure
#[derive(Debug, Clone, Copy, Default)]
#[repr(C, packed)]
pub struct Effect {
    pub used: u8,
    pub flags: u8,

    pub effect_type: u8, // what type of effect (FX_)

    pub duration: u32, // time effect will stay

    pub data: [u32; 10], // some data
}

impl Effect {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < std::mem::size_of::<Effect>() {
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
