use std::time::Duration;
use tempfile::tempdir;
use tokio::time::sleep;

use poom_storage::{
    commit_batch, BatchWriter, StorageEngine, StoragePruner, StorageReader,
};
use poom_types::{
    EvaluationRecord, SpanKind, SpanMetrics, SpanRecord, TraceId,
};

#[test]
fn test_insert_and_point_lookup() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_point.redb");
    let engine = StorageEngine::open(&db_path).unwrap();

    let trace_id = TraceId::generate();
    let span = SpanRecord::root(trace_id, "llm_inference", SpanKind::Llm)
        .with_attribute("model", "gpt-4o")
        .with_attribute("temperature", 0.2)
        .with_metrics(SpanMetrics::new().with_tokens(500, 120).with_cost(0.005));

    let span_id = span.span_id;

    commit_batch(engine.database(), &[span.clone()], &[]).unwrap();

    let reader = StorageReader::new(engine.database());
    let retrieved = reader.get_span(span_id).unwrap().expect("Span must exist");

    assert_eq!(retrieved.name, "llm_inference");
    assert_eq!(retrieved.kind, SpanKind::Llm);
    assert_eq!(retrieved.metrics.total_tokens, Some(620));
    assert_eq!(retrieved.attributes.get("model").and_then(|v| v.as_str()), Some("gpt-4o"));
}

#[test]
fn test_trace_spans_and_tree_reconstruction() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_tree.redb");
    let engine = StorageEngine::open(&db_path).unwrap();

    let trace_id = TraceId::generate();

    // Build hierarchy:
    // Root (Agent)
    //  ├── Chain 1
    //  │    ├── Tool Call
    //  │    └── LLM Call
    //  └── Chain 2
    //       └── LLM Call 2
    let root = SpanRecord::root(trace_id, "root_agent", SpanKind::Agent);
    let mut chain1 = SpanRecord::child(trace_id, root.span_id, "chain_1", SpanKind::Chain);
    chain1.start_time_unix_nanos = root.start_time_unix_nanos + 10;

    let mut tool = SpanRecord::child(trace_id, chain1.span_id, "search_tool", SpanKind::Tool);
    tool.start_time_unix_nanos = root.start_time_unix_nanos + 20;

    let mut llm1 = SpanRecord::child(trace_id, chain1.span_id, "summarize_llm", SpanKind::Llm);
    llm1.start_time_unix_nanos = root.start_time_unix_nanos + 30;

    let mut chain2 = SpanRecord::child(trace_id, root.span_id, "chain_2", SpanKind::Chain);
    chain2.start_time_unix_nanos = root.start_time_unix_nanos + 40;

    let mut llm2 = SpanRecord::child(trace_id, chain2.span_id, "final_llm", SpanKind::Llm);
    llm2.start_time_unix_nanos = root.start_time_unix_nanos + 50;

    let all_spans = vec![root.clone(), chain1.clone(), tool.clone(), llm1.clone(), chain2.clone(), llm2.clone()];
    commit_batch(engine.database(), &all_spans, &[]).unwrap();

    let reader = StorageReader::new(engine.database());

    // 1. Verify all 6 spans fetched
    let retrieved_spans = reader.get_trace_spans(trace_id).unwrap();
    assert_eq!(retrieved_spans.len(), 6);

    // 2. Verify tree reconstruction
    let tree = reader.get_trace_tree(trace_id).unwrap().expect("Tree must exist");
    assert_eq!(tree.span.name, "root_agent");
    assert_eq!(tree.children.len(), 2); // Chain 1 and Chain 2
    assert_eq!(tree.children[0].span.name, "chain_1");
    assert_eq!(tree.children[0].children.len(), 2); // Tool and LLM 1
    assert_eq!(tree.children[1].span.name, "chain_2");
    assert_eq!(tree.children[1].children.len(), 1); // LLM 2
    assert_eq!(tree.total_nodes(), 6);
}

#[test]
fn test_reverse_chronological_pagination() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_pagination.redb");
    let engine = StorageEngine::open(&db_path).unwrap();

    let mut root_spans = Vec::new();
    let base_time = 1_000_000_000u64;

    for i in 0..20 {
        let trace_id = TraceId::generate();
        let mut span = SpanRecord::root(trace_id, format!("trace_{i}"), SpanKind::Function);
        span.start_time_unix_nanos = base_time + (i as u64 * 1_000_000); // 1ms step
        root_spans.push(span);
    }

    commit_batch(engine.database(), &root_spans, &[]).unwrap();

    let reader = StorageReader::new(engine.database());

    // Page 1: top 10 recent
    let page1 = reader.list_recent_traces(10, 0).unwrap();
    assert_eq!(page1.len(), 10);
    for window in page1.windows(2) {
        assert!(window[0].0 > window[1].0, "Must be descending order");
    }

    // Page 2: next 10 recent
    let page2 = reader.list_recent_traces(10, 10).unwrap();
    assert_eq!(page2.len(), 10);
    assert!(page1.last().unwrap().0 > page2.first().unwrap().0);
}

#[tokio::test]
async fn test_batch_writer_dual_triggers() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_batch_writer.redb");
    let engine = StorageEngine::open(&db_path).unwrap();

    let writer = BatchWriter::start(engine.database().clone());

    let trace_id = TraceId::generate();
    let spans: Vec<SpanRecord> = (0..25)
        .map(|i| SpanRecord::root(trace_id, format!("writer_span_{i}"), SpanKind::Function))
        .collect();

    // Send 25 items (well below 1,000 size trigger)
    writer.send_spans(spans).await.unwrap();

    // Sleep 80ms to exceed the 50ms timeout trigger
    sleep(Duration::from_millis(80)).await;

    let reader = StorageReader::new(engine.database());
    let retrieved = reader.get_trace_spans(trace_id).unwrap();
    assert_eq!(retrieved.len(), 25);
}

#[test]
fn test_ttl_pruner_cascading_deletion() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_pruner.redb");
    let engine = StorageEngine::open(&db_path).unwrap();

    // Insert 3 old traces (timestamp 100)
    let mut old_spans = Vec::new();
    let mut old_evals = Vec::new();
    for _ in 0..3 {
        let trace_id = TraceId::generate();
        let mut root = SpanRecord::root(trace_id, "old_trace", SpanKind::Function);
        root.start_time_unix_nanos = 100;
        let eval = EvaluationRecord::new(trace_id, None, "score", 0.5);
        old_spans.push(root);
        old_evals.push(eval);
    }

    // Insert 2 new traces (timestamp 500)
    let mut new_spans = Vec::new();
    for _ in 0..2 {
        let trace_id = TraceId::generate();
        let mut root = SpanRecord::root(trace_id, "new_trace", SpanKind::Function);
        root.start_time_unix_nanos = 500;
        new_spans.push(root);
    }

    commit_batch(engine.database(), &old_spans, &old_evals).unwrap();
    commit_batch(engine.database(), &new_spans, &[]).unwrap();

    let pruner = StoragePruner::new(engine.database());
    let pruned_count = pruner.prune_older_than(200).unwrap();
    assert_eq!(pruned_count, 3);

    let reader = StorageReader::new(engine.database());
    let remaining = reader.list_recent_traces(10, 0).unwrap();
    assert_eq!(remaining.len(), 2);
    for (ts, _) in remaining {
        assert_eq!(ts, 500);
    }
}

#[test]
fn test_tag_filtering() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_tags.redb");
    let engine = StorageEngine::open(&db_path).unwrap();

    let trace_prod = TraceId::generate();
    let span_prod = SpanRecord::root(trace_prod, "prod_call", SpanKind::Function)
        .with_attribute("env", "production");

    let trace_staging = TraceId::generate();
    let span_staging = SpanRecord::root(trace_staging, "staging_call", SpanKind::Function)
        .with_attribute("env", "staging");

    commit_batch(engine.database(), &[span_prod, span_staging], &[]).unwrap();

    let reader = StorageReader::new(engine.database());
    let prod_traces = reader.find_traces_by_tag("env", "production", 10).unwrap();
    assert_eq!(prod_traces, vec![trace_prod]);

    let staging_traces = reader.find_traces_by_tag("env", "staging", 10).unwrap();
    assert_eq!(staging_traces, vec![trace_staging]);
}
