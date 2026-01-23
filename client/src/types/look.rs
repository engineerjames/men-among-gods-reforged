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
    /// Create a zero-initialized look record.
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
    /// Return the character number.
    pub fn nr(&self) -> u16 {
        self.nr
    }

    /// Return the character id.
    pub fn id(&self) -> u16 {
        self.id
    }

    /// Return the sprite id.
    pub fn sprite(&self) -> u16 {
        self.sprite
    }

    /// Return the worn sprite id at `index`, or 0 if out of bounds.
    pub fn worn(&self, index: usize) -> u16 {
        self.worn.get(index).copied().unwrap_or(0)
    }

    /// Return the points value.
    pub fn points(&self) -> u32 {
        self.points
    }

    /// Return current hit points.
    pub fn hp(&self) -> u32 {
        self.hp
    }

    /// Return current endurance points.
    pub fn end(&self) -> u32 {
        self.end
    }

    /// Return current mana points.
    pub fn mana(&self) -> u32 {
        self.mana
    }

    /// Return maximum hit points.
    pub fn a_hp(&self) -> u32 {
        self.a_hp
    }

    /// Return maximum endurance points.
    pub fn a_end(&self) -> u32 {
        self.a_end
    }

    /// Return maximum mana points.
    pub fn a_mana(&self) -> u32 {
        self.a_mana
    }

    /// Return the item id at `index`, or 0 if out of bounds.
    pub fn item(&self, index: usize) -> u16 {
        self.item.get(index).copied().unwrap_or(0)
    }

    /// Return the price at `index`, or 0 if out of bounds.
    pub fn price(&self, index: usize) -> u32 {
        self.price.get(index).copied().unwrap_or(0)
    }

    /// Return the player price field.
    pub fn pl_price(&self) -> u32 {
        self.pl_price
    }

    /// Report whether extended shop data is present.
    pub fn is_extended(&self) -> bool {
        self.extended != 0
    }

    /// Return the autoflag value.
    pub fn autoflag(&self) -> u8 {
        self.autoflag
    }

    /// Set the autoflag value.
    pub fn set_autoflag(&mut self, autoflag: u8) {
        self.autoflag = autoflag;
    }

    /// Set the extended flag value.
    pub fn set_extended(&mut self, extended: u8) {
        self.extended = extended;
    }

    /// Set a worn slot value if `index` is valid.
    pub fn set_worn(&mut self, index: usize, value: u16) {
        if index < self.worn.len() {
            self.worn[index] = value;
        }
    }

    /// Set the sprite id.
    pub fn set_sprite(&mut self, sprite: u16) {
        self.sprite = sprite;
    }

    /// Set the points value.
    pub fn set_points(&mut self, points: u32) {
        self.points = points;
    }

    /// Set current hit points.
    pub fn set_hp(&mut self, hp: u32) {
        self.hp = hp;
    }

    /// Set current endurance points from a 16-bit value.
    pub fn set_end(&mut self, end: u16) {
        self.end = u32::from(end);
    }

    /// Set current mana points from a 16-bit value.
    pub fn set_mana(&mut self, mana: u16) {
        self.mana = u32::from(mana);
    }

    /// Set max hit points from a 16-bit value.
    pub fn set_a_hp(&mut self, a_hp: u16) {
        self.a_hp = u32::from(a_hp);
    }

    /// Set max endurance points from a 16-bit value.
    pub fn set_a_end(&mut self, a_end: u16) {
        self.a_end = u32::from(a_end);
    }

    /// Set max mana points from a 16-bit value.
    pub fn set_a_mana(&mut self, a_mana: u16) {
        self.a_mana = u32::from(a_mana);
    }

    /// Set the character number.
    pub fn set_nr(&mut self, nr: u16) {
        self.nr = nr;
    }

    /// Set the character id.
    pub fn set_id(&mut self, id: u16) {
        self.id = id;
    }

    /// Set the player price value.
    pub fn set_pl_price(&mut self, price: u32) {
        self.pl_price = price;
    }

    /// Set the NUL-terminated name, truncating to fit.
    pub fn set_name(&mut self, name: &str) {
        self.name.fill(0);
        let bytes = name.as_bytes();
        let n = std::cmp::min(bytes.len(), self.name.len().saturating_sub(1));
        self.name[..n].copy_from_slice(&bytes[..n]);
    }

    /// Return the UTF-8 name string, if valid.
    pub fn name(&self) -> Option<&str> {
        // Convert null-terminated byte array to string
        let end = self
            .name
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.name.len());
        std::str::from_utf8(&self.name[..end]).ok()
    }

    /// Set a shop entry item and price if the index is valid.
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
    /// Ensure `set_name` round-trips via `name`.
    fn set_name_and_name_roundtrip() {
        let mut look = Look::default();
        look.set_name("Bob");
        assert_eq!(look.name(), Some("Bob"));
    }

    #[test]
    /// Verify that names are truncated to leave space for a NUL terminator.
    fn set_name_truncates_to_fit_null_terminator() {
        let mut look = Look::default();
        let long = "a".repeat(100);
        look.set_name(&long);
        let s = look.name().unwrap();
        assert_eq!(s.len(), 39);
        assert!(s.chars().all(|c| c == 'a'));
    }

    #[test]
    /// Confirm out-of-bounds item reads return 0.
    fn item_out_of_bounds_returns_zero() {
        let look = Look::default();
        assert_eq!(look.item(0), 0);
        assert_eq!(look.item(9999), 0);
    }

    #[test]
    /// Ensure shop entries are clamped to valid indices.
    fn set_shop_entry_checks_bounds() {
        let mut look = Look::default();
        look.set_shop_entry(0, 123, 456);
        assert_eq!(look.item(0), 123);
        assert_eq!(look.price(0), 456);

        look.set_shop_entry(250, 999, 999);
        assert_eq!(look.item(250), 0);
    }

    #[test]
    /// Verify the extended flag toggles `is_extended`.
    fn extended_flag_controls_is_extended() {
        let mut look = Look::default();
        assert!(!look.is_extended());
        look.set_extended(1);
        assert!(look.is_extended());
    }
}
