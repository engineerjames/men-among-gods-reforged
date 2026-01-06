#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // TODO: Check usage and remove if unnecessary

/// Mirrors the original `LogoutReason` values used in the server protocol
/// and internal logic. These codes indicate why a client/character was
/// disconnected or removed.
pub enum LogoutReason {
    /// Unknown logout reason.
    Unknown = 0,
    /// Failed authentication challenge (bad login handshake).
    ChallengeFailed = 1,
    /// Disconnected due to inactivity / idle timeout.
    IdleTooLong = 2,
    /// Server full; no room for new connection.
    NoRoom = 3,
    /// Invalid parameters in the connection request.
    ParamsInvalid = 4,
    /// Non-active account disconnected.
    NonActive = 5,
    /// Incorrect password supplied.
    PasswordIncorrect = 6,
    /// Client is too slow / cannot keep up with server ticks.
    ClientTooSlow = 7,
    /// Generic failure / internal error caused logout.
    Failure = 8,
    /// Server shutdown initiated.
    Shutdown = 9,
    /// Logout requested due to tavern (special server mode).
    Tavern = 10,
    /// Client/server protocol version mismatch.
    VersionMismatch = 11,
    /// Normal exit initiated by the client.
    Exit = 12,
    /// Logout due to usurpation (another process took control).
    Usurp = 13,
    /// Player was kicked by an administrator.
    Kicked = 14,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    /// Character state flags.
    ///
    /// These flags mirror the original server's `CharacterFlags` bitfield and
    /// are used to encode persistent and transient properties of a character
    /// (god status, visibility, special modes, permissions, etc.). Each
    /// constant corresponds to one bit in the `u64` flag field.
    pub struct CharacterFlags: u64 {
        /// Immortal / cannot die.
        const Immortal = 1u64 << 0;
        /// Character has god privileges.
        const God = 1u64 << 1;
        /// Creator role.
        const Creator = 1u64 << 2;
        /// Build mode active.
        const BuildMode = 1u64 << 3;
        /// Respawn-in-progress flag.
        const Respawn = 1u64 << 4;
        /// Marks a player-controlled character.
        const Player = 1u64 << 5;
        /// New user / newbie status.
        const NewUser = 1u64 << 6;
        /// Prevents receiving tells.
        const NoTell = 1u64 << 8;
        /// Prevents receiving shouts.
        const NoShout = 1u64 << 9;
        /// Merchant / shop keeper.
        const Merchant = 1u64 << 10;
        /// Staff member.
        const Staff = 1u64 << 11;
        /// Disable HP regeneration.
        const NoHpReg = 1u64 << 12;
        /// Disable Endurance regeneration.
        const NoEndReg = 1u64 << 13;
        /// Disable Mana regeneration.
        const NoManaReg = 1u64 << 14;
        /// Invisible to normal players.
        const Invisible = 1u64 << 15;
        /// Infrared vision.
        const Infrared = 1u64 << 16;
        /// Body (creature) flag.
        const Body = 1u64 << 17;
        /// Prevent sleeping.
        const NoSleep = 1u64 << 18;
        /// Undead state.
        const Undead = 1u64 << 19;
        /// No magic allowed.
        const NoMagic = 1u64 << 20;
        /// Stoned / petrified.
        const Stoned = 1u64 << 21;
        /// Being usurped.
        const Usurp = 1u64 << 22;
        /// Imp-level privilege.
        const Imp = 1u64 << 23;
        /// Muted / cannot speak.
        const ShutUp = 1u64 << 24;
        /// No description allowed.
        const NoDesc = 1u64 << 25;
        /// Professional / special status.
        const Prof = 1u64 << 26;
        /// Simple-mode player.
        const Simple = 1u64 << 27;
        /// Kicked marker.
        const Kicked = 1u64 << 28;
        /// Hidden from lists.
        const NoList = 1u64 << 29;
        /// Hidden from who/online lists.
        const NoWho = 1u64 << 30;
        /// Ignore spells from this character.
        const SpellIgnore = 1u64 << 31;
        /// Computer-controlled player (bot).
        const ComputerControlledPlayer = 1u64 << 32;
        /// Safe-mode (protected zone).
        const Safe = 1u64 << 33;
        /// Cannot be targeted by staff actions.
        const NoStaff = 1u64 << 34;
        /// Player belongs to Poh (guild/organization).
        const Poh = 1u64 << 35;
        /// Player is Poh leader.
        const PohLeader = 1u64 << 36;
        /// Thrall / servitor state.
        const Thrall = 1u64 << 37;
        /// LabKeeper role.
        const LabKeeper = 1u64 << 38;
        /// Currently looting an item.
        const IsLooting = 1u64 << 39;
        /// Golden status (special color/privilege).
        const Golden = 1u64 << 40;
        /// Black status (special color/privilege).
        const Black = 1u64 << 41;
        /// Password-related flag.
        const Passwd = 1u64 << 42;
        /// Update pending flag.
        const Update = 1u64 << 43;
        /// SaveMe requested.
        const SaveMe = 1u64 << 44;
        /// Greater god privilege.
        const GreaterGod = 1u64 << 45;
        /// Greater invisibility privilege.
        const GreaterInv = 1u64 << 46;
    }
}

pub(crate) fn character_flags_name(flag: CharacterFlags) -> &'static str {
    match flag {
        CharacterFlags::Immortal => "Immortal",
        CharacterFlags::God => "God",
        CharacterFlags::Creator => "Creator",
        CharacterFlags::BuildMode => "BuildMode",
        CharacterFlags::Respawn => "Respawn",
        CharacterFlags::Player => "Player",
        CharacterFlags::NewUser => "NewUser",
        CharacterFlags::NoTell => "NoTell",
        CharacterFlags::NoShout => "NoShout",
        CharacterFlags::Merchant => "Merchant",
        CharacterFlags::Staff => "Staff",
        CharacterFlags::NoHpReg => "NoHpReg",
        CharacterFlags::NoEndReg => "NoEndReg",
        CharacterFlags::NoManaReg => "NoManaReg",
        CharacterFlags::Invisible => "Invisible",
        CharacterFlags::Infrared => "Infrared",
        CharacterFlags::Body => "Body",
        CharacterFlags::NoSleep => "NoSleep",
        CharacterFlags::Undead => "Undead",
        CharacterFlags::NoMagic => "NoMagic",
        CharacterFlags::Stoned => "Stoned",
        CharacterFlags::Usurp => "Usurp",
        CharacterFlags::Imp => "Imp",
        CharacterFlags::ShutUp => "ShutUp",
        CharacterFlags::NoDesc => "NoDesc",
        CharacterFlags::Prof => "Prof",
        CharacterFlags::Simple => "Simple",
        CharacterFlags::Kicked => "Kicked",
        CharacterFlags::NoList => "NoList",
        CharacterFlags::NoWho => "NoWho",
        CharacterFlags::SpellIgnore => "SpellIgnore",
        CharacterFlags::ComputerControlledPlayer => "ComputerControlledPlayer",
        CharacterFlags::Safe => "Safe",
        CharacterFlags::NoStaff => "NoStaff",
        CharacterFlags::Poh => "Poh",
        CharacterFlags::PohLeader => "PohLeader",
        CharacterFlags::Thrall => "Thrall",
        CharacterFlags::LabKeeper => "LabKeeper",
        CharacterFlags::IsLooting => "IsLooting",
        CharacterFlags::Golden => "Golden",
        CharacterFlags::Black => "Black",
        CharacterFlags::Passwd => "Passwd",
        CharacterFlags::Update => "Update",
        CharacterFlags::SaveMe => "SaveMe",
        CharacterFlags::GreaterGod => "GreaterGod",
        CharacterFlags::GreaterInv => "GreaterInv",
        _ => "UnknownFlag",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_character_flags_basic_operations() {
        // Test individual flag creation
        let immortal = CharacterFlags::Immortal;
        let god = CharacterFlags::God;

        // Test bit values are powers of 2
        assert_eq!(CharacterFlags::Immortal.bits(), 1u64 << 0);
        assert_eq!(CharacterFlags::God.bits(), 1u64 << 1);
        assert_eq!(CharacterFlags::Creator.bits(), 1u64 << 2);
        assert_eq!(CharacterFlags::Player.bits(), 1u64 << 5);

        // Test flag combinations
        let combined = immortal | god;
        assert!(combined.contains(CharacterFlags::Immortal));
        assert!(combined.contains(CharacterFlags::God));
        assert!(!combined.contains(CharacterFlags::Creator));
    }

    #[test]
    fn test_character_flags_operations() {
        let mut flags = CharacterFlags::empty();

        // Test insertion
        flags.insert(CharacterFlags::Player);
        assert!(flags.contains(CharacterFlags::Player));

        // Test multiple insertions
        flags.insert(CharacterFlags::God);
        flags.insert(CharacterFlags::Staff);
        assert!(flags.contains(CharacterFlags::Player));
        assert!(flags.contains(CharacterFlags::God));
        assert!(flags.contains(CharacterFlags::Staff));

        // Test removal
        flags.remove(CharacterFlags::Player);
        assert!(!flags.contains(CharacterFlags::Player));
        assert!(flags.contains(CharacterFlags::God));

        // Test toggle
        flags.toggle(CharacterFlags::Invisible);
        assert!(flags.contains(CharacterFlags::Invisible));
        flags.toggle(CharacterFlags::Invisible);
        assert!(!flags.contains(CharacterFlags::Invisible));
    }

    #[test]
    fn test_character_flags_set_operations() {
        let set1 = CharacterFlags::Player | CharacterFlags::God;
        let set2 = CharacterFlags::God | CharacterFlags::Staff;

        // Test intersection
        let intersection = set1 & set2;
        assert!(intersection.contains(CharacterFlags::God));
        assert!(!intersection.contains(CharacterFlags::Player));
        assert!(!intersection.contains(CharacterFlags::Staff));

        // Test union
        let union = set1 | set2;
        assert!(union.contains(CharacterFlags::Player));
        assert!(union.contains(CharacterFlags::God));
        assert!(union.contains(CharacterFlags::Staff));

        // Test difference
        let diff = set1 - set2;
        assert!(diff.contains(CharacterFlags::Player));
        assert!(!diff.contains(CharacterFlags::God));
        assert!(!diff.contains(CharacterFlags::Staff));
    }

    #[test]
    fn test_character_flags_name_function() {
        // Test all individual flags
        assert_eq!(character_flags_name(CharacterFlags::Immortal), "Immortal");
        assert_eq!(character_flags_name(CharacterFlags::God), "God");
        assert_eq!(character_flags_name(CharacterFlags::Creator), "Creator");
        assert_eq!(character_flags_name(CharacterFlags::BuildMode), "BuildMode");
        assert_eq!(character_flags_name(CharacterFlags::Respawn), "Respawn");
        assert_eq!(character_flags_name(CharacterFlags::Player), "Player");
        assert_eq!(character_flags_name(CharacterFlags::NewUser), "NewUser");
        assert_eq!(character_flags_name(CharacterFlags::NoTell), "NoTell");
        assert_eq!(character_flags_name(CharacterFlags::NoShout), "NoShout");
        assert_eq!(character_flags_name(CharacterFlags::Merchant), "Merchant");
        assert_eq!(character_flags_name(CharacterFlags::Staff), "Staff");
        assert_eq!(character_flags_name(CharacterFlags::Invisible), "Invisible");
        assert_eq!(character_flags_name(CharacterFlags::Body), "Body");
        assert_eq!(character_flags_name(CharacterFlags::Undead), "Undead");
        assert_eq!(character_flags_name(CharacterFlags::Stoned), "Stoned");
        assert_eq!(
            character_flags_name(CharacterFlags::GreaterGod),
            "GreaterGod"
        );
        assert_eq!(
            character_flags_name(CharacterFlags::GreaterInv),
            "GreaterInv"
        );

        // Test combined flags (should return "UnknownFlag")
        let combined = CharacterFlags::Player | CharacterFlags::God;
        assert_eq!(character_flags_name(combined), "UnknownFlag");

        // Test empty flags
        assert_eq!(character_flags_name(CharacterFlags::empty()), "UnknownFlag");
    }

    #[test]
    fn test_character_flags_debug_and_display() {
        // Test Debug formatting
        let flags = CharacterFlags::Player | CharacterFlags::God;
        let debug_str = format!("{:?}", flags);
        assert!(debug_str.contains("Player"));
        assert!(debug_str.contains("God"));

        // Test empty flags (bitflags uses different format)
        let empty = CharacterFlags::empty();
        let empty_str = format!("{:?}", empty);
        assert!(empty_str.contains("0x0") || empty_str == "(empty)");

        // Test single flag
        let single = CharacterFlags::Immortal;
        let single_str = format!("{:?}", single);
        assert!(single_str.contains("Immortal") || single_str.contains("0x1"));
    }

    #[test]
    fn test_character_flags_bit_positions() {
        // Test that all flags have unique bit positions
        let all_flags = [
            CharacterFlags::Immortal,
            CharacterFlags::God,
            CharacterFlags::Creator,
            CharacterFlags::BuildMode,
            CharacterFlags::Respawn,
            CharacterFlags::Player,
            CharacterFlags::NewUser,
            CharacterFlags::NoTell,
            CharacterFlags::NoShout,
            CharacterFlags::Merchant,
            CharacterFlags::Staff,
            CharacterFlags::NoHpReg,
            CharacterFlags::NoEndReg,
            CharacterFlags::NoManaReg,
            CharacterFlags::Invisible,
            CharacterFlags::Infrared,
            CharacterFlags::Body,
            CharacterFlags::NoSleep,
            CharacterFlags::Undead,
            CharacterFlags::NoMagic,
            CharacterFlags::Stoned,
            CharacterFlags::Usurp,
            CharacterFlags::Imp,
            CharacterFlags::ShutUp,
            CharacterFlags::NoDesc,
            CharacterFlags::Prof,
            CharacterFlags::Simple,
            CharacterFlags::Kicked,
            CharacterFlags::NoList,
            CharacterFlags::NoWho,
            CharacterFlags::SpellIgnore,
            CharacterFlags::ComputerControlledPlayer,
            CharacterFlags::Safe,
            CharacterFlags::NoStaff,
            CharacterFlags::Poh,
            CharacterFlags::PohLeader,
            CharacterFlags::Thrall,
            CharacterFlags::LabKeeper,
            CharacterFlags::IsLooting,
            CharacterFlags::Golden,
            CharacterFlags::Black,
            CharacterFlags::Passwd,
            CharacterFlags::Update,
            CharacterFlags::SaveMe,
            CharacterFlags::GreaterGod,
            CharacterFlags::GreaterInv,
        ];

        // Verify each flag is a power of 2 (has exactly one bit set)
        for flag in all_flags.iter() {
            let bits = flag.bits();
            assert_ne!(bits, 0, "Flag should not be empty");
            assert_eq!(
                bits & (bits - 1),
                0,
                "Flag should be a power of 2: {:?}",
                flag
            );
        }

        // Verify all flags are unique
        for i in 0..all_flags.len() {
            for j in (i + 1)..all_flags.len() {
                assert_ne!(
                    all_flags[i].bits(),
                    all_flags[j].bits(),
                    "Flags should have unique bit positions: {:?} vs {:?}",
                    all_flags[i],
                    all_flags[j]
                );
            }
        }
    }

    #[test]
    fn test_character_flags_from_bits() {
        // Test creating flags from raw bits
        let bits = CharacterFlags::Player.bits() | CharacterFlags::God.bits();
        let flags = CharacterFlags::from_bits_truncate(bits);
        assert!(flags.contains(CharacterFlags::Player));
        assert!(flags.contains(CharacterFlags::God));

        // Test invalid bits are truncated
        let invalid_bits = u64::MAX;
        let truncated = CharacterFlags::from_bits_truncate(invalid_bits);
        // Should contain all valid flags
        assert!(truncated.contains(CharacterFlags::Player));
        assert!(truncated.contains(CharacterFlags::God));
    }
}
