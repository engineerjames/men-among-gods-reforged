use crate::string_operations::c_string_to_str;

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

    pub fn address(&self) -> u32 {
        self.address
    }

    pub fn set_address(&mut self, address: u32) {
        self.address = address;
    }

    pub fn creator(&self) -> &str {
        c_string_to_str(&self.creator)
    }

    pub fn set_creator(&mut self, name: &str) {
        let bytes = name.as_bytes();
        let len = bytes.len().min(79);
        self.creator[..len].copy_from_slice(&bytes[..len]);
        self.creator[len] = 0;
    }

    pub fn victim(&self) -> &str {
        c_string_to_str(&self.victim)
    }

    pub fn set_victim(&mut self, name: &str) {
        let bytes = name.as_bytes();
        let len = bytes.len().min(79);
        self.victim[..len].copy_from_slice(&bytes[..len]);
        self.victim[len] = 0;
    }
}
