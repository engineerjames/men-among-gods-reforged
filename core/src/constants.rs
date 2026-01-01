//! Constants module - contains all game constants ported from the original C++ headers

use bitflags::bitflags;

// =============================================================================
// General Definitions (from gendefs.h)
// =============================================================================

/// Data directory
pub const DATDIR: &str = ".dat";

/// Version number encoded as major.minor.patch in hex
pub const VERSION: u32 = 0x020E07;
pub const MINVERSION: u32 = 0x020E06;

/// Ticks per second
pub const TICKS: i32 = 20;
/// Microseconds per tick
pub const TICK: i64 = 1_000_000 / TICKS as i64;

/// Server map dimensions
pub const SERVER_MAPX: i32 = 1024;
pub const SERVER_MAPY: i32 = 1024;

/// Maximum entities
pub const MAXCHARS: usize = 8192;
pub const MAXITEM: usize = 96 * 1024;
pub const MAXEFFECT: usize = 4096;
pub const MAXMISSION: usize = 1024;
pub const MAXSKILL: usize = 50;

/// Maximum templates
pub const MAXTCHARS: usize = 4548;
pub const MAXTITEM: usize = 4548;

/// Light distance
pub const LIGHTDIST: i32 = 10;
/// Description length
pub const LENDESC: usize = 200;

/// Home position for mercenaries
pub const HOME_MERCENARY_X: i32 = 512;
pub const HOME_MERCENARY_Y: i32 = 512;

/// Say constants
pub const CNTSAY: i32 = TICKS;
pub const MAXSAY: i32 = TICKS * 7;

/// God password
pub const GODPASSWORD: &str = "devpassword";

// =============================================================================
// Wear Positions (from gendefs.h)
// =============================================================================

pub const WN_HEAD: usize = 0;
pub const WN_NECK: usize = 1;
pub const WN_BODY: usize = 2;
pub const WN_ARMS: usize = 3;
pub const WN_BELT: usize = 4;
pub const WN_LEGS: usize = 5;
pub const WN_FEET: usize = 6;
pub const WN_LHAND: usize = 7; // shield
pub const WN_RHAND: usize = 8; // weapon
pub const WN_CLOAK: usize = 9;
pub const WN_LRING: usize = 10;
pub const WN_RRING: usize = 11;

// =============================================================================
// Placement Bits (from gendefs.h)
// =============================================================================

pub const PL_HEAD: u16 = 1;
pub const PL_NECK: u16 = 2;
pub const PL_BODY: u16 = 4;
pub const PL_ARMS: u16 = 8;
pub const PL_BELT: u16 = 32;
pub const PL_LEGS: u16 = 64;
pub const PL_FEET: u16 = 128;
pub const PL_WEAPON: u16 = 256;
pub const PL_SHIELD: u16 = 512; // not usable with two-handed weapons
pub const PL_CLOAK: u16 = 1024;
pub const PL_TWOHAND: u16 = 2048;
pub const PL_RING: u16 = 4096;

// =============================================================================
// Direction Constants (from gendefs.h)
// =============================================================================

pub const DX_RIGHT: u8 = 1;
pub const DX_LEFT: u8 = 2;
pub const DX_UP: u8 = 3;
pub const DX_DOWN: u8 = 4;
pub const DX_LEFTUP: u8 = 5;
pub const DX_LEFTDOWN: u8 = 6;
pub const DX_RIGHTUP: u8 = 7;
pub const DX_RIGHTDOWN: u8 = 8;

// =============================================================================
// Notification Types (from gendefs.h)
// =============================================================================

pub const NT_NONE: u8 = 0;
pub const NT_GOTHIT: u8 = 1;
pub const NT_GOTMISS: u8 = 2;
pub const NT_DIDHIT: u8 = 3;
pub const NT_DIDMISS: u8 = 4;
pub const NT_DIDKILL: u8 = 5;
pub const NT_GOTEXP: u8 = 6;
pub const NT_SEEHIT: u8 = 7;
pub const NT_SEEMISS: u8 = 8;
pub const NT_SEEKILL: u8 = 9;
pub const NT_GIVE: u8 = 11;
pub const NT_SEE: u8 = 12;
pub const NT_DIED: u8 = 13;
pub const NT_SHOUT: u8 = 14;
pub const NT_HITME: u8 = 15;

// =============================================================================
// Spell Flags (from gendefs.h)
// =============================================================================

pub const SP_LIGHT: u32 = 1 << 0;
pub const SP_PROTECT: u32 = 1 << 1;
pub const SP_ENHANCE: u32 = 1 << 2;
pub const SP_BLESS: u32 = 1 << 3;
pub const SP_HEAL: u32 = 1 << 4;
pub const SP_CURSE: u32 = 1 << 5;
pub const SP_STUN: u32 = 1 << 6;
pub const SP_DISPEL: u32 = 1 << 7;

// =============================================================================
// Constants.h - Client Frame and Kindred Constants
// =============================================================================

pub const CLIENT_FRAME_LIMIT: u32 = 24;
pub const LOOK_TIME_IN_SECONDS: f32 = 10.0;

// Kindred flags
pub const KIN_MERCENARY: u32 = 1 << 0;
pub const KIN_SEYAN_DU: u32 = 1 << 1;
pub const KIN_PURPLE: u32 = 1 << 2;
pub const KIN_MONSTER: u32 = 1 << 3;
pub const KIN_TEMPLAR: u32 = 1 << 4;
pub const KIN_ARCHTEMPLAR: u32 = 1 << 5;
pub const KIN_HARAKIM: u32 = 1 << 6;
pub const KIN_MALE: u32 = 1 << 7;
pub const KIN_FEMALE: u32 = 1 << 8;
pub const KIN_ARCHHARAKIM: u32 = 1 << 9;
pub const KIN_WARRIOR: u32 = 1 << 10;
pub const KIN_SORCERER: u32 = 1 << 11;

// =============================================================================
// Character Flags (from Constants.h)
// =============================================================================

bitflags! {
    /// Character flags - 64-bit flags for character state and properties
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CharacterFlags: u64 {
        /// will not suffer any damage
        const CF_IMMORTAL = 1 << 0;
        /// may issue #god commands
        const CF_GOD = 1 << 1;
        /// may use #build
        const CF_CREATOR = 1 << 2;
        /// does use #build
        const CF_BUILDMODE = 1 << 3;
        /// will respawn after death - not for players
        const CF_RESPAWN = 1 << 4;
        /// is a player
        const CF_PLAYER = 1 << 5;
        /// new account created. player may change name
        const CF_NEWUSER = 1 << 6;
        /// tell will only work on him if used by a god
        const CF_NOTELL = 1 << 8;
        /// shout will only work in him if used by a god
        const CF_NOSHOUT = 1 << 9;
        /// will sell his inventory if looked at
        const CF_MERCHANT = 1 << 10;
        /// member of the staff
        const CF_STAFF = 1 << 11;
        /// no hp regeneration
        const CF_NOHPREG = 1 << 12;
        /// no endurance regeneration
        const CF_NOENDREG = 1 << 13;
        /// no mana regeneration
        const CF_NOMANAREG = 1 << 14;
        /// character is completely invisible
        const CF_INVISIBLE = 1 << 15;
        /// sees in the dark
        const CF_INFRARED = 1 << 16;
        /// dead body
        const CF_BODY = 1 << 17;
        /// stay awake all the time
        const CF_NOSLEEP = 1 << 18;
        /// is undead, can be killed with holy water
        const CF_UNDEAD = 1 << 19;
        /// no magic zone
        const CF_NOMAGIC = 1 << 20;
        /// turned to stone due to lag
        const CF_STONED = 1 << 21;
        /// NPC is being played by player
        const CF_USURP = 1 << 22;
        /// may impersonate monsters
        const CF_IMP = 1 << 23;
        /// player is unable to talk till next day
        const CF_SHUTUP = 1 << 24;
        /// player cannot change his description
        const CF_NODESC = 1 << 25;
        /// profiler listing
        const CF_PROF = 1 << 26;
        /// uses simple animation system (move, turn, 1 attack)
        const CF_SIMPLE = 1 << 27;
        /// player got kicked, may not login again for a certain time
        const CF_KICKED = 1 << 28;
        /// dont list character in top ten
        const CF_NOLIST = 1 << 29;
        /// don't list character in #WHO
        const CF_NOWHO = 1 << 30;
        /// ignore spells cast on me
        const CF_SPELLIGNORE = 1 << 31;
        /// Computer Controlled Player, does NOT log out and may have some extra logic
        const CF_CCP = 1 << 32;
        /// safety measures for gods
        const CF_SAFE = 1 << 33;
        /// #stell will only work if flag off
        const CF_NOSTAFF = 1 << 34;
        /// clan purples of honor
        const CF_POH = 1 << 35;
        /// clan purples of honor
        const CF_POH_LEADER = 1 << 36;
        /// is enthralled NPC
        const CF_THRALL = 1 << 37;
        /// is labkeeper
        const CF_LABKEEPER = 1 << 38;
        /// is currently looting a grave
        const CF_ISLOOTING = 1 << 39;
        /// is on "golden list" aka good player
        const CF_GOLDEN = 1 << 40;
        /// is on "black list" aka bad player
        const CF_BLACK = 1 << 41;
        /// has passwd set
        const CF_PASSWD = 1 << 42;
        /// client side update needed
        const CF_UPDATE = 1 << 43;
        /// save this player to disk
        const CF_SAVEME = 1 << 44;
        /// greater god
        const CF_GREATERGOD = 1 << 45;
        /// no one sees me, ever
        const CF_GREATERINV = 1 << 46;
    }
}

/// Empty sprite constant
pub const SPR_EMPTY: u16 = 999;

// =============================================================================
// Map Constants (from MapConstants.h)
// =============================================================================

pub const TILEX: usize = 34;
pub const TILEY: usize = 34;
pub const MAPX: usize = TILEX;
pub const MAPY: usize = TILEY;
pub const YPOS: i32 = 440;
pub const XPOS: i32 = 0;

// Map flags for client display
pub const INJURED: u32 = 1 << 0;
pub const INJURED1: u32 = 1 << 1;
pub const INJURED2: u32 = 1 << 2;
pub const STONED: u32 = 1 << 3;
pub const INFRARED: u32 = 1 << 4;
pub const UWATER: u32 = 1 << 5;
pub const ISUSABLE: u32 = 1 << 7;
pub const ISITEM: u32 = 1 << 8;
pub const ISCHAR: u32 = 1 << 9;
pub const INVIS: u32 = 1 << 10;
pub const STUNNED: u32 = 1 << 11;
pub const TOMB: u32 = (1 << 12) | (1 << 13) | (1 << 14) | (1 << 15) | (1 << 16);
pub const TOMB1: u32 = 1 << 12;
pub const DEATH: u32 = (1 << 17) | (1 << 18) | (1 << 19) | (1 << 20) | (1 << 21);
pub const DEATH1: u32 = 1 << 17;
pub const EMAGIC: u32 = (1 << 22) | (1 << 23) | (1 << 24);
pub const EMAGIC1: u32 = 1 << 22;
pub const GMAGIC: u32 = (1 << 25) | (1 << 26) | (1 << 27);
pub const GMAGIC1: u32 = 1 << 25;
pub const CMAGIC: u32 = (1 << 28) | (1 << 29) | (1 << 30);
pub const CMAGIC1: u32 = 1 << 28;

// Map tile flags
pub const MF_MOVEBLOCK: u32 = 1 << 0;
pub const MF_SIGHTBLOCK: u32 = 1 << 1;
pub const MF_INDOORS: u32 = 1 << 2;
pub const MF_UWATER: u32 = 1 << 3;
pub const MF_NOLAG: u32 = 1 << 4;
pub const MF_NOMONST: u32 = 1 << 5;
pub const MF_BANK: u32 = 1 << 6;
pub const MF_TAVERN: u32 = 1 << 7;
pub const MF_NOMAGIC: u32 = 1 << 8;
pub const MF_DEATHTRAP: u32 = 1 << 9;
pub const MF_ARENA: u32 = 1 << 11;
pub const MF_NOEXPIRE: u32 = 1 << 13;
pub const MF_NOFIGHT: u64 = 1 << 14;

// Dynamic map flags (32 bits offset)
pub const MF_GFX_INJURED: u64 = 1 << 32;
pub const MF_GFX_INJURED1: u64 = 1 << 33;
pub const MF_GFX_INJURED2: u64 = 1 << 34;
pub const MF_GFX_TOMB: u64 = (1 << 35) | (1 << 36) | (1 << 37) | (1 << 38) | (1 << 39);
pub const MF_GFX_TOMB1: u64 = 1 << 35;
pub const MF_GFX_DEATH: u64 = (1 << 40) | (1 << 41) | (1 << 42) | (1 << 43) | (1 << 44);
pub const MF_GFX_DEATH1: u64 = 1 << 40;
pub const MF_GFX_EMAGIC: u64 = (1 << 45) | (1 << 46) | (1 << 47);
pub const MF_GFX_EMAGIC1: u64 = 1 << 45;
pub const MF_GFX_GMAGIC: u64 = (1 << 48) | (1 << 49) | (1 << 50);
pub const MF_GFX_GMAGIC1: u64 = 1 << 48;
pub const MF_GFX_CMAGIC: u64 = (1 << 51) | (1 << 52) | (1 << 53);
pub const MF_GFX_CMAGIC1: u64 = 1 << 51;

// =============================================================================
// Use States (from data.h)
// =============================================================================

pub const USE_EMPTY: u8 = 0;
pub const USE_ACTIVE: u8 = 1;
pub const USE_NONACTIVE: u8 = 2;

// =============================================================================
// Global Flags (from data.h)
// =============================================================================

pub const GF_LOOTING: i32 = 1 << 0;
pub const GF_MAYHEM: i32 = 1 << 1;
pub const GF_CLOSEENEMY: i32 = 1 << 2;
pub const GF_CAP: i32 = 1 << 3;
pub const GF_SPEEDY: i32 = 1 << 4;
pub const GF_DIRTY: i32 = 1 << 5;

// =============================================================================
// Skill Indices (from data.h)
// =============================================================================

pub const SK_HAND: usize = 0;
pub const SK_KARATE: usize = 1;
pub const SK_SWORD: usize = 3;
pub const SK_AXE: usize = 4;
pub const SK_DAGGER: usize = 2;
pub const SK_STAFF: usize = 5;
pub const SK_TWOHAND: usize = 6; // two handed weapon
pub const SK_LOCK: usize = 7;
pub const SK_STEALTH: usize = 8;
pub const SK_PERCEPT: usize = 9;
pub const SK_SWIM: usize = 10;
pub const SK_MSHIELD: usize = 11;
pub const SK_BARTER: usize = 12;
pub const SK_REPAIR: usize = 13;
pub const SK_LIGHT: usize = 14;
pub const SK_RECALL: usize = 15;
pub const SK_WIMPY: usize = 16;
pub const SK_PROTECT: usize = 17;
pub const SK_ENHANCE: usize = 18;
pub const SK_STUN: usize = 19;
pub const SK_CURSE: usize = 20;
pub const SK_BLESS: usize = 21;
pub const SK_IDENT: usize = 22;
pub const SK_RESIST: usize = 23;
pub const SK_BLAST: usize = 24;
pub const SK_DISPEL: usize = 25;
pub const SK_HEAL: usize = 26;
pub const SK_GHOST: usize = 27;
pub const SK_REGEN: usize = 28;
pub const SK_REST: usize = 29;
pub const SK_MEDIT: usize = 30;
pub const SK_SENSE: usize = 31;
pub const SK_IMMUN: usize = 32;
pub const SK_SURROUND: usize = 33;
pub const SK_CONCEN: usize = 34;
pub const SK_WARCRY: usize = 35;
pub const SK_WARCRY2: usize = SK_WARCRY + 100;

// =============================================================================
// Character Data Indices (from data.h)
// =============================================================================

pub const CHD_AFK: usize = 0;
pub const CHD_MINGROUP: usize = 1;
pub const CHD_MAXGROUP: usize = 9;
pub const CHD_FIGHTBACK: usize = 11;
pub const CHD_GROUP: usize = 42;
pub const CHD_MASTER: usize = 63;
pub const CHD_COMPANION: usize = 64;
pub const CHD_ALLOW: usize = 65;
pub const CHD_CORPSEOWNER: usize = 66;
pub const CHD_RIDDLER: usize = 67;
pub const CHD_ATTACKTIME: usize = 68;
pub const CHD_ATTACKVICT: usize = 69;
pub const CHD_TALKATIVE: usize = 71;
pub const CHD_ENEMY1ST: usize = 80;
pub const CHD_ENEMYZZZ: usize = 91;

pub const RANKS: usize = 24;

/// level differences permitted for attack / group
pub const ATTACK_RANGE: i32 = 3;
pub const GROUP_RANGE: i32 = 3;

// =============================================================================
// Item Flags (from data.h)
// =============================================================================

bitflags! {
    /// Item flags - 64-bit flags for item properties
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ItemFlags: u64 {
        const IF_MOVEBLOCK = 1 << 0;
        const IF_SIGHTBLOCK = 1 << 1;
        const IF_TAKE = 1 << 2;
        const IF_MONEY = 1 << 3;
        const IF_LOOK = 1 << 4;
        const IF_LOOKSPECIAL = 1 << 5;
        const IF_SPELL = 1 << 6;
        const IF_NOREPAIR = 1 << 7;
        /// is a piece of armor
        const IF_ARMOR = 1 << 8;
        const IF_USE = 1 << 9;
        const IF_USESPECIAL = 1 << 10;
        /// don't use age[1] even if it is active
        const IF_SINGLEAGE = 1 << 11;
        const IF_SHOPDESTROY = 1 << 12;
        const IF_UPDATE = 1 << 13;
        /// expire even if not laying in the open and when non-active
        const IF_ALWAYSEXP1 = 1 << 14;
        /// expire ... when active
        const IF_ALWAYSEXP2 = 1 << 15;
        /// is a weapon - sword
        const IF_WP_SWORD = 1 << 16;
        /// is a weapon - dagger
        const IF_WP_DAGGER = 1 << 17;
        /// is a weapon - axe
        const IF_WP_AXE = 1 << 18;
        /// is a weapon - staff
        const IF_WP_STAFF = 1 << 19;
        /// is a weapon - two-handed sword
        const IF_WP_TWOHAND = 1 << 20;
        /// using it destroys the object
        const IF_USEDESTROY = 1 << 21;
        /// may be turned on (activated)
        const IF_USEACTIVATE = 1 << 22;
        /// may be turned off (deactivated)
        const IF_USEDEACTIVATE = 1 << 23;
        /// is magical
        const IF_MAGIC = 1 << 24;
        /// is neither weapon nor armor nor magical
        const IF_MISC = 1 << 25;
        /// reactive item whenever it expires
        const IF_REACTIVATE = 1 << 26;
        /// permanent spell (may take mana to keep up)
        const IF_PERMSPELL = 1 << 27;
        /// unique item
        const IF_UNIQUE = 1 << 28;
        /// auto-donate this item
        const IF_DONATE = 1 << 29;
        /// destroy when leaving labyrinth
        const IF_LABYDESTROY = 1 << 30;
        /// dont change the price for this item
        const IF_NOMARKET = 1 << 31;
        /// hard to see, uses data[9] for difficulty
        const IF_HIDDEN = 1 << 32;
        /// special routine to call when stepped on
        const IF_STEPACTION = 1 << 33;
        /// not storable in depot
        const IF_NODEPOT = 1 << 34;
        /// ages faster when exposed to light
        const IF_LIGHTAGE = 1 << 35;
        /// special procedure for expire
        const IF_EXPIREPROC = 1 << 36;
        /// item has been identified
        const IF_IDENTIFIED = 1 << 37;
        /// dont expire item
        const IF_NOEXPIRE = 1 << 38;
        /// item was enhanced by a soulstone
        const IF_SOULSTONE = 1 << 39;

        /// Composite: all weapon types
        const IF_WEAPON = Self::IF_WP_SWORD.bits() | Self::IF_WP_DAGGER.bits()
                        | Self::IF_WP_AXE.bits() | Self::IF_WP_STAFF.bits()
                        | Self::IF_WP_TWOHAND.bits();
        /// Composite: sellable items
        const IF_SELLABLE = Self::IF_WEAPON.bits() | Self::IF_MISC.bits()
                          | Self::IF_MAGIC.bits() | Self::IF_ARMOR.bits();
    }
}

// =============================================================================
// Effect Flags (from data.h)
// =============================================================================

pub const EF_MOVEBLOCK: u8 = 1;
pub const EF_SIGHTBLOCK: u8 = 2;

pub const FX_INJURED: u8 = 1;

// =============================================================================
// Numbers (from numbers.h)
// =============================================================================

pub const CT_LGUARD: i32 = 15;
pub const CT_COMPANION: i32 = 158;
pub const CT_PRIEST: i32 = 180;

pub const COMPANION_TIMEOUT: i32 = 5 * 60 * TICKS;

pub const IT_TOMBSTONE: i32 = 170;
pub const IT_LAGSCROLL: i32 = 500;

// =============================================================================
// Client Message Types (from client.h)
// =============================================================================

pub const CL_EMPTY: u8 = 0;
pub const CL_NEWLOGIN: u8 = 1;
pub const CL_LOGIN: u8 = 2;
pub const CL_CHALLENGE: u8 = 3;
pub const CL_PERF_REPORT: u8 = 4;
pub const CL_CMD_MOVE: u8 = 5;
pub const CL_CMD_PICKUP: u8 = 6;
pub const CL_CMD_ATTACK: u8 = 7;
pub const CL_CMD_MODE: u8 = 8;
pub const CL_CMD_INV: u8 = 9;
pub const CL_CMD_STAT: u8 = 10;
pub const CL_CMD_DROP: u8 = 11;
pub const CL_CMD_GIVE: u8 = 12;
pub const CL_CMD_LOOK: u8 = 13;
pub const CL_CMD_INPUT1: u8 = 14;
pub const CL_CMD_INPUT2: u8 = 15;
pub const CL_CMD_INV_LOOK: u8 = 16;
pub const CL_CMD_LOOK_ITEM: u8 = 17;
pub const CL_CMD_USE: u8 = 18;
pub const CL_CMD_SETUSER: u8 = 19;
pub const CL_CMD_TURN: u8 = 20;
pub const CL_CMD_AUTOLOOK: u8 = 21;
pub const CL_CMD_INPUT3: u8 = 22;
pub const CL_CMD_INPUT4: u8 = 23;
pub const CL_CMD_RESET: u8 = 24;
pub const CL_CMD_SHOP: u8 = 25;
pub const CL_CMD_SKILL: u8 = 26;
pub const CL_CMD_INPUT5: u8 = 27;
pub const CL_CMD_INPUT6: u8 = 28;
pub const CL_CMD_INPUT7: u8 = 29;
pub const CL_CMD_INPUT8: u8 = 30;
pub const CL_CMD_EXIT: u8 = 31;
pub const CL_CMD_UNIQUE: u8 = 32;
pub const CL_PASSWD: u8 = 33;
pub const CL_CMD_CTICK: u8 = 255;

// =============================================================================
// Server Message Types (from client.h)
// =============================================================================

pub const SV_EMPTY: u8 = 0;
pub const SV_CHALLENGE: u8 = 1;
pub const SV_NEWPLAYER: u8 = 2;
pub const SV_SETCHAR_NAME1: u8 = 3;
pub const SV_SETCHAR_NAME2: u8 = 4;
pub const SV_SETCHAR_NAME3: u8 = 5;
pub const SV_SETCHAR_MODE: u8 = 6;
pub const SV_SETCHAR_ATTRIB: u8 = 7;
pub const SV_SETCHAR_SKILL: u8 = 8;
pub const SV_SETCHAR_HP: u8 = 12;
pub const SV_SETCHAR_ENDUR: u8 = 13;
pub const SV_SETCHAR_MANA: u8 = 14;
pub const SV_SETCHAR_AHP: u8 = 20;
pub const SV_SETCHAR_PTS: u8 = 21;
pub const SV_SETCHAR_GOLD: u8 = 22;
pub const SV_SETCHAR_ITEM: u8 = 23;
pub const SV_SETCHAR_WORN: u8 = 24;
pub const SV_SETCHAR_OBJ: u8 = 25;
pub const SV_TICK: u8 = 27;
pub const SV_LOOK1: u8 = 29;
pub const SV_SCROLL_RIGHT: u8 = 30;
pub const SV_SCROLL_LEFT: u8 = 31;
pub const SV_SCROLL_UP: u8 = 32;
pub const SV_SCROLL_DOWN: u8 = 33;
pub const SV_LOGIN_OK: u8 = 34;
pub const SV_SCROLL_RIGHTUP: u8 = 35;
pub const SV_SCROLL_RIGHTDOWN: u8 = 36;
pub const SV_SCROLL_LEFTUP: u8 = 37;
pub const SV_SCROLL_LEFTDOWN: u8 = 38;
pub const SV_LOOK2: u8 = 39;
pub const SV_LOOK3: u8 = 40;
pub const SV_LOOK4: u8 = 41;
pub const SV_SETTARGET: u8 = 42;
pub const SV_SETMAP2: u8 = 43;
pub const SV_SETORIGIN: u8 = 44;
pub const SV_SETMAP3: u8 = 45;
pub const SV_SETCHAR_SPELL: u8 = 46;
pub const SV_PLAYSOUND: u8 = 47;
pub const SV_EXIT: u8 = 48;
pub const SV_MSG: u8 = 49;
pub const SV_LOOK5: u8 = 50;
pub const SV_LOOK6: u8 = 51;
pub const SV_LOG: u8 = 52;
pub const SV_LOG0: u8 = 52;
pub const SV_LOG1: u8 = 53;
pub const SV_LOG2: u8 = 54;
pub const SV_LOG3: u8 = 55;
pub const SV_LOAD: u8 = 56;
pub const SV_CAP: u8 = 57;
pub const SV_MOD1: u8 = 58;
pub const SV_MOD2: u8 = 59;
pub const SV_MOD3: u8 = 60;
pub const SV_MOD4: u8 = 61;
pub const SV_MOD5: u8 = 62;
pub const SV_MOD6: u8 = 63;
pub const SV_MOD7: u8 = 64;
pub const SV_MOD8: u8 = 65;
pub const SV_SETMAP4: u8 = 66;
pub const SV_SETMAP5: u8 = 67;
pub const SV_SETMAP6: u8 = 68;
pub const SV_SETCHAR_AEND: u8 = 69;
pub const SV_SETCHAR_AMANA: u8 = 70;
pub const SV_SETCHAR_DIR: u8 = 71;
pub const SV_UNIQUE: u8 = 72;
pub const SV_IGNORE: u8 = 73;
pub const SV_SETMAP: u8 = 128; // 128-255 are used !!!

// =============================================================================
// Logout Reasons (from client.h)
// =============================================================================

pub const LO_CHALLENGE: u8 = 1;
pub const LO_IDLE: u8 = 2;
pub const LO_NOROOM: u8 = 3;
pub const LO_PARAMS: u8 = 4;
pub const LO_NONACTIVE: u8 = 5;
pub const LO_PASSWORD: u8 = 6;
pub const LO_SLOW: u8 = 7;
pub const LO_FAILURE: u8 = 8;
pub const LO_SHUTDOWN: u8 = 9;
pub const LO_TAVERN: u8 = 10;
pub const LO_VERSION: u8 = 11;
pub const LO_EXIT: u8 = 12;
pub const LO_USURP: u8 = 13;
pub const LO_KICKED: u8 = 14;

// =============================================================================
// Player States (from client.h)
// =============================================================================

pub const ST_CONNECT: u32 = 0;
pub const ST_NEW_CHALLENGE: u32 = 1;
pub const ST_LOGIN_CHALLENGE: u32 = 2;
pub const ST_NEWLOGIN: u32 = 3;
pub const ST_LOGIN: u32 = 4;
pub const ST_NEWCAP: u32 = 5;
pub const ST_CAP: u32 = 6;
pub const ST_NORMAL: u32 = 10;
pub const ST_CHALLENGE: u32 = 11;
pub const ST_EXIT: u32 = 12;

// =============================================================================
// Sprite Constants (from client.h)
// =============================================================================

pub const SPR_TUNDRA_GROUND: u16 = 1001;
pub const SPR_DESERT_GROUND: u16 = 1002;
pub const SPR_HELMET: u16 = 1003;
pub const SPR_BODY_ARMOR: u16 = 1004;
pub const SPR_LEG_ARMOR: u16 = 1005;
pub const SPR_SWORD: u16 = 1006;
pub const SPR_DAGGER: u16 = 1007;
pub const SPR_GROUND1: u16 = 1008;
pub const SPR_KEY: u16 = 1009;
pub const SPR_STONE_GROUND1: u16 = 1010;
pub const SPR_TORCH1: u16 = 1011;
pub const SPR_LIZARD_POOL: u16 = 1012;
pub const SPR_WOOD_GROUND: u16 = 1013;
pub const SPR_CLOAK: u16 = 1014;
pub const SPR_BELT: u16 = 1015;
pub const SPR_AMULET: u16 = 1016;
pub const SPR_BOOTS: u16 = 1017;
pub const SPR_ARM_ARMOR: u16 = 1018;
pub const SPR_TEMPLAR_POOL: u16 = 1019;
pub const SPR_TORCH2: u16 = 1026;
pub const SPR_TAVERN_GROUND: u16 = 1034;
pub const SPR_STONE_GROUND2: u16 = 1052;

pub const SPR_TEMPLAR: u16 = 2000;
pub const SPR_LIZARD: u16 = SPR_TEMPLAR + 1024;
pub const SPR_HARAKIM: u16 = SPR_LIZARD + 1024;
pub const SPR_MERCENARY: u16 = SPR_HARAKIM + 1024;

// =============================================================================
// Maximum Players
// =============================================================================

pub const MAXPLAYER: usize = 250;

// =============================================================================
// Buffer Sizes (from server.cpp)
// =============================================================================

pub const TBUFSIZE: usize = 4096 * 16;
pub const OBUFSIZE: usize = TBUFSIZE;

// =============================================================================
// Driver Error Codes (from driver.h)
// =============================================================================

pub const ERR_NONE: i32 = 0;
pub const ERR_SUCCESS: i32 = 1; // operation finished, successfully
pub const ERR_FAILED: i32 = 2; // failed and will never succeed

// =============================================================================
// Lab9 Constants (from lab9.h)
// =============================================================================

/// How many NPCs are giving out riddles in Lab 9
pub const RIDDLEGIVERS: usize = 5;

/// How many riddles each riddle giver knows
pub const MAX_RIDDLES: usize = 11;

/// How long before the time for the riddle is up (3 minutes)
pub const RIDDLE_TIMEOUT: i32 = 3 * 60 * TICKS;

pub const AREA_SIZE: i32 = 12;

/// Areas of knowledge define a riddlegiver
pub const RIDDLE_MIN_AREA: i32 = 21;
pub const RIDDLE_MAX_AREA: i32 = 25;

/// how many attempts a player has to solve.
pub const RIDDLE_ATTEMPTS: i32 = 3;

/// Number of true-false switch banks
pub const BANKS: usize = 5;
/// Number of switches per bank
pub const SWITCHES: usize = 6;
/// Number of questions available (PER_BLOCK chosen at random)
pub const BANK_QUESTIONS: usize = 8;

// =============================================================================
// Profiling Constants (from server.cpp)
// =============================================================================

pub const MAX_BEST: usize = 10;
/// Profiling frequency in seconds
pub const PROF_FREQ: i32 = 2;

// =============================================================================
// Driver Constants (from driverconstants.h)
// =============================================================================

pub const DR_IDLE: u32 = 0;
pub const DR_DROP: u32 = 1;
pub const DR_PICKUP: u32 = 2;
pub const DR_GIVE: u32 = 3;
pub const DR_USE: u32 = 4;
pub const DR_BOW: u32 = 5;
pub const DR_WAVE: u32 = 6;
pub const DR_TURN: u32 = 7;
pub const DR_SINGLEBUILD: u32 = 8;
pub const DR_AREABUILD1: u32 = 9;
pub const DR_AREABUILD2: u32 = 10;

// =============================================================================
// Attribute Indices (from SkillTab.h)
// =============================================================================

pub const AT_BRAVE: i32 = 0;
pub const AT_WILL: i32 = 1;
pub const AT_INT: i32 = 2;
pub const AT_AGIL: i32 = 3;
pub const AT_STREN: i32 = 4;
