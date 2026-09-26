//! Documented meanings of `Character.data[]` and `Item.data[]` slots, and their editor.
//!
//! Sources: the legend in `server/orig/driver.c`, `server/orig/use_driver.c`, and the Rust
//! drivers in `server/src/driver/`. Character meanings are for NPC templates; item meanings
//! depend on `Item::driver`.

use super::TemplateViewerApp;
use super::widgets::{ATTRIBUTE_NAMES, drag, flag_grid};
use eframe::egui;
use egui::emath::Numeric;
use mag_core::constants::{
    ItemFlags, SERVER_MAPX, SP_BLESS, SP_CURSE, SP_DISPEL, SP_ENHANCE, SP_HEAL, SP_LIGHT,
    SP_PROTECT, SP_STUN, TICKS,
};
use mag_core::skills;
use mag_core::types::Item;

/// How a data slot's value is interpreted.
#[derive(Clone, Copy, Debug)]
pub(super) enum FieldKind {
    /// Plain number.
    Number,
    /// Signed number stored in an unsigned slot.
    Signed,
    /// Zero / non-zero toggle.
    Bool,
    /// Absolute server ticker timestamp.
    Ticks,
    /// Duration in ticks.
    Duration,
    /// Map tile packed as `x + y * SERVER_MAPX`.
    MapPos,
    /// Character instance id.
    CharacterId,
    /// Kill-list entry: character id in the low 16 bits, unique id above.
    EnemyEntry,
    /// Item template id.
    ItemTemplate,
    /// Character template id.
    CharacterTemplate,
    /// Skill number.
    Skill,
    /// NPC group id; `0x10000 + owner` marks a summon's group.
    Group,
    /// One of a fixed set of values.
    Enum(&'static [(i64, &'static str)]),
    /// Bit flags; an empty list means "any of 32 unnamed bits".
    Flags(&'static [(u32, &'static str)]),
}

/// Documentation for one data slot, or `count` consecutive slots.
#[derive(Debug)]
pub(super) struct FieldDef {
    pub(super) index: usize,
    pub(super) count: usize,
    pub(super) name: &'static str,
    pub(super) kind: FieldKind,
    /// Written by the server during play; editing it in a template is usually pointless.
    pub(super) runtime: bool,
    pub(super) help: &'static str,
}

/// Configuration slot.
const fn cfg(index: usize, name: &'static str, kind: FieldKind, help: &'static str) -> FieldDef {
    FieldDef {
        index,
        count: 1,
        name,
        kind,
        runtime: false,
        help,
    }
}

/// Runtime (server-written) slot.
const fn rt(index: usize, name: &'static str, kind: FieldKind, help: &'static str) -> FieldDef {
    FieldDef {
        index,
        count: 1,
        name,
        kind,
        runtime: true,
        help,
    }
}

/// Run of `count` consecutive slots sharing one meaning.
const fn range(
    index: usize,
    count: usize,
    runtime: bool,
    name: &'static str,
    kind: FieldKind,
    help: &'static str,
) -> FieldDef {
    FieldDef {
        index,
        count,
        name,
        kind,
        runtime,
        help,
    }
}

use FieldKind::{
    Bool, CharacterId, CharacterTemplate, Duration, EnemyEntry, Enum, Flags, Group, ItemTemplate,
    MapPos, Number, Signed, Skill, Ticks,
};

const PREVENT_FIGHT: &[(i64, &str)] = &[
    (-1, "Defend evil"),
    (0, "Don't interfere"),
    (1, "Defend good"),
];
const SPECIAL_DRIVER: &[(i64, &str)] = &[
    (0, "Generic NPC"),
    (1, "Stunrun"),
    (2, "City attack"),
    (3, "Malte"),
];
const SPECIAL_SUB_DRIVER: &[(i64, &str)] = &[
    (0, "None"),
    (1, "City guard (city)"),
    (2, "Shiva (candle spawner)"),
    (3, "City guard (outpost)"),
    (4, "Trap hit (not ported)"),
    (5, "Greeter (god swords)"),
];
const DIRECTIONS: &[(i64, &str)] = &[
    (0, "None"),
    (1, "Right"),
    (2, "Left"),
    (3, "Up"),
    (4, "Down"),
    (5, "Left-up"),
    (6, "Left-down"),
    (7, "Right-up"),
    (8, "Right-down"),
];
const GARBAGE: &[(i64, &str)] = &[
    (0, "Off"),
    (1, "Donate at (497, 512)"),
    (2, "Donate at (560, 542)"),
];
const JOB_IMPORTANCE: &[(i64, &str)] = &[(0, "Low"), (1, "Medium"), (2, "High / fighting")];
const CREATE_LIGHT: &[(i64, &str)] = &[
    (0, "Never"),
    (1, "When dark"),
    (2, "Dark and not resting"),
    (3, "Dark and fighting"),
];
const KNOWLEDGE_AREA: &[(i64, &str)] = &[
    (0, "General"),
    (1, "Thieves"),
    (2, "Castle"),
    (3, "Grolms"),
    (4, "Aston"),
    (5, "Thieves 2"),
    (6, "Tomb"),
    (7, "Joe"),
    (8, "Purple"),
    (9, "Skeleton lord"),
    (10, "Outlaws"),
    (11, "Magic maze"),
    (12, "Lizards"),
    (13, "Underground I"),
    (14, "Knights"),
    (15, "Mine"),
    (16, "Black Stronghold"),
    (17, "Nest"),
    (21, "Riddle 1"),
    (22, "Riddle 2"),
    (23, "Riddle 3"),
    (24, "Riddle 4"),
    (25, "Riddle 5"),
    (12345, "All areas"),
];
const KEYWORD_ACTION: &[(i64, &str)] = &[
    (0, "None"),
    (1, "Password: warn, then attack near home"),
    (2, "Attack only near home"),
];
const QUEUED_SPELLS: &[(u32, &str)] = &[
    (SP_LIGHT, "Light"),
    (SP_PROTECT, "Protect"),
    (SP_ENHANCE, "Enhance"),
    (SP_BLESS, "Bless"),
    (SP_HEAL, "Heal"),
    (SP_CURSE, "Curse"),
    (SP_STUN, "Stun"),
    (SP_DISPEL, "Dispel"),
];

/// NPC-template meanings of `Character.data[0..100]`.
pub(super) const CHARACTER_DATA_FIELDS: &[FieldDef] = &[
    cfg(
        0,
        "Merchant buy filter",
        ItemTemplate,
        "Merchants buy item types matching this template's armor/weapon/... flags. Other drivers use slot 0 privately (spawner link, creator).",
    ),
    range(
        1,
        9,
        true,
        "Driver-private",
        Number,
        "Reserved for exclusive/special drivers (Malte, lab keeper, stunrun, city attack).",
    ),
    range(
        10,
        9,
        false,
        "Patrol stop",
        MapPos,
        "Patrol waypoints; patrol is on when the first is set. A zero entry wraps back to the first.",
    ),
    rt(
        19,
        "Next patrol stop",
        Number,
        "Slot index (10..=18) of the patrol stop being walked to.",
    ),
    range(
        20,
        4,
        false,
        "Door to close",
        MapPos,
        "Door tiles this NPC closes when idle.",
    ),
    cfg(
        24,
        "Prevent fights",
        Enum(PREVENT_FIGHT),
        "Intervene in fights by alignment (any negative = defend evil, positive = defend good).",
    ),
    cfg(
        25,
        "Special driver",
        Enum(SPECIAL_DRIVER),
        "Replaces the generic NPC AI. Special drivers reuse many slots below for their own state.",
    ),
    cfg(
        26,
        "Special sub-driver",
        Enum(SPECIAL_SUB_DRIVER),
        "Extra behaviour hooked into the generic AI.",
    ),
    rt(
        27,
        "Password heard at",
        Ticks,
        "Last time text[6] (password/stop) was heard; suppresses keyword aggression for 120 s.",
    ),
    rt(
        28,
        "Exp earned",
        Number,
        "Exp gained since creation; ghost companions hand it to their master on \"transfer\".",
    ),
    cfg(
        29,
        "Resting position",
        MapPos,
        "Home tile: returns here when idle; centre for random walk and keyword radius.",
    ),
    cfg(
        30,
        "Resting direction",
        Enum(DIRECTIONS),
        "Direction to face while resting.",
    ),
    cfg(
        31,
        "Protect template",
        CharacterTemplate,
        "Help any victim created from this character template.",
    ),
    range(
        32,
        4,
        false,
        "Light to keep lit",
        MapPos,
        "Light items this NPC re-activates.",
    ),
    rt(
        36,
        "Frustration",
        Number,
        "+1 per failed action, reset on success; high values make the NPC skip stops or use magic.",
    ),
    cfg(
        37,
        "Greeting / talked-to #1",
        CharacterId,
        "Set non-zero in the template to enable greetings; then remembers who was greeted.",
    ),
    range(
        38,
        3,
        true,
        "Talked-to",
        CharacterId,
        "Recently greeted characters (cleared every 5 minutes).",
    ),
    cfg(
        41,
        "Light template",
        ItemTemplate,
        "Turns on map items of this template at night.",
    ),
    cfg(
        42,
        "Group",
        Group,
        "NPCs in the same group never fight each other. 1 = generic monsters, 27 = Black Stronghold.",
    ),
    range(
        43,
        4,
        false,
        "Allowed group",
        Number,
        "If the first is set, attack everyone NOT in these groups. 65536 = allow all players and companions.",
    ),
    cfg(
        47,
        "Collect garbage",
        Enum(GARBAGE),
        "Pick up takeable items and donate them; other non-zero values pick a random spot.",
    ),
    cfg(
        48,
        "Death-text chance",
        Number,
        "Percent chance of saying text[3] on death.",
    ),
    cfg(
        49,
        "Wanted item",
        ItemTemplate,
        "Item template accepted as a quest turn-in.",
    ),
    cfg(
        50,
        "Teach skill",
        Skill,
        "Skill taught in exchange for the wanted item.",
    ),
    cfg(
        51,
        "Reward exp",
        Number,
        "Exp given in exchange for the wanted item.",
    ),
    cfg(
        52,
        "Shout code",
        Number,
        "Shouts for help with this code (text[4]) when attacked.",
    ),
    cfg(53, "Help code", Number, "Answers shouts with this code."),
    rt(
        54,
        "Shout place",
        MapPos,
        "Where a heard shout came from (0 when we shouted ourselves).",
    ),
    rt(
        55,
        "Shout time",
        Ticks,
        "When the shout happened; answered for 120 s.",
    ),
    rt(
        56,
        "Next greeting",
        Ticks,
        "A greeting may fire after this time.",
    ),
    rt(57, "Patrol rest until", Ticks, "Next patrol move time."),
    rt(
        58,
        "Job importance",
        Enum(JOB_IMPORTANCE),
        "Current priority; also compared against \"Create light\".",
    ),
    cfg(
        59,
        "Help group",
        Number,
        "Help members of this group when they are attacked.",
    ),
    cfg(
        60,
        "Random walk interval",
        Duration,
        "Non-zero enables random walking; time between walks.",
    ),
    rt(
        61,
        "Random walk countdown",
        Number,
        "Countdown to the next random walk, reloaded from the interval.",
    ),
    cfg(
        62,
        "Create light",
        Enum(CREATE_LIGHT),
        "When to cast Light on itself.",
    ),
    cfg(
        63,
        "Master",
        CharacterId,
        "Obey and protect this character (heal it, help against its attackers).",
    ),
    cfg(
        64,
        "Self-destruct time",
        Duration,
        "Companion lifetime; values under 15 minutes are relative and become an absolute time on spawn.",
    ),
    rt(
        65,
        "Help friend",
        CharacterId,
        "Friend under attack to buff/heal next.",
    ),
    cfg(
        66,
        "Give item",
        ItemTemplate,
        "Item template given in exchange for the wanted item. On corpses: who may loot.",
    ),
    rt(
        67,
        "Greet list reset",
        Ticks,
        "Last time the talked-to list was cleared.",
    ),
    cfg(
        68,
        "Knowledge level",
        Number,
        "Minimum knowledge value this NPC can answer.",
    ),
    cfg(
        69,
        "Follow",
        CharacterId,
        "Follow this character (companions, gargoyles).",
    ),
    rt(
        70,
        "God help at",
        Ticks,
        "Cooldown for summoning Shadow of Peace / refilling mana.",
    ),
    cfg(
        71,
        "Talkative",
        Number,
        "Talk level; > 0 answers knowledge questions. Companions use -10 for canned texts only.",
    ),
    cfg(
        72,
        "Knowledge area",
        Enum(KNOWLEDGE_AREA),
        "Which area's knowledge base this NPC answers from.",
    ),
    cfg(
        73,
        "Random walk distance",
        Number,
        "Maximum tiles (Manhattan) from the resting position.",
    ),
    rt(
        74,
        "Last ghost cast",
        Ticks,
        "Ghost companion spell cooldown.",
    ),
    rt(75, "Last stun cast", Ticks, "Stun spell cooldown."),
    rt(
        76,
        "Last enemy position",
        MapPos,
        "Where the enemy was last seen; searched for 30 s.",
    ),
    rt(
        77,
        "Last enemy seen",
        Ticks,
        "When the enemy was last seen.",
    ),
    rt(
        78,
        "Panic until",
        Ticks,
        "Attacked by an unseen character: panics until this time.",
    ),
    cfg(
        79,
        "Patrol rest time",
        Duration,
        "How long to wait at each patrol stop.",
    ),
    range(
        80,
        12,
        true,
        "Kill list",
        EnemyEntry,
        "Characters to attack on sight; newest first.",
    ),
    rt(
        92,
        "Awake timer",
        Duration,
        "Stays active while > 0; refreshed when seeing/fighting/hearing a shout.",
    ),
    cfg(
        93,
        "Keyword radius",
        Number,
        "Tile radius around the resting position used by the keyword action.",
    ),
    rt(
        94,
        "Last warning",
        Ticks,
        "text[8] warnings are rate-limited to once per 15 ticks.",
    ),
    cfg(
        95,
        "Keyword action",
        Enum(KEYWORD_ACTION),
        "Territory behaviour; see NPCs.md. The radius is the keyword radius.",
    ),
    rt(
        96,
        "Queued spells",
        Flags(QUEUED_SPELLS),
        "Spells currently being cast on this character.",
    ),
    rt(
        97,
        "Usurped by",
        CharacterId,
        "Character controlling this NPC via usurp.",
    ),
    rt(
        98,
        "Companion timeout / body age",
        Ticks,
        "Ghost companions die at this time if they lose sight of their master; bodies use it as an age counter.",
    ),
    rt(
        99,
        "Populate slot / player body",
        Number,
        "Legacy populate index; on corpses non-zero marks a player body.",
    ),
];

const DOOR: &[FieldDef] = &[
    cfg(
        0,
        "Key template / lock",
        ItemTemplate,
        "0 = no lock; 1..65499 = key item template; 65500 = no key opens it; 65501 = star door; 65502 = circle door.",
    ),
    cfg(
        1,
        "Locked",
        Bool,
        "Refuses to open without a key or lock pick. Set again when closed with a key.",
    ),
    cfg(
        2,
        "Lockpick difficulty",
        Number,
        "Pick succeeds when skill >= this + rand(20); 0 = cannot be picked.",
    ),
    cfg(3, "Key consumed", Bool, "The key vanishes when used."),
];
const SPELL_WEAPON: &[FieldDef] = &[
    cfg(
        0,
        "Spell sprite",
        Number,
        "Spell icon (0 = 93). Uncertain for these items.",
    ),
    cfg(
        1,
        "Spell template",
        Number,
        "Spell item template (0 = 101). Uncertain for these items.",
    ),
    cfg(
        2,
        "Replacement weapon",
        ItemTemplate,
        "Weapon given after the spell is cast.",
    ),
];
const XY_DEST: &[FieldDef] = &[
    cfg(0, "Destination X", Number, "Target tile X."),
    cfg(1, "Destination Y", Number, "Target tile Y."),
];
const LAB_NR: &[(i64, &str)] = &[
    (0, "Plain teleport"),
    (1, "Grolms"),
    (2, "Lizards"),
    (3, "Spellcasters"),
    (4, "Knights"),
    (5, "Undead"),
    (6, "Light & dark"),
    (7, "Underwater"),
    (8, "Forest / golems"),
    (9, "Riddles"),
];
const PILE_LEVEL: &[(i64, &str)] = &[
    (0, "Silver, small jewels, skeleton"),
    (1, "Silver, medium jewels, golem"),
    (2, "Gold, big jewels, gargoyle"),
];
const TRAP_TYPE: &[(i64, &str)] = &[
    (0, "Arrow (250 damage)"),
    (1, "Attack trigger"),
    (2, "Acid (destroys a worn item)"),
    (3, "Spawn character template 323"),
    (4, "Spawn character template 324"),
];
const ATTRIBUTE: &[(i64, &str)] = &[
    (0, ATTRIBUTE_NAMES[0]),
    (1, ATTRIBUTE_NAMES[1]),
    (2, ATTRIBUTE_NAMES[2]),
    (3, ATTRIBUTE_NAMES[3]),
    (4, ATTRIBUTE_NAMES[4]),
];

/// Documented meanings for an item driver's `data[]` slots.
///
/// # Arguments
///
/// * `driver` - `Item::driver`.
///
/// # Returns
///
/// * `(driver name, slot docs)`; unknown drivers return `("Unknown driver", [])`.
pub(super) fn item_driver_info(driver: u8) -> (&'static str, &'static [FieldDef]) {
    const NONE: &[FieldDef] = &[];
    // Each arm is a `const` so its `&[cfg(..)]` slice is promoted to `'static`.
    macro_rules! drivers {
        ($($n:literal => $v:expr),* $(,)?) => {
            match driver {
                $($n => {
                    const INFO: (&str, &[FieldDef]) = $v;
                    INFO
                })*
                _ => ("Unknown driver", NONE),
            }
        };
    }
    drivers! {
        0 => ("None", NONE),
        1 => ("Create item (chest)", &[
            cfg(0, "Item given", ItemTemplate, "Template handed to the user; refilled when the chest deactivates."),
            range(1, 9, false, "Required wall", MapPos, "Chest only resets while every listed tile holds a fast wall (driver 26). List ends at the first 0."),
        ]),
        2 => ("Door", DOOR),
        3 => ("Lock pick", &[cfg(0, "Pick bonus", Number, "Added to Lock Picking when picking a door.")]),
        4 => ("Mix potion", NONE),
        5 => ("Sword in stone", &[cfg(0, "Result item", ItemTemplate, "Item created (needs STR >= 100).")]),
        6 => ("Teleport", &[
            cfg(0, "Destination X", Number, "Target tile X (plain teleport)."),
            cfg(1, "Destination Y", Number, "Target tile Y (plain teleport)."),
            cfg(2, "Labyrinth", Enum(LAB_NR), "Non-zero: labyrinth solved, transfer to that lab's keeper."),
            cfg(3, "Labyrinth exp", Number, "Exp given to the solver."),
        ]),
        7 => ("Tombstone", &[rt(0, "Corpse", CharacterId, "Body character searched when the grave is used.")]),
        8 => ("Skill scroll", &[
            cfg(0, "Skill", Skill, "Skill learned or raised by one."),
            cfg(1, "Learn only", Bool, "Refuse if the skill is already known."),
        ]),
        9 => ("Crystal spawner", &[
            cfg(0, "Group", Number, "Group id of spawned NPCs."),
            cfg(1, "Max population", Number, "Keep spawning until this many group members live."),
        ]),
        10 => ("Attribute scroll", &[cfg(0, "Attribute", Enum(ATTRIBUTE), "Attribute raised by one.")]),
        11 => ("Hitpoint scroll", &[cfg(0, "Amount", Number, "Hitpoints added.")]),
        12 => ("Endurance scroll", &[cfg(0, "Amount", Number, "Endurance added.")]),
        13 => ("Mana scroll", &[cfg(0, "Amount", Number, "Mana added.")]),
        14 => ("Lizard-teeth necklace", &[cfg(0, "Max template", ItemTemplate, "Stops upgrading once the template reaches this id.")]),
        15 => ("Labyrinth entrance", NONE),
        16 => ("Ladder", &[
            cfg(0, "Offset X", Signed, "X offset from the ladder."),
            cfg(1, "Offset Y", Signed, "Y offset from the ladder."),
        ]),
        17 => ("Assembly", &[
            range(0, 9, false, "Required part", ItemTemplate, "Part that fits this slot; cleared to 0 once filled."),
            cfg(9, "Result", ItemTemplate, "Created when every slot is filled."),
        ]),
        18 => ("Skua weapon", SPELL_WEAPON),
        19 => ("Lever", &[cfg(0, "Target", MapPos, "Tile whose item is used when the lever is pulled.")]),
        20 => ("Door", DOOR),
        21 => ("Respawn trigger", &[
            cfg(1, "Key", ItemTemplate, "Cursor item required and consumed (0 = none)."),
            cfg(2, "Character", CharacterTemplate, "Respawned at its template home."),
        ]),
        22 => ("Rubble pile", &[cfg(0, "Level", Enum(PILE_LEVEL), "Loot and monster table.")]),
        23 => ("Recall scroll", &[
            cfg(0, "Destination X", Number, "Target tile X (set at logout for lag scrolls)."),
            cfg(1, "Destination Y", Number, "Target tile Y."),
            rt(2, "Created at", Ticks, "Refused when older than 4 minutes; 0 = no check."),
        ]),
        24 => ("Ring crafting", NONE),
        25 => ("Mineable wall", &[
            cfg(0, "Next template", ItemTemplate, "Becomes this item when broken."),
            cfg(1, "Hardness", Number, "Remaining hardness, reduced by each dig."),
            cfg(2, "Mine state", Number, "Uncertain: returned by mining but unused."),
            cfg(3, "Rebuild", Bool, "Respawn the original wall after 15 minutes."),
        ]),
        26 => ("Fast wall", NONE),
        27 => ("Wall spawner", &[
            cfg(0, "Group", Number, "Group id of spawned NPCs."),
            cfg(1, "Character", CharacterTemplate, "NPC spawned."),
            cfg(2, "Max count", Number, "Maximum living group members."),
            range(3, 7, false, "Required wall", MapPos, "Spawns only while every listed tile holds a fast wall. List ends at the first 0."),
        ]),
        28 => ("Gargoyle summon", NONE),
        29 => ("Undead grave", &[rt(0, "Spawned undead", CharacterId, "Respawned once this NPC dies.")]),
        30 => ("Item exchange", &[
            cfg(0, "Result", ItemTemplate, "Item given."),
            cfg(1, "Required", ItemTemplate, "Cursor item required and consumed."),
        ]),
        31 => ("Holy water", &[cfg(0, "Damage", Number, "Damage to an undead it is given to.")]),
        32 => ("Amulet crafting", NONE),
        33 => ("Pentagram", &[
            cfg(0, "Value", Number, "Pentagram number; exp is value^2/7 + 1."),
            range(1, 3, true, "Guard", CharacterId, "Spawned enemies (re-spawned when dead)."),
            rt(8, "Activated by", CharacterId, "Player who activated it."),
            cfg(9, "Enemy level", Number, "Level of spawned enemies; also caps the activator's rank."),
        ]),
        34 => ("Seyan'Du shrine", &[cfg(0, "Shrine bit", Flags(&[]), "One unique bit per shrine, recorded on the visitor; the count sets sword power.")]),
        35 => ("Seyan'Du door", DOOR),
        36 => ("Lab 13 portal", NONE),
        37 => ("Floor trap", &[
            cfg(0, "Trap type", Enum(TRAP_TYPE), "Effect when stepped on."),
            cfg(1, "Disarm link", MapPos, "Acid only: harmless while the item on this tile is active. Uncertain."),
        ]),
        38 => ("Lab 13 final portal", NONE),
        39 => ("Purple One weapon", SPELL_WEAPON),
        40 => ("Seyan'Du sword", &[rt(0, "Owner", CharacterId, "Owner; 0 resets the sword to template 683.")]),
        41 => ("Offering shrine", NONE),
        42 => ("Random item", &[range(0, 10, false, "Candidate", ItemTemplate, "One is chosen at random. List ends at the first 0.")]),
        43 => ("Spider web", &[range(1, 3, true, "Spider", CharacterId, "Spawned spiders.")]),
        44 => ("Undead-slaying weapon", NONE),
        45 => ("Seyan'Du portal", XY_DEST),
        46 => ("Labyrinth exit", XY_DEST),
        47 => ("Arena portal", &[
            cfg(0, "Opponent spawn", MapPos, "Where the opponent is dropped."),
            cfg(1, "Arena top-left", MapPos, "Arena rectangle start."),
            cfg(2, "Arena bottom-right", MapPos, "Arena rectangle end."),
        ]),
        48 => ("Spell scroll", &[
            cfg(0, "Spell", Skill, "Light, Enhance, Protect, Bless, Magic Shield, Curse or Stun."),
            cfg(1, "Power", Number, "Spell power."),
            cfg(2, "Charges", Number, "Remaining casts."),
        ]),
        49 => ("Blood pentagram", &[
            rt(0, "Frame", Number, "Animation frame (0 = idle)."),
            cfg(1, "Base sprite", Number, "Sprite = base + frame."),
        ]),
        50 => ("NPC summoner", &[cfg(0, "Character", CharacterTemplate, "NPC spawned and linked to the user.")]),
        51 => ("Rotatable tile", &[
            cfg(0, "Base sprite", Number, "Sprite = base + rotation."),
            cfg(1, "Rotation", Number, "0..=3; read by the star/circle doors."),
        ]),
        52 => ("Personal item", &[rt(0, "Owner", CharacterId, "Bound to its first wearer (0 = unbound).")]),
        53 => ("Create personal item", &[
            cfg(0, "Item given", ItemTemplate, "Template handed to the user."),
            cfg(1, "Bind to user", Bool, "The created item is engraved and bound to the user."),
        ]),
        54 => ("Create item + alarm", &[cfg(0, "Item given", ItemTemplate, "Template handed to the user, then nearby NPCs attack.")]),
        55 => ("Shrine of change", NONE),
        56 => ("Greenling ball", &[
            cfg(0, "Template offset", Number, "Spawns character template 553 + this."),
            range(1, 3, true, "Greenling", CharacterId, "Spawned greenlings."),
        ]),
        57 => ("Explorer point", &[
            cfg(0, "Explored bits (46)", Flags(&[]), "Bits recorded in the visitor's data[46]; already set = visited."),
            cfg(1, "Explored bits (47)", Flags(&[]), "Bits recorded in the visitor's data[47]."),
            cfg(2, "Explored bits (48)", Flags(&[]), "Bits recorded in the visitor's data[48]."),
            cfg(3, "Explored bits (49)", Flags(&[]), "Bits recorded in the visitor's data[49]."),
            cfg(4, "Base exp", Number, "Exp = base/2 + rand(base), capped at 10% of total points."),
        ]),
        58 => ("Grolm summon", NONE),
        59 => ("Gold", &[cfg(0, "Gold", Number, "Gold given (x100 silver).")]),
        60 => ("Ice egg / cloak", NONE),
        61 => ("Lab 8 key part", &[
            cfg(0, "Fits part A", ItemTemplate, "Cursor part that fits."),
            cfg(1, "Result A", ItemTemplate, "Result when combined with part A."),
            cfg(2, "Fits part B", ItemTemplate, "Optional second fitting part."),
            cfg(3, "Result B", ItemTemplate, "Result when combined with part B."),
        ]),
        62 => ("Teleport tile", XY_DEST),
        63 => ("Lab 8 offering shrine", &[
            cfg(0, "Offering", ItemTemplate, "Item that must be offered."),
            cfg(1, "Gift", ItemTemplate, "Item given back."),
        ]),
        64 => ("Lab 8 money shrine", &[
            cfg(0, "Minimum offering", Number, "Money required (silver)."),
            cfg(1, "Destination X", Number, "Teleport target X."),
            cfg(2, "Destination Y", Number, "Teleport target Y."),
        ]),
        65 => ("Lab 9 switch", &[
            cfg(0, "Bank", Number, "Switch bank 1..=5."),
            rt(1, "State", Bool, "Flipped on use; counts as true when 0."),
        ]),
        66 => ("Lab 9 door", &[
            rt(1, "Closed", Bool, "1 = closed."),
            cfg(3, "Bank", Number, "Switch bank reset when the door closes."),
        ]),
        67 => ("Garbage can", NONE),
        68 => ("Soulstone", &[
            rt(0, "Rank", Number, "Rank derived from the stored exp."),
            rt(1, "Exp", Number, "Stored exp."),
        ]),
        69 => ("Fire floor", NONE),
    }
}

/// Hidden items: extra search difficulty, independent of driver.
const HIDE_DIFFICULTY: FieldDef = cfg(
    9,
    "Hide difficulty",
    Number,
    "Added to the distance score needed to spot this hidden item.",
);

/// Consumables that cast a spell when used (driver 0 with a duration).
const SPELL_CONSUMABLE: [FieldDef; 2] = [
    cfg(
        0,
        "Spell sprite",
        Number,
        "Spell icon shown while active (0 = 93).",
    ),
    cfg(
        1,
        "Spell template",
        Number,
        "Spell item template (0 = 101).",
    ),
];

/// All documented slot meanings for one item, based on its driver and flags.
///
/// # Arguments
///
/// * `item` - Item template or instance.
///
/// # Returns
///
/// * Slot docs; driver-specific entries win over driver-independent ones.
pub(super) fn item_data_fields(item: &Item) -> Vec<&'static FieldDef> {
    let (_, driver_defs) = item_driver_info(item.driver);
    let mut defs: Vec<&'static FieldDef> = driver_defs.iter().collect();
    if item.driver == 0 && item.duration != 0 {
        defs.extend(SPELL_CONSUMABLE.iter());
    }
    if item.flags & ItemFlags::IF_HIDDEN.bits() != 0 && slot_def(&defs, 9).is_none() {
        defs.push(&HIDE_DIFFICULTY);
    }
    defs
}

/// Character data slot docs as a reference list.
pub(super) fn character_data_fields() -> Vec<&'static FieldDef> {
    CHARACTER_DATA_FIELDS.iter().collect()
}

/// The doc covering slot `idx`, with the slot's position within a multi-slot run.
fn slot_def<'a>(defs: &[&'a FieldDef], idx: usize) -> Option<(&'a FieldDef, usize)> {
    defs.iter()
        .find(|def| (def.index..def.index + def.count).contains(&idx))
        .map(|def| (*def, idx - def.index))
}

/// Data slot integer type (`i32` for characters, `u32` for items).
pub(super) trait DataWord: Numeric + Default + PartialEq {
    /// Value widened to `i64`.
    fn to_i64(self) -> i64;
    /// Value from `i64`, saturating at the type's bounds.
    fn from_i64(v: i64) -> Self;
    /// Raw 32-bit pattern.
    fn to_bits(self) -> u32;
    /// Value from a raw 32-bit pattern.
    fn from_bits(bits: u32) -> Self;
}

impl DataWord for i32 {
    fn to_i64(self) -> i64 {
        i64::from(self)
    }
    fn from_i64(v: i64) -> Self {
        v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
    }
    fn to_bits(self) -> u32 {
        self as u32
    }
    fn from_bits(bits: u32) -> Self {
        bits as i32
    }
}

impl DataWord for u32 {
    fn to_i64(self) -> i64 {
        i64::from(self)
    }
    fn from_i64(v: i64) -> Self {
        v.clamp(0, i64::from(u32::MAX)) as u32
    }
    fn to_bits(self) -> u32 {
        self
    }
    fn from_bits(bits: u32) -> Self {
        bits
    }
}

/// Human-readable form of a packed map position.
fn map_pos_label(v: i64) -> Option<String> {
    let mapx = i64::from(SERVER_MAPX);
    if v <= 0 {
        return None;
    }
    let (x, y) = (v % mapx, v / mapx);
    let area =
        mag_core::area::get_area_m(x as i32, y as i32).unwrap_or_else(|| "Unknown".to_owned());
    Some(format!("({x}, {y}) {area}"))
}

impl TemplateViewerApp {
    /// "Driver Data" section: documented, named slots with type-aware editors.
    ///
    /// Documented configuration slots are always shown; runtime and undocumented
    /// slots only when non-zero or when "show all" is on.
    ///
    /// # Arguments
    ///
    /// * `ui` - Target UI.
    /// * `id` - Unique id prefix for the grid and its widgets.
    /// * `data` - The record's data slots.
    /// * `defs` - Slot docs for this record.
    pub(super) fn ui_data_fields<T: DataWord>(
        &mut self,
        ui: &mut egui::Ui,
        id: &str,
        data: &mut [T],
        defs: &[&FieldDef],
    ) {
        ui.separator();
        crate::centered_heading(ui, "Driver Data");
        ui.checkbox(
            &mut self.show_all_data_fields,
            "Show zero-valued runtime and undocumented slots",
        );

        egui::Grid::new(id)
            .num_columns(4)
            .spacing([20.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                for header in ["Slot", "Field", "Value", "Meaning"] {
                    ui.strong(header);
                }
                ui.end_row();

                for (idx, value) in data.iter_mut().enumerate() {
                    let def = slot_def(defs, idx);
                    let documented_config = def.is_some_and(|(d, _)| !d.runtime);
                    if !documented_config && !self.show_all_data_fields && *value == T::default() {
                        continue;
                    }

                    ui.label(format!("data[{idx}]"));
                    match def {
                        Some((def, ordinal)) => {
                            let name = if def.count > 1 {
                                format!("{} #{}", def.name, ordinal + 1)
                            } else {
                                def.name.to_owned()
                            };
                            let label = if def.runtime {
                                egui::RichText::new(format!("{name} (runtime)")).weak()
                            } else {
                                egui::RichText::new(name)
                            };
                            ui.label(label).on_hover_text(def.help);
                            ui_data_value(ui, egui::Id::new((id, idx)), value, def.kind);
                            self.ui_data_meaning(ui, value.to_i64(), def.kind);
                        }
                        None => {
                            ui.weak("undocumented");
                            drag(ui, value);
                            ui.label("");
                        }
                    }
                    ui.end_row();
                }
            });
    }

    /// Decoded meaning of a slot value (names, coordinates, durations).
    fn ui_data_meaning(&mut self, ui: &mut egui::Ui, v: i64, kind: FieldKind) {
        let text = match kind {
            FieldKind::MapPos => map_pos_label(v),
            FieldKind::Duration if v != 0 => Some(format!("{:.1} s", v as f64 / f64::from(TICKS))),
            FieldKind::Skill => usize::try_from(v)
                .ok()
                .map(skills::get_skill_name)
                .filter(|name| !name.is_empty())
                .map(str::to_owned),
            FieldKind::CharacterTemplate => self.record_name(&self.character_templates, v),
            FieldKind::CharacterId => self.record_name(&self.characters, v),
            FieldKind::EnemyEntry => self.record_name(&self.characters, v & 0xFFFF),
            FieldKind::Group if v & 0x1_0000 != 0 => {
                Some(format!("Summon of character {}", v & 0xFFFF))
            }
            FieldKind::ItemTemplate if v > 0 => {
                let name = self.record_name(&self.item_templates, v);
                ui.horizontal(|ui| {
                    ui.label(name.unwrap_or_else(|| "(no such template)".to_owned()));
                    if ui.small_button("View").clicked() {
                        self.item_popup_id = u32::try_from(v).ok();
                    }
                });
                return;
            }
            _ => None,
        };
        ui.label(text.unwrap_or_default());
    }

    /// `"name"` of `records[v]`, if `v` is a valid non-zero index.
    fn record_name<R: NamedRecord>(&self, records: &[R], v: i64) -> Option<String> {
        let idx = usize::try_from(v).ok().filter(|&i| i != 0)?;
        let name = records.get(idx)?.record_name();
        Some(if name.is_empty() {
            format!("#{idx}")
        } else {
            name.to_owned()
        })
    }
}

/// Records with a display name.
trait NamedRecord {
    fn record_name(&self) -> &str;
}

impl NamedRecord for Item {
    fn record_name(&self) -> &str {
        self.get_name()
    }
}

impl NamedRecord for mag_core::types::Character {
    fn record_name(&self) -> &str {
        self.get_name()
    }
}

/// Type-aware editor for one data slot.
fn ui_data_value<T: DataWord>(ui: &mut egui::Ui, id: egui::Id, value: &mut T, kind: FieldKind) {
    match kind {
        FieldKind::Bool => {
            let mut on = value.to_i64() != 0;
            if ui.checkbox(&mut on, "").changed() {
                *value = T::from_i64(i64::from(on));
            }
        }
        FieldKind::Signed => {
            let mut signed = value.to_bits() as i32;
            if drag(ui, &mut signed).changed() {
                *value = T::from_bits(signed as u32);
            }
        }
        FieldKind::Enum(options) => {
            let current = value.to_i64();
            let selected = options
                .iter()
                .find(|(v, _)| *v == current)
                .map_or("Other", |(_, name)| name);
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt(id)
                    .selected_text(selected)
                    .show_ui(ui, |ui| {
                        for (option, name) in options {
                            if ui.selectable_label(*option == current, *name).clicked() {
                                *value = T::from_i64(*option);
                            }
                        }
                    });
                drag(ui, value);
            });
        }
        FieldKind::Flags(names) => {
            let mut bits = u64::from(value.to_bits());
            ui.vertical(|ui| {
                ui.add(
                    egui::DragValue::new(&mut bits)
                        .hexadecimal(8, false, true)
                        .range(0..=u32::MAX),
                );
                let unnamed: Vec<(u64, String)>;
                let entries: Vec<(u64, &str)> = if names.is_empty() {
                    unnamed = (0..32).map(|b| (1u64 << b, format!("bit {b}"))).collect();
                    unnamed.iter().map(|(m, n)| (*m, n.as_str())).collect()
                } else {
                    names.iter().map(|(m, n)| (u64::from(*m), *n)).collect()
                };
                let columns = if names.is_empty() { 8 } else { 4 };
                flag_grid(ui, id.with("flags"), &mut bits, entries, columns, false);
            });
            let bits = u32::try_from(bits).unwrap_or(u32::MAX);
            if bits != value.to_bits() {
                *value = T::from_bits(bits);
            }
        }
        _ => {
            drag(ui, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::TemplateViewerApp;
    use super::{
        CHARACTER_DATA_FIELDS, DataWord, FieldDef, FieldKind, character_data_fields,
        item_data_fields, item_driver_info, map_pos_label, slot_def,
    };
    use eframe::egui;
    use mag_core::constants::{ItemFlags, SERVER_MAPX};
    use mag_core::types::Item;

    /// Every doc fits in `len` slots, no two docs overlap, and flag/enum tables are sane.
    fn assert_valid(defs: &[FieldDef], len: usize, what: &str) {
        let mut covered = vec![false; len];
        for def in defs {
            assert!(def.count >= 1, "{what}: {} has no slots", def.name);
            for idx in def.index..def.index + def.count {
                assert!(idx < len, "{what}: {} slot {idx} out of range", def.name);
                assert!(!covered[idx], "{what}: slot {idx} documented twice");
                covered[idx] = true;
            }
            match def.kind {
                FieldKind::Flags(names) => {
                    for (mask, name) in names {
                        assert_eq!(mask.count_ones(), 1, "{what}: {name} is not a single bit");
                    }
                }
                FieldKind::Enum(options) => {
                    for (i, (a, _)) in options.iter().enumerate() {
                        assert!(
                            options[i + 1..].iter().all(|(b, _)| a != b),
                            "{what}: duplicate enum value {a}"
                        );
                    }
                }
                _ => {}
            }
        }
    }

    #[test]
    fn character_catalog_is_consistent() {
        assert_valid(CHARACTER_DATA_FIELDS, 100, "character");
    }

    #[test]
    fn item_catalogs_are_consistent() {
        for driver in 0..=u8::MAX {
            let (name, defs) = item_driver_info(driver);
            assert!(!name.is_empty());
            assert_valid(defs, 10, &format!("driver {driver}"));
        }
        assert_eq!(item_driver_info(2).0, "Door");
        assert_eq!(item_driver_info(200).0, "Unknown driver");
    }

    #[test]
    fn item_data_fields_adds_driver_independent_slots() {
        let mut item = Item::default();
        assert!(item_data_fields(&item).is_empty());

        item.duration = 100;
        let defs = item_data_fields(&item);
        assert_eq!(
            slot_def(&defs, 1).map(|(d, _)| d.name),
            Some("Spell template")
        );

        item.flags = ItemFlags::IF_HIDDEN.bits();
        assert!(slot_def(&item_data_fields(&item), 9).is_some());

        // Driver 17 already uses slot 9; the hidden-difficulty doc must not override it.
        item.driver = 17;
        assert_eq!(
            slot_def(&item_data_fields(&item), 9).map(|(d, _)| d.name),
            Some("Result")
        );
    }

    #[test]
    fn slot_def_reports_position_in_range() {
        let defs: Vec<&FieldDef> = CHARACTER_DATA_FIELDS.iter().collect();
        let (def, ordinal) = slot_def(&defs, 12).expect("patrol stop");
        assert_eq!((def.name, ordinal), ("Patrol stop", 2));
    }

    #[test]
    fn data_word_conversions_saturate_and_roundtrip_bits() {
        assert_eq!(<u32 as DataWord>::from_i64(-5), 0);
        assert_eq!(<i32 as DataWord>::from_i64(i64::MAX), i32::MAX);
        assert_eq!(<i32 as DataWord>::from_bits((-7i32).to_bits()), -7);
        assert_eq!((0xFFFF_FFFFu32).to_bits(), u32::MAX);
    }

    #[test]
    fn map_pos_label_decodes_packed_coordinates() {
        let label = map_pos_label(i64::from(SERVER_MAPX) * 7 + 3).expect("label");
        assert!(label.starts_with("(3, 7)"));
        assert_eq!(map_pos_label(0), None);
    }

    #[test]
    fn rendering_without_input_leaves_every_slot_unchanged() {
        let mut app = TemplateViewerApp {
            show_all_data_fields: true,
            ..Default::default()
        };
        let mut character_data: [i32; 100] = std::array::from_fn(|i| i as i32 * 7919 - 3000);
        let before_character = character_data;
        let mut item_data: [u32; 10] = std::array::from_fn(|i| (i as u32) * 0x0101_0101);
        let before_item = item_data;
        let item = Item {
            driver: 57,
            ..Default::default()
        };

        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let defs = character_data_fields();
                app.ui_data_fields(ui, "chars", &mut character_data, &defs);
                let defs = item_data_fields(&item);
                app.ui_data_fields(ui, "items", &mut item_data, &defs);
            });
        });

        assert_eq!(character_data, before_character);
        assert_eq!(item_data, before_item);
    }
}
