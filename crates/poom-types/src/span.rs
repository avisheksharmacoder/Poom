use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::error::TypeError;
use crate::event::EventRecord;
use crate::id::{SpanId, TraceId};
use crate::kind::SpanKind;
use crate::metrics::SpanMetrics;
use crate::status::SpanStatus;
use crate::value::AttributeValue;

/// The primary timed execution unit in Poom.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpanRecord {
    /// Root execution trace boundary identifier.
    pub trace_id: TraceId,
    /// Unique identifier for this discrete span.
    pub span_id: SpanId,
    /// Parent span ID if this is a nested sub-operation; `None` if this is the root span.
    pub parent_span_id: Option<SpanId>,
    /// Semantic identifier or function name.
    pub name: SmolStr,
    /// Operational category (Agent, Chain, LLM, Tool, Function, Http).
    pub kind: SpanKind,
    /// Execution start timestamp in Unix nanoseconds.
    pub start_time_unix_nanos: u64,
    /// Execution finish timestamp in Unix nanoseconds (`None` if currently executing).
    pub end_time_unix_nanos: Option<u64>,
    /// Execution status outcome (Ok / Error).
    pub status: SpanStatus,
    /// Contextual attributes and metadata, canonically ordered by key.
    pub attributes: BTreeMap<SmolStr, AttributeValue>,
    /// Instantaneous point-in-time events/logs recorded during execution.
    pub events: Vec<EventRecord>,
    /// Token counts, latencies, and financial cost metrics.
    pub metrics: SpanMetrics,
}

impl SpanRecord {
    /// Starts a root span for a given trace.
    pub fn root(trace_id: TraceId, name: impl Into<SmolStr>, kind: SpanKind) -> Self {
        let nanos = current_unix_nanos();
        Self {
            trace_id,
            span_id: SpanId::generate(),
            parent_span_id: None,
            name: name.into(),
            kind,
            start_time_unix_nanos: nanos,
            end_time_unix_nanos: None,
            status: SpanStatus::Ok,
            attributes: BTreeMap::new(),
            events: Vec::new(),
            metrics: SpanMetrics::default(),
        }
    }

    /// Starts a child span adopting an existing trace ID and parent span ID.
    pub fn child(
        trace_id: TraceId,
        parent_span_id: SpanId,
        name: impl Into<SmolStr>,
        kind: SpanKind,
    ) -> Self {
        let nanos = current_unix_nanos();
        Self {
            trace_id,
            span_id: SpanId::generate(),
            parent_span_id: Some(parent_span_id),
            name: name.into(),
            kind,
            start_time_unix_nanos: nanos,
            end_time_unix_nanos: None,
            status: SpanStatus::Ok,
            attributes: BTreeMap::new(),
            events: Vec::new(),
            metrics: SpanMetrics::default(),
        }
    }

    /// Returns `true` if this is a top-level root span without a parent.
    #[inline]
    pub const fn is_root(&self) -> bool {
        self.parent_span_id.is_none()
    }

    /// Returns the span duration in nanoseconds if completed.
    pub fn duration_nanos(&self) -> Option<u64> {
        self.end_time_unix_nanos
            .map(|end| end.saturating_sub(self.start_time_unix_nanos))
    }

    /// Returns the span duration in milliseconds if completed.
    pub fn duration_millis(&self) -> Option<f64> {
        self.duration_nanos().map(|ns| ns as f64 / 1_000_000.0)
    }

    /// Marks the span as completed with the current system time.
    pub fn finish_now(&mut self) {
        self.end_time_unix_nanos = Some(current_unix_nanos());
    }

    /// Marks the span as completed with an explicit nanosecond timestamp.
    pub fn finish(&mut self, end_time_unix_nanos: u64) {
        self.end_time_unix_nanos = Some(end_time_unix_nanos);
    }

    /// Validates internal integrity invariants.
    pub fn validate(&self) -> Result<(), TypeError> {
        if self.name.is_empty() {
            return Err(TypeError::EmptyName);
        }

        if let Some(end) = self.end_time_unix_nanos {
            if end < self.start_time_unix_nanos {
                return Err(TypeError::InvalidTimestampRange {
                    start_nanos: self.start_time_unix_nanos,
                    end_nanos: end,
                });
            }
        }

        Ok(())
    }

    /// Fluent builder helper to insert an attribute.
    pub fn with_attribute(
        mut self,
        key: impl Into<SmolStr>,
        val: impl Into<AttributeValue>,
    ) -> Self {
        self.attributes.insert(key.into(), val.into());
        self
    }

    /// Fluent builder helper to attach metrics.
    pub fn with_metrics(mut self, metrics: SpanMetrics) -> Self {
        self.metrics = metrics;
        self
    }

    /// Fluent builder helper to set status.
    pub fn with_status(mut self, status: SpanStatus) -> Self {
        self.status = status;
        self
    }

    /// Fluent builder helper to add an event.
    pub fn with_event(mut self, event: EventRecord) -> Self {
        self.events.push(event);
        self
    }
}

#[inline]
fn current_unix_nanos() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}
