use thiserror::Error;

#[derive(Debug, Error)]
pub enum UiError {
    #[error("Storage engine error: {0}")]
    Storage(#[from] poom_storage::StorageError),

    #[error("Iced UI runtime error: {0}")]
    Iced(String),

    #[error("Trace not found: {0}")]
    TraceNotFound(String),

    #[error("Channel error: {0}")]
    Channel(String),
}
