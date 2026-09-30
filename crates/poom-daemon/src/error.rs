use thiserror::Error;

#[derive(Error, Debug)]
pub enum DaemonError {
    #[error("Transport error: {0}")]
    Transport(#[from] poom_transport::TransportError),

    #[error("Storage error: {0}")]
    Storage(#[from] poom_storage::StorageError),

    #[error("Protocol error: {0}")]
    Protocol(#[from] poom_protocol::ProtocolError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Daemon server error: {0}")]
    Server(String),
}
