#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]

/// These codes indicate why a client/character was
/// disconnected or removed.
pub enum LogoutReason {
    /// Unknown / internal-only; never sent over the wire.
    Unknown = 0,
    /// Failed authentication challenge (bad login handshake). `LO_CHALLENGE = 1`
    ChallengeFailed = 1,
    /// Disconnected due to inactivity / idle timeout. `LO_IDLE = 2`
    IdleTooLong = 2,
    /// Server full; no room for new connection. `LO_NOROOM = 3`
    NoRoom = 3,
    /// Invalid parameters in the connection request. `LO_PARAMS = 4`
    ParamsInvalid = 4,
    /// Character slot is non-active. `LO_NONACTIVE = 5`
    NonActive = 5,
    /// Incorrect password supplied. `LO_PASSWORD = 6`
    PasswordIncorrect = 6,
    /// Client is too slow / cannot keep up with server ticks. `LO_SLOW = 7`
    ClientTooSlow = 7,
    /// Generic failure / internal error caused logout. `LO_FAILURE = 8`
    Failure = 8,
    /// Server shutdown initiated. `LO_SHUTDOWN = 9`
    Shutdown = 9,
    /// Logout requested due to tavern (special server mode). `LO_TAVERN = 10`
    Tavern = 10,
    /// Client/server protocol version mismatch. `LO_VERSION = 11`
    VersionMismatch = 11,
    /// Normal exit initiated by the client. `LO_EXIT = 12`
    Exit = 12,
    /// Logout due to usurpation (another process took control). `LO_USURP = 13`
    /// Not transmitted over the wire; used as an internal sentinel in `plr_logout`.
    Usurp = 13,
    /// Player was kicked by an administrator. `LO_KICKED = 14`
    Kicked = 14,
}

impl From<u8> for LogoutReason {
    fn from(value: u8) -> Self {
        match value {
            1 => LogoutReason::ChallengeFailed,
            2 => LogoutReason::IdleTooLong,
            3 => LogoutReason::NoRoom,
            4 => LogoutReason::ParamsInvalid,
            5 => LogoutReason::NonActive,
            6 => LogoutReason::PasswordIncorrect,
            7 => LogoutReason::ClientTooSlow,
            8 => LogoutReason::Failure,
            9 => LogoutReason::Shutdown,
            10 => LogoutReason::Tavern,
            11 => LogoutReason::VersionMismatch,
            12 => LogoutReason::Exit,
            13 => LogoutReason::Usurp,
            14 => LogoutReason::Kicked,
            _ => LogoutReason::Unknown,
        }
    }
}

/// Returns a human-readable description of a server exit/logout reason code.
///
/// # Arguments
///
/// * `reason` - The `LogoutReason` code to describe.
///
/// # Returns
///
/// * A bracketed label and description, e.g. `"[IDLE] Player idle too long"`.
pub fn get_exit_reason(reason: LogoutReason) -> &'static str {
    match reason {
        LogoutReason::ChallengeFailed => "[CHALLENGE] Challenge failure",
        LogoutReason::IdleTooLong => "[IDLE] Player idle too long",
        LogoutReason::NoRoom => "[NOROOM] No room left on server",
        LogoutReason::ParamsInvalid => "[PARAMS] Invalid parameters",
        LogoutReason::NonActive => "[NONACTIVE] Player not active",
        LogoutReason::PasswordIncorrect => "[PASSWORD] Invalid password",
        LogoutReason::ClientTooSlow => "[SLOW] Connection too slow",
        LogoutReason::Failure => "[FAILURE] Login failure",
        LogoutReason::Shutdown => "[SHUTDOWN] Server shutting down",
        LogoutReason::Tavern => "[TAVERN] Returned to tavern",
        LogoutReason::VersionMismatch => "[VERSION] Version mismatch",
        LogoutReason::Exit => "[EXIT] Client exit",
        LogoutReason::Usurp => "[USURP] Logged in elsewhere",
        LogoutReason::Kicked => "[KICKED] Kicked from server",
        _ => "[UNKNOWN] Unrecognized reason code",
    }
}
