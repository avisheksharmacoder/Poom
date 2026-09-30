use thiserror::Error;

#[derive(Error, Debug)]
pub enum CliError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Storage error: {0}")]
    Storage(#[from] poom_storage::StorageError),

    #[error("Daemon error: {0}")]
    Daemon(#[from] poom_daemon::DaemonError),

    #[error("Database error: {0}")]
    Database(#[from] redb::DatabaseError),

    #[error("Transaction error: {0}")]
    Transaction(#[from] redb::TransactionError),

    #[error("Table error: {0}")]
    Table(#[from] redb::TableError),

    #[error("Redb storage error: {0}")]
    RedbStorage(#[from] redb::StorageError),

    #[error("Type error: {0}")]
    Type(#[from] poom_types::TypeError),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("UUID parse error: {0}")]
    Uuid(#[from] uuid::Error),

    #[error("Span not found: {0}")]
    SpanNotFound(String),

    #[error("Trace not found: {0}")]
    TraceNotFound(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("GUI error: {0}")]
    Gui(String),
}

pub type CliResult<T> = Result<T, CliError>;
