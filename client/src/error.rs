//! Shared error type across every RESP-over-x transport (BLE, TCP, ...).

#[derive(thiserror::Error, Debug)]
pub enum LinkError {
    #[error("Bluetooth error: {0}")]
    Btleplug(#[from] btleplug::Error),
    #[error("No Bluetooth adapter found")]
    NoAdapter,
    #[error("Target device not found")]
    DeviceNotFound,
    #[error("NUS characteristic '{0}' not found on device")]
    CharacteristicNotFound(&'static str),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Communication timed out")]
    Timeout,
    #[error("Channel closed")]
    ChannelClosed,
    #[error("Protocol error: {0}")]
    Protocol(String),
}
