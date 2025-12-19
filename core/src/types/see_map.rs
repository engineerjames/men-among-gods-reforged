/// Visibility map for a character
#[derive(Clone)]
pub struct SeeMap {
    pub x: i32,
    pub y: i32,
    pub vis: [i8; 40 * 40],
}

impl Default for SeeMap {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            vis: [0; 40 * 40],
        }
    }
}
