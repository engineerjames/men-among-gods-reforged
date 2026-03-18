/// Per-character visibility map.
///
/// Tracks which tiles around the character are currently visible.
/// `vis[i]` is non-zero when tile offset `i` (in a
/// [`VISI_STRIDE`](crate::constants::VISI_STRIDE) × `VISI_STRIDE` grid centerd
/// on the character) can be seen.
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
