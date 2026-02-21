/// Detailed "look-at" data for a character or shop, matching the original C
/// client's `look` struct (496 bytes).
///
/// Populated incrementally from `SV_LOOK1`–`SV_LOOK6` server commands.
#[repr(C)]
#[derive(Clone, Copy)]
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
    pub fn nr(&self) -> u16 {
        self.nr
    }

    pub fn id(&self) -> u16 {
        self.id
    }

    #[allow(dead_code)]
    pub fn sprite(&self) -> u16 {
        self.sprite
    }

    #[allow(dead_code)]
    pub fn worn(&self, index: usize) -> u16 {
        self.worn.get(index).copied().unwrap_or(0)
    }

    #[allow(dead_code)]
    pub fn points(&self) -> u32 {
        self.points
    }

    pub fn a_hp(&self) -> u32 {
        self.a_hp
    }

    pub fn a_end(&self) -> u32 {
        self.a_end
    }

    pub fn a_mana(&self) -> u32 {
        self.a_mana
    }

    pub fn item(&self, index: usize) -> u16 {
        self.item.get(index).copied().unwrap_or(0)
    }

    pub fn price(&self, index: usize) -> u32 {
        self.price.get(index).copied().unwrap_or(0)
    }

    pub fn pl_price(&self) -> u32 {
        self.pl_price
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

    pub fn name(&self) -> Option<&str> {
        let end = self
            .name
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.name.len());
        std::str::from_utf8(&self.name[..end]).ok()
    }

    pub fn set_shop_entry(&mut self, index: u8, item: u16, price: u32) {
        let idx = index as usize;
        if idx < self.item.len() {
            self.item[idx] = item;
            self.price[idx] = price;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_fields_zeroed() {
        let l = Look::default();
        assert_eq!(l.nr(), 0);
        assert_eq!(l.id(), 0);
        assert!(!l.is_extended());
        assert_eq!(l.name(), Some(""));
        assert_eq!(l.autoflag(), 0);
    }

    #[test]
    fn set_and_get_name() {
        let mut l = Look::default();
        l.set_name("TestName");
        assert_eq!(l.name(), Some("TestName"));
    }

    #[test]
    fn name_truncated_to_39_chars() {
        let mut l = Look::default();
        let long = "A".repeat(50);
        l.set_name(&long);
        assert_eq!(l.name().unwrap().len(), 39);
    }

    #[test]
    fn set_and_get_worn() {
        let mut l = Look::default();
        l.set_worn(5, 42);
        assert_eq!(l.worn(5), 42);
    }

    #[test]
    fn worn_out_of_bounds() {
        let l = Look::default();
        assert_eq!(l.worn(99), 0);
    }

    #[test]
    fn set_and_get_shop_entry() {
        let mut l = Look::default();
        l.set_shop_entry(3, 100, 500);
        assert_eq!(l.item(3), 100);
        assert_eq!(l.price(3), 500);
    }

    #[test]
    fn shop_entry_out_of_bounds() {
        let l = Look::default();
        assert_eq!(l.item(99), 0);
        assert_eq!(l.price(99), 0);
    }

    #[test]
    fn set_extended() {
        let mut l = Look::default();
        l.set_extended(1);
        assert!(l.is_extended());
        l.set_extended(0);
        assert!(!l.is_extended());
    }

    #[test]
    fn autoflag_round_trip() {
        let mut l = Look::default();
        l.set_autoflag(7);
        assert_eq!(l.autoflag(), 7);
    }

    #[test]
    fn nr_id_round_trip() {
        let mut l = Look::default();
        l.set_nr(123);
        l.set_id(456);
        assert_eq!(l.nr(), 123);
        assert_eq!(l.id(), 456);
    }

    #[test]
    fn hp_end_mana_round_trip() {
        let mut l = Look::default();
        l.set_hp(1000);
        l.set_a_hp(500);
        assert_eq!(l.a_hp(), 500);

        l.set_end(200);
        l.set_a_end(100);
        assert_eq!(l.a_end(), 100);

        l.set_mana(300);
        l.set_a_mana(150);
        assert_eq!(l.a_mana(), 150);
    }

    #[test]
    fn pl_price_round_trip() {
        let mut l = Look::default();
        l.set_pl_price(9999);
        assert_eq!(l.pl_price(), 9999);
    }
}
