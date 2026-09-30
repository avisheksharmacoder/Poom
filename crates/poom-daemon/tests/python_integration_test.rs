use std::process::Command;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use tokio::time::sleep;

use poom_daemon::{DaemonConfig, PoomDaemon};
use poom_storage::{assemble_span_tree, StorageReader};
use poom_types::{AttributeValue, EvaluationValue, SpanKind, TraceId};

#[tokio::test]
async fn test_full_python_sdk_to_daemon_pipeline() {
    let dir = tempdir().unwrap();
    let sock_path = dir.path().join("poom_py_e2e.sock");
    let db_path = dir.path().join("poom_py_e2e.redb");

    let config = DaemonConfig::new(&sock_path, &db_path);
    let daemon = Arc::new(PoomDaemon::start(config).await.expect("Failed to start daemon"));

    let daemon_clone = Arc::clone(&daemon);
    tokio::spawn(async move {
        let _ = daemon_clone.run().await;
    });

    // Wait for daemon to bind socket
    sleep(Duration::from_millis(50)).await;

    let sock_str = sock_path.to_str().unwrap();
    let python_script = format!(
        r#"
import sys, os
sys.path.insert(0, os.path.abspath("python/poom"))
import poom
from poom import trace, trace_tool, SpanKind, evaluate

assert poom.init(socket_path="{sock_str}")

@trace(name="generate_answer", kind=SpanKind.LLM)
def call_llm(prompt: str):
    poom.set_attribute("model", "gpt-4o")
    poom.set_tokens(input_tokens=120, output_tokens=45)
    return f"Answer for: {{prompt}}"

@trace_tool(name="search_kb")
def lookup_knowledge(query: str):
    return call_llm(query)

@trace(name="agent_workflow", kind=SpanKind.AGENT)
def run_agent(task: str):
    tid = poom.get_current_trace_id()
    ans = lookup_knowledge(task)
    return tid, ans

trace_id, res = run_agent("quantum computing")
print(f"TRACE_ID:{{trace_id}}")

evaluate(trace_id=trace_id, name="correctness", score=0.98, comment="Verified by E2E test")

flushed = poom.flush(timeout_seconds=2.0)
assert flushed, "Flush must succeed"
poom.shutdown()
"#
    );

    let output = Command::new("python3")
        .arg("-c")
        .arg(&python_script)
        .current_dir(std::env::current_dir().unwrap().parent().unwrap().parent().unwrap())
        .output()
        .expect("Failed to execute python3");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("PY STDOUT:\n{stdout}\nPY STDERR:\n{stderr}");
    assert!(output.status.success(), "Python execution failed!\nStdout: {stdout}\nStderr: {stderr}");

    // Extract TRACE_ID
    let trace_id_str = stdout
        .lines()
        .find(|line| line.starts_with("TRACE_ID:"))
        .expect("Missing TRACE_ID in stdout")
        .trim_start_matches("TRACE_ID:")
        .trim();

    let trace_id = TraceId::from_str(trace_id_str).expect("Valid TraceId required");

    // Allow daemon batch writer to flush to disk
    sleep(Duration::from_millis(200)).await;

    // Verify stored spans in redb
    let reader = StorageReader::new(daemon.storage().database());
    let stored_spans = reader.get_trace_spans(trace_id).expect("Query spans failed");

    assert_eq!(stored_spans.len(), 3, "Expected 3 spans, got {:?}", stored_spans.len());

    // Find each span
    let agent_span = stored_spans.iter().find(|s| s.name.as_str() == "agent_workflow").unwrap();
    let tool_span = stored_spans.iter().find(|s| s.name.as_str() == "search_kb").unwrap();
    let llm_span = stored_spans.iter().find(|s| s.name.as_str() == "generate_answer").unwrap();

    // Verify hierarchy
    assert!(agent_span.parent_span_id.is_none(), "Agent must be root span");
    assert_eq!(tool_span.parent_span_id, Some(agent_span.span_id), "Tool must have Agent parent");
    assert_eq!(llm_span.parent_span_id, Some(tool_span.span_id), "LLM must have Tool parent");

    // Verify kinds
    assert_eq!(agent_span.kind, SpanKind::Agent);
    assert_eq!(tool_span.kind, SpanKind::Tool);
    assert_eq!(llm_span.kind, SpanKind::Llm);

    // Verify metrics and attributes on LLM span
    assert_eq!(llm_span.metrics.input_tokens, Some(120));
    assert_eq!(llm_span.metrics.output_tokens, Some(45));
    assert_eq!(llm_span.attributes.get("model"), Some(&AttributeValue::from("gpt-4o")));

    // Verify tool inputs
    assert!(tool_span.attributes.contains_key("tool.inputs"), "Tool inputs must be captured");

    // Verify hierarchy reconstruction via assemble_span_tree
    let tree = assemble_span_tree(stored_spans).expect("Must reconstruct root node");
    assert_eq!(tree.span.name.as_str(), "agent_workflow");
    assert_eq!(tree.children.len(), 1);
    assert_eq!(tree.children[0].span.name.as_str(), "search_kb");
    assert_eq!(tree.children[0].children.len(), 1);
    assert_eq!(tree.children[0].children[0].span.name.as_str(), "generate_answer");

    // Verify evaluations stored in redb
    let evals = reader.get_trace_evaluations(trace_id).expect("Query evaluations failed");
    assert_eq!(evals.len(), 1);
    assert_eq!(evals[0].name.as_str(), "correctness");
    assert_eq!(evals[0].value, EvaluationValue::Numeric(0.98));
    assert_eq!(evals[0].comment.as_deref(), Some("Verified by E2E test"));
}
