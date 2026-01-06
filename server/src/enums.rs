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
