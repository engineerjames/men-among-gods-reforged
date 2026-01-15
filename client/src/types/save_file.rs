// key from original C headers
#[repr(C)]
pub struct SaveFile {
    usnr: u32,
    pass1: u32,
    pass2: u32,
    name: [u8; 40],
    race: i32,
}

const _: () = {
    assert!(std::mem::size_of::<SaveFile>() == 56);
};
