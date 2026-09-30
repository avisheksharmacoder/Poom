use std::path::PathBuf;
use thiserror::Error;
use poom_protocol::ProtocolError;

#[derive(Error, Debug)]
pub enum TransportError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Protocol error: {0}")]
    Protocol(#[from] ProtocolError),

    #[error("IPC socket address already in use by an active daemon: {0}")]
    AddressInUse(PathBuf),

    #[error("Connection refused: {0}")]
    ConnectionRefused(String),

    #[error("IPC operation timed out: {0}")]
    Timeout(String),

    #[error("Client connection disconnected unexpectedly")]
    Disconnected,

    #[error("Invalid IPC path: {0}")]
    InvalidPath(String),
}
