/// Detailed "look-at" data for a character or shop, matching the original C
/// client's `look` struct (496 bytes).
///
/// Populated incrementally from `SV_LOOK1`–`SV_LOOK6` server commands.
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
    /// Returns the looked-at entity number from the server payload.
    ///
    /// # Returns
    ///
    /// * Entity number associated with this look record.
    pub fn nr(&self) -> u16 {
        self.nr
    }

    /// Returns the looked-at entity id from the server payload.
    ///
    /// # Returns
    ///
    /// * Entity id associated with this look record.
    pub fn id(&self) -> u16 {
        self.id
    }

    /// Returns the display sprite id for the looked-at entity.
    ///
    /// # Returns
    ///
    /// * Sprite id from the look payload.
    pub fn sprite(&self) -> u16 {
        self.sprite
    }

    /// Returns the worn item sprite at `index`.
    ///
    /// # Arguments
    ///
    /// * `index` - Worn slot index to read.
    ///
    /// # Returns
    ///
    /// * Worn item sprite id, or `0` when `index` is out of bounds.
    pub fn worn(&self, index: usize) -> u16 {
        self.worn.get(index).copied().unwrap_or(0)
    }

    /// Returns the looked-at character's total points.
    ///
    /// # Returns
    ///
    /// * Point total from the look payload.
    pub fn points(&self) -> u32 {
        self.points
    }

    /// Returns the looked-at character's maximum hit points.
    ///
    /// # Returns
    ///
    /// * Maximum hit point value from the look payload.
    pub fn hp(&self) -> u32 {
        self.hp
    }

    /// Returns the looked-at character's maximum endurance.
    ///
    /// # Returns
    ///
    /// * Maximum endurance value from the look payload.
    pub fn end(&self) -> u32 {
        self.end
    }

    /// Returns the looked-at character's maximum mana.
    ///
    /// # Returns
    ///
    /// * Maximum mana value from the look payload.
    pub fn mana(&self) -> u32 {
        self.mana
    }

    /// Returns the looked-at character's current hit points.
    ///
    /// # Returns
    ///
    /// * Current hit point value from the look payload.
    pub fn a_hp(&self) -> u32 {
        self.a_hp
    }

    /// Returns the looked-at character's current endurance.
    ///
    /// # Returns
    ///
    /// * Current endurance value from the look payload.
    pub fn a_end(&self) -> u32 {
        self.a_end
    }

    /// Returns the looked-at character's current mana.
    ///
    /// # Returns
    ///
    /// * Current mana value from the look payload.
    pub fn a_mana(&self) -> u32 {
        self.a_mana
    }

    /// Returns the shop item id at `index`.
    ///
    /// # Arguments
    ///
    /// * `index` - Shop slot index to read.
    ///
    /// # Returns
    ///
    /// * Item id at the requested shop slot, or `0` when `index` is out of bounds.
    pub fn item(&self, index: usize) -> u16 {
        self.item.get(index).copied().unwrap_or(0)
    }

    /// Returns the shop price at `index`.
    ///
    /// # Arguments
    ///
    /// * `index` - Shop slot index to read.
    ///
    /// # Returns
    ///
    /// * Price at the requested shop slot, or `0` when `index` is out of bounds.
    pub fn price(&self, index: usize) -> u32 {
        self.price.get(index).copied().unwrap_or(0)
    }

    /// Returns the player-specific shop price.
    ///
    /// # Returns
    ///
    /// * Player-specific price from the look payload.
    pub fn pl_price(&self) -> u32 {
        self.pl_price
    }

    /// Returns whether this look payload contains extended shop data.
    ///
    /// # Returns
    ///
    /// * `true` when the extended flag is set, otherwise `false`.
    pub fn is_extended(&self) -> bool {
        self.extended != 0
    }

    /// Returns the automatic-look flag from the payload.
    ///
    /// # Returns
    ///
    /// * Automatic-look flag value.
    pub fn autoflag(&self) -> u8 {
        self.autoflag
    }

    /// Sets the automatic-look flag.
    ///
    /// # Arguments
    ///
    /// * `autoflag` - New automatic-look flag value.
    pub fn set_autoflag(&mut self, autoflag: u8) {
        self.autoflag = autoflag;
    }

    /// Sets the extended-data flag.
    ///
    /// # Arguments
    ///
    /// * `extended` - New extended-data flag value.
    pub fn set_extended(&mut self, extended: u8) {
        self.extended = extended;
    }

    /// Sets a worn item sprite when `index` is in range.
    ///
    /// # Arguments
    ///
    /// * `index` - Worn slot index to update.
    /// * `value` - New item sprite id for the slot.
    pub fn set_worn(&mut self, index: usize, value: u16) {
        if index < self.worn.len() {
            self.worn[index] = value;
        }
    }

    /// Sets the display sprite id.
    ///
    /// # Arguments
    ///
    /// * `sprite` - New display sprite id.
    pub fn set_sprite(&mut self, sprite: u16) {
        self.sprite = sprite;
    }

    /// Sets the looked-at character's point total.
    ///
    /// # Arguments
    ///
    /// * `points` - New point total.
    pub fn set_points(&mut self, points: u32) {
        self.points = points;
    }

    /// Sets the looked-at character's maximum hit points.
    ///
    /// # Arguments
    ///
    /// * `hp` - New maximum hit point value.
    pub fn set_hp(&mut self, hp: u32) {
        self.hp = hp;
    }

    /// Sets the looked-at character's maximum endurance.
    ///
    /// # Arguments
    ///
    /// * `end` - New maximum endurance value.
    pub fn set_end(&mut self, end: u16) {
        self.end = u32::from(end);
    }

    /// Sets the looked-at character's maximum mana.
    ///
    /// # Arguments
    ///
    /// * `mana` - New maximum mana value.
    pub fn set_mana(&mut self, mana: u16) {
        self.mana = u32::from(mana);
    }

    /// Sets the looked-at character's current hit points.
    ///
    /// # Arguments
    ///
    /// * `a_hp` - New current hit point value.
    pub fn set_a_hp(&mut self, a_hp: u16) {
        self.a_hp = u32::from(a_hp);
    }

    /// Sets the looked-at character's current endurance.
    ///
    /// # Arguments
    ///
    /// * `a_end` - New current endurance value.
    pub fn set_a_end(&mut self, a_end: u16) {
        self.a_end = u32::from(a_end);
    }

    /// Sets the looked-at character's current mana.
    ///
    /// # Arguments
    ///
    /// * `a_mana` - New current mana value.
    pub fn set_a_mana(&mut self, a_mana: u16) {
        self.a_mana = u32::from(a_mana);
    }

    /// Sets the looked-at entity number.
    ///
    /// # Arguments
    ///
    /// * `nr` - New entity number.
    pub fn set_nr(&mut self, nr: u16) {
        self.nr = nr;
    }

    /// Sets the looked-at entity id.
    ///
    /// # Arguments
    ///
    /// * `id` - New entity id.
    pub fn set_id(&mut self, id: u16) {
        self.id = id;
    }

    /// Sets the player-specific shop price.
    ///
    /// # Arguments
    ///
    /// * `price` - New player-specific shop price.
    pub fn set_pl_price(&mut self, price: u32) {
        self.pl_price = price;
    }

    /// Sets the display name, truncating to fit the legacy fixed-width buffer.
    ///
    /// # Arguments
    ///
    /// * `name` - Display name to copy into the look record.
    pub fn set_name(&mut self, name: &str) {
        self.name.fill(0);
        let bytes = name.as_bytes();
        let n = std::cmp::min(bytes.len(), self.name.len().saturating_sub(1));
        self.name[..n].copy_from_slice(&bytes[..n]);
    }

    /// Returns the display name as UTF-8 text.
    ///
    /// # Returns
    ///
    /// * `Some(&str)` when the stored bytes are valid UTF-8, otherwise `None`.
    pub fn name(&self) -> Option<&str> {
        let end = self
            .name
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.name.len());
        std::str::from_utf8(&self.name[..end]).ok()
    }

    /// Sets a shop item and price when `index` is in range.
    ///
    /// # Arguments
    ///
    /// * `index` - Shop slot index to update.
    /// * `item` - Item id to store in the slot.
    /// * `price` - Price to store in the slot.
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
