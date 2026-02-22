/// Colour used to render a chat log message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogMessageColor {
    Yellow,
    Green,
    Blue,
    Red,
}

/// A single entry in the chat log buffer.
pub struct LogMessage {
    pub message: String,
    pub color: LogMessageColor,
}
