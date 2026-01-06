use chrono::Timelike;
use core::constants::{MAXPLAYER, TILEX, TILEY};
use core::stat_buffer::StatisticsBuffer;
use core::types::{CMap, Map, ServerPlayer};
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
use crate::{driver, player, populate};
use flate2::write::ZlibEncoder;
use flate2::Compression;

/// Global array of server player slots protected by a reentrant mutex.
///
/// Stored in a `OnceLock` and containing `MAXPLAYER` `ServerPlayer` entries.
/// Accessors `Server::with_players` and `Server::with_players_mut` provide
/// thread-safe read or mutable access via closures.
static PLAYERS: OnceLock<ReentrantMutex<UnsafeCell<Box<[core::types::ServerPlayer; MAXPLAYER]>>>> =
    OnceLock::new();

/// Per-character scheduling hints used by `game_tick`.
///
/// Determines which processing path a character should take on a tick:
/// - `Empty`: unused slot
/// - `NeedsUpdate`: character flagged for immediate update
/// - `CheckExpire`: non-active slot that should be checked for expiration
/// - `Body`: a corpse/body that needs body handling
/// - `Active`: normal active processing
#[derive(Debug, Clone, Copy, PartialEq)]
enum CharacterTickState {
    Empty,
    NeedsUpdate,
    CheckExpire,
    Body,
    Active,
}

/// The server runtime object which manages networking and tick timing.
///
/// Holds the listener socket and timing state used by the main loop. Create
/// with `Server::new()` and call `initialize()` prior to running ticks.
pub struct Server {
    sock: Option<TcpListener>,
    last_tick_time: Option<Instant>,

    /// Tick rate performance statistics buffer.
    tick_perf_stats: StatisticsBuffer<f32>,

    /// Network I/O performance statistics buffer.
    net_io_perf_stats: StatisticsBuffer<f32>,

    /// Measurement interval in ticks for performance statistics.
    measurement_interval: u32,
}

impl Server {
    /// Construct a new `Server` instance with uninitialized socket and
    /// counters. Call `initialize()` to bind the port and set up subsystems.
    pub fn new() -> Self {
        Server {
            sock: None,
            last_tick_time: None,
            tick_perf_stats: StatisticsBuffer::new(100),
            net_io_perf_stats: StatisticsBuffer::new(100),
            measurement_interval: 20,
        }
    }

    /// Allocate and initialize the global player slot array.
    ///
    /// Creates `MAXPLAYER` `ServerPlayer` entries and stores them inside the
    /// `PLAYERS` `OnceLock`, wrapped with a `ReentrantMutex`. Returns an error
    /// if the players array is already initialized or conversion fails.
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

    /// Execute `f` with a read-only view of the player slots.
    ///
    /// This helper acquires the `PLAYERS` mutex and provides a shared slice of
    /// `ServerPlayer` to the closure while the lock is held. Use this to
    /// safely read player fields.
    pub fn with_players<F, R>(f: F) -> R
    where
        F: FnOnce(&[core::types::ServerPlayer]) -> R,
    {
        let lock = PLAYERS.get().expect("Players not initialized");
        let guard = lock.lock();
        let inner: &UnsafeCell<Box<[core::types::ServerPlayer; MAXPLAYER]>> = &guard;
        // SAFETY: We are holding the mutex so creating a shared reference is safe.
        let boxed: &Box<[core::types::ServerPlayer; MAXPLAYER]> = unsafe { &*inner.get() };
        f(&boxed[..])
    }

    /// Execute `f` with a mutable view of the player slots.
    ///
    /// Provides exclusive mutable access to the player array while the
    /// repository mutex is held. Use this to initialize or update player
    /// connection state.
    pub fn with_players_mut<F, R>(f: F) -> R
    where
        F: FnOnce(&mut [core::types::ServerPlayer]) -> R,
    {
        let lock = PLAYERS.get().expect("Players not initialized");
        let guard = lock.lock();
        let inner: &UnsafeCell<Box<[core::types::ServerPlayer; MAXPLAYER]>> = &guard;
        // SAFETY: We are holding the mutex so creating a unique mutable reference is safe.
        let boxed_mut: &mut Box<[core::types::ServerPlayer; MAXPLAYER]> =
            unsafe { &mut *inner.get() };
        f(&mut boxed_mut[..])
    }

    /// Check whether an item carried by a player is a 'labyrinth' item and
    /// remove it when the player is inside designated lab coordinates.
    ///
    /// This mirrors the original `tmplabcheck` behavior and sets `used` to
    /// `USE_EMPTY` and transfers ownership back to God when appropriate.
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

    /// Initialize the server: bind listening socket and initialize subsystems.
    ///
    /// Actions performed:
    /// - Bind to 0.0.0.0:5555 and set the socket non-blocking
    /// - Initialize the `PLAYERS` array, `State`, `NetworkManager` and other
    ///   subsystems
    /// - Mark repository data as dirty and perform startup cleanup (force
    ///   logout of active characters from prior runs)
    ///
    /// Returns an error if socket bind or subsystem initialization fails.
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
        for i in 0..core::constants::MAXCHARS {
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
                        ch_temp[n].get_name(),
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

    /// Advance the server by a single scheduling tick.
    ///
    /// When it's time, `tick` will call `game_tick()` to run world logic, then
    /// compress and send tick updates to players, perform slower network I/O
    /// periodically (every 8 ticks), and finally sleep to maintain the target
    /// tick rate.
    pub fn tick(&mut self) {
        // Initialize last_tick_time if not set (equivalent to: if (ltime == 0) ltime = timel())
        if self.last_tick_time.is_none() {
            self.last_tick_time = Some(Instant::now());
        }

        let now = Instant::now();
        let last_time = self.last_tick_time.unwrap();

        // Check if it's time for a game tick (equivalent to: if (ttime > ltime))
        if now > last_time {
            let pre_tick_time = Instant::now();

            self.last_tick_time =
                Some(last_time + Duration::from_micros(core::constants::TICK as u64));

            // Call main game tick (equivalent to: tick() in C++)
            self.game_tick();

            // Compress and send tick data to clients
            self.compress_ticks();

            let new_now = Instant::now();
            let new_last = self.last_tick_time.unwrap();

            // Check if server is running too slow (serious slowness detection)
            // In the original C++ this threshold was `TICK * TICKS * 10` (10 seconds).
            if new_now > new_last + Duration::from_secs(10) {
                log::warn!("Server too slow");
                self.last_tick_time = Some(new_now);
            }

            let post_tick_time = Instant::now();

            if Repository::with_globals(|globs| {
                globs
                    .ticker
                    .unsigned_abs()
                    .is_multiple_of(self.measurement_interval)
            }) {
                let tick_duration =
                    post_tick_time.duration_since(pre_tick_time).as_secs_f32() * 1000.0;
                self.tick_perf_stats.push(tick_duration);

                const DESIRED_TICK_TIME_MS: f32 = core::constants::TICK as f32 / 1000.0; // 1000 microseconds per millisecond

                Repository::with_globals_mut(|globs| {
                    globs.load = ((tick_duration / DESIRED_TICK_TIME_MS) * 100.0) as i64;

                    // TODO: Update this to be a proper moving average of the load
                    // globs.load_avg = self.tick_perf_stats.stats().mean as i32;

                    log::debug!(
                        "Tick time: {:.2} ms (max: {:.2} ms), Load: {:.2}%",
                        tick_duration,
                        self.tick_perf_stats.stats().max,
                        globs.load,
                    );
                })
            }
        }

        // Handle network I/O every scheduling tick.
        // Limiting this to every Nth game tick introduces noticeable input lag
        // and delayed map/tick packet delivery.
        let pre_io_time = Instant::now();
        self.handle_network_io();

        if Repository::with_globals(|globs| {
            globs
                .ticker
                .unsigned_abs()
                .is_multiple_of(self.measurement_interval)
        }) {
            let io_duration = Instant::now().duration_since(pre_io_time).as_secs_f32() * 1000.0;
            self.net_io_perf_stats.push(io_duration);

            log::debug!(
                "Network I/O time: {:.2} ms (max: {:.2} ms)",
                io_duration,
                self.net_io_perf_stats.stats().max,
            );
        }

        // Sleep for remaining time until next tick
        let current_time = Instant::now();
        let target_time = self.last_tick_time.unwrap();

        if current_time < target_time {
            let sleep_duration = target_time.duration_since(current_time);
            std::thread::sleep(sleep_duration);
        }
    }

    /// Execute the main game tick logic.
    ///
    /// Responsibilities include:
    /// - Updating tick-rate statistics
    /// - Incrementing global counters and uptime
    /// - Driving player ticks and processing commands
    /// - Running character and NPC actions, expiration checks, and body handling
    /// - Updating global statistics and letting subsystems tick (populate, effects,
    ///   item driver)
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
                            if ch[n].data[98] > (core::constants::TICKS * 60 * 30) {
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
        driver::item_tick();

        self.global_tick();
    }

    // Helper enum for character tick state
    /// Wake up one character in a round-robin fashion.
    ///
    /// This sets the single-character awake timer (`data[92]`) for one template
    /// index each call, cycling through `MAXCHARS` over time.
    fn wakeup_character(&mut self) {
        // Wakeup one character per 64 ticks
        static WAKEUP: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);

        let mut wakeup_idx = WAKEUP.load(std::sync::atomic::Ordering::Relaxed);
        if wakeup_idx >= core::constants::MAXCHARS {
            wakeup_idx = 1;
        }

        Repository::with_characters_mut(|ch| {
            ch[wakeup_idx].data[92] = core::constants::TICKS * 60;
        });

        WAKEUP.store(wakeup_idx + 1, std::sync::atomic::Ordering::Relaxed);
    }

    /// Return true if the character `cn` should be considered active.
    ///
    /// Characters are active if they are players/usurpers, flagged with
    /// `NoSleep`, currently `USE_ACTIVE`, or have a non-zero single-awake
    /// timer (`data[92]`).
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

    /// Check whether a non-active character `cn` should be expired/erased.
    ///
    /// Uses a tiered expiration policy based on total points and last login
    /// date (e.g., zero-point characters are removed after 3 days; higher
    /// ranks get longer grace periods). When expiration triggers, the
    /// character is marked `USE_EMPTY` and logged.
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
            if ld + week < now {
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

    /// Validate character `cn`'s internal consistency and position.
    ///
    /// Performs several checks ported from the original C++ server:
    /// - Bounds checks for `x`/`y` coordinates
    /// - Map tile ownership consistency (`map[idx].ch`)
    /// - Inventory consistency (carried, depot, worn, spell slots)
    /// - Special-case checks (building mode, stoned non-player target validity)
    ///
    /// Returns `true` if character passes validation; otherwise cleans up and
    /// returns `false`.
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
                    ch[cn].get_name(),
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
                                ch[cn].get_name(),
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
                                ch[cn].get_name()
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
                                ch[cn].get_name()
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
                                "Reset spell item {} from char {}.",
                                it[spell_id].get_name(),
                                ch[cn].get_name()
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

    /// Handle global (world) time progression and daily events.
    ///
    /// Advances `mdtime`, rolls day/year counters, updates daylight/moon phase
    /// and, when a new day begins, performs daily maintenance such as depot
    /// payments and miscellaneous per-player adjustments.
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
            for cn in 1..core::constants::MAXCHARS {
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
            for cn in 1..core::constants::MAXCHARS {
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

    /// Compress outgoing per-player tick buffers using zlib when beneficial.
    ///
    /// Iterates connected players and attempts to compress their `tbuf` data
    /// into each player's `zs` encoder. Updates buffer pointers and resets
    /// `tptr` after compressing.
    fn compress_ticks(&mut self) {
        // For each connected player, compress their tick buffer (`tbuf`) if worthwhile.
        // This is intended to match the original C++ `compress_ticks()` logic closely.
        Server::with_players_mut(|players| {
            let header_from_int = |v: i32| {
                // C++ does: csend(n, reinterpret_cast<unsigned char*>(&olen), 2)
                // where `olen` is an `int`. That sends the first two bytes of the native-endian
                // `int` representation (not explicitly little-endian).
                let b = v.to_ne_bytes();
                [b[0], b[1]]
            };

            let ring_free_space = |iptr: usize, optr: usize, cap: usize| -> usize {
                // Keep one byte empty to distinguish full vs empty.
                let used = if iptr >= optr {
                    iptr - optr
                } else {
                    cap - optr + iptr
                };
                cap.saturating_sub(used + 1)
            };

            for n in 1..players.len() {
                if players[n].sock.is_none() {
                    continue;
                }
                if players[n].ticker_started == 0 {
                    continue;
                }

                // Work on a single player slot.
                let p = &mut players[n];

                if p.usnr >= core::constants::MAXCHARS {
                    p.usnr = 0;
                }

                let ilen = p.tptr;
                let olen_uncompressed_i32: i32 = (ilen + 2) as i32;

                // Snapshot tick data (so we can freely borrow `zs` / `obuf` later).
                let tbuf_data: Vec<u8> = if ilen > 0 {
                    p.tbuf[..ilen].to_vec()
                } else {
                    Vec::new()
                };

                // Build packet contents first, then write into the output ring.
                let (olen_i32, header, payload): (i32, [u8; 2], Vec<u8>) = if olen_uncompressed_i32
                    > 16
                {
                    if let Some(zs) = p.zs.as_mut() {
                        let before = zs.get_ref().len();
                        let _ = zs.write_all(&tbuf_data);
                        let _ = zs.flush();

                        let after = zs.get_ref().len();
                        let produced = after.saturating_sub(before);
                        let csize = produced.min(core::constants::OBUFSIZE);

                        // C++ truncates to OBUFSIZE (fixed `obuf`)
                        if produced > csize {
                            log::warn!(
                                    "compress_ticks: compressed output truncated for player {} (produced {}, capped {}, ilen {}, usnr {})",
                                    n,
                                    produced,
                                    csize,
                                    ilen,
                                    p.usnr
                                );
                            zs.get_mut().truncate(before + csize);
                        }

                        // The protocol uses the 0x8000 bit as a compression flag.
                        // If (csize + 2) reaches or exceeds 0x8000, clients which mask
                        // the flag bit to obtain length will desync.
                        if csize + 2 >= 0x8000 {
                            log::error!(
                                    "compress_ticks: compressed packet length too large for player {} (csize {}, len_with_header {}, ilen {}, usnr {})",
                                    n,
                                    csize,
                                    csize + 2,
                                    ilen,
                                    p.usnr
                                );
                        }

                        let olen_i32 = ((csize + 2) as i32) | 0x8000;
                        let header = header_from_int(olen_i32);
                        let payload = zs.get_ref()[before..before + csize].to_vec();
                        (olen_i32, header, payload)
                    } else {
                        // If compression state is missing, fall back to uncompressed.
                        let header = header_from_int(olen_uncompressed_i32);
                        (olen_uncompressed_i32, header, tbuf_data)
                    }
                } else {
                    // Uncompressed path: always send the 2-byte header, even if ilen == 0.
                    let header = header_from_int(olen_uncompressed_i32);
                    (olen_uncompressed_i32, header, tbuf_data)
                };

                // Write header and payload into the ring buffer.
                let needed = 2usize + payload.len();
                let free = ring_free_space(p.iptr, p.optr, p.obuf.len());
                if needed > free {
                    log::warn!(
                        "compress_ticks: obuf overflow risk for player {} (need {}, free {}, iptr {}, optr {}, ilen {}, olen_i32 {}, usnr {})",
                        n,
                        needed,
                        free,
                        p.iptr,
                        p.optr,
                        ilen,
                        olen_i32,
                        p.usnr
                    );
                    // Don't overwrite unsent bytes; drop this tick packet.
                    p.tptr = 0;
                    continue;
                }

                let mut iptr = p.iptr;
                let obuf_len = p.obuf.len();
                let mut write_into = |data: &[u8]| {
                    for &b in data {
                        p.obuf[iptr] = b;
                        iptr += 1;
                        if iptr >= obuf_len {
                            iptr = 0;
                        }
                    }
                };

                write_into(&header);
                if !payload.is_empty() {
                    write_into(&payload);
                }

                p.iptr = iptr;

                // Stats update (C++ does this unconditionally, with `olen` including 0x8000
                // in the compressed case).
                let usnr = p.usnr;
                Repository::with_characters_mut(|ch| {
                    if usnr < core::constants::MAXCHARS {
                        ch[usnr].comp_volume = ch[usnr].comp_volume.wrapping_add(olen_i32 as u32);
                        ch[usnr].raw_volume = ch[usnr].raw_volume.wrapping_add(ilen as u32);
                    }
                });

                p.tptr = 0;
            }
        });
    }

    /// Accept new connections and perform per-player network IO.
    ///
    /// Accepts new TCP connections on the listener, assigning them a free
    /// /// player slot via `new_player`. For existing connections, it calls
    /// `rec_player` and `send_player` as necessary to handle receive and send
    /// activity.
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
            let has_socket = Self::with_players(|players| !players[player_idx].sock.is_none());

            if !has_socket {
                continue;
            }

            self.rec_player(player_idx);

            self.send_player(player_idx);
        }
    }

    /// Accept a new incoming connection and assign it a player slot.
    ///
    /// Converts the peer address into a u32 (IPv4) and initializes a fresh
    /// `ServerPlayer` including zlib compression state. If no free slot is
    /// available, the connection is closed.
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

            // Set initial state values
            players[n] = core::types::ServerPlayer::new();
            players[n].sock = Some(stream);
            players[n].addr = addr_u32;
            // Initialize compression (deflateInit level 9 equivalent)
            players[n].zs = Some(ZlibEncoder::new(Vec::new(), Compression::best()));
            players[n].state = core::constants::ST_CONNECT;
            players[n].lasttick = Repository::with_globals(|g| g.ticker as u32);
            players[n].lasttick2 = players[n].lasttick;
            players[n].prio = 0;
            players[n].ticker_started = 0;
            players[n].inbuf[0] = 0;
            players[n].in_len = 0;
            players[n].iptr = 0;
            players[n].optr = 0;
            players[n].tptr = 0;
            players[n].challenge = 0;
            players[n].usnr = 0;
            players[n].pass1 = 0;
            players[n].pass2 = 0;

            players[n].cmap.fill(CMap::default());
            players[n].smap.fill(CMap::default());
            players[n].xmap.fill(Map::default());
            players[n].passwd.fill(0);

            for m in 0..(TILEX * TILEY) {
                players[n].cmap[m].ba_sprite = core::constants::SPR_EMPTY as i16;
                players[n].smap[m].ba_sprite = core::constants::SPR_EMPTY as i16;
            }

            log::info!("New connection assigned to slot {}", n);
        });
    }

    /// Read available bytes from a player's socket into their input buffer.
    ///
    /// This method attempts a non-blocking read into `inbuf` and updates
    /// `in_len` accordingly. IO errors and disconnects are handled similarly
    /// to the original server behavior.
    fn rec_player(&self, _player_idx: usize) {
        // Receive incoming bytes from a connected player's socket.
        let idx = _player_idx;
        Server::with_players_mut(|players| {
            if idx >= players.len() {
                log::error!("rec_player: invalid player index {}", idx);
                return;
            }

            // Ensure socket exists
            if players[idx].sock.is_none() {
                log::error!("rec_player: no socket for player index {}", idx);
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

    /// Flush pending output bytes from `obuf` to the player's TCP socket.
    ///
    /// Handles partial writes and advances the circular buffer pointers. On
    /// fatal socket errors the player slot may be disconnected.
    fn send_player(&self, player_idx: usize) {
        // Send pending data from player's output buffer to their socket.
        let idx = player_idx;
        Server::with_players_mut(|players| {
            if idx >= players.len() {
                log::error!("send_player: invalid player index {}", idx);
                return;
            }
            if players[idx].sock.is_none() {
                log::error!("send_player: no socket for player index {}", idx);
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
        Repository::shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test the Server::new() constructor
    #[test]
    fn test_server_new() {
        let server = Server::new();

        // Verify initial state
        assert!(server.sock.is_none());
        assert!(server.last_tick_time.is_none());
        assert_eq!(server.measurement_interval, 20);

        // Verify statistics buffers are initialized with correct capacity
        // Note: We can't directly access the internal state of StatisticsBuffer,
        // but we can verify they were created without panicking
        let _ = &server.tick_perf_stats;
        let _ = &server.net_io_perf_stats;
    }

    /// Test Server struct field access and initialization
    #[test]
    fn test_server_struct_initialization() {
        let server = Server::new();

        // Test that we can access all fields (compilation test)
        let _ = &server.sock;
        let _ = &server.last_tick_time;
        let _ = &server.tick_perf_stats;
        let _ = &server.net_io_perf_stats;
        let _ = &server.measurement_interval;

        // Test that statistics buffers are properly initialized
        // (We can't inspect their internal state, but we can verify they exist)
        let server2 = Server::new();
        // Each server should have its own statistics buffers
        let _ = (&server.tick_perf_stats, &server2.tick_perf_stats);
    }
}
