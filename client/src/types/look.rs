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

impl Look {
    pub fn sprite(&self) -> u16 {
        self.sprite
    }

    pub fn points(&self) -> u32 {
        self.points
    }

    pub fn item(&self, index: usize) -> u16 {
        self.item.get(index).copied().unwrap_or(0)
    }

    pub fn price(&self, index: usize) -> u32 {
        self.price.get(index).copied().unwrap_or(0)
    }

    pub fn is_extended(&self) -> bool {
        self.extended != 0
    }

    pub fn autoflag(&self) -> u8 {
        self.autoflag
    }

    pub fn set_autoflag(&mut self, autoflag: u8) {
        self.autoflag = autoflag;
    }

    pub fn set_extended(&mut self, extended: u8) {
        self.extended = extended;
    }

    pub fn set_worn(&mut self, index: usize, value: u16) {
        if index < self.worn.len() {
            self.worn[index] = value;
        }
    }

    pub fn set_sprite(&mut self, sprite: u16) {
        self.sprite = sprite;
    }

    pub fn set_points(&mut self, points: u32) {
        self.points = points;
    }

    pub fn set_hp(&mut self, hp: u32) {
        self.hp = hp;
    }

    pub fn set_end(&mut self, end: u16) {
        self.end = u32::from(end);
    }

    pub fn set_mana(&mut self, mana: u16) {
        self.mana = u32::from(mana);
    }

    pub fn set_a_hp(&mut self, a_hp: u16) {
        self.a_hp = u32::from(a_hp);
    }

    pub fn set_a_end(&mut self, a_end: u16) {
        self.a_end = u32::from(a_end);
    }

    pub fn set_a_mana(&mut self, a_mana: u16) {
        self.a_mana = u32::from(a_mana);
    }

    pub fn set_nr(&mut self, nr: u16) {
        self.nr = nr;
    }

    pub fn set_id(&mut self, id: u16) {
        self.id = id;
    }

    pub fn set_pl_price(&mut self, price: u32) {
        self.pl_price = price;
    }

    pub fn set_name(&mut self, name: &str) {
        self.name.fill(0);
        let bytes = name.as_bytes();
        let n = std::cmp::min(bytes.len(), self.name.len().saturating_sub(1));
        self.name[..n].copy_from_slice(&bytes[..n]);
    }

    pub fn set_shop_entry(&mut self, index: u8, item: u16, price: u32) {
        let idx = index as usize;
        if idx < self.item.len() {
            self.item[idx] = item;
            self.price[idx] = price;
        }
    }
}
