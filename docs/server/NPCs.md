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
