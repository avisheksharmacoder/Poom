use std::path::PathBuf;
use poom_transport::default_ipc_path;

/// Configuration parameters for the Poom Ingestion Daemon.
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    /// Filesystem path to the local IPC socket or named pipe.
    pub socket_path: PathBuf,
    /// Filesystem path to the embedded redb database file.
    pub db_path: PathBuf,
    /// Maximum capacity of the in-memory live UI event broadcast ring buffer.
    pub broadcast_capacity: usize,
    /// Automatically tokenize text if LLM spans lack token metrics.
    pub auto_tokenize: bool,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let default_db_path = PathBuf::from(home).join(".poom").join("data.redb");

        Self {
            socket_path: default_ipc_path(),
            db_path: default_db_path,
            broadcast_capacity: 4096,
            auto_tokenize: true,
        }
    }
}

impl DaemonConfig {
    /// Constructs a configuration pointing to explicit paths.
    pub fn new(socket_path: impl Into<PathBuf>, db_path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: socket_path.into(),
            db_path: db_path.into(),
            broadcast_capacity: 4096,
            auto_tokenize: true,
        }
    }
}
