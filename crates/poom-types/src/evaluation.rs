use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::id::{SpanId, TraceId};

/// Value container for an evaluation assessment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EvaluationValue {
    Boolean(bool),
    Numeric(f64),
    Categorical(SmolStr),
}

impl From<bool> for EvaluationValue {
    #[inline]
    fn from(v: bool) -> Self {
        Self::Boolean(v)
    }
}

impl From<f64> for EvaluationValue {
    #[inline]
    fn from(v: f64) -> Self {
        Self::Numeric(v)
    }
}

impl From<f32> for EvaluationValue {
    #[inline]
    fn from(v: f32) -> Self {
        Self::Numeric(v as f64)
    }
}

impl From<&str> for EvaluationValue {
    #[inline]
    fn from(v: &str) -> Self {
        Self::Categorical(SmolStr::new(v))
    }
}

impl From<SmolStr> for EvaluationValue {
    #[inline]
    fn from(v: SmolStr) -> Self {
        Self::Categorical(v)
    }
}

/// A qualitative assessment or quantitative score attached to a Trace or Span.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluationRecord {
    /// Unique 128-bit time-ordered ID for this evaluation record.
    pub evaluation_id: SpanId,
    /// Associated trace ID.
    pub trace_id: TraceId,
    /// Optional target span ID (if evaluating a specific span instead of the full trace).
    pub span_id: Option<SpanId>,
    /// Name or metric key (e.g. "hallucination", "user_feedback", "relevance").
    pub name: SmolStr,
    /// Score or assessment outcome.
    pub value: EvaluationValue,
    /// Optional human feedback notes or explanation.
    pub comment: Option<String>,
    /// Creation timestamp in Unix nanoseconds.
    pub timestamp_unix_nanos: u64,
}

impl EvaluationRecord {
    /// Constructs a new evaluation record with the current timestamp.
    pub fn new(
        trace_id: TraceId,
        span_id: Option<SpanId>,
        name: impl Into<SmolStr>,
        value: impl Into<EvaluationValue>,
    ) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        Self {
            evaluation_id: SpanId::generate(),
            trace_id,
            span_id,
            name: name.into(),
            value: value.into(),
            comment: None,
            timestamp_unix_nanos: nanos,
        }
    }

    /// Attaches an explanatory comment.
    pub fn with_comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = Some(comment.into());
        self
    }
}
