pub struct Ban {
    creator: [u8; 80],
    victim: [u8; 80],
    address: u32,
}

impl Ban {
    pub fn new() -> Self {
        Ban {
            creator: [0; 80],
            victim: [0; 80],
            address: 0,
        }
    }
}
