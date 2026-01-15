#[derive(Clone, Copy)]
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

impl Default for Look {
    fn default() -> Self {
        Self {
            autoflag: 0,
            worn: [0; 20],
            sprite: 0,
            points: 0,
            name: [0; 40],
            hp: 0,
            end: 0,
            mana: 0,
            a_hp: 0,
            a_end: 0,
            a_mana: 0,
            nr: 0,
            id: 0,
            extended: 0,
            item: [0; 62],
            price: [0; 62],
            pl_price: 0,
        }
    }
}
