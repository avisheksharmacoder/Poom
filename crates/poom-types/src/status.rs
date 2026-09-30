use smol_str::SmolStr;
use serde::{Deserialize, Serialize};

/// Execution status of a Span.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpanStatus {
    /// Execution succeeded normally.
    Ok,
    /// Execution failed with error details.
    Error {
        /// Small error type or class name (e.g. "ValueError", "TimeoutError").
        /// Stored inline on stack without heap allocation if <= 23 bytes.
        error_type: SmolStr,
        /// Detailed human-readable error description.
        message: String,
        /// Optional stack trace or traceback (Python or Rust).
        backtrace: Option<String>,
    },
}

impl SpanStatus {
    /// Constructs an error status with an error type and message.
    pub fn error(error_type: impl Into<SmolStr>, message: impl Into<String>) -> Self {
        Self::Error {
            error_type: error_type.into(),
            message: message.into(),
            backtrace: None,
        }
    }

    /// Constructs an error status with an error type, message, and backtrace.
    pub fn error_with_backtrace(
        error_type: impl Into<SmolStr>,
        message: impl Into<String>,
        backtrace: impl Into<String>,
    ) -> Self {
        Self::Error {
            error_type: error_type.into(),
            message: message.into(),
            backtrace: Some(backtrace.into()),
        }
    }

    /// Returns `true` if the status is `Ok`.
    #[inline]
    pub const fn is_ok(&self) -> bool {
        matches!(self, Self::Ok)
    }

    /// Returns `true` if the status is `Error`.
    #[inline]
    pub const fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }
}

impl Default for SpanStatus {
    #[inline]
    fn default() -> Self {
        Self::Ok
    }
}
