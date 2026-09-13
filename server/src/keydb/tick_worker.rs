//! Background KeyDB worker for operations initiated by the game tick loop.
//!
//! The worker owns the blocking KeyDB calls and returns owned results through
//! an in-process channel. The game loop remains the sole owner of `GameState`.

use super::{ban, connection};
use core::ban_store::BanTarget;
use core::types::{CharacterSummary, api::GameLoginTicketMetadata};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};

/// Owned data needed to resolve one API login attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoginRequest {
    /// Monotonically increasing request identifier assigned by the server.
    pub request_id: u64,
    /// Player slot that initiated the request.
    pub player_id: usize,
    /// Generation of the player slot when the request was submitted.
    pub session_generation: u64,
    /// One-time API login ticket to consume.
    pub ticket: u64,
    /// IPv4 address associated with the connection.
    pub address: u32,
}

/// Data returned after KeyDB successfully resolves an API login.
#[derive(Clone, Debug)]
pub struct LoginResolution {
    /// Ticket metadata issued by the API service.
    pub ticket: GameLoginTicketMetadata,
    /// API-side character record selected by the ticket.
    pub character: CharacterSummary,
    /// Latest MOTD when available; `None` means the cached server value should be used.
    pub motd: Option<String>,
}

/// Reason a login resolution failed in the worker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoginFailureKind {
    /// The ticket was absent or expired.
    TicketMissing,
    /// The API-side character record was absent.
    CharacterMissing,
    /// A durable account, character, or IPv4 ban matched.
    Banned,
    /// KeyDB access or decoding failed.
    KeyDb,
}

/// Structured failure returned for one login request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoginFailure {
    /// Category used by the tick thread to select the client-facing outcome.
    pub kind: LoginFailureKind,
    /// Diagnostic detail suitable for server logs.
    pub message: String,
}

/// Request sent to the KeyDB worker.
pub enum TickKeyDbRequest {
    /// Resolve an API login without touching `GameState`.
    Login(LoginRequest),
    /// Persist the internal gameplay slot for an API character.
    SetCharacterServerId {
        /// API-side character identifier.
        character_id: u64,
        /// Internal server character slot.
        server_id: u32,
    },
    /// Persist selection-screen metadata for an API character.
    SyncCharacterSelectionMetadata {
        /// API-side character identifier.
        character_id: u64,
        /// Gameplay character whose metadata should be mirrored.
        character: core::types::Character,
    },
    /// Stop the worker after all earlier requests have been received.
    Shutdown,
}

/// Event returned by the KeyDB worker.
pub enum TickKeyDbEvent {
    /// Result of one login resolution request.
    LoginResolved {
        /// Original request identity used for stale-result checks.
        request: LoginRequest,
        /// Resolution result produced by the worker.
        result: Result<LoginResolution, LoginFailure>,
    },
    /// Completion of an asynchronous write request.
    WriteCompleted {
        /// Human-readable operation label.
        operation: &'static str,
        /// Result returned by KeyDB.
        result: Result<(), String>,
    },
}

/// Handle for the KeyDB worker used by the server tick loop.
pub struct TickKeyDbWorker {
    tx: Sender<TickKeyDbRequest>,
    rx: Receiver<TickKeyDbEvent>,
    handle: Option<JoinHandle<()>>,
}

impl TickKeyDbWorker {
    /// Spawn a worker thread with a private request and event channel.
    ///
    /// # Returns
    ///
    /// * A live worker handle.
    ///
    /// # Panics
    ///
    /// Panics if the operating system cannot create the worker thread.
    pub fn spawn() -> Self {
        let (request_tx, request_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let handle = thread::Builder::new()
            .name("keydb-tick-worker".to_owned())
            .spawn(move || worker_loop(request_rx, event_tx))
            .expect("failed to spawn KeyDB tick worker");

        Self {
            tx: request_tx,
            rx: event_rx,
            handle: Some(handle),
        }
    }

    /// Queue a login resolution without waiting for KeyDB.
    ///
    /// # Arguments
    ///
    /// * `request` - Owned login request to resolve on the worker thread.
    ///
    /// # Returns
    ///
    /// * `Ok(())` when the request was queued.
    /// * `Err(request)` when the worker has already stopped.
    pub fn submit_login(&self, request: LoginRequest) -> Result<(), LoginRequest> {
        self.tx
            .send(TickKeyDbRequest::Login(request.clone()))
            .map_err(|_| request)
    }

    /// Queue persistence of an API character's internal server slot.
    ///
    /// # Arguments
    ///
    /// * `character_id` - API-side character identifier.
    /// * `server_id` - Internal gameplay character slot.
    ///
    /// # Returns
    ///
    /// * `Ok(())` when the request was queued.
    /// * `Err(())` when the worker has already stopped.
    pub fn submit_server_id(&self, character_id: u64, server_id: u32) -> Result<(), ()> {
        self.tx
            .send(TickKeyDbRequest::SetCharacterServerId {
                character_id,
                server_id,
            })
            .map_err(|_| ())
    }

    /// Queue persistence of selection-screen metadata.
    ///
    /// # Arguments
    ///
    /// * `character_id` - API-side character identifier.
    /// * `character` - Gameplay character containing authoritative metadata.
    ///
    /// # Returns
    ///
    /// * `Ok(())` when the request was queued.
    /// * `Err(())` when the worker has already stopped.
    pub fn submit_selection_metadata(
        &self,
        character_id: u64,
        character: core::types::Character,
    ) -> Result<(), ()> {
        self.tx
            .send(TickKeyDbRequest::SyncCharacterSelectionMetadata {
                character_id,
                character,
            })
            .map_err(|_| ())
    }

    /// Receive one completed worker event without blocking.
    ///
    /// # Returns
    ///
    /// * `Some(event)` when a result is ready.
    /// * `None` when no result is ready or the worker has stopped.
    pub fn try_recv(&self) -> Option<TickKeyDbEvent> {
        match self.rx.try_recv() {
            Ok(event) => Some(event),
            Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => None,
        }
    }

    /// Stop the worker and wait for it to finish.
    ///
    /// Shutdown is idempotent. Requests already accepted by the worker are
    /// processed before the shutdown marker because the channel is FIFO.
    pub fn shutdown(&mut self) {
        if self.handle.is_none() {
            return;
        }

        let _ = self.tx.send(TickKeyDbRequest::Shutdown);
        if let Some(handle) = self.handle.take()
            && let Err(error) = handle.join()
        {
            log::error!("KeyDB tick worker thread panicked: {error:?}");
        }
    }
}

impl Drop for TickKeyDbWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn worker_loop(rx: Receiver<TickKeyDbRequest>, tx: Sender<TickKeyDbEvent>) {
    while let Ok(request) = rx.recv() {
        match request {
            TickKeyDbRequest::Login(request) => {
                let event = TickKeyDbEvent::LoginResolved {
                    request: request.clone(),
                    result: resolve_login(&request),
                };
                if tx.send(event).is_err() {
                    log::info!("KeyDB tick worker: event receiver dropped");
                    return;
                }
            }
            TickKeyDbRequest::SetCharacterServerId {
                character_id,
                server_id,
            } => {
                let result = connection::set_character_server_id(character_id, server_id);
                if tx
                    .send(TickKeyDbEvent::WriteCompleted {
                        operation: "set character server id",
                        result,
                    })
                    .is_err()
                {
                    log::info!("KeyDB tick worker: event receiver dropped");
                    return;
                }
            }
            TickKeyDbRequest::SyncCharacterSelectionMetadata {
                character_id,
                character,
            } => {
                let result =
                    connection::sync_character_selection_metadata(character_id, &character);
                if tx
                    .send(TickKeyDbEvent::WriteCompleted {
                        operation: "sync character selection metadata",
                        result,
                    })
                    .is_err()
                {
                    log::info!("KeyDB tick worker: event receiver dropped");
                    return;
                }
            }
            TickKeyDbRequest::Shutdown => return,
        }
    }
}

fn resolve_login(request: &LoginRequest) -> Result<LoginResolution, LoginFailure> {
    let ticket =
        connection::consume_login_ticket(request.ticket).map_err(|message| LoginFailure {
            kind: LoginFailureKind::KeyDb,
            message,
        })?;
    let ticket = ticket.ok_or_else(|| LoginFailure {
        kind: LoginFailureKind::TicketMissing,
        message: "API login ticket not found or expired".to_owned(),
    })?;

    let character = connection::load_character(ticket.character_id)
        .map_err(|message| LoginFailure {
            kind: LoginFailureKind::KeyDb,
            message,
        })?
        .ok_or_else(|| LoginFailure {
            kind: LoginFailureKind::CharacterMissing,
            message: format!("API character {} not found", ticket.character_id),
        })?;

    let targets = [
        BanTarget::Account {
            account_id: ticket.account_id,
        },
        BanTarget::Character {
            character_id: ticket.character_id,
        },
        BanTarget::Ipv4 {
            address: request.address,
        },
    ];

    for target in targets {
        let banned = ban::target_is_banned(&target).map_err(|message| LoginFailure {
            kind: LoginFailureKind::KeyDb,
            message,
        })?;
        if banned {
            return Err(LoginFailure {
                kind: LoginFailureKind::Banned,
                message: format!("login denied by {} ban {}", target.scope(), target.value()),
            });
        }
    }

    let motd = match connection::load_message_of_the_day() {
        Ok(value) => Some(value),
        Err(error) => {
            log::warn!("KeyDB MOTD read failed during login resolution: {error}");
            None
        }
    };

    Ok(LoginResolution {
        ticket,
        character,
        motd,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_request_preserves_stale_result_identity() {
        let request = LoginRequest {
            request_id: 7,
            player_id: 3,
            session_generation: 11,
            ticket: 99,
            address: 0x0102_0304,
        };

        let event = TickKeyDbEvent::LoginResolved {
            request: request.clone(),
            result: Err(LoginFailure {
                kind: LoginFailureKind::TicketMissing,
                message: "expired".to_owned(),
            }),
        };

        match event {
            TickKeyDbEvent::LoginResolved {
                request: returned,
                result: Err(failure),
            } => {
                assert_eq!(returned, request);
                assert_eq!(failure.kind, LoginFailureKind::TicketMissing);
            }
            TickKeyDbEvent::LoginResolved { result: Ok(_), .. } => {
                panic!("expected a failed login result")
            }
            TickKeyDbEvent::WriteCompleted { .. } => {
                panic!("expected a login result")
            }
        }
    }
}
