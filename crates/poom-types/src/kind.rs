use std::fmt;
use std::str::FromStr;
use serde::{Deserialize, Serialize};

use crate::error::TypeError;

/// Specific operational category of a Span.
///
/// Marked with `#[repr(u8)]` so binary serialization formats like `postcard`
/// encode the discriminant as a single compact byte.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[repr(u8)]
pub enum SpanKind {
    /// High-level autonomous agent loop or multi-step reasoning workflow.
    Agent = 1,
    /// Sequential pipeline, DAG step, or retrieval chain.
    Chain = 2,
    /// Direct model inference call (prompts, completions, tokens, temperature).
    Llm = 3,
    /// Isolated tool or function invocation (arguments, results, schema).
    Tool = 4,
    /// Arbitrary application code block, utility, or database query.
    Function = 5,
    /// Incoming HTTP/ASGI boundary request.
    Http = 6,
}

impl SpanKind {
    /// Returns the static string representation of the span kind.
    #[inline]
    pub const fn as_str(&self) -> &'static str {
        match self {
            SpanKind::Agent => "agent",
            SpanKind::Chain => "chain",
            SpanKind::Llm => "llm",
            SpanKind::Tool => "tool",
            SpanKind::Function => "function",
            SpanKind::Http => "http",
        }
    }
}

impl fmt::Display for SpanKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for SpanKind {
    type Err = TypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "agent" => Ok(SpanKind::Agent),
            "chain" => Ok(SpanKind::Chain),
            "llm" => Ok(SpanKind::Llm),
            "tool" => Ok(SpanKind::Tool),
            "function" => Ok(SpanKind::Function),
            "http" => Ok(SpanKind::Http),
            _ => Err(TypeError::InvalidSpanKind(s.to_string())),
        }
    }
}

impl Default for SpanKind {
    #[inline]
    fn default() -> Self {
        SpanKind::Function
    }
}
