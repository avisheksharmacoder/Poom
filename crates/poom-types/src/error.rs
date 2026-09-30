use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum TypeError {
    #[error("Invalid identifier: {0}")]
    InvalidId(String),

    #[error("Invalid timestamp range: start time ({start_nanos}) must be <= end time ({end_nanos})")]
    InvalidTimestampRange {
        start_nanos: u64,
        end_nanos: u64,
    },

    #[error("Entity name cannot be empty")]
    EmptyName,

    #[error("Invalid span kind: {0}")]
    InvalidSpanKind(String),
}
