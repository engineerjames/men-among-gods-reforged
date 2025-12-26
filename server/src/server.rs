use chrono::Timelike;
use core::constants::MAXPLAYER;
use core::types::{Character, ServerPlayer};
use std::io::ErrorKind;
use std::net::TcpListener;
use std::sync::{OnceLock, RwLock};
use std::time::{Duration, Instant};

use crate::enums::CharacterFlags;
use crate::god::God;
use crate::lab9::Labyrinth9;
use crate::network_manager::NetworkManager;
use crate::repository::Repository;
use crate::state::State;
use crate::{player, populate};

static PLAYERS: OnceLock<RwLock<[core::types::ServerPlayer; MAXPLAYER]>> = OnceLock::new();

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
        let players = std::array::from_fn(|_| core::types::ServerPlayer::new());
        PLAYERS
            .set(RwLock::new(players))
            .map_err(|_| "Players already initialized".to_string())?;
        Ok(())
    }

    pub fn with_players<F, R>(f: F) -> R
    where
        F: FnOnce(&[core::types::ServerPlayer]) -> R,
    {
        let players = PLAYERS
            .get()
            .expect("Players not initialized")
            .read()
            .unwrap();
        f(&players[..])
    }

    pub fn with_players_mut<F, R>(f: F) -> R
    where
        F: FnOnce(&mut [core::types::ServerPlayer]) -> R,
    {
        let mut players = PLAYERS
            .get()
            .expect("Players not initialized")
            .write()
            .unwrap();
        f(&mut players[..])
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

        Server::initialize_players()?;
        Repository::initialize()?;
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

        // remove lab items from all players (leave this here for a while!)
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

        // Validate character template positions
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

        // Periodically save characters (every 32 ticks)
        if (ticker & 31) == 0 {
            let char_idx = (ticker as usize) % core::constants::MAXCHARS;
            populate::pop_save_char(char_idx);
        }

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
                    State::with(|state| {
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

            self.do_regenerate(n);
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
        self.effect_tick();
        self.item_tick();
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

    fn check_expire(&self, _cn: usize) {
        // Check character expiration - to be implemented
    }

    fn check_valid(&self, _cn: usize) -> bool {
        // Check if character is valid - to be implemented
        true
    }

    fn do_regenerate(&self, _cn: usize) {
        // Character regeneration - to be implemented
    }

    fn effect_tick(&self) {
        // Process effects - to be implemented
    }

    fn item_tick(&self) {
        // Process items - to be implemented
    }

    fn global_tick(&self) {
        // Global updates (time of day, weather, etc.) - to be implemented
    }

    fn compress_ticks(&mut self) {
        // Compress and send tick data to all connected players
        // This is the equivalent of compress_ticks() in C++
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

    fn new_player(&mut self, _stream: std::net::TcpStream, _addr: std::net::IpAddr) {
        // Process new player connection - to be implemented
        // This is the equivalent of new_player() in C++
    }

    fn rec_player(&self, _player_idx: usize) {
        // Receive data from player - to be implemented
        // This is the equivalent of rec_player() in C++
    }

    fn send_player(&self, _player_idx: usize) {
        // Send data to player - to be implemented
        // This is the equivalent of send_player() in C++
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
