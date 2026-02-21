#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogMessageColor {
    Yellow,
    Green,
    Blue,
    Red,
}

pub struct LogMessage {
    pub message: String,
    pub color: LogMessageColor,
}
