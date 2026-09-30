use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use poom_types::{AttributeValue, SpanKind, SpanRecord, SpanStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtlpExportPayload {
    #[serde(rename = "resourceSpans")]
    pub resource_spans: Vec<OtlpResourceSpans>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtlpResourceSpans {
    pub resource: OtlpResource,
    #[serde(rename = "scopeSpans")]
    pub scope_spans: Vec<OtlpScopeSpans>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtlpResource {
    pub attributes: Vec<OtlpKeyValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtlpScopeSpans {
    pub scope: OtlpScope,
    pub spans: Vec<OtlpSpan>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtlpScope {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtlpSpan {
    #[serde(rename = "traceId")]
    pub trace_id: String,
    #[serde(rename = "spanId")]
    pub span_id: String,
    #[serde(rename = "parentSpanId", skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<String>,
    pub name: String,
    pub kind: u32,
    #[serde(rename = "startTimeUnixNano")]
    pub start_time_unix_nano: String,
    #[serde(rename = "endTimeUnixNano", skip_serializing_if = "Option::is_none")]
    pub end_time_unix_nano: Option<String>,
    pub attributes: Vec<OtlpKeyValue>,
    pub status: OtlpStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtlpKeyValue {
    pub key: String,
    pub value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtlpStatus {
    pub code: u32, // 0 = UNSET, 1 = OK, 2 = ERROR
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl OtlpSpan {
    pub fn from_span_record(span: &SpanRecord) -> Self {
        // Hex representation of TraceId (32 hex chars) and SpanId (16 hex chars)
        let trace_id_hex = hex::encode(span.trace_id.as_bytes());
        let span_id_hex = hex::encode(&span.span_id.as_bytes()[..8]);
        let parent_id_hex = span
            .parent_span_id
            .map(|pid| hex::encode(&pid.as_bytes()[..8]));

        let kind = match span.kind {
            SpanKind::Http => 2,     // SERVER
            SpanKind::Tool => 3,     // CLIENT
            SpanKind::Agent => 1,    // INTERNAL
            SpanKind::Chain => 1,    // INTERNAL
            SpanKind::Llm => 3,      // CLIENT
            SpanKind::Function => 1, // INTERNAL
        };

        let (status_code, error_msg) = match &span.status {
            SpanStatus::Ok => (1, None),
            SpanStatus::Error { message, .. } => (2, Some(message.to_string())),
        };

        let mut attrs = Vec::new();
        for (k, v) in &span.attributes {
            attrs.push(OtlpKeyValue {
                key: k.to_string(),
                value: attribute_to_otlp_value(v),
            });
        }

        // Add standard metrics as OTel attributes
        if let Some(tokens) = span.metrics.input_tokens {
            attrs.push(OtlpKeyValue {
                key: "gen_ai.usage.prompt_tokens".to_string(),
                value: json!({ "intValue": tokens }),
            });
        }
        if let Some(tokens) = span.metrics.output_tokens {
            attrs.push(OtlpKeyValue {
                key: "gen_ai.usage.completion_tokens".to_string(),
                value: json!({ "intValue": tokens }),
            });
        }
        if let Some(cost) = span.metrics.estimated_cost_usd {
            attrs.push(OtlpKeyValue {
                key: "gen_ai.usage.cost_usd".to_string(),
                value: json!({ "doubleValue": cost }),
            });
        }

        Self {
            trace_id: trace_id_hex,
            span_id: span_id_hex,
            parent_span_id: parent_id_hex,
            name: span.name.to_string(),
            kind,
            start_time_unix_nano: span.start_time_unix_nanos.to_string(),
            end_time_unix_nano: span.end_time_unix_nanos.map(|t| t.to_string()),
            attributes: attrs,
            status: OtlpStatus {
                code: status_code,
                message: error_msg,
            },
        }
    }
}

pub fn attribute_to_otlp_value(attr: &AttributeValue) -> Value {
    match attr {
        AttributeValue::Null => Value::Null,
        AttributeValue::String(s) => json!({ "stringValue": s.as_str() }),
        AttributeValue::Smol(s) => json!({ "stringValue": s.as_str() }),
        AttributeValue::Int(i) => json!({ "intValue": i }),
        AttributeValue::Float(f) => json!({ "doubleValue": f }),
        AttributeValue::Bool(b) => json!({ "boolValue": b }),
        AttributeValue::Bytes(b) => json!({ "bytesValue": hex::encode(b) }),
        AttributeValue::Array(arr) => {
            let values: Vec<Value> = arr.iter().map(attribute_to_otlp_value).collect();
            json!({ "arrayValue": { "values": values } })
        }
        AttributeValue::Map(m) => {
            let kv_list: Vec<Value> = m
                .iter()
                .map(|(k, v)| {
                    json!({
                        "key": k.as_str(),
                        "value": attribute_to_otlp_value(v)
                    })
                })
                .collect();
            json!({ "kvlistValue": { "values": kv_list } })
        }
    }
}

pub fn export_spans_to_otlp(
    service_name: &str,
    spans: &[SpanRecord],
) -> Result<String, serde_json::Error> {
    let otlp_spans: Vec<OtlpSpan> = spans.iter().map(OtlpSpan::from_span_record).collect();

    let payload = OtlpExportPayload {
        resource_spans: vec![OtlpResourceSpans {
            resource: OtlpResource {
                attributes: vec![
                    OtlpKeyValue {
                        key: "service.name".to_string(),
                        value: json!({ "stringValue": service_name }),
                    },
                    OtlpKeyValue {
                        key: "telemetry.sdk.name".to_string(),
                        value: json!({ "stringValue": "poom" }),
                    },
                    OtlpKeyValue {
                        key: "telemetry.sdk.language".to_string(),
                        value: json!({ "stringValue": "rust" }),
                    },
                ],
            },
            scope_spans: vec![OtlpScopeSpans {
                scope: OtlpScope {
                    name: "poom-tracer".to_string(),
                    version: "0.1.0".to_string(),
                },
                spans: otlp_spans,
            }],
        }],
    };

    serde_json::to_string_pretty(&payload)
}

mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
}
