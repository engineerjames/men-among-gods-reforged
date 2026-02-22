pub mod client_commands;
mod login;
pub mod server_commands;
pub mod tick_stream;

use std::collections::HashMap;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use client_commands::ClientCommand;

use crate::network::client_commands::ClientCommandType;

/// Commands sent from the main thread to the background network thread.
pub enum NetworkCommand {
    /// Raw bytes to write to the TCP stream.
    Send(Vec<u8>),
    /// Request a graceful disconnect.
    Shutdown,
}

/// Events produced by the background network thread for consumption by the
/// main loop.
pub enum NetworkEvent {
    Status(String),
    Bytes {
        bytes: Vec<u8>,
        received_at: Instant,
    },
    /// One complete framed server tick packet was processed.
    Tick,
    Error(String),
    #[allow(dead_code)]
    NewPlayerCredentials {
        // TODO: Can we remove this?
        _user_id: u32,
        _pass1: u32,
        _pass2: u32,
    },
    LoggedIn,
}

/// Manages the background network thread and its communication channels.
///
/// Owned directly by `AppState` as `Option<NetworkRuntime>`.
pub struct NetworkRuntime {
    command_tx: Option<mpsc::Sender<NetworkCommand>>,
    event_rx: Option<mpsc::Receiver<NetworkEvent>>,
    thread_handle: Option<std::thread::JoinHandle<()>>,

    pub client_ticker: u32,
    pub last_ctick_sent: u32,
    pub logged_in: bool,

    start_instant: Instant,
    pub ping_seq: u32,
    pub last_ping_sent_at: Option<Instant>,
    pings_in_flight: HashMap<u32, Instant>,
    pub last_rtt_ms: Option<u32>,
    pub rtt_ewma_ms: Option<f32>,
}

impl NetworkRuntime {
    /// Creates and starts the network runtime, spawning the background thread.
    pub fn new(host: String, port: u16, ticket: u64, race: i32) -> Self {
        let (command_tx, command_rx) = mpsc::channel::<NetworkCommand>();
        let (event_tx, event_rx) = mpsc::channel::<NetworkEvent>();

        let handle = std::thread::spawn(move || {
            login::run_network_task(host, port, ticket, race, command_rx, event_tx);
        });

        Self {
            command_tx: Some(command_tx),
            event_rx: Some(event_rx),
            thread_handle: Some(handle),
            client_ticker: 0,
            last_ctick_sent: 0,
            logged_in: false,
            start_instant: Instant::now(),
            ping_seq: 0,
            last_ping_sent_at: None,
            pings_in_flight: HashMap::new(),
            last_rtt_ms: None,
            rtt_ewma_ms: None,
        }
    }

    /// Serialises `cmd`, logs it at DEBUG level, and queues the bytes for the network thread.
    pub fn send(&self, cmd: ClientCommand) {
        if let Some(tx) = &self.command_tx {
            if cmd.header == ClientCommandType::CmdCTick || cmd.header == ClientCommandType::Ping {
                log::trace!("Sending command: {:?}", cmd);
            } else {
                log::info!("Sending command: {}", cmd.get_description());
            }
            let _ = tx.send(NetworkCommand::Send(cmd.to_bytes()));
        }
    }

    /// Requests a graceful shutdown and joins the background thread.
    pub fn shutdown(&mut self) {
        if let Some(tx) = self.command_tx.take() {
            let _ = tx.send(NetworkCommand::Shutdown);
        }
        self.event_rx = None;
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }

    /// Non-blocking poll for the next network event.
    pub fn try_recv(&mut self) -> Option<NetworkEvent> {
        self.event_rx.as_ref()?.try_recv().ok()
    }

    /// Returns milliseconds elapsed since the runtime was created.
    pub fn elapsed_ms(&self) -> u32 {
        self.start_instant
            .elapsed()
            .as_millis()
            .min(u128::from(u32::MAX)) as u32
    }

    /// Records a received pong and updates RTT tracking.
    pub fn handle_pong(&mut self, seq: u32, received_at: Instant) {
        if let Some(sent_at) = self.pings_in_flight.remove(&seq) {
            let rtt_ms = received_at
                .duration_since(sent_at)
                .as_millis()
                .min(u128::from(u32::MAX)) as u32;
            self.last_rtt_ms = Some(rtt_ms);
            self.rtt_ewma_ms = Some(match self.rtt_ewma_ms {
                Some(prev) => prev * 0.8 + (rtt_ms as f32) * 0.2,
                None => rtt_ms as f32,
            });
            log::debug!("Ping RTT: {} ms (seq={})", rtt_ms, seq);
        }
    }

    /// Sends a `CL_PING` if the interval has elapsed and we're not over the
    /// in-flight limit. Handles its own timing and sequence numbering.
    pub fn maybe_send_ping(&mut self) {
        const PING_INTERVAL: Duration = Duration::from_secs(5);
        const PING_TIMEOUT: Duration = Duration::from_secs(30);
        const MAX_IN_FLIGHT: usize = 3;

        if self.client_ticker == 0 {
            return;
        }

        let now = Instant::now();
        self.pings_in_flight
            .retain(|_, sent_at| now.duration_since(*sent_at) <= PING_TIMEOUT);

        if self.pings_in_flight.len() >= MAX_IN_FLIGHT {
            return;
        }

        if let Some(last) = self.last_ping_sent_at {
            if now.duration_since(last) < PING_INTERVAL {
                return;
            }
        }

        let client_time_ms = self.elapsed_ms();
        self.ping_seq = self.ping_seq.wrapping_add(1);
        let seq = self.ping_seq;

        self.last_ping_sent_at = Some(now);
        self.pings_in_flight.insert(seq, now);

        let cmd = client_commands::ClientCommand::new_ping(seq, client_time_ms);
        self.send(cmd);
    }

    /// Sends `CL_CMD_CTICK` every 16 processed server ticks if we're logged in.
    pub fn maybe_send_ctick(&mut self) {
        if !self.logged_in {
            return;
        }
        let t = self.client_ticker;
        if t == 0 {
            return;
        }
        if (t & 15) != 0 {
            return;
        }
        if self.last_ctick_sent == t {
            return;
        }
        let tick_cmd = client_commands::ClientCommand::new_tick(t);
        self.send(tick_cmd);
        self.last_ctick_sent = t;
    }
}
