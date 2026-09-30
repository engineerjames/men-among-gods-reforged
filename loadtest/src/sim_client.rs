//! Per-client simulation task.
//!
//! Each call to [`run`] represents one bot client.  The flow is:
//!
//! 1. Sleep for the staggered ramp-up delay.
//! 2. Call the account API to ensure an account + character exist (`bootstrap_client`).
//! 3. Acquire exclusive access to the shared [`LoginGate`] (`run.login_stagger_secs`),
//!    which serializes ticket-mint/connect/handshake across all clients so recently
//!    spawned characters have time to move away from the spawn point before the next
//!    one arrives — the gate is held until this client's login actually finishes (or
//!    fails), not released on a pre-computed schedule.
//! 4. Mint a fresh one-time login ticket (`mint_ticket`).
//! 5. Establish a TLS connection and complete the game-login handshake, then release
//!    the `LoginGate`.
//! 6. Enter the main loop:
//!    - Read framed server packets, track position from `SV_SETORIGIN`, record RTT from `SV_PONG`.
//!    - Increment the client ticker on each frame; send `CL_CTICK` every 16 frames.
//!    - The first time a position is known, and if `movement.enable_dispersion` is set,
//!      say the god password and `/goto` a random in-bounds location once (see
//!      [`maybe_send_dispersion`]), spreading newly spawned characters away from the
//!      shared spawn point.
//!    - Send a random movement command every `movement.interval_ms` milliseconds.
//!    - Optionally send `CL_PING` every `ping.interval_secs` seconds.
//!    - Send each configured `[[commands]]` entry (e.g. `/rank`, `/who`) on its
//!      own `interval_secs` (see [`maybe_send_commands`]).
//!    - When enabled under `[behavior]`, periodically cast a known spell
//!      (`behavior.cast`), say a chat line (`behavior.chat`), or `CL_USE` a
//!      nearby usable item (`behavior.interact`). Decisions come from the
//!      bot's [`BotProfile`] (class, known skills) and its [`WorldView`].
//! 7. On shutdown, the task exits and bumps the disconnect counter.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;

use mag_core::client_commands::ClientCommand;
use mag_core::constants::{SERVER_MAPX, SERVER_MAPY};
use mag_core::server_commands::{ServerCommand, ServerCommandData};
use mag_core::skills::get_skill_name;
use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;
use tokio::io::{AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::sync::broadcast;
use tokio::time::{Duration, MissedTickBehavior, interval};

use crate::api_bootstrap::{BotCharacter, RateLimiter, bootstrap_client, mint_ticket};
use crate::behavior::{BotProfile, pick_chat_message, pick_interact_target};
use crate::config::LoadTestConfig;
use crate::login_gate::LoginGate;
use crate::metrics::Metrics;
use crate::net_impair;
use crate::protocol::{FramedReader, GameStream, TlsGameStream};
use crate::world_view::WorldView;

/// Margin (in tiles) kept away from the map edges when picking a random
/// dispersion `/goto` target, avoiding `usize` underflow in the server's
/// fuzzy-placement logic (`God::transfer_char` tries the target ± 3 tiles).
const DISPERSION_MARGIN: i32 = 20;

// ---------------------------------------------------------------------------
// Per-client state
// ---------------------------------------------------------------------------

struct ClientState {
    /// Number of server tick frames received since login.
    client_ticker: u32,
    /// Value of `client_ticker` the last time `CL_CTICK` was sent.
    last_ctick_sent: u32,
    /// Known world X position (from the most recent `SV_SETORIGIN`).
    self_x: Option<i16>,
    /// Known world Y position (from the most recent `SV_SETORIGIN`).
    self_y: Option<i16>,
    /// Sequence counter for outgoing `CL_PING` packets.
    ping_seq: u32,
    /// Map from ping sequence number to the time the ping was sent.
    ping_times: HashMap<u32, Instant>,
    /// Instant of the most recently received tick frame (for gap detection).
    last_tick_instant: Option<Instant>,
    /// Milliseconds elapsed since the task started, used as CL_PING timestamp.
    start: Instant,
    /// Whether the one-shot login dispersion sequence (god password + `/goto`)
    /// has already been sent for this client.
    dispersion_done: bool,
    /// Tracked identity (class, sex, name) and server-reported skills.
    profile: BotProfile,
    /// Bot-side model of the visible map window (usable items, characters).
    world: WorldView,
}

impl ClientState {
    fn new(character: BotCharacter) -> Self {
        Self {
            client_ticker: 0,
            last_ctick_sent: 0,
            self_x: None,
            self_y: None,
            ping_seq: 0,
            ping_times: HashMap::new(),
            last_tick_instant: None,
            start: Instant::now(),
            dispersion_done: false,
            profile: BotProfile::new(character),
            world: WorldView::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Runs a single bot client from bootstrap through the full game session.
///
/// # Arguments
///
/// * `index` - Bot index, used for username derivation and log messages.
/// * `config` - Shared load-test configuration.
/// * `rate_limiter` - Shared API rate limiter.
/// * `login_gate` - Shared gate granting exclusive access to the ticket-mint/
///   connect/handshake sequence, giving recently spawned characters time to
///   move away from the spawn point before the next character logs in.
/// * `god_password` - God password used for one-shot login dispersion; empty
///   when `movement.enable_dispersion` is disabled.
/// * `http` - Shared HTTP client reused across every bot task; building a
///   fresh client per bot/request multiplies connection-pool/TLS-context fd
///   usage and can exhaust the process's open-file limit under load.
/// * `metrics` - Shared metrics store.
/// * `shutdown` - Broadcast receiver; fires when the run duration elapses.
#[allow(clippy::too_many_arguments)]
pub async fn run(
    index: usize,
    config: Arc<LoadTestConfig>,
    rate_limiter: Arc<RateLimiter>,
    login_gate: Arc<LoginGate>,
    god_password: Arc<String>,
    http: Arc<reqwest::Client>,
    metrics: Arc<Metrics>,
    mut shutdown: broadcast::Receiver<()>,
) {
    // Staggered ramp-up: spread connections evenly over `ramp_up_secs`.
    let ramp_delay = if config.run.num_clients <= 1 {
        Duration::ZERO
    } else {
        let slot_secs = config.run.ramp_up_secs / (config.run.num_clients - 1).max(1) as f64;
        Duration::from_secs_f64(slot_secs * index as f64)
    };

    tokio::select! {
        _ = tokio::time::sleep(ramp_delay) => {}
        _ = shutdown.recv() => return,
    }

    // Bootstrap: ensure account and character exist.
    let bootstrap_result = bootstrap_client(index, &config, &http, &rate_limiter).await;
    let (jwt, character) = match bootstrap_result {
        Ok(v) => v,
        Err(e) => {
            log::warn!("Client {index}: bootstrap failed — {e:#}");
            metrics.connect_errors.fetch_add(1, Ordering::Relaxed);
            return;
        }
    };
    let character_id = character.id;
    metrics.record_class(character.class);

    // Wait for exclusive access to the login sequence: only one client mints
    // a ticket, connects, and completes the handshake at a time, and the
    // next client's cooldown only starts once *this* client's login actually
    // finishes (or fails) — see `LoginGate` for why a simpler "pre-scheduled
    // slot" design can be defeated by shared downstream API rate-limit bursts.
    let login_slot = tokio::select! {
        slot = login_gate.acquire() => slot,
        _ = shutdown.recv() => return,
    };

    // Mint a fresh ticket just before connecting (30 s TTL).
    let ticket = match mint_ticket(&jwt, character_id, &config, &http, &rate_limiter).await {
        Ok(t) => t,
        Err(e) => {
            log::warn!("Client {index}: ticket mint failed — {e}");
            metrics.connect_errors.fetch_add(1, Ordering::Relaxed);
            return;
        }
    };

    // Establish TLS connection.
    let mut stream = match GameStream::connect(&config.server.host, config.server.port).await {
        Ok(s) => s,
        Err(e) => {
            log::warn!("Client {index}: connect failed — {e}");
            metrics.connect_errors.fetch_add(1, Ordering::Relaxed);
            return;
        }
    };

    // Game-login handshake.
    if let Err(e) = stream.handshake(ticket).await {
        log::warn!("Client {index}: handshake failed — {e}");
        metrics.connect_errors.fetch_add(1, Ordering::Relaxed);
        return;
    }

    metrics.connected.fetch_add(1, Ordering::Relaxed);
    let connected_at = Instant::now();
    log::info!(
        "Client {index}: logged in (character_id={character_id} name={} class={:?} sex={:?})",
        character.name,
        character.class,
        character.sex,
    );

    // The character has fully spawned into the world — release the gate so
    // the next client's cooldown starts counting from now, giving this
    // character time to move away from the spawn point.
    drop(login_slot);

    // Split the TLS stream so reads and writes are independent.
    let (read_half, write_half) = tokio::io::split(stream.into_inner());

    game_loop(
        index,
        character,
        read_half,
        write_half,
        config,
        god_password,
        metrics.clone(),
        &mut shutdown,
    )
    .await;

    metrics
        .total_client_connected_ms
        .fetch_add(connected_at.elapsed().as_millis() as u64, Ordering::Relaxed);
    metrics.disconnects.fetch_add(1, Ordering::Relaxed);
    log::info!("Client {index}: disconnected");
}

// ---------------------------------------------------------------------------
// Game loop
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
async fn game_loop(
    index: usize,
    character: BotCharacter,
    read_half: ReadHalf<TlsGameStream>,
    mut write_half: WriteHalf<TlsGameStream>,
    config: Arc<LoadTestConfig>,
    god_password: Arc<String>,
    metrics: Arc<Metrics>,
    shutdown: &mut broadcast::Receiver<()>,
) {
    // Background read task: continuously drains the TLS read half into a channel.
    let (data_tx, mut data_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(128);
    let metrics_read = metrics.clone();
    tokio::spawn(async move {
        let mut read_half = read_half;
        let mut buf = vec![0u8; 16 * 1024];
        loop {
            match tokio::io::AsyncReadExt::read(&mut read_half, &mut buf).await {
                Ok(0) => break, // server closed connection
                Ok(n) => {
                    metrics_read.bytes_in.fetch_add(n as u64, Ordering::Relaxed);
                    if data_tx.send(buf[..n].to_vec()).await.is_err() {
                        break; // main loop exited
                    }
                }
                Err(e) => {
                    log::debug!("Client {index}: read error — {e}");
                    break;
                }
            }
        }
    });

    let mut state = ClientState::new(character);
    let mut framed = FramedReader::new();
    let mut rng = StdRng::from_os_rng();

    let mut move_timer = interval(Duration::from_millis(config.movement.interval_ms));
    move_timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

    // In-world behaviours. Each timer is built even when disabled (the
    // select! guard skips it) so the branch set stays static. A small random
    // phase offset per bot keeps hundreds of clients from firing in lockstep.
    let behavior = &config.behavior;
    let mut cast_timer = phased_interval(behavior.cast.interval_ms, &mut rng);
    let mut chat_timer = phased_interval(behavior.chat.interval_ms, &mut rng);
    let mut interact_timer = phased_interval(behavior.interact.interval_ms, &mut rng);

    let ping_interval_ms = (config.ping.interval_secs * 1000.0) as u64;
    let mut ping_timer = interval(Duration::from_millis(ping_interval_ms.max(1)));
    ping_timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
    // Delay first ping until we have at least one server tick.
    ping_timer.reset();

    // Periodic slash-commands (`[[commands]]`): each entry fires on its own
    // interval, tracked as an absolute due-time so entries with different
    // periods don't need one `select!` branch each (which would require a
    // fixed, compile-time-known count). Checked on a fine-grained shared
    // tick; see `maybe_send_commands`.
    let now = Instant::now();
    let mut command_due: Vec<Instant> = config
        .commands
        .iter()
        .map(|c| now + Duration::from_secs_f64(c.interval_secs.max(0.0)))
        .collect();
    let mut command_timer = interval(Duration::from_millis(100));
    command_timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

    'main: loop {
        // Precompute flag so the borrow checker is happy inside select!
        let has_position = state.self_x.is_some();
        let ping_enabled = config.ping.enabled && state.client_ticker > 0;
        let cast_enabled = behavior.cast.enabled && has_position;
        let chat_enabled = behavior.chat.enabled && has_position;
        let interact_enabled = behavior.interact.enabled && has_position;

        tokio::select! {
            // Shutdown signal
            _ = shutdown.recv() => break 'main,

            // Inbound data from the background read task
            maybe_data = data_rx.recv() => {
                let data = match maybe_data {
                    Some(d) => d,
                    None => break 'main, // read task exited (server closed connection)
                };

                framed.feed(&data);

                loop {
                    match framed.next_frame_payload() {
                        Err(e) => {
                            log::warn!("Client {index}: frame error — {e}");
                            break 'main;
                        }
                        Ok(None) => break, // wait for more bytes
                        Ok(Some(payload)) => {
                            // Process server commands inside this frame.
                            if !payload.is_empty() {
                                match FramedReader::split_commands(&payload) {
                                    Err(e) => {
                                        log::debug!("Client {index}: split_commands error — {e}");
                                    }
                                    Ok(cmds) => {
                                        for cmd_bytes in cmds {
                                            process_command(
                                                index,
                                                &cmd_bytes,
                                                &mut state,
                                                &metrics,
                                            );
                                            // Check if process_command signalled disconnect.
                                            if state.client_ticker == u32::MAX {
                                                break 'main;
                                            }
                                        }
                                    }
                                }
                            }

                            // One frame = one server tick.
                            on_tick(index, &mut state, &mut write_half, &metrics).await;

                            // One-shot login dispersion: fires exactly once, right after
                            // the first confirmed world position, before normal periodic
                            // movement continues below.
                            maybe_send_dispersion(
                                index,
                                &mut state,
                                &mut write_half,
                                &config,
                                &god_password,
                                &mut rng,
                                &metrics,
                            )
                            .await;
                        }
                    }
                }
            }

            // Periodic movement
            _ = move_timer.tick(), if has_position => {
                let (x, y) = (state.self_x.unwrap(), state.self_y.unwrap());
                let radius = config.movement.radius;
                let dx: i16 = rng.random_range(-radius..=radius);
                let dy: i16 = rng.random_range(-radius..=radius);
                let tx = x.saturating_add(dx);
                let ty = y.saturating_add(dy);
                let cmd = ClientCommand::new_move(tx, i32::from(ty));
                send_impaired(
                    index,
                    &mut write_half,
                    cmd,
                    &config,
                    &mut rng,
                    &metrics,
                )
                .await;
            }

            // Periodic ping
            _ = ping_timer.tick(), if ping_enabled => {
                state.ping_seq = state.ping_seq.wrapping_add(1);
                let seq = state.ping_seq;
                let elapsed_ms = state.start.elapsed().as_millis() as u32;
                state.ping_times.insert(seq, Instant::now());
                let cmd = ClientCommand::new_ping(seq, elapsed_ms);
                send_impaired(
                    index,
                    &mut write_half,
                    cmd,
                    &config,
                    &mut rng,
                    &metrics,
                )
                .await;
            }

            // Periodic slash-commands (`[[commands]]`)
            _ = command_timer.tick(), if has_position && !config.commands.is_empty() => {
                maybe_send_commands(
                    index,
                    &config,
                    &mut command_due,
                    &mut write_half,
                    &metrics,
                )
                .await;
            }

            // Periodic spell casting (`[behavior.cast]`)
            _ = cast_timer.tick(), if cast_enabled => {
                let choice = state.profile.pick_cast(&state.world, behavior.cast.allow_hostile, &mut rng);
                match choice {
                    Some(c) => {
                        let cmd = ClientCommand::new_skill(
                            c.skill as u32,
                            c.target,
                            u32::from(state.profile.braveness),
                        );
                        log::debug!(
                            "Client {index}: casting {} (skill={}) target={}",
                            get_skill_name(c.skill),
                            c.skill,
                            c.target,
                        );
                        send_impaired(index, &mut write_half, cmd, &config, &mut rng, &metrics).await;
                        metrics.casts_sent.fetch_add(1, Ordering::Relaxed);
                    }
                    None => {
                        metrics.casts_skipped.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }

            // Periodic chat (`[behavior.chat]`)
            _ = chat_timer.tick(), if chat_enabled => {
                let text = pick_chat_message(&behavior.chat.messages, &mut rng).to_owned();
                if send_say_text(&mut write_half, &text, &metrics).await {
                    metrics.chats_sent.fetch_add(1, Ordering::Relaxed);
                    log::trace!("Client {index}: said {text:?}");
                } else {
                    metrics.chats_errors.fetch_add(1, Ordering::Relaxed);
                }
            }

            // Periodic environment interaction (`[behavior.interact]`)
            _ = interact_timer.tick(), if interact_enabled => {
                match pick_interact_target(&state.world, behavior.interact.radius, &mut rng) {
                    Some((x, y)) => {
                        log::debug!("Client {index}: using item at {x},{y}");
                        let cmd = ClientCommand::new_use(x as i16, y);
                        send_impaired(index, &mut write_half, cmd, &config, &mut rng, &metrics).await;
                        metrics.interacts_sent.fetch_add(1, Ordering::Relaxed);
                    }
                    None => {
                        metrics.interacts_skipped.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Builds a periodic timer for a `[behavior.*]` action whose first tick is
/// delayed by a random fraction of the period.
///
/// Every bot builds its timers at login; without a phase offset, bots that
/// logged in during the same second would all cast/chat/interact in the
/// same tick for the rest of the run, which is an unrealistic burst pattern.
///
/// # Arguments
///
/// * `interval_ms` - Period in milliseconds (clamped to at least 1).
/// * `rng` - RNG for the phase offset.
///
/// # Returns
///
/// * A `Skip`-behaviour interval whose first tick is `0..period` in the future.
fn phased_interval(interval_ms: u64, rng: &mut impl Rng) -> tokio::time::Interval {
    let period = Duration::from_millis(interval_ms.max(1));
    let phase = Duration::from_millis(rng.random_range(0..interval_ms.max(1)));
    let mut timer = tokio::time::interval_at(tokio::time::Instant::now() + phase, period);
    timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
    timer
}

/// Processes a single server command byte slice, updating client state.
///
/// Sets `state.client_ticker = u32::MAX` as a sentinel when `SV_EXIT` is received,
/// to signal the game loop to break.
fn process_command(index: usize, cmd_bytes: &[u8], state: &mut ClientState, metrics: &Metrics) {
    let Some(cmd) = ServerCommand::from_bytes(cmd_bytes) else {
        return;
    };

    // Keep the map window model current (scrolls, tile flags, origin).
    state.world.apply(&cmd);

    match cmd.structured_data {
        ServerCommandData::SetOrigin { .. } => {
            // Origin is the top-left of the visible grid; player sits at center.
            let was_unknown = state.self_x.is_none();
            let (x, y) = state.world.self_pos().unwrap_or((0, 0));
            state.self_x = Some(x);
            state.self_y = Some(y);
            if was_unknown {
                log::debug!(
                    "Client {index}: class={:?} castable skills: [{}]",
                    state.profile.class(),
                    state.profile.describe_skills(),
                );
            }
        }
        ServerCommandData::SetCharSkill {
            index: skill,
            values,
        } => {
            state.profile.set_skill(skill, &values);
        }
        ServerCommandData::SetCharAttrib {
            index: attrib,
            values,
        } => {
            state.profile.set_attrib(attrib, &values);
        }
        ServerCommandData::Pong { seq, .. } => {
            if let Some(sent_at) = state.ping_times.remove(&seq) {
                let rtt_ms = sent_at.elapsed().as_millis() as u32;
                metrics.push_rtt(rtt_ms);
                log::trace!("Client {index}: RTT seq={seq} rtt={rtt_ms}ms");
            }
        }
        ServerCommandData::Exit { reason } => {
            log::info!("Client {index}: SV_EXIT reason={reason}");
            state.client_ticker = u32::MAX; // sentinel: break game loop
        }
        _ => {}
    }
}

/// Called once per received tick frame to update timing metrics and send `CL_CTICK`.
///
/// `CL_CTICK` bypasses network impairment; it must reach the server promptly or
/// the server will disconnect the client for being too slow.
async fn on_tick(
    index: usize,
    state: &mut ClientState,
    write_half: &mut WriteHalf<TlsGameStream>,
    metrics: &Metrics,
) {
    // Guard: sentinel value signals requested disconnect.
    if state.client_ticker == u32::MAX {
        return;
    }

    state.client_ticker = state.client_ticker.wrapping_add(1);
    metrics.ticks_total.fetch_add(1, Ordering::Relaxed);

    // Track inter-tick gaps to detect server slowdowns.
    let now = Instant::now();
    if let Some(prev) = state.last_tick_instant.replace(now) {
        let gap_ms = prev.elapsed().as_millis() as u32;
        if gap_ms > 100 {
            metrics.tick_gap_late.fetch_add(1, Ordering::Relaxed);
            log::trace!("Client {index}: late tick gap {gap_ms}ms");
        }
    }

    // Send CL_CTICK every 16 received frames (mirrors real client logic).
    let t = state.client_ticker;
    if t != 0 && (t & 15) == 0 && t != state.last_ctick_sent {
        state.last_ctick_sent = t;
        let cmd = ClientCommand::new_tick(t);
        let bytes = cmd.to_bytes();
        let len = bytes.len();
        if write_half.write_all(&bytes).await.is_err() {
            log::debug!("Client {index}: CTick write failed");
            state.client_ticker = u32::MAX;
            return;
        }
        metrics.bytes_out.fetch_add(len as u64, Ordering::Relaxed);
        metrics.pkts_out.fetch_add(1, Ordering::Relaxed);
    }
}

/// Sends a command with optional latency, jitter, and drop impairment applied.
///
/// `CL_CTICK` is intentionally NOT routed through this function.
async fn send_impaired(
    index: usize,
    write_half: &mut WriteHalf<TlsGameStream>,
    cmd: ClientCommand,
    config: &LoadTestConfig,
    rng: &mut StdRng,
    metrics: &Metrics,
) {
    if net_impair::should_drop(&config.impairment, rng) {
        log::trace!("Client {index}: dropped {:?}", cmd.header);
        return;
    }

    let delay = net_impair::send_delay(&config.impairment, rng);
    if !delay.is_zero() {
        tokio::time::sleep(delay).await;
    }

    let bytes = cmd.to_bytes();
    let len = bytes.len();
    if write_half.write_all(&bytes).await.is_err() {
        log::debug!("Client {index}: send failed");
        return;
    }
    metrics.bytes_out.fetch_add(len as u64, Ordering::Relaxed);
    metrics.pkts_out.fetch_add(1, Ordering::Relaxed);
}

/// Sends the one-shot login dispersion sequence exactly once per client.
///
/// The first time a world position is known (`state.self_x.is_some()`) and
/// dispersion is enabled, this says the god password (granting God/Imp flags
/// server-side) and then `/goto`s a random in-bounds map location, so the
/// character moves away from the crowded shared spawn point before normal
/// periodic movement takes over. No-op (and cheap to call every frame) once
/// `state.dispersion_done` is set or dispersion is disabled/unconfigured.
///
/// # Arguments
///
/// * `index` - Bot index, used in log messages.
/// * `state` - Mutable per-client state; consulted for the current position
///   and updated to mark dispersion as done.
/// * `write_half` - Write half of the TLS connection to send packets on.
/// * `config` - Shared load-test configuration (`movement.enable_dispersion`).
/// * `god_password` - God password to say; dispersion is skipped when empty.
/// * `rng` - Shared RNG used to pick the random `/goto` target.
/// * `metrics` - Shared metrics store, updated with dispersion counters.
async fn maybe_send_dispersion(
    index: usize,
    state: &mut ClientState,
    write_half: &mut WriteHalf<TlsGameStream>,
    config: &LoadTestConfig,
    god_password: &str,
    rng: &mut StdRng,
    metrics: &Metrics,
) {
    if state.dispersion_done || god_password.is_empty() || !config.movement.enable_dispersion {
        return;
    }
    if state.self_x.is_none() {
        return;
    }

    // Mark done immediately so this only ever fires once, even if this
    // function runs again before the sends below complete.
    state.dispersion_done = true;

    let (tx, ty) = random_dispersion_target(rng);
    let goto_text = format!("/goto {tx} {ty}");

    let ok = send_say_text(write_half, god_password, metrics).await
        && send_say_text(write_half, &goto_text, metrics).await;

    if ok {
        metrics.dispersion_sent.fetch_add(1, Ordering::Relaxed);
        log::info!("Client {index}: dispersion — said god password, /goto {tx} {ty}");
    } else {
        metrics.dispersion_errors.fetch_add(1, Ordering::Relaxed);
        log::warn!("Client {index}: dispersion send failed");
    }
}

/// Picks a random, in-bounds `/goto` target on the server map.
///
/// Coordinates are drawn uniformly from within [`DISPERSION_MARGIN`] tiles of
/// each map edge. This is a best-effort "valid" location: it guarantees the
/// coordinates are within `SERVER_MAPX`/`SERVER_MAPY` bounds with enough
/// margin to avoid edge-underflow in the server's fuzzy placement logic, but
/// does not guarantee the target tile itself is walkable — the server's
/// `/goto` handler already retries a few nearby tiles and simply logs a
/// failure message if the whole area is blocked.
///
/// # Arguments
///
/// * `rng` - RNG used to pick the coordinates.
///
/// # Returns
///
/// * A random `(x, y)` pair within the safe map bounds.
fn random_dispersion_target(rng: &mut StdRng) -> (i32, i32) {
    let x = rng.random_range(DISPERSION_MARGIN..SERVER_MAPX - DISPERSION_MARGIN);
    let y = rng.random_range(DISPERSION_MARGIN..SERVER_MAPY - DISPERSION_MARGIN);
    (x, y)
}

/// Sends a chat/say command, bypassing network impairment.
///
/// Splits `text` into the 8 `CmdInput1..8` packets expected by the server
/// (mirrors the real client's `new_say_packets` usage) and writes them in
/// order. Like `CL_CTICK`, dispersion setup traffic is not subject to
/// simulated latency/jitter/drop, since it is one-shot setup rather than
/// simulated gameplay traffic.
///
/// # Arguments
///
/// * `write_half` - Write half of the TLS connection to send packets on.
/// * `text` - Chat text to send (up to 120 bytes; longer text is truncated).
/// * `metrics` - Shared metrics store, updated with bytes/packets sent.
///
/// # Returns
///
/// * `true` if all 8 packets were written successfully, `false` on the first
///   write error (the caller should treat this as a failed dispersion attempt).
async fn send_say_text(
    write_half: &mut WriteHalf<TlsGameStream>,
    text: &str,
    metrics: &Metrics,
) -> bool {
    for pkt in ClientCommand::new_say_packets(text.as_bytes()) {
        let bytes = pkt.to_bytes();
        let len = bytes.len();
        if write_half.write_all(&bytes).await.is_err() {
            return false;
        }
        metrics.bytes_out.fetch_add(len as u64, Ordering::Relaxed);
        metrics.pkts_out.fetch_add(1, Ordering::Relaxed);
    }
    true
}

/// Sends every configured `[[commands]]` entry whose due time has elapsed,
/// then reschedules it for `now + interval_secs`.
///
/// Called on a fixed, coarse-grained shared timer (see `command_timer` in
/// [`game_loop`]) rather than one `select!` branch per entry, since the
/// number of configured commands is only known at runtime. Rescheduling from
/// `now` (rather than accumulating `due += interval`) means a delayed check
/// only pushes that one send back instead of causing a catch-up burst.
///
/// # Arguments
///
/// * `index` - Bot index, used in log messages.
/// * `config` - Shared load-test configuration (`commands` entries).
/// * `command_due` - Mutable per-client due-time for each `config.commands`
///   entry, indexed in the same order; updated in place after a send.
/// * `write_half` - Write half of the TLS connection to send packets on.
/// * `metrics` - Shared metrics store, updated with sent/error counters.
async fn maybe_send_commands(
    index: usize,
    config: &LoadTestConfig,
    command_due: &mut [Instant],
    write_half: &mut WriteHalf<TlsGameStream>,
    metrics: &Metrics,
) {
    let now = Instant::now();
    for (i, entry) in config.commands.iter().enumerate() {
        if now < command_due[i] {
            continue;
        }
        command_due[i] = now + Duration::from_secs_f64(entry.interval_secs.max(0.0));

        if send_say_text(write_half, &entry.command, metrics).await {
            metrics.commands_sent.fetch_add(1, Ordering::Relaxed);
            log::trace!("Client {index}: sent command {:?}", entry.command);
        } else {
            metrics.commands_errors.fetch_add(1, Ordering::Relaxed);
            log::debug!("Client {index}: failed to send command {:?}", entry.command);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mag_core::types::api::{Class, Sex};

    fn test_character() -> BotCharacter {
        BotCharacter {
            id: 1,
            name: "loadtesta".into(),
            class: Class::Templar,
            sex: Sex::Male,
        }
    }

    #[test]
    fn client_state_initial_values() {
        let s = ClientState::new(test_character());
        assert_eq!(s.client_ticker, 0);
        assert!(s.self_x.is_none());
        assert!(s.self_y.is_none());
        assert!(s.ping_times.is_empty());
        assert_eq!(s.profile.class(), Class::Templar);
        assert!(s.world.self_pos().is_none());
    }

    #[test]
    fn ramp_delay_distributes_evenly() {
        // First client should start immediately.
        // With num_clients=5 and ramp_up_secs=10.0:
        //   slot = 10.0 / 4 = 2.5s
        //   index 0 -> 0.0s, index 4 -> 10.0s
        let ramp_up_secs = 10.0f64;
        let num_clients = 5usize;
        let delay_for = |i: usize| {
            if num_clients <= 1 {
                0.0
            } else {
                let slot = ramp_up_secs / (num_clients - 1).max(1) as f64;
                slot * i as f64
            }
        };
        assert!((delay_for(0) - 0.0).abs() < 1e-9);
        assert!((delay_for(4) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn client_state_dispersion_starts_undone() {
        let s = ClientState::new(test_character());
        assert!(!s.dispersion_done);
    }

    #[test]
    fn set_origin_updates_position_and_world_view() {
        let metrics = Metrics::new();
        let mut s = ClientState::new(test_character());
        // Wire encoding: SetOrigin opcode followed by two little-endian i16s.
        let mut bytes = vec![mag_core::server_commands::ServerCommandType::SetOrigin as u8];
        bytes.extend_from_slice(&500i16.to_le_bytes());
        bytes.extend_from_slice(&600i16.to_le_bytes());
        process_command(0, &bytes, &mut s, &metrics);
        assert_eq!(s.self_x, Some(500 + mag_core::constants::TILEX as i16 / 2));
        assert_eq!(s.self_y, Some(600 + mag_core::constants::TILEY as i16 / 2));
        assert_eq!(
            s.world.self_pos(),
            Some((s.self_x.unwrap(), s.self_y.unwrap()))
        );
    }

    #[tokio::test]
    async fn phased_interval_handles_zero_period() {
        let mut rng = StdRng::seed_from_u64(9);
        let t = phased_interval(0, &mut rng);
        assert_eq!(t.period(), Duration::from_millis(1));
    }

    #[test]
    fn random_dispersion_target_within_bounds() {
        let mut rng = StdRng::seed_from_u64(42);
        for _ in 0..1000 {
            let (x, y) = random_dispersion_target(&mut rng);
            assert!((DISPERSION_MARGIN..SERVER_MAPX - DISPERSION_MARGIN).contains(&x));
            assert!((DISPERSION_MARGIN..SERVER_MAPY - DISPERSION_MARGIN).contains(&y));
        }
    }
}
