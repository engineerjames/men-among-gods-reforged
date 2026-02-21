/// Compact "look" data for auto-look overlays (name + health percentage),
/// matching the original C `look_simple` struct (26 bytes).
#[repr(C)]
pub struct LookSimple {
    known: u8,
    name: [u8; 21],
    proz: u8,
    id: u16,
}

const _: () = {
    assert!(std::mem::size_of::<LookSimple>() == 26);
};
