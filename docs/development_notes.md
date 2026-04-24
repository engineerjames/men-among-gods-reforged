# Development Notes

Internal notes about the engine that don't fit cleanly into the other design
documents. Add new sections as we untangle more of the legacy data model.

## `Character` talent and scratch fields: `future1` / `future2` / `future3` / `data` / `text`

The `Character` struct (see [core/src/types/character.rs](core/src/types/character.rs))
is a direct port of the legacy C `struct character` from
[server/orig/data.h](server/orig/data.h). It contains five generic, fixed-size
arrays whose meaning depends entirely on whether the character is a player or
an NPC, and on which driver is running. This section documents what each one
is currently used for.

### `future1: [u8; 25]`

**Status: active packed talent-tree state.**

`future1` now stores player talent progression. Byte `0` is the unspent
talent-point pool, and bytes `1..24` are per-layer bit fields where each bit
represents one talent slot. See [core/src/talent_trees/mod.rs](core/src/talent_trees/mod.rs)
for the shared layout helpers and class metadata.

The field still occupies the original 25-byte legacy expansion area, but the
Rust representation is unsigned because talent storage is byte-oriented.

### `future2: [i16; 49]`, `future3: [i32; 12]`

**Status: unused. Pure padding.**

These fields exist solely because the original C code declared them as "space
for future expansion" inside the on-disk character record. They were never
populated by any driver in the legacy server, and the reforged codebase
preserves them for binary-layout / save-file compatibility only.

| Field     | Type        | Origin (`server/orig/data.h`) |
|-----------|-------------|-------------------------------|
| `future2` | `[i16; 49]` | `short future2[49];` |
| `future3` | `[i32; 12]` | `int future3[12];` |

Rules of thumb:

- Do **not** repurpose these without first understanding the snapshot /
  bincode layout in [core/src/character_store.rs](core/src/character_store.rs)
  and [server/src/snapshot.rs](server/src/snapshot.rs). They are part of the
  serialized form.
- If you need scratch state for a new system, prefer adding a properly named
  field at the end of `Character` (and bumping the snapshot version) rather
  than carving a slot out of `futureN`.
- The same advice applies to `Item::future3` (see
  [core/src/types/item.rs](core/src/types/item.rs)), which is also unused
  legacy padding.

### `data: [i32; 100]`

A generic 100-slot scratchpad attached to every character. The meaning of each
slot is **completely different for players and NPCs**, and there is no type
system to enforce that — the convention is documented in the original
`server/orig/README` (players) and at the top of `server/orig/driver.c`
(NPCs).

The tables below are transcribed from those original sources and from how the
reforged Rust code currently uses each slot. When porting code, treat these as
the authoritative legend for what `gs.characters[cn].data[N]` means.

#### Player slots (`flags & CF_PLAYER` set)

| Slot     | Meaning                                                                 |
|----------|-------------------------------------------------------------------------|
| 0        | Away From Keyboard flag                                                  |
| 1–9      | Group invitation slots (`CHD_MINGROUP` … `CHD_MAXGROUP`)                 |
| 10       | Currently following character X                                          |
| 11       | "No fightback" flag (`CHD_FIGHTBACK`)                                    |
| 12       | Follow-suspension timeout (ticker timestamp)                             |
| 13       | Money in bank                                                            |
| 14       | Number of deaths                                                         |
| 15       | Killed by character X                                                    |
| 16       | Date (last death?)                                                       |
| 17       | Area (last death?)                                                       |
| 18       | Current pentagram experience                                             |
| 19       | Lag timer (drives `CF_STONED`)                                           |
| 20       | Highest gorge solved                                                     |
| 21       | Seyan'Du sword: bitmask of shrines already used                          |
| 22       | Current monster index in arena                                           |
| 23       | Overall kill counter — below own rank                                    |
| 24       | Overall kill counter — at own rank                                       |
| 25       | Overall kill counter — above own rank                                    |
| 26       | Black Stronghold kill counter — below own rank                           |
| 27       | Black Stronghold kill counter — at own rank                              |
| 28       | Black Stronghold kill counter — above own rank                           |
| 29       | Other players killed outside arena                                       |
| 30–39    | Soft ignore list                                                         |
| 40       | Shopkeepers etc. killed                                                  |
| 41       | Black Stronghold points already spent                                    |
| 42       | Group id (`CHD_GROUP`)                                                   |
| 43       | Black candles returned                                                   |
| 44       | Number of times player was saved by the gods                             |
| 45       | Level X got hp/end/mana bonus for                                        |
| 46–49    | Flags of explorer points visited (bits 1, 2, 4, 8, 16)                   |
| 50–59    | Hard ignore list                                                         |
| 60–63    | Flags of killed NPC of certain class                                     |
| 64       | Current ghost companion (character id)                                   |
| 65       | Player `#ALLOW`ed to search corpse                                       |
| 66       | Corpse's owner                                                           |
| 67       | Riddle giver (lab 9)                                                     |
| 68       | Date of last player attack (`CHD_ATTACKTIME`)                            |
| 69       | Last player attacked (`CHD_ATTACKVICT`)                                  |
| 70       | Number of fire points                                                    |
| 71       | Says/tells/looks/lights counter (`CHD_TALKATIVE`, anti-spam)             |
| 72       | Bad word score                                                           |
| 80–89    | Seen logging in from these class-C nets                                  |
| 90       | Number in database (unconfirmed)                                         |
| 92       | Reserved (also reused as "stay-awake" timer in shared paths)             |
| 96       | Queued spells                                                            |
| 97       | Computation time used last action                                        |
| 98       | Computation time used (cumulative)                                       |
| 99       | Used by `populate`                                                       |

#### NPC slots (no `CF_PLAYER`)

NPC slots 0–9 are reserved for *exclusive drivers* (e.g. merchants) — each
specialty driver may use 0–9 however it wants. Slots 10+ have a shared
convention used by `driver.c` and friends:

| Slot     | Meaning                                                                 |
|----------|-------------------------------------------------------------------------|
| 0–9      | Driver-private scratch (merchants, special NPCs, …)                      |
| 10–18    | Patrol stops, encoded as `x + y * MAPX`                                  |
| 19       | Index of next patrol stop                                                |
| 20–23    | Doors to close                                                           |
| 24       | Prevent fight mode (`-1` defend evil, `0` no interference, `1` defend good) |
| 25       | Special driver id                                                        |
| 26       | Special sub-driver id                                                    |
| 27       | Last time we got stop command (password logic)                           |
| 28       | Exp earned since creation (ghost companions et al.)                      |
| 29       | Resting position (`x + y * MAPX`)                                        |
| 30       | Resting direction                                                        |
| 31       | Protect characters created from template X                               |
| 32–35    | Lights to keep burning                                                   |
| 36       | Frust timer (≥100 → use NPC magic to reach objective)                    |
| 37–40    | Last characters we talked to; set 37 ≠ 0 to enable greeting              |
| 41       | Turn lights on/off from this template at dusk/dawn                       |
| 42       | Group id                                                                 |
| 43–46    | If 43 set, attack everyone *not* in groups 43–46                         |
| 47       | Pick up and destroy / donate garbage                                     |
| 48       | Probability (%) to say `text[3]` on death                                |
| 49       | Wants item template X                                                    |
| 50       | Teaches skill X in exchange for item from slot 49                        |
| 51       | Raises EXP by X in exchange for item                                     |
| 52       | Shout for help if attacked, using code X                                 |
| 53       | Come to help if called by code X                                         |
| 54       | Place of shout (`x + y * MAPX`)                                          |
| 55       | Timeout for shout; if 54==0 and 55!=0, *we* shouted for help             |
| 56       | Greet timeout                                                            |
| 57       | Rest time between patrol stops (driver-managed)                          |
| 58       | Importance of current job (0 low, 1 medium, 2 high) — endurance hint    |
| 59       | Help all members of group X                                              |
| 60       | Random walk: time between walks                                          |
| 61       | Time elapsed (random walk)                                               |
| 62       | Create light: 0 never, 1 when dark, 2 when dark & not resting, 3 fighting|
| 63       | Obey and protect character X (`CHD_MASTER`)                              |
| 64       | Self-destruct in X ticks (`CHD_COMPANION`)                               |
| 65       | Friend under attack — help by magic etc.                                 |
| 66       | Gives item template X in exchange for item from slot 49                  |
| 67       | Greeting timeout                                                         |
| 68       | Value of knowledge                                                       |
| 69       | Follow character X                                                       |
| 70       | Last time we called our god for help                                     |
| 71       | Talkativity counter (`CHD_TALKATIVE`)                                    |
| 72       | Area of knowledge (also: riddler "area" in lab 9)                        |
| 73       | Random walk: max distance from origin                                    |
| 74       | Last time we created a ghost                                             |
| 75       | Last time we stunned someone                                             |
| 76       | Last known position of an enemy                                          |
| 77       | Timeout for slot 76                                                      |
| 78       | Attacked by invisible (timer)                                            |
| 79       | Rest time between patrol stops (admin-set value)                         |
| 80–91    | Kill list (`CHD_ENEMY1ST` … `CHD_ENEMYZZZ`); each entry packs `co | (char_id<<16)` |
| 92       | No-sleep bonus (also: temporary "stay awake" timer)                      |
| 93       | Attack distance (warns target first)                                     |
| 94       | Last warning time                                                        |
| 95       | Keyword action: 1 = wait for password & attack if missing, 2 = stay within slot 93 of resting position |
| 96       | Queued spells                                                            |
| 97       | Currently being usurped by character X                                   |
| 98       | Ghost-companion: no-see-master timeout                                   |
| 99       | Used by `populate`                                                       |

### `text: [[u8; 160]; 10]`

10 fixed-length 160-byte strings. Used by NPC drivers for canned dialogue and
by players for things like the AFK message. The header doc-comment in
[core/src/types/character.rs](core/src/types/character.rs) lists the player
slots; the NPC slots are:

| Slot | NPC usage                                          |
|------|----------------------------------------------------|
| 0    | Said when killing an enemy (`%1` = victim)          |
| 1    | Said when first attacking a new enemy (`%1` = foe) |
| 2    | Greeting (`%1` = passerby)                          |
| 3    | Said on death (`%1` = killer), gated by `data[48]` |
| 4    | Shouting for help (against `%1`)                   |
| 5    | Coming to help `%1`                                |
| 6    | Keyword to listen for                              |
| 7    | Reaction to keyword                                |
| 8    | Warning message (used with `data[93]`/`data[95]`)  |
| 9    | Reused as memory of already-searched graves        |

For players, slots 0–2 get clobbered at login; slot 3 holds the `mark`
message.

### Practical guidance

- When adding new gameplay state, prefer a named field on `Character` over
  a new `data[N]` slot. Reach for `data[N]` only when porting legacy logic
  that already touches that slot.
- When porting a piece of legacy C, treat the tables above as the legend —
  most of the magic numbers in `server/orig/*.c` are slot indices into
  `data[]`.
- Anything labelled "reserved" (slot 92 for players, the gaps in the NPC
  range) should be assumed to be load-bearing somewhere; grep before reusing.
