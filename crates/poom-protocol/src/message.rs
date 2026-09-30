use serde::{Deserialize, Serialize};
use poom_types::{EvaluationRecord, SpanRecord};

/// Top-level messages dispatched from an instrumented client to the Poom daemon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClientMessage {
    /// Initial connection handshake with client metadata.
    Handshake {
        client_version: String,
        pid: u32,
        app_name: Option<String>,
    },
    /// High-throughput batch ingestion of spans.
    IngestSpans(Vec<SpanRecord>),
    /// Ingestion of trace or span evaluation assessments.
    IngestEvaluations(Vec<EvaluationRecord>),
    /// Keep-alive heartbeat and round-trip latency probe.
    Ping {
        timestamp_nanos: u64,
    },
    /// Graceful client disconnect notice.
    Disconnect,
}

/// Top-level responses and control messages dispatched from the Poom daemon to clients.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ServerMessage {
    /// Handshake acknowledgment establishing session parameters.
    HandshakeAck {
        server_version: String,
        negotiated_protocol_version: u16,
        max_batch_size: u32,
    },
    /// Acknowledgment confirming persistence or buffering of ingested records.
    IngestAck {
        spans_accepted: u32,
        evaluations_accepted: u32,
    },
    /// Heartbeat response echoing the client's ping timestamp.
    Pong {
        timestamp_nanos: u64,
    },
    /// Rejection notice or fatal protocol violation error.
    Error {
        code: u16,
        reason: String,
    },
}
