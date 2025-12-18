# Server Implementation Plan - Comprehensive Review

## Executive Summary

Reviewed entire Rust server project (12 source files, 3,200+ lines of Rust code, ported from original 1997-2001 C++ server). **63 outstanding "Would call" stubs identified** that must be implemented for a functional game server.

**Overall Status: 35% Complete** - Core infrastructure working, needs game systems implementation

---

## Completed Components ✅

### Core Systems (100% Done)
- Global type definitions (Character, Item, Effect, Map, Player structures)
- Game constants and enumerations  
- Logging system (xlog, chlog, plog macros)
- Profiling infrastructure with CPU cycle counting
- Signal handling (SIGINT, SIGTERM, SIGHUP, SIGQUIT)
- Configuration and startup/shutdown sequences

### Game Loop Foundation (70% Done)
- tick() main loop with character/player iteration ✅
- Character validation (check_char_valid) with inventory checking ✅
- Player lag/stoning system (plr_tick) ✅
- Character expiration system (check_expire) ✅
- Group activity checking (group_active) ✅
- Character wakeup system for NPC activity ✅
- Player logout handling (plr_logout) ✅
- Online statistics tracking ✅

### Network/Player Management (40% Done)
- Basic player connection management ✅
- Player state tracking ✅
- Socket creation and binding ✅
- **MISSING**: Command processing (plr_cmd)
- **MISSING**: Player timeout handling (plr_idle)
- **MISSING**: Login state machine (plr_state)
- **MISSING**: Map data transmission (plr_getmap)
- **MISSING**: Character update transmission (plr_change)

---

## Outstanding Implementation Items (63 Found)

### **Category 1: Network I/O & Player Communication** (Priority: CRITICAL)
These are blocking - no gameplay without these.

| Function | Location | Status | Description |
|----------|----------|--------|-------------|
| `plr_cmd()` | game_loop.rs:179 | STUB | Parse incoming client packets and dispatch to handlers |
| `plr_idle()` | game_loop.rs:184 | STUB | Check for idle timeouts (60s protocol, 15min player level) |
| `plr_state()` | game_loop.rs:195 | STUB | Handle login state machine transitions |
| `plr_getmap()` | game_loop.rs:210 | STUB | Send map tile data to player |
| `plr_change()` | game_loop.rs:210 | STUB | Send character/item changes to player |
| Socket read/write loops | game_loop.rs:700-800 | PARTIAL | Need completion of network I/O |
| Packet parsing protocol | network.rs | STUB | Protocol buffer format unknown |
| Command dispatch table | svr_tick.cpp:1060+ | REFERENCE | 30+ command handlers needed |

**Estimated Effort**: 40-50 hours

---

### **Category 2: Game State Persistence** (Priority: CRITICAL)
Without these, no progress persists between restarts.

| Function | Location | Status | Description |
|----------|----------|--------|-------------|
| `load()` | game_loop.rs:331 | STUB | Load global state, all characters, items from disk |
| `unload()` | game_loop.rs:338 | STUB | Save all game state on shutdown |
| `save_char()` | state_mgmt.rs:85 | STUB | Periodically save individual character files |
| `load_char()` | state_mgmt.rs:95 | STUB | Load character from file by index |
| `pop_load_all_chars()` | game_loop.rs:393 | STUB | Batch load all character data at startup |
| `pop_save_all_chars()` | game_loop.rs:399 | STUB | Batch save all characters periodically |
| Binary serialization | types.rs | MISSING | Need to define character/item disk format |

**Estimated Effort**: 30-40 hours

---

### **Category 3: Character Management & Updates** (Priority: CRITICAL)
Core gameplay mechanics depend on these.

| Function | Location | Status | Description |
|----------|----------|--------|-------------|
| `really_update_char()` | game_loop.rs:236 | STUB | Recalculate character HP, mana, stats |
| `do_char_killed()` | game_loop.rs:552 | STUB | Handle death, create corpse, drop items |
| `do_regenerate()` | game_loop.rs:307 | STUB | Heal HP and regenerate mana each tick |
| `plr_act()` | game_loop.rs:303 | STUB | Process character actions (move, attack, etc) |
| `god_drop_char_fuzzy_large()` | game_loop.rs:566 | STUB | Relocate character to valid position |
| Stat calculation formulas | unknown | MISSING | HP, mana, damage calculations |

**Estimated Effort**: 30-35 hours

---

### **Category 4: World Systems** (Priority: HIGH)
Needed for world coherence and NPC activity.

| Function | Location | Status | Description |
|----------|----------|--------|-------------|
| `pop_tick()` | game_loop.rs:320 | STUB | Update all NPCs, handle AI, movement |
| `effect_tick()` | game_loop.rs:321 | STUB | Process active effects/buffs/debuffs each tick |
| `item_tick()` | game_loop.rs:322 | STUB | Handle items - decay, poison, etc |
| `global_tick()` | game_loop.rs:323 | STUB | World updates - day/night, weather |
| `init_lights()` | game_loop.rs:373 | PARTIAL | Compute world lighting for vision system |
| `pop_skill()` | game_loop.rs:387 | STUB | Initialize NPC skill tables |
| `populate()` | game_loop.rs:344 | STUB | Spawn NPCs from templates at world load |
| NPC AI system | unknown | MISSING | Driver system for NPC behavior |

**Estimated Effort**: 40-50 hours

---

### **Category 5: Admin & God Mode Systems** (Priority: MEDIUM)
Not needed for basic gameplay but used for server management.

| Function | Location | Status | Description |
|----------|----------|--------|-------------|
| `god_init_badnames()` | game_loop.rs:438 | STUB | Load badnames.txt list |
| `init_badwords()` | game_loop.rs:444 | STUB | Load badwords.txt list |
| `god_read_banlist()` | game_loop.rs:450 | STUB | Load banned player list |
| `god_init_freelist()` | game_loop.rs:432 | PARTIAL | Initialize free item pool |
| `god_destroy_items()` | game_loop.rs:251 | STUB | Remove items from character |
| `reset_changed_items()` | game_loop.rs:456 | STUB | Mark items as unchanged |
| Ban/kick commands | unknown | MISSING | Kick player commands |

**Estimated Effort**: 10-15 hours

---

### **Category 6: Specialized Systems** (Priority: LOW)
Special areas and niche features.

| Function | Location | Status | Description |
|----------|----------|--------|-------------|
| `init_lab9()` | game_loop.rs:426 | STUB | Initialize Lab9 special area |
| `init_node()` | game_loop.rs:420 | STUB | Initialize server node system |
| `load_mod()` | game_loop.rs:414 | STUB | Load mod/extension files |
| `tmplabcheck()` | game_loop.rs:473 | STUB | Check for lab items on players |
| `stone_gc()` | game_loop.rs:526 | STUB | Handle stoning spell system |
| Lab9 item handling | types.rs | MISSING | Special item system |
| Soulstone system | unknown | MISSING | Special item with max_damage tracking |

**Estimated Effort**: 15-20 hours

---

## Implementation Roadmap

### Phase 1: Core Game Loop (Days 1-2)
**Goal**: Get basic gameplay working - players can login, move, see world

- [ ] Implement `plr_idle()` - Connection timeout detection
- [ ] Implement `plr_cmd()` - Basic command parsing and dispatch
- [ ] Implement `plr_state()` - Login state machine
- [ ] Implement `plr_getmap()` / `plr_change()` - Client synchronization
- [ ] Complete game_loop network I/O loops

**Tests**: Player can login, stay connected without timeout, see map updates

### Phase 2: Character Lifecycle (Days 2-3)
**Goal**: Characters have health, can take damage, can die

- [ ] Implement `load()` / `unload()` - Save/load game state
- [ ] Implement `really_update_char()` - Stat recalculation
- [ ] Implement `do_regenerate()` - Healing system
- [ ] Implement `do_char_killed()` - Death system
- [ ] Implement save_char / load_char functions

**Tests**: Create character, take damage, regenerate health, character survives restart

### Phase 3: Character Actions (Days 3-4)
**Goal**: Players can perform actions - move, attack, use items

- [ ] Implement `plr_act()` - Action processing
- [ ] Implement movement/pathfinding  
- [ ] Implement attack system
- [ ] Implement item use system
- [ ] Implement talk/communication

**Tests**: Players can move around, attack NPCs, use items

### Phase 4: World Systems (Days 4-5)
**Goal**: NPCs exist and act, items spawn, world has effects

- [ ] Implement `pop_tick()` - NPC behavior updates
- [ ] Implement `effect_tick()` - Buff/debuff system
- [ ] Implement `item_tick()` - Item decay/effects
- [ ] Implement `global_tick()` - Environmental updates
- [ ] Implement `populate()` - NPC spawning

**Tests**: NPCs move and act, world feels alive

### Phase 5: Polish (Days 5-6)
**Goal**: Server is stable and performant

- [ ] Admin/god systems
- [ ] Ban/kick implementation
- [ ] Bad word/name filtering
- [ ] Performance optimization
- [ ] Error handling improvements

**Tests**: Server handles load, admin commands work

---

## Critical Blockers

### 1. **Packet Protocol Unknown**
The exact format of client->server packets is undefined. Need to:
- Determine 16-byte command packet structure
- Identify command payload layouts
- Implement parser for each command type

**Solution**: Examine original C++ client code or sniff network traffic

### 2. **NPC Template System Missing**
No definition for how NPCs are spawned from templates. Need:
- Template data structures
- Spawn point definitions
- NPC population list

**Solution**: Look at populate.cpp and data files in .dat/

### 3. **Item Drop/Pickup Not Defined**
The mechanics for item handling are unclear. Need:
- Item carrying limits
- Inventory structure
- Drop position algorithm

**Solution**: Study Item.h and helper functions

### 4. **Combat System Undefined**
No combat calculations implemented. Need:
- Damage formula
- Hit/miss chance
- Status effects from attacks

**Solution**: Search svr_act.cpp for combat logic

---

## File-by-File Analysis

### main.rs (306 lines) - 90% Complete
✅ Startup sequence mostly done
✅ Signal handling complete
✅ Shutdown sequence defined
❌ Command-line argument handling (pop, rem, wipe, light, skill, load, save)
⚠️ Some functions call stubs that need implementation

### game_loop.rs (820 lines) - 35% Complete
✅ Tick loop structure
✅ Character validation
✅ Player tracking
✅ Statistics tracking
❌ ~15 stub functions (plr_act, do_regenerate, etc.)
❌ Network I/O in game_loop function

### network.rs (427 lines) - 40% Complete
✅ Socket creation
✅ TCP listener setup
⚠️ new_player() partially done
❌ Packet receiving loop
❌ Packet sending loop
❌ Compression implementation incomplete

### player.rs (241 lines) - 60% Complete
✅ Player structure definition
✅ Connection management
✅ Basic state tracking
❌ Player command buffer handling
❌ Output buffer compression

### types.rs (789 lines) - 80% Complete
✅ All game structures defined
✅ Helper methods implemented
⚠️ Some methods not fully implemented
❌ Serialization/deserialization for persistence

### state_mgmt.rs (233 lines) - 20% Complete
✅ Directory structure
❌ File I/O not implemented
❌ Character serialization missing
❌ Global state persistence missing

### population.rs (175 lines) - 10% Complete
✅ Manager structure
❌ NPC spawning not implemented
❌ NPC behavior not implemented
❌ Population logic missing

### player_control.rs (211 lines) - 70% Complete
✅ plr_logout() implemented
✅ Exit punishment system
✅ Lag scroll distribution
❌ Some called functions missing

### profiling.rs (213 lines) - 80% Complete
✅ Profiler working
✅ CPU cycle measurement
⚠️ Display to players incomplete

### logging.rs (222 lines) - 90% Complete
✅ File and stdout logging
✅ Timestamp formatting
✅ Character and player logs
⚠️ Rotation not fully tested

### god.rs (262 lines) - 30% Complete
✅ Manager structures created
❌ Item creation not implemented
❌ Ban list loading missing
❌ Bad word/name lists missing

### constants.rs (650+ lines) - 95% Complete
✅ All constants defined
✅ Command codes
✅ State constants
✅ Item/effect/status constants

---

## Estimated Total Effort

| Phase | Priority | Effort | Difficulty |
|-------|----------|--------|------------|
| Phase 1 (Network/Commands) | CRITICAL | 40-50h | Medium |
| Phase 2 (Persistence) | CRITICAL | 30-40h | Medium |
| Phase 3 (Character/Actions) | CRITICAL | 30-35h | Hard |
| Phase 4 (World Systems) | HIGH | 40-50h | Hard |
| Phase 5 (Polish) | MEDIUM | 15-20h | Easy |
| **TOTAL** | - | **155-195 hours** | Medium-Hard |

**Timeline**: 3-4 weeks for a full server implementation, or 5-7 days for basic playable version.

---

## Recommendations

1. **Start with Phase 1** - Get networking working first, as it's foundation for everything else
2. **Use original C++ as reference** - All logic is defined, just needs translation
3. **Implement incrementally** - Add one system, test, then move to next
4. **Create test cases** - For each major system
5. **Profile early** - Identify bottlenecks before optimization needed
6. **Document protocol** - As you reverse-engineer packet formats

---

## Next Steps

Ready to begin implementing Phase 1: Network I/O & Commands. Start with:

1. `plr_idle()` - Simple 60-second timeout checker
2. `plr_cmd()` - Command dispatch based on packet[0]
3. `plr_state()` - Login state machine transitions
4. `plr_getmap()` / `plr_change()` - Send data to clients

Would you like me to proceed with Phase 1 implementation?
