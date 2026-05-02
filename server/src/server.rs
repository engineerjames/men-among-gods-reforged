use chrono::Timelike;
use core::constants::{CharacterFlags, TILEX, TILEY};
use core::logout_reasons::LogoutReason;
use core::stat_buffer::StatisticsBuffer;
use core::types::Map;
use std::io::ErrorKind;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::effect::EffectManager;
use crate::game_state::GameState;
use crate::god::God;
use crate::tls::{self, GameStream};
use crate::types::cmap::CMap;
use crate::types::server_player::ServerPlayer;
use crate::{driver, player, populate};
use flate2::Compression;
use flate2::write::ZlibEncoder;
use server::keydb::background_saver::{self, BackgroundSaver, SaveJob};

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

    /// TLS configuration (loaded from `SERVER_TLS_CERT` / `SERVER_TLS_KEY`).
    tls_config: Option<Arc<rustls::ServerConfig>>,

    /// Tick rate performance statistics buffer.
    tick_perf_stats: StatisticsBuffer<f32>,

    /// Network I/O performance statistics buffer.
    net_io_perf_stats: StatisticsBuffer<f32>,

    /// Measurement interval in ticks for performance statistics.
    measurement_interval: u32,

    /// Background saver handle (only present when using KeyDB backend).
    background_saver: Option<BackgroundSaver>,

    /// Background watcher that surfaces admin-issued template reload
    /// requests to the tick loop.
    template_reload_watcher: Option<server::keydb::template_reload::TemplateReloadWatcher>,

    /// Background watcher that surfaces admin-issued text reload requests to
    /// the tick loop.
    text_reload_watcher: Option<server::keydb::text_reload::TextReloadWatcher>,

    /// Background watcher that surfaces admin-issued map-tile patches to
    /// the tick loop.
    map_patch_watcher: Option<server::keydb::map_patch::MapPatchWatcher>,

    /// Background watcher that surfaces admin-issued item patches to the
    /// tick loop.
    item_patch_watcher: Option<server::keydb::item_patch::ItemPatchWatcher>,

    /// Background watcher that surfaces admin-issued character patches to
    /// the tick loop.
    character_patch_watcher: Option<server::keydb::character_patch::CharacterPatchWatcher>,

    /// Counter that drives the rotating save schedule (increments each tick
    /// when using KeyDB backend).
    save_tick_counter: u32,
}

impl Server {
    /// Construct a new `Server` instance with uninitialized socket and
    /// counters. Call `initialize()` to bind the port and set up subsystems.
    pub fn new() -> Self {
        Server {
            sock: None,
            last_tick_time: None,
            tls_config: None,
            tick_perf_stats: StatisticsBuffer::new(100),
            net_io_perf_stats: StatisticsBuffer::new(100),
            measurement_interval: 20,
            background_saver: None,
            template_reload_watcher: None,
            text_reload_watcher: None,
            map_patch_watcher: None,
            item_patch_watcher: None,
            character_patch_watcher: None,
            save_tick_counter: 0,
        }
    }

    /// Check whether an item carried by a player is a 'labyrinth' item and
    /// remove it when the player is inside designated lab coordinates.
    ///
    /// This mirrors the original `tmplabcheck` behavior and sets `used` to
    /// `USE_EMPTY` and transfers ownership back to God when appropriate.
    ///
    /// # Arguments
    ///
    /// * `gs` - Mutable reference to the unified game state.
    /// * `item_idx` - The item index to check.
    fn tmplabcheck(gs: &mut GameState, item_idx: usize) {
        let cn = gs.items[item_idx].carried as usize;
        if cn == 0 || !ServerPlayer::is_sane_player(cn) {
            return;
        }

        // player is inside a lab?
        if gs.characters[cn].temple_x != 512
            && gs.characters[cn].temple_x != 558
            && gs.characters[cn].temple_x != 813
        {
            return;
        }

        God::take_from_char(gs, item_idx, cn);
        gs.items[item_idx].used = core::constants::USE_EMPTY;

        log::warn!(
            "Removed Lab Item {} from player {}",
            gs.items[item_idx].get_name(),
            cn
        );
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
    ///
    /// # Arguments
    ///
    /// * `gs` - Mutable reference to the unified game state.
    ///
    /// # Returns
    ///
    /// * `Ok(())` on success.
    /// * `Err(String)` if socket bind or subsystem initialization fails.
    pub fn initialize(&mut self, gs: &mut GameState) -> Result<(), String> {
        // Create and configure TCP socket (matching server.cpp socket setup)
        let listener = TcpListener::bind("0.0.0.0:5555")
            .map_err(|e| format!("Failed to bind socket: {}", e))?;

        listener
            .set_nonblocking(true)
            .map_err(|e| format!("Failed to set non-blocking mode: {}", e))?;

        self.sock = Some(listener);
        log::info!("Socket bound to port 5555");

        // Load TLS configuration (mandatory).
        let tls_config =
            tls::load_tls_config().map_err(|e| format!("TLS initialization failed: {e}"))?;
        log::info!("TLS enabled — accepting encrypted connections on port 5555");
        self.tls_config = Some(tls_config);

        crate::network_manager::initialize_packet_stats()?;

        // Mark data as dirty so a crash before clean shutdown is detectable.
        gs.globals.set_dirty(true);

        // Log out all active characters (cleanup from previous run)
        for i in 0..core::constants::MAXCHARS {
            let should_logout = gs.characters[i].used == core::constants::USE_ACTIVE
                && gs.characters[i].flags & CharacterFlags::Player.bits() != 0;

            if !should_logout {
                continue;
            }

            log::info!(
                "Logging out character '{}' on server startup",
                gs.characters[i].get_name(),
            );

            player::connection::plr_logout(gs, i, 0, LogoutReason::Shutdown);
        }

        // Initialize subsystems
        crate::lab9::lab9_initialize(gs);
        populate::reset_changed_items(gs);

        log::info!("Checking for lab items on players...");
        for n in 1..core::constants::MAXITEM {
            if gs.items[n].used == core::constants::USE_EMPTY {
                continue;
            }
            if gs.items[n].has_laby_destroy() {
                Self::tmplabcheck(gs, n);
            }
            if gs.items[n].has_soulstone() {
                let max_damage = gs.items[n].max_damage;
                if max_damage == 0 {
                    gs.items[n].max_damage = 60000;
                    let name = gs.items[n].get_name();
                    log::info!("Set {} ({}) max_damage to 60000", name, n);
                }
            }
        }

        log::info!("Validating character template positions...");
        for n in 1..core::constants::MAXTCHARS {
            if gs.character_templates[n].used == core::constants::USE_EMPTY {
                continue;
            }

            let x = gs.character_templates[n].data[29] % core::constants::SERVER_MAPX;
            let y = gs.character_templates[n].data[29] / core::constants::SERVER_MAPX;

            if x == 0 && y == 0 {
                continue;
            }

            let ch_x = gs.character_templates[n].x as i32;
            let ch_y = gs.character_templates[n].y as i32;

            if (x - ch_x).abs() + (y - ch_y).abs() > 200 {
                log::error!(
                    "RESET {} ({}): {} {} -> {} {}",
                    n,
                    gs.character_templates[n].get_name(),
                    ch_x,
                    ch_y,
                    x,
                    y
                );
                return Result::Err("Character template has invalid resting position.".to_string());
            }
        }

        // Always spawn the background KeyDB saver.
        log::info!("Starting background KeyDB saver thread...");
        self.background_saver = Some(background_saver::spawn());

        // Spawn the admin template-reload watcher (no-op when disabled).
        self.template_reload_watcher =
            server::keydb::template_reload::TemplateReloadWatcher::spawn();

        // Spawn the admin text-reload watcher (no-op when disabled).
        self.text_reload_watcher = server::keydb::text_reload::TextReloadWatcher::spawn();

        // Spawn the admin map-patch watcher (no-op when disabled).
        self.map_patch_watcher = server::keydb::map_patch::MapPatchWatcher::spawn();

        // Spawn the admin item-patch watcher (no-op when disabled).
        self.item_patch_watcher = server::keydb::item_patch::ItemPatchWatcher::spawn();

        // Spawn the admin character-patch watcher (no-op when disabled).
        self.character_patch_watcher =
            server::keydb::character_patch::CharacterPatchWatcher::spawn();

        Ok(())
    }

    /// Advance the server by a single scheduling tick.
    ///
    /// When it's time, `tick` will call `game_tick()` to run world logic, then
    /// compress and send tick updates to players, perform slower network I/O
    /// periodically (every 8 ticks), and finally sleep to maintain the target
    /// tick rate.
    ///
    /// # Arguments
    ///
    /// * `gs` - Mutable reference to the unified game state.
    pub fn tick(&mut self, gs: &mut GameState) {
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
            self.game_tick(gs);

            // Compress and send tick data to clients
            self.compress_ticks(gs);

            let new_now = Instant::now();
            let new_last = self.last_tick_time.unwrap();

            // Check if server is running too slow (serious slowness detection)
            // In the original C++ this threshold was `TICK * TICKS * 10` (10 seconds).
            if new_now > new_last + Duration::from_secs(10) {
                log::warn!("Server too slow");
                self.last_tick_time = Some(new_now);
            }

            let post_tick_time = Instant::now();

            if gs
                .globals
                .ticker
                .unsigned_abs()
                .is_multiple_of(self.measurement_interval)
            {
                let tick_duration =
                    post_tick_time.duration_since(pre_tick_time).as_secs_f32() * 1000.0;
                self.tick_perf_stats.push(tick_duration);

                const DESIRED_TICK_TIME_MS: f32 = core::constants::TICK as f32 / 1000.0; // 1000 microseconds per millisecond

                gs.globals.load = ((tick_duration / DESIRED_TICK_TIME_MS) * 100.0) as i64;

                // TODO: Update this to be a proper moving average of the load
                // gs.globals.load_avg = self.tick_perf_stats.stats().mean as i32;

                log::debug!(
                    "Tick time: {:.2} ms (max: {:.2} ms), Load: {:.2}%",
                    tick_duration,
                    self.tick_perf_stats.stats().max,
                    gs.globals.load,
                );
            }
        }

        // Handle network I/O every scheduling tick.
        // Limiting this to every Nth game tick introduces noticeable input lag
        // and delayed map/tick packet delivery.
        let pre_io_time = Instant::now();
        self.handle_network_io(gs);

        if gs
            .globals
            .ticker
            .unsigned_abs()
            .is_multiple_of(self.measurement_interval)
        {
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
    ///
    /// # Arguments
    ///
    /// * `gs` - Mutable reference to the unified game state.
    fn game_tick(&mut self, gs: &mut GameState) {
        // Get current hour for statistics
        let hour = chrono::Local::now().hour() as usize;

        // Increment global tick counters
        gs.globals.ticker = gs.globals.ticker.wrapping_add(1);
        gs.globals.uptime = gs.globals.uptime.wrapping_add(1);
        gs.globals.uptime_per_hour[hour] = gs.globals.uptime_per_hour[hour].wrapping_add(1);

        let ticker = gs.globals.ticker;

        // Background save scheduling (KeyDB only)
        self.maybe_enqueue_background_save(gs);

        // Send tick to players and count online
        let mut online = 0;
        for n in 1..gs.players.len() {
            if gs.players[n].sock.is_none() {
                continue;
            }
            let state = gs.players[n].state;
            let is_normal_or_exit =
                state == core::constants::ST_NORMAL || state == core::constants::ST_EXIT;
            let is_normal = state == core::constants::ST_NORMAL;

            if !is_normal_or_exit {
                continue;
            }

            player::tick::plr_tick(gs, n);
            // Weather (especially area-driven effects) is temporarily disabled
            // while we tune things — re-enable once areas are configured.
            // crate::state::weather::weather_tick(gs, n);

            if is_normal {
                online += 1;
            }
        }

        // Update max online statistics
        if online > gs.globals.max_online {
            gs.globals.max_online = online;
        }
        if online > gs.globals.max_online_per_hour[hour] {
            gs.globals.max_online_per_hour[hour] = online;
        }

        // Check for player commands and translate to character commands
        for n in 1..gs.players.len() {
            if gs.players[n].sock.is_none() {
                continue;
            }

            // Process all pending commands (16 bytes each)
            loop {
                if gs.players[n].in_len < 16 {
                    break;
                }

                player::plr_cmd(gs, n);

                gs.players[n].in_len -= 16;
                gs.players[n].inbuf.copy_within(16..256, 0);
            }

            player::tick::plr_idle(gs, n);
        }

        // Do login stuff for players not in normal state
        for n in 1..gs.players.len() {
            if gs.players[n].sock.is_none() {
                continue;
            }
            if gs.players[n].state == core::constants::ST_NORMAL {
                continue;
            }

            player::tick::plr_state(gs, n);
        }

        // Send changes to players in normal state
        for n in 1..gs.players.len() {
            if gs.players[n].sock.is_none() {
                continue;
            }
            if gs.players[n].state != core::constants::ST_NORMAL {
                continue;
            }

            player::map::plr_getmap(gs, n);
            player::tick::plr_change(gs, n);
        }

        // Let characters act
        let mut cnt = 0;
        let mut awake = 0;
        let mut body = 0;
        let mut plon = 0;

        // Wakeup mechanism (every 64 ticks)
        if (ticker & 63) == 0 {
            self.wakeup_character(gs);
        }

        for n in 1..core::constants::MAXCHARS {
            let char_state = {
                if gs.characters[n].used == core::constants::USE_EMPTY {
                    CharacterTickState::Empty
                } else if gs.characters[n].flags & CharacterFlags::Update.bits() != 0 {
                    CharacterTickState::NeedsUpdate
                } else if gs.characters[n].used == core::constants::USE_NONACTIVE
                    && (n & 1023) == (ticker as usize & 1023)
                {
                    CharacterTickState::CheckExpire
                } else if gs.characters[n].flags & CharacterFlags::Body.bits() != 0 {
                    CharacterTickState::Body
                } else {
                    CharacterTickState::Active
                }
            };

            match char_state {
                CharacterTickState::Empty => continue,
                CharacterTickState::NeedsUpdate => {
                    cnt += 1;
                    gs.really_update_char(n);

                    gs.characters[n].flags &= !CharacterFlags::Update.bits();
                }
                CharacterTickState::CheckExpire => {
                    cnt += 1;
                    self.check_expire(gs, n);
                }
                CharacterTickState::Body => {
                    cnt += 1;
                    if gs.characters[n].flags & CharacterFlags::Player.bits() == 0 {
                        gs.characters[n].data[98] += 1;
                        if gs.characters[n].data[98] > (core::constants::TICKS * 60 * 30) {
                            log::info!("Removing lost body for character {}", n);
                            God::destroy_items(gs, n);
                            gs.characters[n].used = core::constants::USE_EMPTY;
                            continue;
                        }
                    }
                    body += 1;
                    continue;
                }
                CharacterTickState::Active => {
                    cnt += 1;
                }
            }

            // Reduce single awake timer
            if gs.characters[n].data[92] > 0 {
                gs.characters[n].data[92] -= 1;
            }

            // Check if character should be active
            if gs.characters[n].status < 8 && !self.group_active(gs, n) {
                continue;
            }

            awake += 1;

            if gs.characters[n].used == core::constants::USE_ACTIVE {
                // Periodic validation
                if (n & 1023) == (ticker as usize & 1023) && !self.check_valid(gs, n) {
                    continue;
                }

                gs.characters[n].current_online_time += 1;
                gs.characters[n].total_online_time += 1;

                let is_player_or_usurp = (gs.characters[n].flags & CharacterFlags::Player.bits()
                    != 0)
                    || (gs.characters[n].flags & CharacterFlags::Usurp.bits() != 0);
                let is_player = gs.characters[n].flags & CharacterFlags::Player.bits() != 0;
                let is_visible = gs.characters[n].flags & CharacterFlags::Invisible.bits() == 0;

                if is_player_or_usurp {
                    gs.globals.total_online_time += 1;
                    gs.globals.online_per_hour[hour] += 1;

                    if is_player {
                        if gs.characters[n].data[71] > 0 {
                            gs.characters[n].data[71] -= 1;
                        }
                        if gs.characters[n].data[72] > 0 {
                            gs.characters[n].data[72] -= 1;
                        }

                        if is_visible {
                            plon += 1;
                        }
                    }
                }

                player::tick::plr_act(gs, n)
            }

            gs.do_regenerate(n);
        }

        // Update global stats
        gs.globals.character_cnt = cnt;
        gs.globals.awake = awake;
        gs.globals.body = body;
        gs.globals.players_online = plon;

        // Run subsystem ticks
        populate::pop_tick(gs);
        EffectManager::effect_tick(gs);
        driver::item_tick(gs);

        self.global_tick(gs);
    }

    // Helper enum for character tick state
    /// Wake up one character in a round-robin fashion.
    ///
    /// This sets the single-character awake timer (`data[92]`) for one template
    /// index each call, cycling through `MAXCHARS` over time.
    ///
    /// # Arguments
    ///
    /// * `gs` - Mutable reference to the unified game state.
    fn wakeup_character(&mut self, gs: &mut GameState) {
        // Wakeup one character per 64 ticks
        static WAKEUP: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);

        let mut wakeup_idx = WAKEUP.load(std::sync::atomic::Ordering::Relaxed);
        if wakeup_idx >= core::constants::MAXCHARS {
            wakeup_idx = 1;
        }

        gs.characters[wakeup_idx].data[92] = core::constants::TICKS * 60;

        WAKEUP.store(wakeup_idx + 1, std::sync::atomic::Ordering::Relaxed);
    }

    /// Return true if the character `cn` should be considered active.
    ///
    /// Characters are active if they are players/usurpers, flagged with
    /// `NoSleep`, currently `USE_ACTIVE`, or have a non-zero single-awake
    /// timer (`data[92]`).
    ///
    /// # Arguments
    ///
    /// * `gs` - Reference to the unified game state.
    /// * `cn` - Character index to check.
    ///
    /// # Returns
    ///
    /// * `true` if the character should be considered active.
    fn group_active(&self, gs: &GameState, cn: usize) -> bool {
        if ((gs.characters[cn].flags & CharacterFlags::Player.bits() != 0)
            || (gs.characters[cn].flags & CharacterFlags::Usurp.bits() != 0)
            || (gs.characters[cn].flags & CharacterFlags::NoSleep.bits() != 0))
            && gs.characters[cn].used == core::constants::USE_ACTIVE
        {
            return true;
        }
        if gs.characters[cn].data[92] > 0 {
            return true;
        }
        false
    }

    /// Check whether a non-active character `cn` should be expired/erased.
    ///
    /// Uses a tiered expiration policy based on total points and last login
    /// date (e.g., zero-point characters are removed after 3 days; higher
    /// ranks get longer grace periods). When expiration triggers, the
    /// character is marked `USE_EMPTY` and logged.
    ///
    /// # Arguments
    ///
    /// * `gs` - Mutable reference to the unified game state.
    /// * `cn` - Character index to check.
    fn check_expire(&self, gs: &mut GameState, cn: usize) {
        // Check character expiration similar to the original C++ logic.

        let week: i64 = 60 * 60 * 24 * 7;
        let day: i64 = 60 * 60 * 24;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let points_tot = gs.characters[cn].points_tot;
        let login_date = gs.characters[cn].login_date;

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
            let total_exp = gs.characters[cn].points_tot;
            log::info!(
                "erased player {}, {} exp",
                gs.characters[cn].get_name(),
                total_exp,
            );
            gs.characters[cn].used = core::constants::USE_EMPTY;
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
    ///
    /// # Arguments
    ///
    /// * `gs` - Mutable reference to the unified game state.
    /// * `cn` - Character index to validate.
    ///
    /// # Returns
    ///
    /// * `true` if the character passes validation.
    fn check_valid(&self, gs: &mut GameState, cn: usize) -> bool {
        // Full validation ported from the original C++ check_valid

        // Bounds check
        let (x, y) = (gs.characters[cn].x, gs.characters[cn].y);
        if x < 1
            || y < 1
            || x > (core::constants::SERVER_MAPX as i16 - 2)
            || y > (core::constants::SERVER_MAPY as i16 - 2)
        {
            log::warn!(
                "Killed character {} ({}) for invalid data",
                gs.characters[cn].get_name(),
                cn
            );
            // Best-effort: destroy carried items and mark as unused
            God::destroy_items(gs, cn);
            gs.characters[cn].used = core::constants::USE_EMPTY;
            return false;
        }

        // Map consistency check: map[n].ch should point to this character
        let map_idx = (x as usize) + (y as usize) * core::constants::SERVER_MAPX as usize;
        let map_ch = gs.map[map_idx].ch as usize;
        if map_ch != cn {
            log::warn!(
                "Not on map (map has {}), fixing char {} at {}",
                map_ch,
                cn,
                gs.characters[cn].get_name()
            );

            if map_ch != 0 {
                // Try to drop character items near their position as in original
                let (cx, cy) = (gs.characters[cn].x as usize, gs.characters[cn].y as usize);
                if !God::drop_char_fuzzy_large(gs, cn, cx, cy, cx, cy) {
                    // couldn't drop items; leave as-is (original tried a few options)
                }
            } else {
                // claim the map tile for this character
                gs.map[map_idx].ch = cn as u32;
            }
        }

        // Validate carried items (inventory)
        for slot in 0..40 {
            let in_id = gs.characters[cn].item[slot] as usize;
            if in_id != 0 {
                let bad = gs.items[in_id].carried as usize != cn
                    || gs.items[in_id].used != core::constants::USE_ACTIVE;
                if bad {
                    log::warn!(
                        "Reset item {} ({},{}) from char {} ({})",
                        in_id,
                        gs.items[in_id].get_name(),
                        gs.items[in_id].used,
                        cn,
                        gs.characters[cn].get_name(),
                    );
                    gs.characters[cn].item[slot] = 0;
                }
            }
        }

        // Validate depot items
        for slot in 0..62 {
            let in_id = gs.characters[cn].depot[slot] as usize;
            if in_id != 0 {
                let bad = gs.items[in_id].carried as usize != cn
                    || gs.items[in_id].used != core::constants::USE_ACTIVE;
                if bad {
                    log::warn!(
                        "Reset depot item {} ({},{}) from char {} ({})",
                        in_id,
                        gs.items[in_id].get_name(),
                        gs.items[in_id].used,
                        cn,
                        gs.characters[cn].get_name()
                    );
                    gs.characters[cn].depot[slot] = 0;
                }
            }
        }

        // Validate worn and spell items
        for slot in 0..20 {
            let worn_id = gs.characters[cn].worn[slot] as usize;
            if worn_id != 0 {
                let bad = gs.items[worn_id].carried as usize != cn
                    || gs.items[worn_id].used != core::constants::USE_ACTIVE;
                if bad {
                    log::warn!(
                        "Reset worn item {} ({},{}) from char {} ({})",
                        worn_id,
                        gs.items[worn_id].get_name(),
                        gs.items[worn_id].used,
                        cn,
                        gs.characters[cn].get_name()
                    );
                    gs.characters[cn].worn[slot] = 0;
                }
            }

            let spell_id = gs.characters[cn].spell[slot] as usize;
            if spell_id != 0 {
                let bad = gs.items[spell_id].carried as usize != cn
                    || gs.items[spell_id].used != core::constants::USE_ACTIVE;
                if bad {
                    log::debug!(
                        "Reset spell item {} from char {}.",
                        gs.items[spell_id].get_name(),
                        gs.characters[cn].get_name()
                    );
                    gs.characters[cn].spell[slot] = 0;
                }
            }
        }

        // If stoned and not a player, verify the stoned target is valid
        let is_stoned_nonplayer = (gs.characters[cn].flags & CharacterFlags::Stoned.bits()) != 0
            && (gs.characters[cn].flags & CharacterFlags::Player.bits()) == 0;
        if is_stoned_nonplayer {
            let co = gs.characters[cn].data[63] as usize;
            let ok = co != 0 && gs.characters[co].used == core::constants::USE_ACTIVE;
            if !ok {
                gs.characters[cn].flags &= !CharacterFlags::Stoned.bits();
                log::info!("oops, stoned removed");
            }
        }

        true
    }

    /// Handle global (world) time progression and daily events.
    ///
    /// Advances `mdtime`, rolls day/year counters, updates daylight/moon phase
    /// and, when a new day begins, performs daily maintenance such as depot
    /// payments and miscellaneous per-player adjustments.
    ///
    /// # Arguments
    ///
    /// * `gs` - Mutable reference to the unified game state.
    fn global_tick(&self, gs: &mut GameState) {
        // Port of svr_glob.cpp::global_tick
        const MD_HOUR: i32 = 3600;
        const MD_DAY: i32 = MD_HOUR * 24;
        const MD_YEAR: i32 = 300;

        // Increment mdtime and compute day rollover + daylight/moon state
        gs.globals.mdtime += 1;

        let mut day_rolled = false;
        if gs.globals.mdtime >= MD_DAY {
            gs.globals.mdday += 1;
            gs.globals.mdtime = 0;
            day_rolled = true;
            log::info!(
                "day {} of the year {} begins",
                gs.globals.mdday,
                gs.globals.mdyear
            );
        }

        if gs.globals.mdday >= MD_YEAR {
            gs.globals.mdyear += 1;
            gs.globals.mdday = 1;
        }

        if gs.globals.mdtime < MD_HOUR * 6 {
            gs.globals.dlight = 0;
        } else if gs.globals.mdtime < MD_HOUR * 7 {
            gs.globals.dlight = (gs.globals.mdtime - MD_HOUR * 6) * 255 / MD_HOUR;
        } else if gs.globals.mdtime < MD_HOUR * 22 {
            gs.globals.dlight = 255;
        } else if gs.globals.mdtime < MD_HOUR * 23 {
            gs.globals.dlight = (MD_HOUR * 23 - gs.globals.mdtime) * 255 / MD_HOUR;
        } else {
            gs.globals.dlight = 0;
        }

        let mut tmp = gs.globals.mdday % 28 + 1;

        gs.globals.newmoon = 0;
        gs.globals.fullmoon = 0;

        if tmp == 1 {
            gs.globals.newmoon = 1;
            return;
        }
        if tmp == 15 {
            gs.globals.fullmoon = 1;
        }

        if tmp > 14 {
            tmp = 28 - tmp;
        }
        if tmp > gs.globals.dlight {
            gs.globals.dlight = tmp;
        }

        // If a new day began, run pay_rent() and do_misc()
        if day_rolled {
            // pay_rent: call depot payment routine for each player
            for cn in 1..core::constants::MAXCHARS {
                let is_player = gs.characters[cn].used != core::constants::USE_EMPTY
                    && (gs.characters[cn].flags & CharacterFlags::Player.bits()) != 0;
                if !is_player {
                    continue;
                }
                gs.do_pay_depot(cn);
            }

            // do_misc: adjust luck and clear temporary flags for players
            for cn in 1..core::constants::MAXCHARS {
                let is_player = gs.characters[cn].used != core::constants::USE_EMPTY
                    && (gs.characters[cn].flags & CharacterFlags::Player.bits()) != 0;
                if !is_player {
                    continue;
                }

                let uniques = crate::driver::count_uniques(&gs.characters[cn], &gs.items);

                if uniques > 1 {
                    // reduce luck for multi-unique holders if active
                    if gs.characters[cn].used == core::constants::USE_ACTIVE {
                        gs.characters[cn].luck -= 5;
                        let luck_to_log = gs.characters[cn].luck;
                        log::info!(
                            "reduced luck by 5 to {} for having more than one unique",
                            luck_to_log,
                        );
                    }
                } else {
                    // slowly recover luck towards 0
                    if gs.characters[cn].luck < 0 {
                        gs.characters[cn].luck += 1;
                    }
                    if gs.characters[cn].luck < 0 {
                        gs.characters[cn].luck += 1;
                    }
                    // clear temporary punishment flags
                    let mask = CharacterFlags::ShutUp.bits()
                        | CharacterFlags::NoDesc.bits()
                        | CharacterFlags::Kicked.bits();
                    gs.characters[cn].flags &= !mask;
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    //  Background saver scheduling (KeyDB backend only)
    // -----------------------------------------------------------------------

    /// Check whether it is time to enqueue a background save job, and if so,
    /// clone the next slice of data and send it to the background saver thread.
    ///
    /// # Arguments
    ///
    /// * `gs` - Reference to the unified game state (read-only cloning).
    fn maybe_enqueue_background_save(&mut self, gs: &GameState) {
        let saver = match &self.background_saver {
            Some(s) => s,
            None => return,
        };

        self.save_tick_counter += 1;
        if self.save_tick_counter < background_saver::SAVE_INTERVAL_TICKS {
            return;
        }
        self.save_tick_counter = 0;

        // Determine which cycle we're on (wraps around)
        let cycle = (gs.globals.ticker.unsigned_abs() / background_saver::SAVE_INTERVAL_TICKS)
            % background_saver::SAVE_CYCLE_COUNT;

        match cycle {
            0 => {
                // Characters
                let data = gs.characters.clone();
                saver.send(SaveJob::Characters(data));
            }
            1 => {
                // Items first half
                let half = core::constants::MAXITEM / 2;
                let data = gs.items[..half].to_vec();
                saver.send(SaveJob::Items(data, 0));
            }
            2 => {
                // Items second half
                let half = core::constants::MAXITEM / 2;
                let data = gs.items[half..].to_vec();
                saver.send(SaveJob::Items(data, half));
            }
            3 => {
                // Small data: effects + globals
                let effects = gs.effects.clone();
                let globals = gs.globals.clone();
                saver.send(SaveJob::SmallData { effects, globals });
            }
            4 => {
                // Map first half
                let total = (core::constants::SERVER_MAPX as usize)
                    * (core::constants::SERVER_MAPY as usize);
                let half = total / 2;
                let data = gs.map[..half].to_vec();
                saver.send(SaveJob::MapTiles(data, 0));
            }
            5 => {
                // Map second half
                let total = (core::constants::SERVER_MAPX as usize)
                    * (core::constants::SERVER_MAPY as usize);
                let half = total / 2;
                let data = gs.map[half..].to_vec();
                saver.send(SaveJob::MapTiles(data, half));
            }
            _ => {}
        }
    }

    /// Drain pending admin reload requests and apply them to `gs`.
    ///
    /// Each drained request causes the watcher's KeyDB connection to be
    /// reopened here on the tick thread (the watcher only signals; the swap
    /// runs synchronously on the tick thread to keep `GameState` access
    /// single-threaded). After applying, an `applied:{ts}` status entry is
    /// written so the API can confirm completion.
    ///
    /// # Arguments
    ///
    /// * `gs` - Mutable game state whose template slices will be replaced.
    pub fn drain_template_reloads(&mut self, gs: &mut GameState) {
        let Some(watcher) = self.template_reload_watcher.as_ref() else {
            return;
        };
        while let Some(req) = watcher.try_recv() {
            self.apply_template_reload(gs, req);
        }
    }

    fn apply_template_reload(
        &self,
        gs: &mut GameState,
        req: server::keydb::template_reload::ReloadRequest,
    ) {
        let mut con = match server::keydb::connection::connect() {
            Ok(c) => c,
            Err(e) => {
                log::warn!(
                    "template reload {}: keydb connect failed: {}",
                    req.request_id,
                    e
                );
                return;
            }
        };

        if req.reload_items {
            match server::keydb::store::load_item_templates(&mut con) {
                Ok(items) => {
                    log::info!(
                        "template reload {}: swapped {} item templates",
                        req.request_id,
                        items.len()
                    );
                    gs.item_templates = items;
                }
                Err(e) => {
                    log::warn!(
                        "template reload {}: load item templates failed: {}",
                        req.request_id,
                        e
                    );
                    return;
                }
            }
        }

        if req.reload_characters {
            match server::keydb::store::load_character_templates(&mut con) {
                Ok(chars) => {
                    log::info!(
                        "template reload {}: swapped {} character templates",
                        req.request_id,
                        chars.len()
                    );
                    gs.character_templates = chars;
                }
                Err(e) => {
                    log::warn!(
                        "template reload {}: load character templates failed: {}",
                        req.request_id,
                        e
                    );
                    return;
                }
            }
        }

        if let Err(e) =
            server::keydb::template_reload::write_applied_status(&mut con, &req.request_id)
        {
            log::warn!(
                "template reload {}: status write failed: {}",
                req.request_id,
                e
            );
        }
    }

    /// Drain pending admin text reload requests and apply them to `gs`.
    ///
    /// Each drained request reloads externally managed text data from KeyDB on
    /// the tick thread, preserving single-threaded access to `GameState`.
    ///
    /// # Arguments
    ///
    /// * `gs` - Mutable game state whose text-data fields will be replaced.
    pub fn drain_text_reloads(&mut self, gs: &mut GameState) {
        let Some(watcher) = self.text_reload_watcher.as_ref() else {
            return;
        };
        while let Some(req) = watcher.try_recv() {
            self.apply_text_reload(gs, req);
        }
    }

    fn apply_text_reload(
        &self,
        gs: &mut GameState,
        req: server::keydb::text_reload::TextReloadRequest,
    ) {
        let mut con = match server::keydb::connection::connect() {
            Ok(connection) => connection,
            Err(error) => {
                log::warn!(
                    "text reload {}: keydb connect failed: {}",
                    req.request_id,
                    error
                );
                return;
            }
        };

        if req.reload_badwords {
            match server::keydb::store::load_bad_words(&mut con) {
                Ok(bad_words) => {
                    log::info!(
                        "text reload {}: swapped {} badwords",
                        req.request_id,
                        bad_words.len()
                    );
                    gs.bad_words = bad_words;
                }
                Err(error) => {
                    log::warn!(
                        "text reload {}: load badwords failed: {}",
                        req.request_id,
                        error
                    );
                    return;
                }
            }
        }

        if let Err(error) =
            server::keydb::text_reload::write_applied_status(&mut con, &req.request_id)
        {
            log::warn!(
                "text reload {}: status write failed: {}",
                req.request_id,
                error
            );
        }
    }

    /// Drain any pending admin map-tile patches and apply them to `gs.map`.
    ///
    /// Called once per tick from the main loop (outside `tick`) so the swap
    /// runs on the tick thread, keeping `GameState` single-threaded. Each
    /// [`server::keydb::map_patch::MapPatchEvent::Apply`] overwrites only the static
    /// fields of the target tile, preserving the dynamic fields (`ch`,
    /// `to_ch`, `it`, `light`, `dlight`). On
    /// [`server::keydb::map_patch::MapPatchEvent::ReloadCompleted`] we write the
    /// `applied` status entry so the API can confirm completion.
    ///
    /// # Arguments
    ///
    /// * `gs` - Mutable game state whose `map` slice will be patched.
    pub fn drain_map_patches(&mut self, gs: &mut GameState) {
        let Some(watcher) = self.map_patch_watcher.as_ref() else {
            return;
        };

        let mut any_applied = false;
        let mut completed_requests: Vec<String> = Vec::new();

        while let Some(event) = watcher.try_recv() {
            match event {
                server::keydb::map_patch::MapPatchEvent::Apply(patch) => {
                    if Self::apply_map_patch(gs, &patch) {
                        any_applied = true;
                    }
                }
                server::keydb::map_patch::MapPatchEvent::ReloadCompleted { request_id } => {
                    completed_requests.push(request_id);
                }
            }
        }

        if any_applied {
            gs.globals.set_dirty(true);
        }

        if completed_requests.is_empty() {
            return;
        }

        let mut con = match server::keydb::connection::connect() {
            Ok(c) => c,
            Err(e) => {
                log::warn!("map patch reload: keydb connect failed: {}", e);
                return;
            }
        };
        for request_id in completed_requests {
            if let Err(e) = server::keydb::map_patch::write_applied_status(&mut con, &request_id) {
                log::warn!(
                    "map patch reload {}: status write failed: {}",
                    request_id,
                    e
                );
            } else {
                log::info!("map patch reload {}: applied", request_id);
            }
        }
    }

    /// Merge a single patch into `gs.map`, preserving dynamic fields.
    ///
    /// # Arguments
    ///
    /// * `gs`    - Mutable game state.
    /// * `patch` - Static-field overrides from the admin API.
    ///
    /// # Returns
    ///
    /// * `true` when the tile was updated.
    /// * `false` when the patch targets out-of-range coordinates.
    fn apply_map_patch(gs: &mut GameState, patch: &core::map_store::MapPatch) -> bool {
        let x = patch.x as usize;
        let y = patch.y as usize;
        let map_x = core::constants::SERVER_MAPX as usize;
        let map_y = core::constants::SERVER_MAPY as usize;
        if x >= map_x || y >= map_y {
            log::warn!(
                "map patch: dropping out-of-range coords ({}, {})",
                patch.x,
                patch.y
            );
            return false;
        }
        let idx = y * map_x + x;
        let Some(tile) = gs.map.get_mut(idx) else {
            return false;
        };
        tile.sprite = patch.sprite;
        tile.fsprite = patch.fsprite;
        tile.flags = patch.flags;
        true
    }

    /// Drain any pending admin item patches and apply them to `gs.items`.
    ///
    /// Called once per tick from the main loop. Each
    /// [`server::keydb::item_patch::ItemPatchEvent::Apply`] overwrites only the
    /// static authoring fields of the target item, preserving dynamic
    /// runtime fields (position, damage state, current age/damage,
    /// runtime sprite override). On
    /// [`server::keydb::item_patch::ItemPatchEvent::ReloadCompleted`] we write
    /// the `applied` status entry so the API can confirm completion.
    ///
    /// # Arguments
    ///
    /// * `gs` - Mutable game state whose `items` slice will be patched.
    pub fn drain_item_patches(&mut self, gs: &mut GameState) {
        let Some(watcher) = self.item_patch_watcher.as_ref() else {
            return;
        };

        let mut any_applied = false;
        let mut completed_requests: Vec<String> = Vec::new();

        while let Some(event) = watcher.try_recv() {
            match event {
                server::keydb::item_patch::ItemPatchEvent::Apply(patch) => {
                    if Self::apply_item_patch(gs, &patch) {
                        any_applied = true;
                    }
                }
                server::keydb::item_patch::ItemPatchEvent::ReloadCompleted { request_id } => {
                    completed_requests.push(request_id);
                }
            }
        }

        if any_applied {
            gs.globals.set_dirty(true);
        }

        if completed_requests.is_empty() {
            return;
        }

        let mut con = match server::keydb::connection::connect() {
            Ok(c) => c,
            Err(e) => {
                log::warn!("item patch reload: keydb connect failed: {}", e);
                return;
            }
        };
        for request_id in completed_requests {
            if let Err(e) = server::keydb::item_patch::write_applied_status(&mut con, &request_id) {
                log::warn!(
                    "item patch reload {}: status write failed: {}",
                    request_id,
                    e
                );
            } else {
                log::info!("item patch reload {}: applied", request_id);
            }
        }
    }

    /// Merge a single patch into `gs.items`, preserving dynamic fields.
    ///
    /// # Arguments
    ///
    /// * `gs`    - Mutable game state.
    /// * `patch` - Static-field overrides from the admin API.
    ///
    /// # Returns
    ///
    /// * `true` when the slot was updated.
    /// * `false` when the patch targets an out-of-range slot.
    fn apply_item_patch(gs: &mut GameState, patch: &core::item_store::ItemPatch) -> bool {
        let idx = patch.id as usize;
        let Some(slot) = gs.items.get_mut(idx) else {
            log::warn!("item patch: dropping out-of-range slot {}", patch.id);
            return false;
        };
        patch.apply_to(slot);
        true
    }

    /// Drain any pending admin character patches and apply them to
    /// `gs.characters`.
    ///
    /// Called once per tick from the main loop. Each
    /// [`server::keydb::character_patch::CharacterPatchEvent::Apply`] overwrites
    /// only the static authoring fields of the target character,
    /// preserving dynamic runtime fields (position, combat AI, current
    /// resources, inventory, networking).
    ///
    /// # Arguments
    ///
    /// * `gs` - Mutable game state whose `characters` slice will be patched.
    pub fn drain_character_patches(&mut self, gs: &mut GameState) {
        let Some(watcher) = self.character_patch_watcher.as_ref() else {
            return;
        };

        let mut any_applied = false;
        let mut completed_requests: Vec<String> = Vec::new();

        while let Some(event) = watcher.try_recv() {
            match event {
                server::keydb::character_patch::CharacterPatchEvent::Apply(patch) => {
                    if Self::apply_character_patch(gs, &patch) {
                        any_applied = true;
                    }
                }
                server::keydb::character_patch::CharacterPatchEvent::ReloadCompleted {
                    request_id,
                } => {
                    completed_requests.push(request_id);
                }
            }
        }

        if any_applied {
            gs.globals.set_dirty(true);
        }

        if completed_requests.is_empty() {
            return;
        }

        let mut con = match server::keydb::connection::connect() {
            Ok(c) => c,
            Err(e) => {
                log::warn!("character patch reload: keydb connect failed: {}", e);
                return;
            }
        };
        for request_id in completed_requests {
            if let Err(e) =
                server::keydb::character_patch::write_applied_status(&mut con, &request_id)
            {
                log::warn!(
                    "character patch reload {}: status write failed: {}",
                    request_id,
                    e
                );
            } else {
                log::info!("character patch reload {}: applied", request_id);
            }
        }
    }

    /// Merge a single patch into `gs.characters`, preserving dynamic fields.
    ///
    /// # Arguments
    ///
    /// * `gs`    - Mutable game state.
    /// * `patch` - Static-field overrides from the admin API.
    ///
    /// # Returns
    ///
    /// * `true` when the slot was updated.
    /// * `false` when the patch targets an out-of-range slot.
    fn apply_character_patch(
        gs: &mut GameState,
        patch: &core::character_store::CharacterPatch,
    ) -> bool {
        let idx = patch.id as usize;
        let Some(slot) = gs.characters.get_mut(idx) else {
            log::warn!("character patch: dropping out-of-range slot {}", patch.id);
            return false;
        };
        patch.apply_to(slot);
        true
    }

    /// Flush all pending background save jobs and then shut down the saver thread.
    ///
    /// `flush()` provides an explicit, observable synchronization point: it
    /// sends a `Flush` sentinel through the FIFO channel and blocks until the
    /// background thread acknowledges it, guaranteeing every queued write that
    /// was enqueued before this call has completed.  Only then is `Shutdown`
    /// sent and the thread joined.
    ///
    /// Safe to call multiple times — if the saver has already been taken
    /// (e.g. called explicitly before `Drop`), subsequent calls are no-ops.
    ///
    /// Call this during server shutdown, after the game loop has exited.
    pub fn shutdown_background_saver(&mut self) {
        if let Some(mut watcher) = self.template_reload_watcher.take() {
            log::info!("Stopping template reload watcher...");
            watcher.shutdown();
        }
        if let Some(mut watcher) = self.text_reload_watcher.take() {
            log::info!("Stopping text reload watcher...");
            watcher.shutdown();
        }
        if let Some(mut watcher) = self.map_patch_watcher.take() {
            log::info!("Stopping map patch watcher...");
            watcher.shutdown();
        }
        if let Some(mut watcher) = self.item_patch_watcher.take() {
            log::info!("Stopping item patch watcher...");
            watcher.shutdown();
        }
        if let Some(mut watcher) = self.character_patch_watcher.take() {
            log::info!("Stopping character patch watcher...");
            watcher.shutdown();
        }
        if let Some(mut saver) = self.background_saver.take() {
            log::info!("Flushing pending background save jobs...");
            if let Err(e) = saver.flush() {
                log::warn!("Background saver flush failed during shutdown: {e}");
            }
            log::info!("Shutting down background saver thread...");
            saver.shutdown();
            log::info!("Background saver thread stopped.");
        }
    }

    /// Compress outgoing per-player tick buffers using zlib when beneficial.
    ///
    /// Iterates connected players and attempts to compress their `tbuf` data
    /// into each player's `zs` encoder. Updates buffer pointers and resets
    /// `tptr` after compressing.
    ///
    /// # Arguments
    ///
    /// * `gs` - Mutable reference to the unified game state.
    fn compress_ticks(&mut self, gs: &mut GameState) {
        let header_from_int = |v: i32| {
            let b = v.to_ne_bytes();
            [b[0], b[1]]
        };

        let ring_free_space = |iptr: usize, optr: usize, cap: usize| -> usize {
            let used = if iptr >= optr {
                iptr - optr
            } else {
                cap - optr + iptr
            };
            cap.saturating_sub(used + 1)
        };

        for n in 1..gs.players.len() {
            if gs.players[n].sock.is_none() {
                continue;
            }
            if gs.players[n].ticker_started == 0 {
                continue;
            }

            let p = &mut gs.players[n];

            if p.usnr >= core::constants::MAXCHARS {
                p.usnr = 0;
            }

            let ilen = p.tptr;
            let olen_uncompressed_i32: i32 = (ilen + 2) as i32;

            let tbuf_data: Vec<u8> = if ilen > 0 {
                p.tbuf[..ilen].to_vec()
            } else {
                Vec::new()
            };

            let (olen_i32, header, payload): (i32, [u8; 2], Vec<u8>) = if olen_uncompressed_i32 > 16
            {
                if let Some(zs) = p.zs.as_mut() {
                    let before = zs.get_ref().len();
                    let _ = zs.write_all(&tbuf_data);
                    let _ = zs.flush();

                    let after = zs.get_ref().len();
                    let produced = after.saturating_sub(before);
                    let csize = produced.min(core::constants::OBUFSIZE);

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
                    let header = header_from_int(olen_uncompressed_i32);
                    (olen_uncompressed_i32, header, tbuf_data)
                }
            } else {
                let header = header_from_int(olen_uncompressed_i32);
                (olen_uncompressed_i32, header, tbuf_data)
            };

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
                p.tptr = 0;
                continue;
            }

            let mut iptr = p.iptr;
            let obuf_len = p.obuf.len();
            for &b in header.iter().chain(payload.iter()) {
                p.obuf[iptr] = b;
                iptr += 1;
                if iptr >= obuf_len {
                    iptr = 0;
                }
            }

            p.iptr = iptr;

            let usnr = p.usnr;
            if usnr < core::constants::MAXCHARS {
                gs.characters[usnr].comp_volume = gs.characters[usnr]
                    .comp_volume
                    .wrapping_add(olen_i32 as u32);
                gs.characters[usnr].raw_volume =
                    gs.characters[usnr].raw_volume.wrapping_add(ilen as u32);
            }

            p.tptr = 0;
        }
    }

    /// Accept new connections and perform per-player network IO.
    ///
    /// Accepts new TCP connections on the listener, assigning them a free
    /// /// player slot via `new_player`. For existing connections, it calls
    /// `rec_player` and `send_player` as necessary to handle receive and send
    /// activity.
    ///
    /// # Arguments
    ///
    /// * `gs` - Mutable reference to the unified game state.
    fn handle_network_io(&mut self, gs: &mut GameState) {
        // Handle new connections
        if let Some(ref listener) = self.sock {
            match listener.accept() {
                Ok((stream, addr)) => {
                    log::info!("New connection from {}", addr);
                    let config = self
                        .tls_config
                        .as_ref()
                        .expect("TLS config must be initialized before handle_network_io");
                    match tls::accept_tls(stream, config.clone()) {
                        Ok(tls_stream) => {
                            log::info!("TLS handshake completed for {}", addr);
                            self.new_player(gs, tls_stream, addr.ip());
                        }
                        Err(e) => {
                            log::warn!("TLS handshake failed for {}: {}", addr, e);
                        }
                    }
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
        for player_idx in 1..gs.players.len() {
            if gs.players[player_idx].sock.is_none() {
                continue;
            }

            self.rec_player(gs, player_idx);

            self.send_player(gs, player_idx);
        }
    }

    /// Accept a new incoming connection and assign it a player slot.
    ///
    /// Converts the peer address into a u32 (IPv4) and initializes a fresh
    /// `ServerPlayer` including zlib compression state. If no free slot is
    /// available, the connection is closed.
    ///
    /// # Arguments
    ///
    /// * `gs` - Reference to the unified game state (for reading ticker).
    /// * `stream` - The accepted game stream (plain or TLS).
    /// * `addr` - The peer IP address.
    fn new_player(&mut self, gs: &mut GameState, stream: GameStream, addr: std::net::IpAddr) {
        let _ = stream.set_nonblocking(true);

        let addr_u32: u32 = match addr {
            std::net::IpAddr::V4(a) => u32::from_be_bytes(a.octets()),
            _ => 0,
        };

        let ticker = gs.globals.ticker as u32;

        let mut slot: Option<usize> = None;
        for n in 1..gs.players.len() {
            if gs.players[n].sock.is_none() {
                slot = Some(n);
                break;
            }
        }

        let Some(n) = slot else {
            log::warn!("new_player: MAXPLAYER reached");
            return;
        };

        gs.players[n] = ServerPlayer::new();
        gs.players[n].sock = Some(stream);
        gs.players[n].addr = addr_u32;
        gs.players[n].zs = Some(ZlibEncoder::new(Vec::new(), Compression::best()));
        gs.players[n].state = core::constants::ST_CONNECT;
        gs.players[n].lasttick = ticker;
        gs.players[n].lasttick2 = ticker;
        gs.players[n].prio = 0;
        gs.players[n].ticker_started = 0;
        gs.players[n].inbuf[0] = 0;
        gs.players[n].in_len = 0;
        gs.players[n].iptr = 0;
        gs.players[n].optr = 0;
        gs.players[n].tptr = 0;
        gs.players[n].challenge = 0;
        gs.players[n].usnr = 0;

        gs.players[n].cmap.fill(CMap::default());
        gs.players[n].smap.fill(CMap::default());
        gs.players[n].xmap.fill(Map::default());

        for m in 0..(TILEX * TILEY) {
            gs.players[n].cmap[m].ba_sprite = core::constants::SPR_EMPTY as i16;
            gs.players[n].smap[m].ba_sprite = core::constants::SPR_EMPTY as i16;
        }

        log::info!("New connection assigned to slot {}", n);
    }

    /// Read available bytes from a player's socket into their input buffer.
    ///
    /// This method attempts a non-blocking read into `inbuf` and updates
    /// `in_len` accordingly. IO errors and disconnects are handled similarly
    /// to the original server behavior.
    ///
    /// # Arguments
    ///
    /// * `gs` - Mutable reference to the unified game state.
    /// * `_player_idx` - The player slot index.
    fn rec_player(&self, gs: &mut GameState, player_idx: usize) {
        if player_idx >= gs.players.len() {
            log::error!("rec_player: invalid player index {}", player_idx);
            return;
        }

        if gs.players[player_idx].sock.is_none() {
            log::error!("rec_player: no socket for player index {}", player_idx);
            return;
        }

        let in_len = gs.players[player_idx].in_len;
        if in_len >= gs.players[player_idx].inbuf.len() {
            return;
        }

        if let Some(mut sock) = gs.players[player_idx].sock.take() {
            match sock.read(&mut gs.players[player_idx].inbuf[in_len..]) {
                Ok(0) => {
                    log::info!("Connection closed (recv)");
                    let cn = gs.players[player_idx].usnr;
                    gs.players[player_idx].ltick = 0;
                    gs.players[player_idx].rtick = 0;
                    gs.players[player_idx].zs = None;
                    player::connection::plr_logout(gs, cn, player_idx, LogoutReason::Unknown);
                }
                Ok(len) => {
                    gs.players[player_idx].in_len += len;
                    gs.globals.recv += len as i64;
                    gs.players[player_idx].sock = Some(sock);
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                    gs.players[player_idx].sock = Some(sock);
                }
                Err(e) => {
                    log::error!("Connection closed (recv error): {}", e);
                    let cn = gs.players[player_idx].usnr;
                    gs.players[player_idx].ltick = 0;
                    gs.players[player_idx].rtick = 0;
                    gs.players[player_idx].zs = None;
                    player::connection::plr_logout(gs, cn, player_idx, LogoutReason::Unknown);
                }
            }
        }
    }

    /// Flush pending output bytes from `obuf` to the player's TCP socket.
    ///
    /// Handles partial writes and advances the circular buffer pointers. On
    /// fatal socket errors the player slot may be disconnected.
    ///
    /// # Arguments
    ///
    /// * `gs` - Mutable reference to the unified game state.
    /// * `player_idx` - The player slot index.
    fn send_player(&self, gs: &mut GameState, player_idx: usize) {
        if player_idx >= gs.players.len() {
            log::error!("send_player: invalid player index {}", player_idx);
            return;
        }
        if gs.players[player_idx].sock.is_none() {
            log::error!("send_player: no socket for player index {}", player_idx);
            return;
        }

        let iptr = gs.players[player_idx].iptr;
        let optr = gs.players[player_idx].optr;
        let obuf_len = gs.players[player_idx].obuf.len();

        let (len, slice_start) = if iptr < optr {
            (obuf_len - optr, optr)
        } else {
            (iptr - optr, optr)
        };

        if len == 0 {
            return;
        }

        if let Some(mut sock) = gs.players[player_idx].sock.take() {
            let end = slice_start + len;
            let to_send = &gs.players[player_idx].obuf
                [slice_start..end.min(gs.players[player_idx].obuf.len())];

            match sock.write(to_send) {
                Ok(0) => {
                    log::error!("Connection closed (send, wrote 0)");
                    let cn = gs.players[player_idx].usnr;
                    gs.players[player_idx].ltick = 0;
                    gs.players[player_idx].rtick = 0;
                    gs.players[player_idx].zs = None;
                    player::connection::plr_logout(gs, cn, player_idx, LogoutReason::Unknown);
                }
                Ok(ret) => {
                    gs.globals.send += ret as i64;
                    gs.players[player_idx].optr += ret;
                    if gs.players[player_idx].optr >= gs.players[player_idx].obuf.len() {
                        gs.players[player_idx].optr = 0;
                    }
                    gs.players[player_idx].sock = Some(sock);
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                    gs.players[player_idx].sock = Some(sock);
                }
                Err(e) => {
                    log::error!("Connection closed (send error): {}", e);
                    let cn = gs.players[player_idx].usnr;
                    gs.players[player_idx].ltick = 0;
                    gs.players[player_idx].rtick = 0;
                    gs.players[player_idx].zs = None;
                    player::connection::plr_logout(gs, cn, player_idx, LogoutReason::Unknown);
                }
            }
        }
    }
}

impl Drop for Server {
    /// Ensure background writes drain before process teardown.
    ///
    /// In the normal shutdown path `shutdown_background_saver()` will already
    /// have been called explicitly (and will have taken the saver out of
    /// `self.background_saver`), so this call is a no-op there.  In abnormal
    /// exit / panic scenarios the saver may still hold queued jobs; calling
    /// `shutdown_background_saver()` here ensures those writes complete and
    /// that the KeyDB connection is cleanly closed during teardown.
    fn drop(&mut self) {
        self.shutdown_background_saver();
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

    /// `apply_map_patch` overwrites only the static tile fields and leaves
    /// the dynamic fields (`ch`, `to_ch`, `it`, `light`, `dlight`)
    /// untouched, so in-flight character and item state survives an admin
    /// edit.
    #[test]
    fn apply_map_patch_preserves_dynamic_fields() {
        crate::test_helpers::with_test_gs(|gs| {
            let map_x = core::constants::SERVER_MAPX as usize;
            let x = 7usize;
            let y = 11usize;
            let idx = y * map_x + x;

            // Seed dynamic fields on the target tile.
            let tile = &mut gs.map[idx];
            tile.sprite = 1;
            tile.fsprite = 2;
            tile.flags = 0x00FF;
            tile.ch = 4242;
            tile.to_ch = 9000;
            tile.it = 333;
            tile.light = 5;
            tile.dlight = 6;

            let patch = core::map_store::MapPatch {
                x: x as u32,
                y: y as u32,
                sprite: 100,
                fsprite: 200,
                flags: 0xDEADBEEF,
            };
            assert!(Server::apply_map_patch(gs, &patch));

            let tile = gs.map[idx];
            assert_eq!(tile.sprite, 100);
            assert_eq!(tile.fsprite, 200);
            assert_eq!(tile.flags, 0xDEADBEEF);
            assert_eq!(tile.ch, 4242, "ch must be preserved");
            assert_eq!(tile.to_ch, 9000, "to_ch must be preserved");
            assert_eq!(tile.it, 333, "it must be preserved");
            assert_eq!(tile.light, 5, "light must be preserved");
            assert_eq!(tile.dlight, 6, "dlight must be preserved");
        });
    }

    /// Patches with coordinates outside the map are dropped without
    /// clobbering neighboring tiles.
    #[test]
    fn apply_map_patch_rejects_out_of_range_coords() {
        crate::test_helpers::with_test_gs(|gs| {
            let map_x = core::constants::SERVER_MAPX as usize;
            let patch = core::map_store::MapPatch {
                x: map_x as u32, // one past the last valid x
                y: 0,
                sprite: 9,
                fsprite: 9,
                flags: 9,
            };
            assert!(!Server::apply_map_patch(gs, &patch));
            assert_eq!(gs.map[0].sprite, 0);
        });
    }

    /// `apply_item_patch` overwrites the static authoring fields and leaves
    /// dynamic runtime fields (position, damage state, current age/damage,
    /// runtime sprite override) untouched.
    #[test]
    fn apply_item_patch_preserves_dynamic_fields() {
        crate::test_helpers::with_test_gs(|gs| {
            let idx = 12usize;
            let slot = &mut gs.items[idx];
            slot.x = 50;
            slot.y = 60;
            slot.carried = 7;
            slot.damage_state = 3;
            slot.current_age = [11, 22];
            slot.current_damage = 9;
            slot.sprite_override = 555;
            slot.value = 1;

            let mut new_item = core::types::Item::default();
            new_item.value = 9_999;
            new_item.flags = 0xAA;
            let patch = core::item_store::ItemPatch::from_item(idx, &new_item);
            assert!(Server::apply_item_patch(gs, &patch));

            let slot = gs.items[idx];
            assert_eq!(slot.value, 9_999);
            assert_eq!(slot.flags, 0xAA);
            assert_eq!(slot.x, 50, "x must be preserved");
            assert_eq!(slot.y, 60, "y must be preserved");
            assert_eq!(slot.carried, 7, "carried must be preserved");
            assert_eq!(slot.damage_state, 3, "damage_state must be preserved");
            assert_eq!(slot.current_age, [11, 22], "current_age must be preserved");
            assert_eq!(slot.current_damage, 9, "current_damage must be preserved");
            assert_eq!(
                slot.sprite_override, 555,
                "sprite_override must be preserved"
            );
        });
    }

    /// Patches addressing slots outside `MAXITEM` are dropped.
    #[test]
    fn apply_item_patch_rejects_out_of_range_slot() {
        crate::test_helpers::with_test_gs(|gs| {
            let mut new_item = core::types::Item::default();
            new_item.value = 1;
            let patch = core::item_store::ItemPatch::from_item(core::constants::MAXITEM, &new_item);
            assert!(!Server::apply_item_patch(gs, &patch));
        });
    }

    /// `apply_character_patch` overwrites only the static authoring fields
    /// and leaves dynamic runtime state (position, combat AI, current
    /// resources, inventory, networking) untouched.
    #[test]
    fn apply_character_patch_preserves_dynamic_fields() {
        crate::test_helpers::with_test_gs(|gs| {
            let idx = 5usize;
            let slot = &mut gs.characters[idx];
            slot.x = 100;
            slot.y = 200;
            slot.tox = 101;
            slot.toy = 201;
            slot.dir = 3;
            slot.status = 9;
            slot.a_hp = 555;
            slot.a_end = 444;
            slot.a_mana = 333;
            slot.gold = 12_345;
            slot.item[0] = 99;
            slot.worn[1] = 88;
            slot.spell[2] = 77;
            slot.citem = 66;
            slot.attack_cn = 4;
            slot.skill_nr = 5;
            slot.goto_x = 110;
            slot.goto_y = 210;
            slot.idle = 222;
            slot.addr = 0xDEADBEEF;
            slot.depot[0] = 17;
            slot.depot_cost = 4;
            slot.luck = 50;
            slot.kindred = 1;

            let mut new_char = core::types::Character::default();
            new_char.kindred = 9;
            new_char.flags = 0xBEEF;
            new_char.alignment = -7;
            let patch = core::character_store::CharacterPatch::from_character(idx, &new_char);
            assert!(Server::apply_character_patch(gs, &patch));

            let slot = gs.characters[idx];
            assert_eq!(slot.kindred, 9);
            assert_eq!(slot.flags, 0xBEEF);
            assert_eq!(slot.alignment, -7);
            assert_eq!(slot.x, 100, "x must be preserved");
            assert_eq!(slot.y, 200, "y must be preserved");
            assert_eq!(slot.tox, 101, "tox must be preserved");
            assert_eq!(slot.toy, 201, "toy must be preserved");
            assert_eq!(slot.dir, 3, "dir must be preserved");
            assert_eq!(slot.status, 9, "status must be preserved");
            assert_eq!(slot.a_hp, 555, "a_hp must be preserved");
            assert_eq!(slot.a_end, 444, "a_end must be preserved");
            assert_eq!(slot.a_mana, 333, "a_mana must be preserved");
            assert_eq!(slot.gold, 12_345, "gold must be preserved");
            assert_eq!(slot.item[0], 99, "item[0] must be preserved");
            assert_eq!(slot.worn[1], 88, "worn[1] must be preserved");
            assert_eq!(slot.spell[2], 77, "spell[2] must be preserved");
            assert_eq!(slot.citem, 66, "citem must be preserved");
            assert_eq!(slot.attack_cn, 4, "attack_cn must be preserved");
            assert_eq!(slot.skill_nr, 5, "skill_nr must be preserved");
            assert_eq!(slot.goto_x, 110, "goto_x must be preserved");
            assert_eq!(slot.goto_y, 210, "goto_y must be preserved");
            assert_eq!(slot.idle, 222, "idle must be preserved");
            assert_eq!(slot.addr, 0xDEADBEEF, "addr must be preserved");
            assert_eq!(slot.depot[0], 17, "depot[0] must be preserved");
            assert_eq!(slot.depot_cost, 4, "depot_cost must be preserved");
            assert_eq!(slot.luck, 50, "luck must be preserved");
        });
    }

    /// Patches addressing slots outside `MAXCHARS` are dropped.
    #[test]
    fn apply_character_patch_rejects_out_of_range_slot() {
        crate::test_helpers::with_test_gs(|gs| {
            let new_char = core::types::Character::default();
            let patch = core::character_store::CharacterPatch::from_character(
                core::constants::MAXCHARS,
                &new_char,
            );
            assert!(!Server::apply_character_patch(gs, &patch));
        });
    }
}
