/// Visibility map for a character
#[derive(Clone, Copy)]
pub struct SeeMap {
    pub x: i32,
    pub y: i32,
    pub vis: [i8; crate::constants::VISI_BUFFER_LEN],
}

impl Default for SeeMap {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            vis: [0; crate::constants::VISI_BUFFER_LEN],
        }
    }
}
