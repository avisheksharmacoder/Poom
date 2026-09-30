use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use poom_storage::SpanNode;
use poom_types::{AttributeValue, EvaluationRecord, SpanMetrics, SpanRecord, TraceId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonSpanNode {
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub name: String,
    pub kind: String,
    pub start_time_unix_nanos: u64,
    pub end_time_unix_nanos: Option<u64>,
    pub duration_ms: Option<f64>,
    pub status: String,
    pub error_message: Option<String>,
    pub attributes: BTreeMap<String, Value>,
    pub metrics: SpanMetrics,
    pub events: Vec<JsonEventRecord>,
    pub children: Vec<JsonSpanNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonEventRecord {
    pub timestamp_unix_nanos: u64,
    pub name: String,
    pub attributes: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonTraceExport {
    pub version: String,
    pub exported_at_unix_nanos: u64,
    pub trace_id: String,
    pub root_span: JsonSpanNode,
    pub evaluations: Vec<EvaluationRecord>,
}

impl JsonSpanNode {
    pub fn from_span_node(node: &SpanNode) -> Self {
        let duration_ms = node.span.duration_nanos().map(|n| n as f64 / 1_000_000.0);
        let status_str = match &node.span.status {
            poom_types::SpanStatus::Ok => "Ok".to_string(),
            poom_types::SpanStatus::Error { error_type, .. } => format!("Error({error_type})"),
        };
        let error_message = match &node.span.status {
            poom_types::SpanStatus::Ok => None,
            poom_types::SpanStatus::Error { message, .. } => Some(message.to_string()),
        };

        let mut attrs_map = BTreeMap::new();
        for (k, v) in &node.span.attributes {
            attrs_map.insert(k.to_string(), attribute_to_json(v));
        }

        let events = node
            .span
            .events
            .iter()
            .map(|e| {
                let mut e_attrs = BTreeMap::new();
                for (k, v) in &e.attributes {
                    e_attrs.insert(k.to_string(), attribute_to_json(v));
                }
                JsonEventRecord {
                    timestamp_unix_nanos: e.timestamp_unix_nanos,
                    name: e.name.to_string(),
                    attributes: e_attrs,
                }
            })
            .collect();

        let children = node
            .children
            .iter()
            .map(JsonSpanNode::from_span_node)
            .collect();

        Self {
            span_id: node.span.span_id.to_string(),
            parent_span_id: node.span.parent_span_id.map(|id| id.to_string()),
            name: node.span.name.to_string(),
            kind: node.span.kind.as_str().to_string(),
            start_time_unix_nanos: node.span.start_time_unix_nanos,
            end_time_unix_nanos: node.span.end_time_unix_nanos,
            duration_ms,
            status: status_str,
            error_message,
            attributes: attrs_map,
            metrics: node.span.metrics.clone(),
            events,
            children,
        }
    }
}

pub fn attribute_to_json(attr: &AttributeValue) -> Value {
    match attr {
        AttributeValue::Null => Value::Null,
        AttributeValue::Bool(b) => Value::Bool(*b),
        AttributeValue::Int(i) => Value::Number((*i).into()),
        AttributeValue::Float(f) => serde_json::Number::from_f64(*f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        AttributeValue::String(s) => Value::String(s.to_string()),
        AttributeValue::Smol(s) => Value::String(s.to_string()),
        AttributeValue::Bytes(bytes) => {
            Value::String(format!("<{} bytes>", bytes.len()))
        }
        AttributeValue::Array(arr) => {
            Value::Array(arr.iter().map(attribute_to_json).collect())
        }
        AttributeValue::Map(map) => {
            let mut obj = serde_json::Map::new();
            for (k, v) in map {
                obj.insert(k.to_string(), attribute_to_json(v));
            }
            Value::Object(obj)
        }
    }
}

pub fn export_trace_to_json(
    trace_id: TraceId,
    tree: &SpanNode,
    evaluations: Vec<EvaluationRecord>,
) -> Result<String, serde_json::Error> {
    let now_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);

    let export = JsonTraceExport {
        version: "1.0".to_string(),
        exported_at_unix_nanos: now_nanos,
        trace_id: trace_id.to_string(),
        root_span: JsonSpanNode::from_span_node(tree),
        evaluations,
    };

    serde_json::to_string_pretty(&export)
}

pub fn export_spans_to_json(spans: &[SpanRecord]) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(spans)
}
