//! Per-client simulation task.
//!
//! Each call to [`run`] represents one bot client.  The flow is:
//!
//! 1. Sleep for the staggered ramp-up delay.
//! 2. Call the account API to ensure an account + character exist (`bootstrap_client`).
//! 3. Mint a fresh one-time login ticket (`mint_ticket`).
//! 4. Establish a TLS connection and complete the game-login handshake.
//! 5. Enter the main loop:
//!    - Read framed server packets, track position from `SV_SETORIGIN`, record RTT from `SV_PONG`.
//!    - Increment the client ticker on each frame; send `CL_CTICK` every 16 frames.
//!    - Send a random movement command every `movement.interval_ms` milliseconds.
//!    - Optionally send `CL_PING` every `ping.interval_secs` seconds.
//! 6. On shutdown, the task exits and bumps the disconnect counter.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;

use mag_core::client_commands::ClientCommand;
use mag_core::constants::{TILEX, TILEY};
use mag_core::server_commands::{ServerCommand, ServerCommandData};
use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;
use tokio::io::{AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::sync::broadcast;
use tokio::time::{Duration, MissedTickBehavior, interval};

use crate::api_bootstrap::{RateLimiter, bootstrap_client, mint_ticket};
use crate::config::LoadTestConfig;
use crate::metrics::Metrics;
use crate::net_impair;
use crate::protocol::{FramedReader, GameStream, TlsGameStream};

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
}

impl ClientState {
    fn new() -> Self {
        Self {
            client_ticker: 0,
            last_ctick_sent: 0,
            self_x: None,
            self_y: None,
            ping_seq: 0,
            ping_times: HashMap::new(),
            last_tick_instant: None,
            start: Instant::now(),
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
/// * `metrics` - Shared metrics store.
/// * `shutdown` - Broadcast receiver; fires when the run duration elapses.
pub async fn run(
    index: usize,
    config: Arc<LoadTestConfig>,
    rate_limiter: Arc<RateLimiter>,
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
    let bootstrap_result = bootstrap_client(index, &config, &rate_limiter).await;
    let (jwt, character_id) = match bootstrap_result {
        Ok(v) => v,
        Err(e) => {
            log::warn!("Client {index}: bootstrap failed — {e}");
            metrics.connect_errors.fetch_add(1, Ordering::Relaxed);
            return;
        }
    };

    // Mint a fresh ticket just before connecting (30 s TTL).
    let ticket = match mint_ticket(&jwt, character_id, &config, &rate_limiter).await {
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
    log::info!("Client {index}: logged in (character_id={character_id})");

    // Split the TLS stream so reads and writes are independent.
    let (read_half, write_half) = tokio::io::split(stream.into_inner());

    game_loop(
        index,
        read_half,
        write_half,
        config,
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

async fn game_loop(
    index: usize,
    read_half: ReadHalf<TlsGameStream>,
    mut write_half: WriteHalf<TlsGameStream>,
    config: Arc<LoadTestConfig>,
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

    let mut state = ClientState::new();
    let mut framed = FramedReader::new();
    let mut rng = StdRng::from_entropy();

    let mut move_timer = interval(Duration::from_millis(config.movement.interval_ms));
    move_timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let ping_interval_ms = (config.ping.interval_secs * 1000.0) as u64;
    let mut ping_timer = interval(Duration::from_millis(ping_interval_ms.max(1)));
    ping_timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
    // Delay first ping until we have at least one server tick.
    ping_timer.reset();

    'main: loop {
        // Precompute flag so the borrow checker is happy inside select!
        let has_position = state.self_x.is_some();
        let ping_enabled = config.ping.enabled && state.client_ticker > 0;

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
                        }
                    }
                }
            }

            // Periodic movement
            _ = move_timer.tick(), if has_position => {
                let (x, y) = (state.self_x.unwrap(), state.self_y.unwrap());
                let radius = config.movement.radius;
                let dx: i16 = rng.gen_range(-radius..=radius);
                let dy: i16 = rng.gen_range(-radius..=radius);
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
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Processes a single server command byte slice, updating client state.
///
/// Sets `state.client_ticker = u32::MAX` as a sentinel when `SV_EXIT` is received,
/// to signal the game loop to break.
fn process_command(index: usize, cmd_bytes: &[u8], state: &mut ClientState, metrics: &Metrics) {
    let Some(cmd) = ServerCommand::from_bytes(cmd_bytes) else {
        return;
    };

    match cmd.structured_data {
        ServerCommandData::SetOrigin { x, y } => {
            // Origin is the top-left of the visible grid; player sits at center.
            state.self_x = Some(x.wrapping_add(TILEX as i16 / 2));
            state.self_y = Some(y.wrapping_add(TILEY as i16 / 2));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_state_initial_values() {
        let s = ClientState::new();
        assert_eq!(s.client_ticker, 0);
        assert!(s.self_x.is_none());
        assert!(s.self_y.is_none());
        assert!(s.ping_times.is_empty());
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
}
