// key from original C headers
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

// key from original C headers
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[repr(C)]
pub struct SaveFile {
    pub usnr: u32,
    pub pass1: u32,
    pub pass2: u32,
    #[serde(with = "BigArray")]
    pub name: [u8; 40],
    pub race: i32,
}

const _: () = {
    assert!(std::mem::size_of::<SaveFile>() == 56);
};

impl Default for SaveFile {
    /// Create a default save file record.
    fn default() -> Self {
        Self {
            usnr: 0,
            pass1: 0,
            pass2: 0,
            name: [0; 40],
            race: 0,
        }
    }
}
