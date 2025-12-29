use chrono::Timelike;
use core::constants::MAXPLAYER;
use core::types::ServerPlayer;
use parking_lot::ReentrantMutex;
use std::cell::UnsafeCell;
use std::io::ErrorKind;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::effect::EffectManager;
use crate::enums::CharacterFlags;
use crate::god::God;
use crate::lab9::Labyrinth9;
use crate::network_manager::NetworkManager;
use crate::repository::Repository;
use crate::state::State;
use crate::{driver_use, player, populate};
use flate2::write::ZlibEncoder;
use flate2::Compression;

static PLAYERS: OnceLock<ReentrantMutex<UnsafeCell<Box<[core::types::ServerPlayer; MAXPLAYER]>>>> =
    OnceLock::new();

// TICK constant - microseconds per tick (matching C++ TICK value)
const TICK: u64 = 40000; // 40ms per tick
const TICKS: u64 = 25; // ticks per second

#[derive(Debug, Clone, Copy, PartialEq)]
enum CharacterTickState {
    Empty,
    NeedsUpdate,
    CheckExpire,
    Body,
    Active,
}

pub struct Server {
    sock: Option<TcpListener>,
    last_tick_time: Option<Instant>,
}

impl Server {
    pub fn new() -> Self {
        Server {
            sock: None,
            last_tick_time: None,
        }
    }

    pub fn initialize_players() -> Result<(), String> {
        let players: Vec<ServerPlayer> = (1..=MAXPLAYER).map(|_x| ServerPlayer::new()).collect();
        let players: Box<[ServerPlayer; MAXPLAYER]> = players
            .into_boxed_slice()
            .try_into()
            .map_err(|_| "Failed to convert Vec to Box<[ServerPlayer; MAXPLAYER]>")?;

        PLAYERS
            .set(ReentrantMutex::new(UnsafeCell::new(players)))
            .map_err(|_| "Players already initialized".to_string())?;
        Ok(())
    }

    pub fn with_players<F, R>(f: F) -> R
    where
        F: FnOnce(&[core::types::ServerPlayer]) -> R,
    {
        let lock = PLAYERS.get().expect("Players not initialized");
        let guard = lock.lock();
        let inner: &UnsafeCell<Box<[core::types::ServerPlayer; MAXPLAYER]>> = &*guard;
        // SAFETY: We are holding the mutex so creating a shared reference is safe.
        let boxed: &Box<[core::types::ServerPlayer; MAXPLAYER]> = unsafe { &*inner.get() };
        f(&boxed[..])
    }

    pub fn with_players_mut<F, R>(f: F) -> R
    where
        F: FnOnce(&mut [core::types::ServerPlayer]) -> R,
    {
        let lock = PLAYERS.get().expect("Players not initialized");
        let guard = lock.lock();
        let inner: &UnsafeCell<Box<[core::types::ServerPlayer; MAXPLAYER]>> = &*guard;
        // SAFETY: We are holding the mutex so creating a unique mutable reference is safe.
        let boxed_mut: &mut Box<[core::types::ServerPlayer; MAXPLAYER]> =
            unsafe { &mut *inner.get() };
        f(&mut boxed_mut[..])
    }

    fn tmplabcheck(item_idx: usize) {
        Repository::with_characters(|ch| {
            Repository::with_items_mut(|it| {
                let cn = it[item_idx].carried as usize;
                if cn == 0 || !ServerPlayer::is_sane_player(cn) {
                    return;
                }

                // player is inside a lab?
                if ch[cn].temple_x != 512 && ch[cn].temple_x != 558 && ch[cn].temple_x != 813 {
                    return;
                }

                God::take_from_char(item_idx, cn);
                it[item_idx].used = core::constants::USE_EMPTY;

                log::warn!(
                    "Removed Lab Item {} from player {}",
                    it[item_idx].get_name(),
                    cn
                );
            });
        });
    }

    pub fn initialize(&mut self) -> Result<(), String> {
        // Create and configure TCP socket (matching server.cpp socket setup)
        let listener = TcpListener::bind("0.0.0.0:5555")
            .map_err(|e| format!("Failed to bind socket: {}", e))?;

        listener
            .set_nonblocking(true)
            .map_err(|e| format!("Failed to set non-blocking mode: {}", e))?;

        self.sock = Some(listener);
        log::info!("Socket bound to port 5555");

        // Repository is already initialized at this point (currently)
        Server::initialize_players()?;
        State::initialize()?;
        NetworkManager::initialize()?;

        // Mark data as dirty (in use)
        Repository::with_globals_mut(|globals| {
            globals.set_dirty(true);
        });

        // Log out all active characters (cleanup from previous run)
        for i in 0..core::constants::MAXCHARS as usize {
            let should_logout = Repository::with_characters(|characters| {
                characters[i].used == core::constants::USE_ACTIVE
                    && characters[i].flags & CharacterFlags::Player.bits() != 0
            });

            if !should_logout {
                continue;
            }

            Repository::with_characters(|characters| {
                log::info!(
                    "Logging out character '{}' on server startup",
                    characters[i].get_name(),
                );
            });

            player::plr_logout(i, 0, crate::enums::LogoutReason::Shutdown);
        }

        // Initialize subsystems
        Labyrinth9::initialize()?;
        populate::reset_changed_items();

        log::info!("Checking for lab items on players...");
        Repository::with_items_mut(|it| {
            for n in 1..core::constants::MAXITEM {
                if it[n].used == core::constants::USE_EMPTY {
                    continue;
                }
                if it[n].has_laby_destroy() {
                    Self::tmplabcheck(n);
                }
                if it[n].has_soulstone() {
                    // Copy from packed struct to avoid unaligned reference
                    let max_damage = { it[n].max_damage };
                    if max_damage == 0 {
                        it[n].max_damage = 60000;
                        let name = it[n].get_name();
                        log::info!("Set {} ({}) max_damage to 60000", name, n);
                    }
                }
            }
        });

        log::info!("Validating character template positions...");
        Repository::with_character_templates_mut(|ch_temp| {
            for n in 1..core::constants::MAXTCHARS {
                if ch_temp[n].used == core::constants::USE_EMPTY {
                    continue;
                }

                let x = ch_temp[n].data[29] % core::constants::SERVER_MAPX;
                let y = ch_temp[n].data[29] / core::constants::SERVER_MAPX;

                if x == 0 && y == 0 {
                    continue;
                }

                let ch_x = ch_temp[n].x as i32;
                let ch_y = ch_temp[n].y as i32;

                if (x - ch_x).abs() + (y - ch_y).abs() > 200 {
                    log::warn!(
                        "RESET {} ({}): {} {} -> {} {}",
                        n,
                        std::str::from_utf8(&ch_temp[n].name)
                            .unwrap_or("*unknown*")
                            .trim_end_matches('\0'),
                        ch_x,
                        ch_y,
                        x,
                        y
                    );
                    ch_temp[n].data[29] =
                        ch_temp[n].x as i32 + ch_temp[n].y as i32 * core::constants::SERVER_MAPX;
                }
            }
        });

        Ok(())
    }

    pub fn tick(&mut self) {
        // Initialize last_tick_time if not set (equivalent to: if (ltime == 0) ltime = timel())
        if self.last_tick_time.is_none() {
            self.last_tick_time = Some(Instant::now());
        }

        let now = Instant::now();
        let last_time = self.last_tick_time.unwrap();

        // Check if it's time for a game tick (equivalent to: if (ttime > ltime))
        if now > last_time {
            self.last_tick_time = Some(last_time + Duration::from_micros(TICK));

            // Call main game tick (equivalent to: tick() in C++)
            self.game_tick();

            // Compress and send tick data to clients
            self.compress_ticks();

            let new_now = Instant::now();
            let new_last = self.last_tick_time.unwrap();

            // Check if server is running too slow (serious slowness detection)
            if new_now > new_last + Duration::from_micros(TICK * TICKS * 10) {
                log::warn!("Server too slow");
                self.last_tick_time = Some(new_now);
            }
        }

        // Handle network I/O every 8th tick (equivalent to: if (globs->ticker % 8 == 0))
        let should_handle_network = Repository::with_globals(|globals| globals.ticker % 8 == 0);

        if should_handle_network {
            self.handle_network_io();
        }

        // Sleep for remaining time until next tick
        let current_time = Instant::now();
        let target_time = self.last_tick_time.unwrap();

        if current_time < target_time {
            let sleep_duration = target_time.duration_since(current_time);
            std::thread::sleep(sleep_duration);
        }
    }

    fn game_tick(&mut self) {
        // Get current hour for statistics
        let hour = chrono::Local::now().hour() as usize;

        // Increment global tick counters
        Repository::with_globals_mut(|globals| {
            globals.ticker = globals.ticker.wrapping_add(1);
            globals.uptime = globals.uptime.wrapping_add(1);
            globals.uptime_per_hour[hour] = globals.uptime_per_hour[hour].wrapping_add(1);
        });

        let ticker = Repository::with_globals(|globals| globals.ticker);

        // Send tick to players and count online
        let mut online = 0;
        for n in 1..MAXPLAYER {
            let (has_socket, is_normal_or_exit, is_normal) = Self::with_players(|players| {
                if players[n].sock.is_none() {
                    return (false, false, false);
                }
                let state = players[n].state;
                let is_normal_or_exit =
                    state == core::constants::ST_NORMAL || state == core::constants::ST_EXIT;
                let is_normal = state == core::constants::ST_NORMAL;
                (true, is_normal_or_exit, is_normal)
            });

            if !has_socket {
                continue;
            }
            if !is_normal_or_exit {
                continue;
            }

            player::plr_tick(n);

            if is_normal {
                online += 1;
            }
        }

        // Update max online statistics
        Repository::with_globals_mut(|globals| {
            if online > globals.max_online {
                globals.max_online = online;
            }
            if online > globals.max_online_per_hour[hour] {
                globals.max_online_per_hour[hour] = online;
            }
        });

        // Check for player commands and translate to character commands
        for n in 1..MAXPLAYER {
            let has_socket = Self::with_players(|players| players[n].sock.is_some());
            if !has_socket {
                continue;
            }

            // Process all pending commands (16 bytes each)
            loop {
                let in_len = Self::with_players(|players| players[n].in_len);
                if in_len < 16 {
                    break;
                }

                player::plr_cmd(n);

                Self::with_players_mut(|players| {
                    players[n].in_len -= 16;
                    // Shift buffer: memmove(inbuf, inbuf + 16, 240)
                    players[n].inbuf.copy_within(16..256, 0);
                });
            }

            player::plr_idle(n);
        }

        // Do login stuff for players not in normal state
        for n in 1..MAXPLAYER {
            let (has_socket, is_normal) = Self::with_players(|players| {
                if players[n].sock.is_none() {
                    return (false, true);
                }
                (true, players[n].state == core::constants::ST_NORMAL)
            });

            if !has_socket || is_normal {
                continue;
            }

            player::plr_state(n);
        }

        // Send changes to players in normal state
        for n in 1..MAXPLAYER {
            let (has_socket, is_normal) = Self::with_players(|players| {
                if players[n].sock.is_none() {
                    return (false, false);
                }
                (true, players[n].state == core::constants::ST_NORMAL)
            });

            if !has_socket || !is_normal {
                continue;
            }

            player::plr_getmap(n);
            player::plr_change(n);
        }

        // Let characters act
        let mut cnt = 0;
        let mut awake = 0;
        let mut body = 0;
        let mut plon = 0;

        // Wakeup mechanism (every 64 ticks)
        if (ticker & 63) == 0 {
            self.wakeup_character();
        }

        for n in 1..core::constants::MAXCHARS {
            let char_state = Repository::with_characters(|ch| {
                if ch[n].used == core::constants::USE_EMPTY {
                    return CharacterTickState::Empty;
                }

                if ch[n].flags & CharacterFlags::Update.bits() != 0 {
                    return CharacterTickState::NeedsUpdate;
                }

                if ch[n].used == core::constants::USE_NONACTIVE
                    && (n & 1023) == (ticker as usize & 1023)
                {
                    return CharacterTickState::CheckExpire;
                }

                if ch[n].flags & CharacterFlags::Body.bits() != 0 {
                    return CharacterTickState::Body;
                }

                CharacterTickState::Active
            });

            match char_state {
                CharacterTickState::Empty => continue,
                CharacterTickState::NeedsUpdate => {
                    cnt += 1;
                    State::with_mut(|state| {
                        state.really_update_char(n);
                    });

                    Repository::with_characters_mut(|ch| {
                        ch[n].flags &= !CharacterFlags::Update.bits();
                    });
                }
                CharacterTickState::CheckExpire => {
                    cnt += 1;
                    self.check_expire(n);
                }
                CharacterTickState::Body => {
                    cnt += 1;
                    let should_remove = Repository::with_characters_mut(|ch| {
                        if ch[n].flags & CharacterFlags::Player.bits() == 0 {
                            ch[n].data[98] += 1;
                            if ch[n].data[98] > (TICKS * 60 * 30) as i32 {
                                return true;
                            }
                        }
                        false
                    });

                    if should_remove {
                        log::info!("Removing lost body for character {}", n);
                        God::destroy_items(n);
                        Repository::with_characters_mut(|ch| {
                            ch[n].used = core::constants::USE_EMPTY;
                        });
                        continue;
                    }
                    body += 1;
                    continue;
                }
                CharacterTickState::Active => {
                    cnt += 1;
                }
            }

            // Reduce single awake timer
            Repository::with_characters_mut(|ch| {
                if ch[n].data[92] > 0 {
                    ch[n].data[92] -= 1;
                }
            });

            // Check if character should be active
            let should_continue =
                Repository::with_characters(|ch| ch[n].status < 8 && !self.group_active(n));

            if should_continue {
                continue;
            }

            awake += 1;

            let is_active =
                Repository::with_characters(|ch| ch[n].used == core::constants::USE_ACTIVE);

            if is_active {
                // Periodic validation
                if (n & 1023) == (ticker as usize & 1023) && !self.check_valid(n) {
                    continue;
                }

                Repository::with_characters_mut(|ch| {
                    ch[n].current_online_time += 1;
                    ch[n].total_online_time += 1;
                });

                let (is_player_or_usurp, is_player, is_visible) =
                    Repository::with_characters(|ch| {
                        let is_player_or_usurp = (ch[n].flags & CharacterFlags::Player.bits() != 0)
                            || (ch[n].flags & CharacterFlags::Usurp.bits() != 0);
                        let is_player = ch[n].flags & CharacterFlags::Player.bits() != 0;
                        let is_visible = ch[n].flags & CharacterFlags::Invisible.bits() == 0;
                        (is_player_or_usurp, is_player, is_visible)
                    });

                if is_player_or_usurp {
                    Repository::with_globals_mut(|globals| {
                        globals.total_online_time += 1;
                        globals.online_per_hour[hour] += 1;
                    });

                    if is_player {
                        Repository::with_characters_mut(|ch| {
                            if ch[n].data[71] > 0 {
                                ch[n].data[71] -= 1;
                            }
                            if ch[n].data[72] > 0 {
                                ch[n].data[72] -= 1;
                            }
                        });

                        if is_visible {
                            plon += 1;
                        }
                    }
                }

                player::plr_act(n)
            }

            State::with(|state| {
                state.do_regenerate(n);
            });
        }

        // Update global stats
        Repository::with_globals_mut(|globals| {
            globals.character_cnt = cnt;
            globals.awake = awake;
            globals.body = body;
            globals.players_online = plon;
        });

        // Run subsystem ticks
        populate::pop_tick();
        EffectManager::effect_tick();
        driver_use::item_tick();

        self.global_tick();
    }

    // Helper enum for character tick state
    fn wakeup_character(&mut self) {
        // Wakeup one character per 64 ticks
        static WAKEUP: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);

        let mut wakeup_idx = WAKEUP.load(std::sync::atomic::Ordering::Relaxed);
        if wakeup_idx >= core::constants::MAXCHARS {
            wakeup_idx = 1;
        }

        Repository::with_characters_mut(|ch| {
            ch[wakeup_idx].data[92] = (TICKS * 60) as i32;
        });

        WAKEUP.store(wakeup_idx + 1, std::sync::atomic::Ordering::Relaxed);
    }

    fn group_active(&self, cn: usize) -> bool {
        Repository::with_characters(|ch| {
            if ((ch[cn].flags & CharacterFlags::Player.bits() != 0)
                || (ch[cn].flags & CharacterFlags::Usurp.bits() != 0)
                || (ch[cn].flags & CharacterFlags::NoSleep.bits() != 0))
                && ch[cn].used == core::constants::USE_ACTIVE
            {
                return true;
            }
            if ch[cn].data[92] > 0 {
                return true;
            }
            false
        })
    }

    fn check_expire(&self, cn: usize) {
        // Check character expiration similar to the original C++ logic.

        let week: i64 = 60 * 60 * 24 * 7;
        let day: i64 = 60 * 60 * 24;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Grab relevant fields for decision without holding a mutable lock
        let (points_tot, login_date) = Repository::with_characters(|ch| {
            let pts = ch[cn].points_tot;
            let ld = ch[cn].login_date;
            (pts, ld)
        });

        let mut erase = false;
        let pts = points_tot as i64;
        let ld = login_date as i64;

        if pts == 0 {
            if ld + 3 * day < now {
                erase = true;
            }
        } else if pts < 10_000 {
            if ld + 1 * week < now {
                erase = true;
            }
        } else if pts < 100_000 {
            if ld + 2 * week < now {
                erase = true;
            }
        } else if pts < 1_000_000 {
            if ld + 4 * week < now {
                erase = true;
            }
        } else {
            if ld + 6 * week < now {
                erase = true;
            }
        }

        if erase {
            // Log and mark the character as unused. Detailed item cleanup
            // (god_destroy_items) should be implemented elsewhere if needed.
            Repository::with_characters_mut(|ch| {
                let total_exp = ch[cn].points_tot;
                log::info!("erased player {}, {} exp", ch[cn].get_name(), total_exp,);
                ch[cn].used = core::constants::USE_EMPTY;
            });
        }
    }

    fn check_valid(&self, cn: usize) -> bool {
        // Full validation ported from the original C++ check_valid

        // Bounds check
        let (x, y) = Repository::with_characters(|ch| (ch[cn].x, ch[cn].y));
        if x < 1
            || y < 1
            || x > (core::constants::SERVER_MAPX as i16 - 2)
            || y > (core::constants::SERVER_MAPY as i16 - 2)
        {
            Repository::with_characters_mut(|ch| {
                log::warn!(
                    "Killed character {} ({}) for invalid data",
                    String::from_utf8_lossy(&ch[cn].name),
                    cn
                );
                // Best-effort: destroy carried items and mark as unused
                God::destroy_items(cn);
                ch[cn].used = core::constants::USE_EMPTY;
            });
            return false;
        }

        // Map consistency check: map[n].ch should point to this character
        let map_idx = (x as usize) + (y as usize) * core::constants::SERVER_MAPX as usize;
        let map_ch = Repository::with_map(|map| map[map_idx].ch as usize);
        if map_ch != cn {
            Repository::with_characters(|ch| {
                log::warn!(
                    "Not on map (map has {}), fixing char {} at {}",
                    map_ch,
                    cn,
                    ch[cn].get_name()
                );
            });

            if map_ch != 0 {
                // Try to drop character items near their position as in original
                let (cx, cy) =
                    Repository::with_characters(|ch| (ch[cn].x as usize, ch[cn].y as usize));
                if !God::drop_char_fuzzy_large(cn, cx, cy, cx, cy) {
                    // couldn't drop items; leave as-is (original tried a few options)
                }
            } else {
                // claim the map tile for this character
                Repository::with_map_mut(|map| map[map_idx].ch = cn as u32);
            }
        }

        // If character is in build mode accept validity
        let is_building = Repository::with_characters(|ch| ch[cn].is_building());
        if is_building {
            return true;
        }

        // Validate carried items (inventory)
        for slot in 0..40 {
            let in_id = Repository::with_characters(|ch| ch[cn].item[slot] as usize);
            if in_id != 0 {
                let bad = Repository::with_items(|it| {
                    it[in_id].carried as usize != cn
                        || it[in_id].used != core::constants::USE_ACTIVE
                });
                if bad {
                    Repository::with_characters_mut(|ch| {
                        Repository::with_items(|it| {
                            log::warn!(
                                "Reset item {} ({},{}) from char {} ({})",
                                in_id,
                                it[in_id].get_name(),
                                it[in_id].used,
                                cn,
                                String::from_utf8_lossy(&ch[cn].name)
                            );
                        });
                        ch[cn].item[slot] = 0;
                    });
                }
            }
        }

        // Validate depot items
        for slot in 0..62 {
            let in_id = Repository::with_characters(|ch| ch[cn].depot[slot] as usize);
            if in_id != 0 {
                let bad = Repository::with_items(|it| {
                    it[in_id].carried as usize != cn
                        || it[in_id].used != core::constants::USE_ACTIVE
                });
                if bad {
                    Repository::with_characters_mut(|ch| {
                        Repository::with_items(|it| {
                            log::warn!(
                                "Reset depot item {} ({},{}) from char {} ({})",
                                in_id,
                                it[in_id].get_name(),
                                it[in_id].used,
                                cn,
                                String::from_utf8_lossy(&ch[cn].name)
                            );
                        });
                        ch[cn].depot[slot] = 0;
                    });
                }
            }
        }

        // Validate worn and spell items
        for slot in 0..20 {
            let worn_id = Repository::with_characters(|ch| ch[cn].worn[slot] as usize);
            if worn_id != 0 {
                let bad = Repository::with_items(|it| {
                    it[worn_id].carried as usize != cn
                        || it[worn_id].used != core::constants::USE_ACTIVE
                });
                if bad {
                    Repository::with_characters_mut(|ch| {
                        Repository::with_items(|it| {
                            log::warn!(
                                "Reset worn item {} ({},{}) from char {} ({})",
                                worn_id,
                                it[worn_id].get_name(),
                                it[worn_id].used,
                                cn,
                                String::from_utf8_lossy(&ch[cn].name)
                            );
                        });
                        ch[cn].worn[slot] = 0;
                    });
                }
            }

            let spell_id = Repository::with_characters(|ch| ch[cn].spell[slot] as usize);
            if spell_id != 0 {
                let bad = Repository::with_items(|it| {
                    it[spell_id].carried as usize != cn
                        || it[spell_id].used != core::constants::USE_ACTIVE
                });
                if bad {
                    Repository::with_characters_mut(|ch| {
                        Repository::with_items(|it| {
                            log::warn!(
                                "Reset spell item {} ({},{}) from char {} ({})",
                                spell_id,
                                it[spell_id].get_name(),
                                it[spell_id].used,
                                cn,
                                String::from_utf8_lossy(&ch[cn].name)
                            );
                        });
                        ch[cn].spell[slot] = 0;
                    });
                }
            }
        }

        // If stoned and not a player, verify the stoned target is valid
        let is_stoned_nonplayer = Repository::with_characters(|ch| {
            (ch[cn].flags & crate::enums::CharacterFlags::Stoned.bits()) != 0
                && (ch[cn].flags & crate::enums::CharacterFlags::Player.bits()) == 0
        });
        if is_stoned_nonplayer {
            let co = Repository::with_characters(|ch| ch[cn].data[63] as usize);
            let ok = Repository::with_characters(|ch| {
                co != 0 && ch[co].used == core::constants::USE_ACTIVE
            });
            if !ok {
                Repository::with_characters_mut(|ch| {
                    ch[cn].flags &= !crate::enums::CharacterFlags::Stoned.bits();
                    log::info!("oops, stoned removed");
                });
            }
        }

        true
    }

    fn global_tick(&self) {
        // Port of svr_glob.cpp::global_tick
        const MD_HOUR: i32 = 3600;
        const MD_DAY: i32 = MD_HOUR * 24;
        const MD_YEAR: i32 = 300;

        // Increment mdtime and compute day rollover + daylight/moon state
        let (day_rolled, early_return) = Repository::with_globals_mut(|globals| {
            globals.mdtime += 1;

            let mut rolled = false;
            if globals.mdtime >= MD_DAY {
                globals.mdday += 1;
                globals.mdtime = 0;
                rolled = true;
                log::info!(
                    "day {} of the year {} begins",
                    globals.mdday,
                    globals.mdyear
                );
            }

            if globals.mdday >= MD_YEAR {
                globals.mdyear += 1;
                globals.mdday = 1;
            }

            if globals.mdtime < MD_HOUR * 6 {
                globals.dlight = 0;
            } else if globals.mdtime < MD_HOUR * 7 {
                globals.dlight = (globals.mdtime - MD_HOUR * 6) * 255 / MD_HOUR;
            } else if globals.mdtime < MD_HOUR * 22 {
                globals.dlight = 255;
            } else if globals.mdtime < MD_HOUR * 23 {
                globals.dlight = (MD_HOUR * 23 - globals.mdtime) * 255 / MD_HOUR;
            } else {
                globals.dlight = 0;
            }

            let mut tmp = globals.mdday % 28 + 1;

            globals.newmoon = 0;
            globals.fullmoon = 0;

            if tmp == 1 {
                globals.newmoon = 1;
                return (rolled, true);
            }
            if tmp == 15 {
                globals.fullmoon = 1;
            }

            if tmp > 14 {
                tmp = 28 - tmp;
            }
            if tmp > globals.dlight {
                globals.dlight = tmp;
            }

            (rolled, false)
        });

        if early_return {
            return;
        }

        // If a new day began, run pay_rent() and do_misc()
        if day_rolled {
            // pay_rent: call depot payment routine for each player
            for cn in 1..core::constants::MAXCHARS as usize {
                let is_player = Repository::with_characters(|ch| {
                    ch[cn].used != core::constants::USE_EMPTY
                        && (ch[cn].flags & crate::enums::CharacterFlags::Player.bits()) != 0
                });
                if !is_player {
                    continue;
                }
                State::with(|s| s.do_pay_depot(cn));
            }

            // do_misc: adjust luck and clear temporary flags for players
            for cn in 1..core::constants::MAXCHARS as usize {
                let is_player = Repository::with_characters(|ch| {
                    ch[cn].used != core::constants::USE_EMPTY
                        && (ch[cn].flags & crate::enums::CharacterFlags::Player.bits()) != 0
                });
                if !is_player {
                    continue;
                }

                let uniques = crate::driver::count_uniques(cn);

                if uniques > 1 {
                    // reduce luck for multi-unique holders if active
                    let is_active = Repository::with_characters(|ch| {
                        ch[cn].used == core::constants::USE_ACTIVE
                    });
                    if is_active {
                        Repository::with_characters_mut(|ch| {
                            ch[cn].luck -= 5;
                            let luck_to_log = ch[cn].luck;
                            log::info!(
                                "reduced luck by 5 to {} for having more than one unique",
                                luck_to_log,
                            );
                        });
                    }
                } else {
                    // slowly recover luck towards 0
                    Repository::with_characters_mut(|ch| {
                        if ch[cn].luck < 0 {
                            ch[cn].luck += 1;
                        }
                        if ch[cn].luck < 0 {
                            ch[cn].luck += 1;
                        }
                        // clear temporary punishment flags
                        let mask = crate::enums::CharacterFlags::ShutUp.bits()
                            | crate::enums::CharacterFlags::NoDesc.bits()
                            | crate::enums::CharacterFlags::Kicked.bits();
                        ch[cn].flags &= !mask;
                    });
                }
            }
        }
    }

    fn compress_ticks(&mut self) {
        // For each connected player, compress their tick buffer (`tbuf`) if worthwhile
        Server::with_players_mut(|players| {
            for n in 1..players.len() {
                // quick checks
                if players[n].sock.is_none() {
                    continue;
                }
                if players[n].ticker_started == 0 {
                    continue;
                }

                // ensure sane usnr
                if players[n].usnr >= core::constants::MAXCHARS as usize {
                    players[n].usnr = 0;
                }

                let ilen = players[n].tptr;
                let olen = ilen + 2;

                if olen > 16 {
                    // compress into encoder's inner buffer
                    if let Some(zs) = players[n].zs.as_mut() {
                        let before = zs.get_ref().len();
                        let _ = zs.write_all(&players[n].tbuf[..ilen]);
                        // flush to ensure we get compressed bytes out (Z_SYNC_FLUSH equivalent)
                        let _ = zs.flush();
                        let after = zs.get_ref().len();
                        let csize = after.saturating_sub(before);

                        // prepare 2-byte header with high bit set to indicate compressed
                        let header = (((csize + 2) as u16) | 0x8000u16).to_le_bytes();

                        // Extract compressed data before creating the closure
                        let compressed = zs.get_ref()[before..after].to_vec();

                        // write header then compressed bytes into obuf with wrap
                        let obuf_len = players[n].obuf.len();
                        let mut iptr = players[n].iptr;

                        // helper to copy slice into obuf with wrap
                        let mut write_into = |data: &[u8]| {
                            for &b in data {
                                players[n].obuf[iptr] = b;
                                iptr += 1;
                                if iptr >= obuf_len {
                                    iptr = 0;
                                }
                            }
                        };

                        write_into(&header);
                        write_into(&compressed);

                        players[n].iptr = iptr;

                        // update character stats
                        let usnr = players[n].usnr;
                        Repository::with_characters_mut(|ch| {
                            if usnr < core::constants::MAXCHARS as usize {
                                ch[usnr].comp_volume =
                                    ch[usnr].comp_volume.wrapping_add((csize + 2) as u32);
                                ch[usnr].raw_volume = ch[usnr].raw_volume.wrapping_add(ilen as u32);
                            }
                        });
                    }
                } else {
                    // send raw (no compression)
                    let header = (olen as u16).to_le_bytes();
                    let obuf_len = players[n].obuf.len();
                    let mut iptr = players[n].iptr;

                    // Copy tbuf data before defining the closure
                    let tbuf_data: Vec<u8> = if ilen > 0 {
                        players[n].tbuf[..ilen].to_vec()
                    } else {
                        Vec::new()
                    };

                    let mut write_into = |data: &[u8]| {
                        for &b in data {
                            players[n].obuf[iptr] = b;
                            iptr += 1;
                            if iptr >= obuf_len {
                                iptr = 0;
                            }
                        }
                    };

                    write_into(&header);
                    if ilen > 0 {
                        write_into(&tbuf_data);
                    }
                    players[n].iptr = iptr;

                    // update character stats
                    let usnr = players[n].usnr;
                    Repository::with_characters_mut(|ch| {
                        if usnr < core::constants::MAXCHARS as usize {
                            ch[usnr].comp_volume = ch[usnr].comp_volume.wrapping_add(olen as u32);
                            ch[usnr].raw_volume = ch[usnr].raw_volume.wrapping_add(ilen as u32);
                        }
                    });
                }

                // reset tptr
                players[n].tptr = 0;
            }
        });
    }

    fn handle_network_io(&mut self) {
        // Handle new connections
        if let Some(ref listener) = self.sock {
            match listener.accept() {
                Ok((stream, addr)) => {
                    log::info!("New connection from {}", addr);
                    self.new_player(stream, addr.ip());
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                    // No pending connections, this is normal in non-blocking mode
                }
                Err(e) => {
                    log::error!("Error accepting connection: {}", e);
                }
            }
        }

        // Handle existing player connections
        for player_idx in 1..MAXPLAYER {
            let (has_socket, needs_recv, needs_send) = Self::with_players(|players| {
                if players[player_idx].sock.is_none() {
                    return (false, false, false);
                }
                let needs_recv = players[player_idx].in_len < 256;
                let needs_send = players[player_idx].iptr != players[player_idx].optr;
                (true, needs_recv, needs_send)
            });

            if !has_socket {
                continue;
            }

            if needs_recv {
                self.rec_player(player_idx);
            }

            if needs_send {
                self.send_player(player_idx);
            }
        }
    }

    fn new_player(&mut self, stream: std::net::TcpStream, addr: std::net::IpAddr) {
        // Accept and initialize a new player slot. Mirrors server.cpp::new_player

        // Set non-blocking mode on the socket
        let _ = stream.set_nonblocking(true);

        // Convert IPv4 address to u32 (use 0 for IPv6)
        let addr_u32: u32 = match addr {
            std::net::IpAddr::V4(a) => u32::from_be_bytes(a.octets()),
            _ => 0,
        };

        // Prepare a fresh ServerPlayer and find a free slot
        let mut slot: Option<usize> = None;
        Server::with_players_mut(move |players| {
            for n in 1..players.len() {
                if players[n].sock.is_none() {
                    slot = Some(n);
                    break;
                }
            }

            if slot.is_none() {
                // No free slot; drop the socket and return
                log::warn!("new_player: MAXPLAYER reached");
                return;
            }

            let n = slot.unwrap();

            // Build fresh player state similar to ServerPlayer::new()
            let mut newp = core::types::ServerPlayer::new();
            newp.sock = Some(stream);
            newp.addr = addr_u32;

            // Initialize compression (deflateInit level 9 equivalent)
            newp.zs = Some(ZlibEncoder::new(Vec::new(), Compression::best()));

            // Set initial state values
            newp.state = core::constants::ST_CONNECT;
            newp.lasttick = Repository::with_globals(|g| g.ticker as u32);
            newp.lasttick2 = newp.lasttick;
            newp.prio = 0;
            newp.ticker_started = 0;

            players[n] = newp;

            log::info!("New connection assigned to slot {}", n);
        });
    }

    fn rec_player(&self, _player_idx: usize) {
        // Receive incoming bytes from a connected player's socket.
        let idx = _player_idx;
        Server::with_players_mut(|players| {
            if idx >= players.len() {
                return;
            }

            // Ensure socket exists
            if players[idx].sock.is_none() {
                return;
            }

            // Prepare slice for reading
            let in_len = players[idx].in_len;
            if in_len >= players[idx].inbuf.len() {
                return;
            }

            // Borrow socket mutably and read into available buffer
            if let Some(ref mut sock) = players[idx].sock {
                match sock.read(&mut players[idx].inbuf[in_len..]) {
                    Ok(0) => {
                        // Connection closed by peer
                        log::info!("Connection closed (recv)");
                        let cn = players[idx].usnr;
                        players[idx].sock = None;
                        players[idx].ltick = 0;
                        players[idx].rtick = 0;
                        players[idx].zs = None;
                        player::plr_logout(cn, idx, crate::enums::LogoutReason::Unknown);
                    }
                    Ok(len) => {
                        players[idx].in_len += len;
                        Repository::with_globals_mut(|g| {
                            g.recv += len as i64;
                        });
                    }
                    Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                        // No data to read now
                    }
                    Err(e) => {
                        log::error!("Connection closed (recv error): {}", e);
                        let cn = players[idx].usnr;
                        players[idx].sock = None;
                        players[idx].ltick = 0;
                        players[idx].rtick = 0;
                        players[idx].zs = None;
                        player::plr_logout(cn, idx, crate::enums::LogoutReason::Unknown);
                    }
                }
            }
        });
    }

    fn send_player(&self, player_idx: usize) {
        // Send pending data from player's output buffer to their socket.
        let idx = player_idx;
        Server::with_players_mut(|players| {
            if idx >= players.len() {
                return;
            }
            if players[idx].sock.is_none() {
                return;
            }

            let iptr = players[idx].iptr;
            let optr = players[idx].optr;
            let obuf_len = players[idx].obuf.len();

            let (len, slice_start) = if iptr < optr {
                (obuf_len - optr, optr)
            } else {
                (iptr - optr, optr)
            };

            if len == 0 {
                return;
            }

            if let Some(ref mut sock) = players[idx].sock {
                // Write the available contiguous slice
                let end = slice_start + len;
                let to_send = &players[idx].obuf[slice_start..end.min(players[idx].obuf.len())];
                match sock.write(to_send) {
                    Ok(0) => {
                        log::error!("Connection closed (send, wrote 0)");
                        let cn = players[idx].usnr;
                        players[idx].sock = None;
                        players[idx].ltick = 0;
                        players[idx].rtick = 0;
                        players[idx].zs = None;
                        player::plr_logout(cn, idx, crate::enums::LogoutReason::Unknown);
                    }
                    Ok(ret) => {
                        Repository::with_globals_mut(|g| g.send += ret as i64);
                        players[idx].optr += ret;
                        if players[idx].optr >= players[idx].obuf.len() {
                            players[idx].optr = 0;
                        }
                    }
                    Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                        // socket not ready for writing
                    }
                    Err(e) => {
                        log::error!("Connection closed (send error): {}", e);
                        let cn = players[idx].usnr;
                        players[idx].sock = None;
                        players[idx].ltick = 0;
                        players[idx].rtick = 0;
                        players[idx].zs = None;
                        player::plr_logout(cn, idx, crate::enums::LogoutReason::Unknown);
                    }
                }
            }
        });
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        log::info!("Server shutting down, marking data as clean.");
        Repository::with_globals_mut(|globals| {
            globals.set_dirty(false);
        });
    }
}
