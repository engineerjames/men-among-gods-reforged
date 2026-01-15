#[repr(C)]
pub struct Look {
    autoflag: u8,
    worn: [u16; 20],
    sprite: u16,
    points: u32,
    name: [u8; 40],
    hp: u32,
    end: u32,
    mana: u32,
    a_hp: u32,
    a_end: u32,
    a_mana: u32,
    nr: u16,
    id: u16,
    extended: u8,
    item: [u16; 62],
    price: [u32; 62],
    pl_price: u32,
}

const _: () = {
    assert!(std::mem::size_of::<Look>() == 496);
};
