#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]

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
    /// Incorrect password supplied.
    PasswordIncorrect = 5,
    /// Client is too slow / cannot keep up with server ticks.
    ClientTooSlow = 6,
    /// Generic failure / internal error caused logout.
    Failure = 7,
    /// Server shutdown initiated.
    Shutdown = 8,
    /// Logout requested due to tavern (special server mode).
    Tavern = 9,
    /// Client/server protocol version mismatch.
    VersionMismatch = 10,
    /// Normal exit initiated by the client.
    Exit = 11,
    /// Logout due to usurpation (another process took control).
    Usurp = 12,
    /// Player was kicked by an administrator.
    Kicked = 13,
}
