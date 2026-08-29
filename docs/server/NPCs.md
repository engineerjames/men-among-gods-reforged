# NPC System

## `text[]` Slot Reference

Each character (NPC and player alike) has 10 `text[]` slots. Their meaning is **context-dependent**: NPC and player characters reuse the same slots for entirely different purposes.

### NPC usage

| Index | When spoken / purpose |
|-------|-----------------------|
| `text[0]` | **Kill cry** — said when the NPC kills an enemy (`npc_didkill`). `%s` is replaced with the victim's name. |
| `text[1]` | **Battle cry** — said when the NPC first spots/engages an enemy, retaliates after being attacked, or witnesses an ally being attacked. `%s` is replaced with the enemy's name. |
| `text[2]` | **Friendly greeting** — said when a passing player is spotted. Can contain special tokens `#stunspec` or `#cursespec` to trigger quest-giver dialogue instead of a literal greeting. `%s` is replaced with the player's name. |
| `text[3]` | **Death cry** — said by the NPC itself when it dies. Only fires with a random chance controlled by `data[48]`. `%s` is replaced with the killer's name. |
| `text[4]` | **Help shout** — said when the NPC is low on HP and broadcasts a shout for allies (`npc_gotattack`, `npc_cityguard_see`). `%s` is replaced with the attacker's name. |
| `text[5]` | **Shout response** — said when the NPC hears a shout notification from a nearby ally (`npc_shout`). `%s` is replaced with the reported enemy's name. |
| `text[6]` | **Stop keyword** — a magic word that, when spoken by a player, causes the NPC to drop all enemies and reset aggro (`npc_hear`). Compared case-insensitively. |
| `text[7]` | **Stop response** — the NPC's acknowledgment reply after the stop keyword is used. |
| `text[8]` | **Password warning** — said when a player enters the outer warning radius of a password-guarded territory, before the NPC turns hostile. |
| `text[9]` | **Binary grave log** — not a string. Repurposed as a 40-entry ring buffer of raw `i32` item indices representing graves this NPC has already looted. Managed by `npc_already_searched_grave` / `npc_add_searched_grave`. |

### Player usage

| Index | Purpose |
|-------|---------|
| `text[0]` | **AFK message** — shown to other players who look at this character when `data[0]` is non-zero. Also used as the **name staging buffer** during new-character creation (written by the client before being committed to `name[]`). |
| `text[1]` | **Description buffer (part 1)** — written by the client during character creation. Appended with `text[2]` if the content exceeds 77 characters. |
| `text[2]` | **Description buffer (part 2)** — overflow continuation of the description buffer. |
| `text[3]` | **Player title / custom description** — displayed as a yellow line in the look panel below equipment. |
| `text[4]`–`text[9]` | Unused for players. |

### Key points

- Indexes **0–3 are dual-purpose**: the same slot means something entirely different for an NPC vs. a player character.
- Index **9 is not a string for NPCs**; it holds packed binary data for the grave-looting system.
- Indexes **6 and 7 are a pair**: the stop keyword and its acknowledgment reply.
- Index **8** is the only pre-hostility speech slot — the NPC warns before attacking.

## Combat text flow

When an NPC spots an enemy the call chain is:

1. `act_idle` broadcasts `NT_SEE` to the area.
2. `npc_see` is called for each NPC that receives the notification.
3. If the spotted character is in a hostile group (`data[43..47]`) or inside a guarded territory without the password, `npc_add_enemy` is called and then `npc_saytext_n(gs, cn, 1, ...)` fires the battle cry (`text[1]`).

The same `text[1]` slot is also triggered by:
- `npc_gotattack` — when the NPC is directly attacked and retaliates.
- `npc_seeattack` — when the NPC witnesses an ally being attacked.
- `state/combat.rs` `do_enemy` — when a god-mode operator manually sets an enemy.

## How NPCs decide to attack a player

NPC aggression is driven by a **kill list** (also called the enemy list) stored in
`data[80..91]` (`CHD_ENEMY1ST`..`CHD_ENEMYZZZ`, `core/src/constants.rs`). Each slot packs
a character index and a generation id: `co | (char_id(co) << 16)`. As long as any
non-zero entry is on the list, the NPC will path toward and fight that character
(see [Driver priority order](#driver-priority-order) below). Nothing else about combat
readiness matters once an entry is added — the list *is* the decision.

### Entry points that add an enemy

All paths funnel through `npc_add_enemy` (`server/src/driver/npc.rs`), which first
calls the pure eligibility check `npc_should_consider_enemy`:

- Same group id (`data[42]`) --> never fight (allies/self are always exempt).
- If `always` flag not set, group-1 mobs will not attack ghost companions (`data[42] & 0x10000`).
- If `always` flag not set, an NPC that is far weaker than the target
  (`(points_tot + 500) * 25 < target.points_tot`) will refuse to engage.
- `always = true` bypasses the group/power checks entirely (used for direct
  retaliation and ally-defense paths below, where the NPC has no real choice).

If eligible, `npc_add_enemy` also opportunistically switches `attack_cn` to the new
target when it's closer than the current target (or ties under certain conditions),
shifts the kill list to insert the new entry at slot 80, and (if this was a new
addition) triggers the battle cry `text[1]`.

### 1. Spotting a stranger — `npc_see`

Whenever any character enters an NPC's sight radius, `act_idle` broadcasts
`NT_SEE` and the NPC's `npc_see` handler (`server/src/driver/npc.rs`) runs. This is
the main "unprovoked aggression" path and layers several independent triggers:

- **Existing kill-list match**: if the newly-seen character's packed id is already
  present in `data[80..91]` (e.g. re-entering sight after breaking line of sight)
  and the NPC isn't already fighting someone, it immediately locks `attack_cn` onto
  them.
- **Hostile-group territory** (`data[43]` non-zero): the NPC treats everyone *not*
  in one of its allowed groups (`data[43..47]`) as hostile, with a special case
  where `data[n] == 65536` matches any player or companion. If the seen character
  doesn't match any allowed group:
  - Normally it's added to the kill list right away.
  - If `data[95] == 2` ("stay within `data[93]` of resting position" mode), the NPC
    only attacks if the target is within `data[93]` tiles (Chebyshev distance) of
    its resting position `data[29]`; otherwise it lets them pass.
- **Password-guarded territory** (`data[95] == 1`, "warn then attack if no
  password"): only applies to players, and only re-checks every 120 ticks
  (`data[27]` cooldown, `TICKS * 120`).
  - If the player is within `data[93]` tiles (Manhattan distance) of the resting
    position, they're added to the kill list (they never gave the password).
  - If they're within `2 * data[93]` tiles, the NPC instead speaks the warning
    `text[8]` (rate-limited to once per 15 ticks via `data[94]`) without attacking.
  - Note: there is no code path that actually clears this state when a player
    *does* speak the password — the intended password-check hook is not wired up
    in the current Rust port, so in practice `data[95] == 1` NPCs behave like
    unconditional territory guards once a player is close enough.
- **Special driver hooks** (`data[25]` non-zero) bypass all of the above and
  delegate entirely to `npc_stunrun_msg`, `npc_cityattack_msg`, or `npc_malte_msg`
  for bespoke scripted NPCs.
- **City-guard hook** (`data[26]` non-zero, values `1`/`3`) additionally runs
  `npc_cityguard_see` before the group/territory checks.
- If none of the above fire and the seen character is a player the NPC hasn't
  talked to recently (`data[37..41]`), it instead just greets them with `text[2]`
  (or a quest-hint reply for `#stunspec`/`#cursespec`) — this is the non-hostile
  fallback.

### 2. Being attacked — `npc_gotattack` / `npc_gothit` / `npc_gotmiss`

`npc_gothit` and `npc_gotmiss` both delegate straight to `npc_gotattack`, so any
hit *or* miss against the NPC triggers the same retaliation logic:

- Refreshes a 60-tick "recently in combat" timer (`data[92]`).
- High-alignment NPCs (`alignment == 10000`) being hit by a player summon a
  "Shadow of Peace" companion to help fight, rate-limited by `data[70]`.
- Good-aligned NPCs (`alignment > 1000`) low on mana call for divine help and
  refill their mana, also rate-limited by `data[70]`.
- If HP is below 66.6% and it's been more than 60 ticks since the last shout
  (`data[55]`), the NPC shouts for help (`do_npc_shout`) using its shout code
  `data[52]`/place `data[54]` and speaks `text[4]`.
- **If the NPC can't currently see the attacker**, it doesn't add them as an
  enemy — instead it enters a 30-tick "panic" mode (`data[78]`), which makes the
  low-priority driver wander randomly instead of pathing toward a known enemy.
- Otherwise, the attacker is added to the kill list with `always = true` — an NPC
  always retaliates against anyone who lands a hit or a miss on it, regardless of
  group or relative power.

### 3. Witnessing an attack — `npc_seeattack`

Fired when the NPC sees character `cc` attack character `co` nearby. This is how
NPCs defend allies, guarded characters, and masters without being hit themselves:

- Both participants must currently be visible to the NPC, or the notification is
  ignored.
- **"Prevent fight" mode** (`data[24]` non-zero): the NPC acts as an alignment-based
  peacekeeper. It compares `cc`'s alignment (minus a 50-point handicap) against
  `co`'s alignment; combined with the sign of `data[24]` (`> 0` defend good, `< 0`
  defend evil), it adds whichever combatant is "more evil" to its kill list.
- **Protect-by-template** (`data[31]` non-zero): if the victim `co` matches the
  protected template id, the attacker `cc` is added as an enemy, and the NPC also
  remembers the victim in `data[65]` ("help friend") so the high-priority driver
  will heal/buff them on subsequent ticks.
- **Protect-by-master** (`data[63]`, `CHD_MASTER`): symmetric check — if either
  participant is the NPC's master, the *other* participant is added as an enemy,
  and the master is recorded in `data[65]` for support magic.

### 4. Group/alliance rules

- `data[42]` is the NPC's group id. Two characters in the same group never fight
  each other (checked first, unconditionally, in `npc_should_consider_enemy`).
- `data[43..47]` optionally restrict aggression to "not one of these groups",
  letting a template define e.g. "attack anyone except groups 3, 7, and 12".
- `data[59]` ("help all members of group X") and the shout system
  (`data[52]`–`data[55]`) let an NPC that hears an ally's shout join the fight
  even without directly seeing the original aggressor — the shout target
  location is stored in `data[54]` and consumed by the low-priority driver
  (`npc_driver_low`) as a high-priority "go help" waypoint.

### 5. Driver priority order

Once per tick, `driver()` (`server/src/driver/generic.rs`) runs `npc_driver_high`
before anything else for non-player characters. It short-circuits through a long
list of higher-priority behaviors (self-destruct timers, healing, buffing,
in-combat spellcasting) — see the "generic fight-magic management" block for how
an NPC that already has an `attack_cn` set will prefer curses/blasts/stuns over
melee before falling through. If `attack_cn` is still set after all of that, the
generic `driver()` dispatch calls `drv_attack_char`, which performs the actual
melee attack via `char_attack_char` --> `act_attack`.

If no high-priority action fires and `attack_cn == 0`, `npc_driver_low` handles
lower-priority behavior: reacting to help shouts (`data[54]`/`data[55]`), pathing
to the last known enemy position (`data[76]`/`data[77]`, valid for 30 ticks),
resting when hurt, closing doors, patrolling, random walking, and returning to a
resting position (`data[29]`) — all subordinate to combat.

### 6. Breaking off an attack

- `npc_killed` / `npc_seekill` clear the kill-list entry, `attack_cn`, and the
  panic/last-seen timers (`data[76]`, `data[77]`, `data[78]`) once the target dies;
  if the NPC itself landed the kill it also speaks the kill cry `text[0]`.
- `npc_remove_enemy` can pull a specific character off the kill list without a
  kill (used by god-mode/admin tooling and the stop-keyword handler).
- Speaking the NPC's `text[6]` stop keyword (case-insensitive) in `npc_hear`
  drops all enemies and resets aggro state, with `text[7]` spoken in
  acknowledgment.

### Notes on permission checks

NPC attacks are not filtered by the same PvP rules players are subject to:
`may_attack_msg` (`server/src/state/combat.rs`) returns `true` unconditionally for
any attacker that isn't flagged `CharacterFlags::Player` (after resolving
companions to their master). The only universal block is `MF_NOFIGHT` map tiles,
which suppress combat for player-vs-anyone interactions but do **not** stop an NPC
from adding a player to its kill list in the first place — it just prevents the
resulting `drv_attack_char` call from landing a hit while both parties stand on a
no-fight tile.
