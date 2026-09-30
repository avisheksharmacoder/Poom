use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::tempdir;

use poom_cli::analytics::{calculate_percentiles, AnalyticsSummary};
use poom_cli::commands::diff::execute_diff;
use poom_cli::commands::export::execute_export;
use poom_cli::commands::prune::execute_prune;
use poom_cli::commands::stats::execute_stats;
use poom_cli::commands::{DiffField, ExportFormat};
use poom_cli::diff::engine::DiffResult;
use poom_cli::diff::render_terminal_diff;
use poom_cli::export::json::export_trace_to_json;
use poom_cli::export::otlp::export_spans_to_otlp;
use poom_storage::{assemble_span_tree, commit_batch};
use poom_types::{
    AttributeValue, EvaluationRecord, EvaluationValue, EventRecord, SpanId, SpanKind, SpanMetrics,
    SpanRecord, SpanStatus, TraceId,
};

#[test]
fn test_diff_engine_line_and_inline_words() {
    let old_text = "You are a helpful financial assistant.\nProvide a 3-bullet summary of Apple revenue.\nInclude gross margin.\n";
    let new_text = "You are a helpful equity assistant.\nProvide a 5-bullet summary of Apple revenue.\nInclude gross margin.\nInclude operating risks.\n";

    let diff = DiffResult::compute(old_text, new_text);

    assert_eq!(diff.unchanged, 1); // line 3 ("Include gross margin.") is the only unchanged line
    assert!(diff.additions >= 2);
    assert!(diff.deletions >= 1);
    assert!(diff.similarity_ratio > 0.5);

    // Verify inline chunks exist on modified lines
    let has_inline = diff.lines.iter().any(|l| !l.inline_chunks.is_empty());
    assert!(has_inline, "Expected inline word chunks for adjacent modifications");

    // Test ANSI terminal diff output
    let mut output = Vec::new();
    render_terminal_diff(&mut output, &diff, Some("Test Diff")).expect("Render failed");
    let output_str = String::from_utf8(output).expect("Invalid utf8");
    assert!(output_str.contains("Test Diff"));
    assert!(output_str.contains("Similarity:"));
}

#[test]
fn test_analytics_metrics_and_percentiles() {
    let mut durations = vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0];
    let percentiles = calculate_percentiles(&mut durations);

    assert_eq!(percentiles.count, 10);
    assert_eq!(percentiles.min_ms, 10.0);
    assert_eq!(percentiles.max_ms, 100.0);
    assert_eq!(percentiles.avg_ms, 55.0);
    assert_eq!(percentiles.p50_ms, 60.0);
    assert_eq!(percentiles.p90_ms, 90.0);

    // Test AnalyticsSummary with Spans
    let trace_id = TraceId::generate();
    let span1 = SpanRecord {
        trace_id,
        span_id: SpanId::generate(),
        parent_span_id: None,
        name: "LlmCall1".into(),
        kind: SpanKind::Llm,
        start_time_unix_nanos: 1_000_000,
        end_time_unix_nanos: Some(11_000_000), // 10ms
        status: SpanStatus::Ok,
        attributes: {
            let mut m = BTreeMap::new();
            m.insert("gen_ai.request.model".into(), AttributeValue::from("gpt-4o"));
            m
        },
        events: vec![],
        metrics: SpanMetrics::new().with_tokens(1000, 200).with_cost(0.005),
    };

    let span2 = SpanRecord {
        trace_id,
        span_id: SpanId::generate(),
        parent_span_id: None,
        name: "LlmCall2".into(),
        kind: SpanKind::Llm,
        start_time_unix_nanos: 20_000_000,
        end_time_unix_nanos: Some(50_000_000), // 30ms
        status: SpanStatus::error("RateLimitError", "Quota exceeded"),
        attributes: {
            let mut m = BTreeMap::new();
            m.insert("gen_ai.request.model".into(), AttributeValue::from("gpt-4o"));
            m
        },
        events: vec![],
        metrics: SpanMetrics::new().with_tokens(500, 100).with_cost(0.0025),
    };

    let summary = AnalyticsSummary::compute(&[span1, span2]);
    assert_eq!(summary.total_spans, 2);
    assert_eq!(summary.ok_spans, 1);
    assert_eq!(summary.error_spans, 1);
    assert_eq!(summary.errors_by_type.get("RateLimitError"), Some(&1));
    assert_eq!(summary.total_cost_usd, 0.0075);

    let gpt4o_vel = summary.model_tokens.get("gpt-4o").unwrap();
    assert_eq!(gpt4o_vel.input_tokens, 1500);
    assert_eq!(gpt4o_vel.output_tokens, 300);
    assert_eq!(gpt4o_vel.call_count, 2);
}

#[test]
fn test_export_json_and_otlp_fidelity() {
    let trace_id = TraceId::generate();
    let root_id = SpanId::generate();
    let child_id = SpanId::generate();

    let root_span = SpanRecord {
        trace_id,
        span_id: root_id,
        parent_span_id: None,
        name: "AgentRoot".into(),
        kind: SpanKind::Agent,
        start_time_unix_nanos: 1_000_000,
        end_time_unix_nanos: Some(101_000_000),
        status: SpanStatus::Ok,
        attributes: {
            let mut m = BTreeMap::new();
            m.insert("workflow".into(), AttributeValue::from("research"));
            m
        },
        events: vec![EventRecord::with_timestamp(2_000_000, "init")],
        metrics: SpanMetrics::new(),
    };

    let child_span = SpanRecord {
        trace_id,
        span_id: child_id,
        parent_span_id: Some(root_id),
        name: "ToolChild".into(),
        kind: SpanKind::Tool,
        start_time_unix_nanos: 5_000_000,
        end_time_unix_nanos: Some(50_000_000),
        status: SpanStatus::Ok,
        attributes: {
            let mut m = BTreeMap::new();
            m.insert("query".into(), AttributeValue::from("search"));
            m
        },
        events: vec![],
        metrics: SpanMetrics::new(),
    };

    let tree = assemble_span_tree(vec![root_span.clone(), child_span.clone()]).expect("Tree assemble failed");
    let eval = EvaluationRecord::new(trace_id, Some(root_id), "quality", EvaluationValue::Numeric(0.95));

    // Test JSON export
    let json_export = export_trace_to_json(trace_id, &tree, vec![eval]).expect("Export to JSON failed");
    assert!(json_export.contains("AgentRoot"));
    assert!(json_export.contains("ToolChild"));
    assert!(json_export.contains("quality"));
    assert!(json_export.contains("0.95"));

    // Test OTLP export
    let otlp_export = export_spans_to_otlp("test_service", &[root_span, child_span]).expect("Export to OTLP failed");
    assert!(otlp_export.contains("resourceSpans"));
    assert!(otlp_export.contains("test_service"));
    assert!(otlp_export.contains("AgentRoot"));
    assert!(otlp_export.contains("ToolChild"));
}

#[test]
fn test_cli_stats_prune_and_diff_commands() {
    let dir = tempdir().expect("Failed to create tempdir");
    let db_path = dir.path().join("test_data.redb");

    let now_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;

    // Seed test traces
    let db = redb::Database::create(&db_path).expect("Failed to create redb");

    let trace_id = TraceId::generate();
    let span1_id = SpanId::generate();
    let span2_id = SpanId::generate();

    let old_timestamp = now_nanos - (20 * 86_400 * 1_000_000_000); // 20 days ago

    let mut attrs1 = BTreeMap::new();
    attrs1.insert("prompt".into(), AttributeValue::from("Original prompt text"));
    attrs1.insert("completion".into(), AttributeValue::from("Original completion"));

    let span1 = SpanRecord {
        trace_id,
        span_id: span1_id,
        parent_span_id: None,
        name: "LlmCall_v1".into(),
        kind: SpanKind::Llm,
        start_time_unix_nanos: old_timestamp,
        end_time_unix_nanos: Some(old_timestamp + 50_000_000),
        status: SpanStatus::Ok,
        attributes: attrs1,
        events: vec![],
        metrics: SpanMetrics::new().with_tokens(500, 100).with_cost(0.002),
    };

    let mut attrs2 = BTreeMap::new();
    attrs2.insert("prompt".into(), AttributeValue::from("Updated prompt text with additions"));
    attrs2.insert("completion".into(), AttributeValue::from("Updated completion"));

    let span2 = SpanRecord {
        trace_id,
        span_id: span2_id,
        parent_span_id: Some(span1_id),
        name: "LlmCall_v2".into(),
        kind: SpanKind::Llm,
        start_time_unix_nanos: now_nanos - 1_000_000_000, // 1 sec ago
        end_time_unix_nanos: Some(now_nanos - 500_000_000),
        status: SpanStatus::Ok,
        attributes: attrs2,
        events: vec![],
        metrics: SpanMetrics::new().with_tokens(600, 120).with_cost(0.0025),
    };

    commit_batch(&db, &[span1, span2], &[]).expect("Failed to commit batch");
    drop(db);

    // 1. Test execute_stats
    let stats_res = execute_stats(&db_path);
    assert!(stats_res.is_ok(), "execute_stats failed: {:?}", stats_res.err());

    // 2. Test execute_diff
    let diff_res = execute_diff(&db_path, &span1_id.to_string(), &span2_id.to_string(), DiffField::All);
    assert!(diff_res.is_ok(), "execute_diff failed: {:?}", diff_res.err());

    // 3. Test execute_export to file
    let export_file = dir.path().join("exported_trace.json");
    let export_res = execute_export(
        &db_path,
        Some(trace_id.to_string()),
        None,
        ExportFormat::Json,
        Some(export_file.clone()),
    );
    assert!(export_res.is_ok(), "execute_export failed: {:?}", export_res.err());
    assert!(export_file.exists());
    let export_content = std::fs::read_to_string(&export_file).unwrap();
    assert!(export_content.contains("LlmCall_v1"));
    assert!(export_content.contains("LlmCall_v2"));

    // 4. Test execute_prune with dry_run
    let dry_run_res = execute_prune(&db_path, 14, true);
    assert!(dry_run_res.is_ok(), "execute_prune dry run failed: {:?}", dry_run_res.err());

    // 5. Test execute_prune actual deletion
    let prune_res = execute_prune(&db_path, 14, false);
    assert!(prune_res.is_ok(), "execute_prune failed: {:?}", prune_res.err());

    // Verify pruned traces
    let db_after = redb::Database::open(&db_path).unwrap();
    let reader_after = poom_storage::StorageReader::new(&db_after);
    let traces_after = reader_after.list_recent_traces(10, 0).unwrap();
    assert!(traces_after.is_empty(), "Expected expired trace to be pruned");
}
