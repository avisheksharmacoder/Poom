use thiserror::Error;
use poom_types::{SpanId, TraceId};

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Database error: {0}")]
    RedbDatabase(#[from] redb::DatabaseError),

    #[error("Transaction error: {0}")]
    RedbTransaction(#[from] redb::TransactionError),

    #[error("Table error: {0}")]
    RedbTable(#[from] redb::TableError),

    #[error("Commit error: {0}")]
    RedbCommit(#[from] redb::CommitError),

    #[error("Storage engine error: {0}")]
    RedbStorage(#[from] redb::StorageError),

    #[error("Postcard serialization error: {0}")]
    Postcard(#[from] postcard::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Corrupt record: {0}")]
    CorruptRecord(String),

    #[error("Span not found: {0}")]
    SpanNotFound(SpanId),

    #[error("Trace not found: {0}")]
    TraceNotFound(TraceId),
}
