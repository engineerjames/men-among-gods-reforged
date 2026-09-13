//! Background KeyDB worker for operations initiated by the game tick loop.
//!
//! The worker owns the blocking KeyDB calls and returns owned results through
//! an in-process channel. The game loop remains the sole owner of `GameState`.

use super::{ban, connection};
use core::ban_store::{BanRecord, BanTarget};
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

/// Durable ban write submitted by the tick thread.
#[derive(Clone, Debug)]
pub struct BanWriteRequest {
    /// Character slot that issued the command, used for the completion message.
    pub issuer_character: usize,
    /// Durable operation to perform.
    pub action: BanWriteAction,
}

/// Durable ban operation handled by the KeyDB worker.
#[derive(Clone, Debug)]
pub enum BanWriteAction {
    /// Store or replace a durable ban record.
    Upsert(BanRecord),
    /// Remove a durable ban record.
    Remove(BanTarget),
}

/// KeyDB status entry written after an admin request is applied.
#[derive(Clone, Debug)]
pub enum AdminStatusKind {
    /// Template reload status.
    Templates,
    /// Text reload status.
    Text,
    /// Map patch status.
    MapPatch,
    /// Item patch status.
    ItemPatch,
    /// Character patch status.
    CharacterPatch,
}

/// Status update for an admin action whose mutation is owned by the tick thread.
pub enum ActionStatusRequest {
    /// Mark a world action as running.
    WorldRunning(core::world_action_store::WorldActionRequest),
    /// Mark a world action as applied.
    WorldApplied {
        /// Original world action request.
        request: core::world_action_store::WorldActionRequest,
        /// Human-readable outcome.
        message: String,
    },
    /// Mark a world action as failed.
    WorldFailed {
        /// Original world action request.
        request: core::world_action_store::WorldActionRequest,
        /// Failure detail.
        message: String,
    },
    /// Mark a live ban action as running.
    BanRunning(core::ban_action_store::BanActionRequest),
    /// Mark a live ban action as applied.
    BanApplied {
        /// Original live ban action request.
        request: core::ban_action_store::BanActionRequest,
        /// Human-readable outcome.
        message: String,
    },
}

/// Result payload from an admin reload read.
///
/// The worker owns the KeyDB data and transfers these vectors to the tick
/// thread so `GameState` remains single-threaded.
pub enum AdminReloadResult {
    /// Loaded template slices requested by the API.
    Templates {
        /// Reloaded item templates, when requested.
        items: Option<Vec<core::types::Item>>,
        /// Reloaded character templates, when requested.
        characters: Option<Vec<core::types::Character>>,
    },
    /// Loaded externally-managed bad-word text.
    BadWords(Vec<String>),
}

/// Result of a durable ban operation.
#[derive(Clone, Debug)]
pub enum BanWriteResult {
    /// Updated ban-store version returned by an upsert.
    Upserted(u64),
    /// Whether a record existed and was removed.
    Removed(bool),
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
    /// Persist or remove a durable ban record.
    BanWrite(BanWriteRequest),
    /// Load admin-managed data on the worker thread.
    AdminReload {
        /// Reload request to execute.
        request: AdminReloadRequest,
    },
    /// Write an admin request's applied status off the tick thread.
    AdminStatus {
        /// Status category.
        kind: AdminStatusKind,
        /// Opaque request identifier.
        request_id: String,
    },
    /// Write a world or live-ban action status off the tick thread.
    ActionStatus(ActionStatusRequest),
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
    /// Completion of a durable ban operation with its original context.
    BanWriteCompleted {
        /// Original request context.
        request: BanWriteRequest,
        /// Result returned by KeyDB.
        result: Result<BanWriteResult, String>,
    },
    /// Completion of an admin reload or status write.
    AdminCompleted {
        /// Operation label for diagnostics.
        operation: &'static str,
        /// Request identifier when applicable.
        request_id: String,
        /// Loaded data or write result.
        result: Result<Option<AdminReloadResult>, String>,
    },
    /// Completion of an admin action status write.
    ActionStatusCompleted {
        /// Request identifier for diagnostics.
        request_id: String,
        /// Result returned by KeyDB.
        result: Result<(), String>,
    },
}

/// Admin data load requested by the tick loop.
///
/// The request contains only watcher-owned data and can therefore cross the
/// worker channel without borrowing the server state.
pub enum AdminReloadRequest {
    /// Load item and/or character templates.
    Templates(super::template_reload::ReloadRequest),
    /// Load bad words.
    Text(super::text_reload::TextReloadRequest),
}

/// Cloneable request sender for KeyDB work initiated by the tick thread.
///
/// Keeping this sender in `GameState` lets gameplay code enqueue persistence
/// without taking ownership of the `Server` or waiting for KeyDB.
#[derive(Clone)]
pub struct TickKeyDbClient {
    tx: Sender<TickKeyDbRequest>,
}

impl TickKeyDbClient {
    /// Queue persistence of selection-screen metadata.
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

    /// Queue a durable ban operation.
    pub fn submit_ban_write(&self, request: BanWriteRequest) -> Result<(), BanWriteRequest> {
        self.tx
            .send(TickKeyDbRequest::BanWrite(request.clone()))
            .map_err(|_| request)
    }

    /// Queue an admin data load.
    pub fn submit_admin_reload(&self, request: AdminReloadRequest) -> Result<(), ()> {
        self.tx
            .send(TickKeyDbRequest::AdminReload { request })
            .map_err(|_| ())
    }

    /// Queue an admin applied-status write.
    pub fn submit_admin_status(&self, kind: AdminStatusKind, request_id: String) -> Result<(), ()> {
        self.tx
            .send(TickKeyDbRequest::AdminStatus { kind, request_id })
            .map_err(|_| ())
    }

    /// Queue an admin action status update.
    pub fn submit_action_status(&self, request: ActionStatusRequest) -> Result<(), ()> {
        self.tx
            .send(TickKeyDbRequest::ActionStatus(request))
            .map_err(|_| ())
    }
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

    /// Return a cloneable sender for non-blocking KeyDB requests.
    pub fn client(&self) -> TickKeyDbClient {
        TickKeyDbClient {
            tx: self.tx.clone(),
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
        self.client()
            .submit_selection_metadata(character_id, character)
    }

    /// Queue a durable ban operation without waiting for KeyDB.
    pub fn submit_ban_write(&self, request: BanWriteRequest) -> Result<(), BanWriteRequest> {
        self.client().submit_ban_write(request)
    }

    /// Queue an admin data load without waiting for KeyDB.
    pub fn submit_admin_reload(&self, request: AdminReloadRequest) -> Result<(), ()> {
        self.client().submit_admin_reload(request)
    }

    /// Queue an admin applied-status write without waiting for KeyDB.
    pub fn submit_admin_status(&self, kind: AdminStatusKind, request_id: String) -> Result<(), ()> {
        self.client().submit_admin_status(kind, request_id)
    }

    /// Queue an admin action status update without waiting for KeyDB.
    pub fn submit_action_status(&self, request: ActionStatusRequest) -> Result<(), ()> {
        self.client().submit_action_status(request)
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
            TickKeyDbRequest::BanWrite(request) => {
                let result = match &request.action {
                    BanWriteAction::Upsert(record) => {
                        ban::upsert_ban_record(record).map(BanWriteResult::Upserted)
                    }
                    BanWriteAction::Remove(target) => {
                        ban::remove_ban_target(target).map(BanWriteResult::Removed)
                    }
                };
                if tx
                    .send(TickKeyDbEvent::BanWriteCompleted { request, result })
                    .is_err()
                {
                    log::info!("KeyDB tick worker: event receiver dropped");
                    return;
                }
            }
            TickKeyDbRequest::AdminReload { request } => {
                let (request_id, result) = match request {
                    AdminReloadRequest::Templates(request) => {
                        let request_id = request.request_id.clone();
                        let result = load_templates(&request);
                        (request_id, result)
                    }
                    AdminReloadRequest::Text(request) => {
                        let request_id = request.request_id.clone();
                        let result = load_bad_words(&request);
                        (request_id, result)
                    }
                };
                if tx
                    .send(TickKeyDbEvent::AdminCompleted {
                        operation: "admin reload",
                        request_id,
                        result,
                    })
                    .is_err()
                {
                    log::info!("KeyDB tick worker: event receiver dropped");
                    return;
                }
            }
            TickKeyDbRequest::AdminStatus { kind, request_id } => {
                let result = write_admin_status(&kind, &request_id).map(|()| None);
                if tx
                    .send(TickKeyDbEvent::AdminCompleted {
                        operation: "admin status",
                        request_id,
                        result,
                    })
                    .is_err()
                {
                    log::info!("KeyDB tick worker: event receiver dropped");
                    return;
                }
            }
            TickKeyDbRequest::ActionStatus(request) => {
                let request_id = action_status_request_id(&request).to_owned();
                let result = write_action_status(request);
                if tx
                    .send(TickKeyDbEvent::ActionStatusCompleted { request_id, result })
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

fn load_templates(
    request: &super::template_reload::ReloadRequest,
) -> Result<Option<AdminReloadResult>, String> {
    let mut con = connection::connect()?;
    let items = if request.reload_items {
        Some(super::store::load_item_templates(&mut con)?)
    } else {
        None
    };
    let characters = if request.reload_characters {
        Some(super::store::load_character_templates(&mut con)?)
    } else {
        None
    };
    Ok(Some(AdminReloadResult::Templates { items, characters }))
}

fn load_bad_words(
    request: &super::text_reload::TextReloadRequest,
) -> Result<Option<AdminReloadResult>, String> {
    let mut con = connection::connect()?;
    if !request.reload_badwords {
        return Ok(None);
    }
    Ok(Some(AdminReloadResult::BadWords(
        super::store::load_bad_words(&mut con)?,
    )))
}

fn write_admin_status(kind: &AdminStatusKind, request_id: &str) -> Result<(), String> {
    let mut con = connection::connect()?;
    match kind {
        AdminStatusKind::Templates => {
            super::template_reload::write_applied_status(&mut con, request_id)
        }
        AdminStatusKind::Text => super::text_reload::write_applied_status(&mut con, request_id),
        AdminStatusKind::MapPatch => super::map_patch::write_applied_status(&mut con, request_id),
        AdminStatusKind::ItemPatch => super::item_patch::write_applied_status(&mut con, request_id),
        AdminStatusKind::CharacterPatch => {
            super::character_patch::write_applied_status(&mut con, request_id)
        }
    }
}

fn action_status_request_id(request: &ActionStatusRequest) -> &str {
    match request {
        ActionStatusRequest::WorldRunning(request)
        | ActionStatusRequest::WorldApplied { request, .. }
        | ActionStatusRequest::WorldFailed { request, .. } => &request.request_id,
        ActionStatusRequest::BanRunning(request)
        | ActionStatusRequest::BanApplied { request, .. } => &request.request_id,
    }
}

fn write_action_status(request: ActionStatusRequest) -> Result<(), String> {
    let mut con = connection::connect()?;
    match request {
        ActionStatusRequest::WorldRunning(request) => {
            super::world_action::write_running_status(&mut con, &request)
        }
        ActionStatusRequest::WorldApplied { request, message } => {
            super::world_action::write_applied_status(&mut con, &request, &message)
        }
        ActionStatusRequest::WorldFailed { request, message } => {
            super::world_action::write_failed_status(&mut con, &request, &message)
        }
        ActionStatusRequest::BanRunning(request) => {
            super::ban_action::write_running_status(&mut con, &request)
        }
        ActionStatusRequest::BanApplied { request, message } => {
            super::ban_action::write_applied_status(&mut con, &request, &message)
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
            TickKeyDbEvent::BanWriteCompleted { .. } => {
                panic!("expected a login result")
            }
            TickKeyDbEvent::AdminCompleted { .. } => {
                panic!("expected a login result")
            }
            TickKeyDbEvent::ActionStatusCompleted { .. } => {
                panic!("expected a login result")
            }
        }
    }
}
