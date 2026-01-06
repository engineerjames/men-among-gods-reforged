use crate::string_operations::c_string_to_str;

pub struct Ban {
    creator: [u8; 80],
    victim: [u8; 80],
    address: u32,
}

impl Ban {
    pub fn new() -> Self {
        Ban {
            creator: [0; 80],
            victim: [0; 80],
            address: 0,
        }
    }

    pub fn address(&self) -> u32 {
        self.address
    }

    pub fn set_address(&mut self, address: u32) {
        self.address = address;
    }

    pub fn creator(&self) -> &str {
        c_string_to_str(&self.creator)
    }

    pub fn set_creator(&mut self, name: &str) {
        let bytes = name.as_bytes();
        let len = bytes.len().min(79);
        self.creator[..len].copy_from_slice(&bytes[..len]);
        self.creator[len] = 0;
    }

    pub fn victim(&self) -> &str {
        c_string_to_str(&self.victim)
    }

    pub fn set_victim(&mut self, name: &str) {
        let bytes = name.as_bytes();
        let len = bytes.len().min(79);
        self.victim[..len].copy_from_slice(&bytes[..len]);
        self.victim[len] = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ban_new() {
        let ban = Ban::new();
        assert_eq!(ban.address(), 0);
        assert_eq!(ban.creator(), "");
        assert_eq!(ban.victim(), "");
    }

    #[test]
    fn test_ban_address() {
        let mut ban = Ban::new();
        assert_eq!(ban.address(), 0);

        ban.set_address(0xC0A80101); // 192.168.1.1
        assert_eq!(ban.address(), 0xC0A80101);
    }

    #[test]
    fn test_ban_creator() {
        let mut ban = Ban::new();

        ban.set_creator("AdminUser");
        assert_eq!(ban.creator(), "AdminUser");

        // Test overwrite
        ban.set_creator("NewAdmin");
        assert_eq!(ban.creator(), "NewAdmin");
    }

    #[test]
    fn test_ban_victim() {
        let mut ban = Ban::new();

        ban.set_victim("BadPlayer");
        assert_eq!(ban.victim(), "BadPlayer");

        // Test overwrite
        ban.set_victim("AnotherBadPlayer");
        assert_eq!(ban.victim(), "AnotherBadPlayer");
    }

    #[test]
    fn test_ban_long_creator_name() {
        let mut ban = Ban::new();
        let long_name =
            "ThisIsAVeryLongCreatorNameThatExceedsTheMaximumAllowedLengthForTheCreatorField";

        ban.set_creator(long_name);
        let stored_name = ban.creator();

        // Should be truncated to 79 bytes (leaving room for null terminator)
        assert!(stored_name.len() <= 79);
        assert!(long_name.starts_with(stored_name));
    }

    #[test]
    fn test_ban_long_victim_name() {
        let mut ban = Ban::new();
        let long_name =
            "ThisIsAVeryLongVictimNameThatExceedsTheMaximumAllowedLengthForTheVictimField";

        ban.set_victim(long_name);
        let stored_name = ban.victim();

        // Should be truncated to 79 bytes (leaving room for null terminator)
        assert!(stored_name.len() <= 79);
        assert!(long_name.starts_with(stored_name));
    }

    #[test]
    fn test_ban_special_characters() {
        let mut ban = Ban::new();

        ban.set_creator("Admin_123");
        assert_eq!(ban.creator(), "Admin_123");

        ban.set_victim("Player#456");
        assert_eq!(ban.victim(), "Player#456");
    }

    #[test]
    fn test_ban_empty_strings() {
        let mut ban = Ban::new();

        ban.set_creator("");
        assert_eq!(ban.creator(), "");

        ban.set_victim("");
        assert_eq!(ban.victim(), "");
    }

    #[test]
    fn test_ban_full_scenario() {
        let mut ban = Ban::new();

        // Full ban scenario
        ban.set_creator("GameMaster");
        ban.set_victim("Cheater42");
        ban.set_address(0xC0A80164); // 192.168.1.100

        assert_eq!(ban.creator(), "GameMaster");
        assert_eq!(ban.victim(), "Cheater42");
        assert_eq!(ban.address(), 0xC0A80164);
    }

    #[test]
    fn test_ban_unicode_handling() {
        let mut ban = Ban::new();

        // UTF-8 characters should work as long as they fit
        ban.set_creator("Admin™");
        assert_eq!(ban.creator(), "Admin™");

        ban.set_victim("Player™");
        assert_eq!(ban.victim(), "Player™");
    }
}
