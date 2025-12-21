#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogoutReason {
    Unknown = 0,
    ChallengeFailed = 1,
    IdleTooLong = 2,
    NoRoom = 3,
    ParamsInvalid = 4,
    NonActive = 5,
    PasswordIncorrect = 6,
    ClientTooSlow = 7,
    Failure = 8,
    Shutdown = 9,
    Tavern = 10,
    VersionMismatch = 11,
    Exit = 12,
    Usurp = 13,
    Kicked = 14,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CharacterFlags: u64 {
        const Immortal = 1u64 << 0;
        const God = 1u64 << 1;
        const Creator = 1u64 << 2;
        const BuildMode = 1u64 << 3;
        const Respawn = 1u64 << 4;
        const Player = 1u64 << 5;
        const NewUser = 1u64 << 6;
        const NoTell = 1u64 << 8;
        const NoShout = 1u64 << 9;
        const Merchant = 1u64 << 10;
        const Staff = 1u64 << 11;
        const NoHpReg = 1u64 << 12;
        const NoEndReg = 1u64 << 13;
        const NoManaReg = 1u64 << 14;
        const Invisible = 1u64 << 15;
        const Infrared = 1u64 << 16;
        const Body = 1u64 << 17;
        const NoSleep = 1u64 << 18;
        const Undead = 1u64 << 19;
        const NoMagic = 1u64 << 20;
        const Stoned = 1u64 << 21;
        const Usurp = 1u64 << 22;
        const Imp = 1u64 << 23;
        const ShutUp = 1u64 << 24;
        const NoDesc = 1u64 << 25;
        const Prof = 1u64 << 26;
        const Simple = 1u64 << 27;
        const Kicked = 1u64 << 28;
        const NoList = 1u64 << 29;
        const NoWho = 1u64 << 30;
        const SpellIgnore = 1u64 << 31;
        const ComputerControlledPlayer = 1u64 << 32;
        const Safe = 1u64 << 33;
        const NoStaff = 1u64 << 34;
        const Poh = 1u64 << 35;
        const PohLeader = 1u64 << 36;
        const Thrall = 1u64 << 37;
        const LabKeeper = 1u64 << 38;
        const IsLooting = 1u64 << 39;
        const Golden = 1u64 << 40;
        const Black = 1u64 << 41;
        const Passwd = 1u64 << 42;
        const Update = 1u64 << 43;
        const SaveMe = 1u64 << 44;
        const GreaterGod = 1u64 << 45;
        const GreaterInv = 1u64 << 46;
    }
}
