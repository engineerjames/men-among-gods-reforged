#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // TODO: Check usage and remove if unnecessary

/// Mirrors the original `LogoutReason` values used in the server protocol
/// and internal logic. These codes indicate why a client/character was
/// disconnected or removed.
pub enum LogoutReason {
    /// Unknown logout reason.
    Unknown = 0,
    /// Failed authentication challenge (bad login handshake).
    ChallengeFailed = 1,
    /// Disconnected due to inactivity / idle timeout.
    IdleTooLong = 2,
    /// Server full; no room for new connection.
    NoRoom = 3,
    /// Invalid parameters in the connection request.
    ParamsInvalid = 4,
    /// Non-active account disconnected.
    NonActive = 5,
    /// Incorrect password supplied.
    PasswordIncorrect = 6,
    /// Client is too slow / cannot keep up with server ticks.
    ClientTooSlow = 7,
    /// Generic failure / internal error caused logout.
    Failure = 8,
    /// Server shutdown initiated.
    Shutdown = 9,
    /// Logout requested due to tavern (special server mode).
    Tavern = 10,
    /// Client/server protocol version mismatch.
    VersionMismatch = 11,
    /// Normal exit initiated by the client.
    Exit = 12,
    /// Logout due to usurpation (another process took control).
    Usurp = 13,
    /// Player was kicked by an administrator.
    Kicked = 14,
}
