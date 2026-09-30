use std::str::FromStr;
use std::thread::sleep;
use std::time::Duration;

use poom_types::*;

#[test]
fn test_id_generation_and_ordering() {
    let id1 = TraceId::generate();
    sleep(Duration::from_millis(5));
    let id2 = TraceId::generate();

    assert!(id2 > id1, "UUIDv7 TraceIds must be chronologically ordered");
    assert!(id2.timestamp_millis() >= id1.timestamp_millis());

    let bytes = id1.to_bytes();
    let reconstructed = TraceId::from_bytes(bytes);
    assert_eq!(id1, reconstructed);

    let str_repr = id1.to_string();
    let parsed = TraceId::from_str(&str_repr).expect("Must parse UUID string");
    assert_eq!(id1, parsed);
}

#[test]
fn test_span_id_and_trace_id_separation() {
    let trace_id = TraceId::generate();
    let span_id = SpanId::generate();

    assert_eq!(trace_id.to_bytes().len(), 16);
    assert_eq!(span_id.to_bytes().len(), 16);
}

#[test]
fn test_span_kind_string_roundtrip() {
    let kinds = [
        (SpanKind::Agent, "agent"),
        (SpanKind::Chain, "chain"),
        (SpanKind::Llm, "llm"),
        (SpanKind::Tool, "tool"),
        (SpanKind::Function, "function"),
        (SpanKind::Http, "http"),
    ];

    for (kind, repr) in kinds {
        assert_eq!(kind.as_str(), repr);
        assert_eq!(SpanKind::from_str(repr).unwrap(), kind);
        assert_eq!(SpanKind::from_str(&repr.to_uppercase()).unwrap(), kind);
    }
}

#[test]
fn test_span_status() {
    let ok = SpanStatus::Ok;
    assert!(ok.is_ok());
    assert!(!ok.is_error());

    let err = SpanStatus::error_with_backtrace("ValueError", "Invalid token range", "traceback here");
    assert!(err.is_error());
    assert!(!err.is_ok());
}

#[test]
fn test_attribute_value_conversions() {
    let int_val: AttributeValue = 42i64.into();
    assert_eq!(int_val.as_i64(), Some(42));
    assert_eq!(int_val.as_f64(), Some(42.0));

    let float_val: AttributeValue = 3.14159f64.into();
    assert_eq!(float_val.as_f64(), Some(3.14159));

    let str_val: AttributeValue = "hello world".into();
    assert_eq!(str_val.as_str(), Some("hello world"));

    let bool_val: AttributeValue = true.into();
    assert_eq!(bool_val.as_bool(), Some(true));

    // Test JSON conversion
    let json: serde_json::Value = serde_json::json!({
        "model": "gpt-4o",
        "temperature": 0.7,
        "tags": ["prod", "us-east"]
    });
    let attr_val: AttributeValue = json.into();
    if let AttributeValue::Map(map) = &attr_val {
        assert_eq!(map.get("model").and_then(|v| v.as_str()), Some("gpt-4o"));
    } else {
        panic!("Expected Map attribute value");
    }
}

#[test]
fn test_metrics_builder() {
    let metrics = SpanMetrics::new()
        .with_tokens(500, 150)
        .with_cached_tokens(200)
        .with_reasoning_tokens(64)
        .with_cost(0.0125);

    assert_eq!(metrics.input_tokens, Some(500));
    assert_eq!(metrics.output_tokens, Some(150));
    assert_eq!(metrics.total_tokens, Some(650));
    assert_eq!(metrics.cached_tokens, Some(200));
    assert_eq!(metrics.reasoning_tokens, Some(64));
    assert_eq!(metrics.estimated_cost_usd, Some(0.0125));
}

#[test]
fn test_span_lifecycle_and_validation() {
    let trace_id = TraceId::generate();
    let mut root = SpanRecord::root(trace_id, "agent_workflow", SpanKind::Agent);
    assert!(root.is_root());
    assert_eq!(root.name, "agent_workflow");
    assert!(root.end_time_unix_nanos.is_none());

    let child = SpanRecord::child(trace_id, root.span_id, "llm_inference", SpanKind::Llm);
    assert!(!child.is_root());
    assert_eq!(child.parent_span_id, Some(root.span_id));

    // Finish span
    root.finish(root.start_time_unix_nanos + 1_500_000_000); // 1.5s
    assert_eq!(root.duration_nanos(), Some(1_500_000_000));
    assert_eq!(root.duration_millis(), Some(1500.0));

    assert!(root.validate().is_ok());

    // Validation failure: inverted timestamps
    let invalid_span = SpanRecord {
        start_time_unix_nanos: 200,
        end_time_unix_nanos: Some(100),
        ..root.clone()
    };
    assert!(invalid_span.validate().is_err());
}

#[test]
fn test_span_postcard_binary_roundtrip() {
    let trace_id = TraceId::generate();
    let span = SpanRecord::root(trace_id, "llm_call", SpanKind::Llm)
        .with_attribute("model", "gpt-4o")
        .with_attribute("temperature", 0.7)
        .with_metrics(SpanMetrics::new().with_tokens(1024, 256).with_cost(0.008))
        .with_event(EventRecord::now("first_token_received"));

    let bytes = postcard::to_stdvec(&span).expect("Postcard serialization must succeed");
    assert!(!bytes.is_empty());

    let deserialized: SpanRecord = postcard::from_bytes(&bytes).expect("Postcard deserialization must succeed");
    assert_eq!(span, deserialized);
}
