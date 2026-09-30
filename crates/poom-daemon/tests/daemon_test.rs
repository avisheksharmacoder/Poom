use std::sync::Arc;
use std::time::Duration;
use futures_util::{SinkExt, StreamExt};
use tempfile::tempdir;
use tokio::time::sleep;

use poom_daemon::{
    DaemonConfig, EnrichmentEngine, LazyTokenizer, PoomDaemon, PricingEngine,
};
use poom_protocol::{ClientMessage, ServerMessage};
use poom_storage::StorageReader;
use poom_transport::IpcClient;
use poom_types::{SpanKind, SpanRecord, TraceId};

#[test]
fn test_pricing_engine_accuracy_and_normalization() {
    let pricing = PricingEngine::new();

    // Verify model normalization
    assert_eq!(pricing.normalize_model_name("openai/gpt-4o-2024-08-06"), "gpt-4o");
    assert_eq!(pricing.normalize_model_name("GPT-4O-MINI"), "gpt-4o-mini");
    assert_eq!(pricing.normalize_model_name("anthropic/claude-3-5-sonnet-20241022"), "claude-3-5-sonnet");
    assert_eq!(pricing.normalize_model_name("gemini-1.5-flash-latest"), "gemini-1.5-flash");
    assert_eq!(pricing.normalize_model_name("deepseek-ai/deepseek-r1"), "deepseek-r1");

    // Exact cost verification for GPT-4o
    // 1,000 input (200 cached, 800 uncached) + 500 output
    // cached: 200 * $1.25 / 1M = $0.00025
    // uncached input: 800 * $2.50 / 1M = $0.00200
    // output: 500 * $10.00 / 1M = $0.00500
    // total: $0.00725
    let cost = pricing.calculate_cost("gpt-4o", 1000, 500, Some(200)).unwrap();
    assert!((cost - 0.00725).abs() < 1e-8, "Calculated cost {cost} must match $0.00725");
}

#[test]
fn test_lazy_tokenizer_counting() {
    let text = "Hello world! This is a test prompt for tokenization.";
    let tokens = LazyTokenizer::count_tokens("gpt-4o", text);
    assert!(tokens >= 8 && tokens <= 15, "Expected ~10-12 tokens, got {tokens}");

    // Non-OpenAI fallback estimator
    let claude_tokens = LazyTokenizer::count_tokens("claude-3-5-sonnet", text);
    assert!(claude_tokens >= 8, "Fallback token count must be positive");
}

#[test]
fn test_enrichment_engine() {
    let enrichment = EnrichmentEngine::new();
    let trace_id = TraceId::generate();

    let mut span = SpanRecord::root(trace_id, "chat_call", SpanKind::Llm)
        .with_attribute("model", "gpt-4o")
        .with_attribute("prompt", "Translate the following sentence to Spanish: The quick brown fox jumps over the lazy dog.")
        .with_attribute("completion", "El rápido zorro marrón salta sobre el perro perezoso.");

    assert!(span.metrics.input_tokens.is_none());
    assert!(span.metrics.estimated_cost_usd.is_none());

    enrichment.enrich_span(&mut span, true);

    assert!(span.metrics.input_tokens.is_some());
    assert!(span.metrics.output_tokens.is_some());
    assert!(span.metrics.total_tokens.is_some());
    assert!(span.metrics.estimated_cost_usd.is_some());

    let cost = span.metrics.estimated_cost_usd.unwrap();
    assert!(cost > 0.0, "Enriched cost must be positive");
}

#[tokio::test]
async fn test_daemon_end_to_end_ingestion() {
    let dir = tempdir().unwrap();
    let sock_path = dir.path().join("daemon_e2e.sock");
    let db_path = dir.path().join("daemon_e2e.redb");

    let config = DaemonConfig::new(&sock_path, &db_path);
    let daemon = Arc::new(PoomDaemon::start(config).await.expect("Failed to start daemon"));

    // Spawn daemon accept loop
    let daemon_clone = Arc::clone(&daemon);
    tokio::spawn(async move {
        let _ = daemon_clone.run().await;
    });

    // Wait for socket to bind
    sleep(Duration::from_millis(50)).await;

    // Connect client
    let mut client = IpcClient::connect(&sock_path).await.expect("Client failed to connect");

    // 1. Handshake
    client.send(ClientMessage::Handshake {
        client_version: "0.1.0".to_string(),
        pid: 9999,
        app_name: Some("integration_test".to_string()),
    }).await.unwrap();

    let ack = client.next().await.unwrap().unwrap();
    assert!(matches!(ack, ServerMessage::HandshakeAck { .. }));

    // 2. Ingest Spans
    let trace_id = TraceId::generate();
    let mut spans = Vec::new();
    for i in 0..5 {
        let span = SpanRecord::root(trace_id, format!("llm_span_{i}"), SpanKind::Llm)
            .with_attribute("model", "gpt-4o")
            .with_attribute("prompt", "What is the capital of France?")
            .with_attribute("completion", "Paris.");
        spans.push(span);
    }

    client.send(ClientMessage::IngestSpans(spans)).await.unwrap();
    let ingest_ack = client.next().await.unwrap().unwrap();
    assert_eq!(
        ingest_ack,
        ServerMessage::IngestAck {
            spans_accepted: 5,
            evaluations_accepted: 0,
        }
    );

    // 3. Ping
    client.send(ClientMessage::Ping { timestamp_nanos: 42 }).await.unwrap();
    let pong = client.next().await.unwrap().unwrap();
    assert_eq!(pong, ServerMessage::Pong { timestamp_nanos: 42 });

    // 4. Disconnect
    client.send(ClientMessage::Disconnect).await.unwrap();

    // Allow batch writer to flush to disk
    sleep(Duration::from_millis(150)).await;

    // Verify stored spans in redb
    let reader = StorageReader::new(daemon.storage().database());
    let stored_spans = reader.get_trace_spans(trace_id).unwrap();

    assert_eq!(stored_spans.len(), 5);
    for span in stored_spans {
        assert!(span.metrics.input_tokens.is_some(), "Tokens must be enriched");
        assert!(span.metrics.estimated_cost_usd.is_some(), "Cost must be enriched");
    }
}

#[tokio::test]
async fn test_live_ui_broadcast() {
    let dir = tempdir().unwrap();
    let sock_path = dir.path().join("daemon_broadcast.sock");
    let db_path = dir.path().join("daemon_broadcast.redb");

    let config = DaemonConfig::new(&sock_path, &db_path);
    let daemon = Arc::new(PoomDaemon::start(config).await.expect("Failed to start daemon"));

    // Subscribe to broadcast channel
    let mut rx = daemon.broadcaster().subscribe();

    let daemon_clone = Arc::clone(&daemon);
    tokio::spawn(async move {
        let _ = daemon_clone.run().await;
    });

    sleep(Duration::from_millis(50)).await;

    let mut client = IpcClient::connect(&sock_path).await.unwrap();

    let trace_id = TraceId::generate();
    let span = SpanRecord::root(trace_id, "broadcast_test_span", SpanKind::Function);

    client.send(ClientMessage::IngestSpans(vec![span.clone()])).await.unwrap();

    // Verify subscriber receives the live span
    let received_live_span = tokio::time::timeout(Duration::from_millis(500), rx.recv())
        .await
        .expect("Broadcast timeout")
        .expect("Broadcast receive failed");

    assert_eq!(received_live_span.name, "broadcast_test_span");
    assert_eq!(received_live_span.trace_id, trace_id);
}
