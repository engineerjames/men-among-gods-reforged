#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogMessageColor {
    Yellow,
    Green,
    Blue,
    Red,
}

#[allow(dead_code)]
pub struct LogMessage {
    pub timestamp: u64,
    pub message: String,
    pub color: LogMessageColor,
}
