use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::runtime::Builder;

use poom_daemon::{DaemonConfig, PoomDaemon};
use poom_storage::{commit_batch, StorageReader};
use poom_types::{
    AttributeValue, EvaluationRecord, EvaluationValue, EventRecord, SpanId, SpanKind, SpanMetrics,
    SpanRecord, SpanStatus, TraceId,
};

use crate::error::{CliError, CliResult};

pub fn execute_studio(db_path: &Path, socket_path: &Path) -> CliResult<()> {
    // Normalize X11 / xkbcommon locale to UTF-8 to prevent Compose parse errors
    #[cfg(unix)]
    {
        if let Ok(lang) = std::env::var("LANG") {
            if !lang.to_lowercase().contains("utf") {
                std::env::set_var("LANG", format!("{lang}.UTF-8"));
            }
        } else {
            std::env::set_var("LANG", "en_US.UTF-8");
        }
        std::env::set_var("LC_ALL", "C.UTF-8");
        std::env::set_var("LC_CTYPE", "C.UTF-8");
    }

    println!("\x1b[1;36m====================== Poom Observability Studio ======================\x1b[0m");
    println!("Database:           \x1b[32m{}\x1b[0m", db_path.display());
    println!("IPC Socket:         \x1b[34m{}\x1b[0m", socket_path.display());

    // 1. Start background Tokio runtime
    let rt = Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;

    let _guard = rt.enter();

    // 2. Start Daemon & Storage
    let mut config = DaemonConfig::default();
    config.db_path = db_path.to_path_buf();
    config.socket_path = socket_path.to_path_buf();

    let (daemon, broadcaster, storage) = rt.block_on(async {
        let daemon = PoomDaemon::start(config).await?;
        let broadcaster = daemon.broadcaster().clone();
        let storage = daemon.storage().clone();
        Ok::<_, poom_daemon::DaemonError>((daemon, broadcaster, storage))
    })?;

    // 3. Seed sample traces if database is currently empty
    {
        let reader = StorageReader::new(storage.database());
        if let Ok(recent) = reader.list_recent_traces(1, 0) {
            if recent.is_empty() {
                println!("\x1b[33mEmpty database detected. Seeding realistic sample AI agent traces...\x1b[0m");
                let _ = seed_sample_traces(storage.database());
            }
        }
    }

    // 4. Spawn accept loop in background task
    let daemon_arc = Arc::new(daemon);
    let d = Arc::clone(&daemon_arc);
    rt.spawn(async move {
        if let Err(e) = d.run().await {
            eprintln!("\x1b[31m[DAEMON ERROR]: {e}\x1b[0m");
        }
    });

    println!("\x1b[1;32m✓ Ingestion server listening on {}\x1b[0m", socket_path.display());
    println!("Launching native Iced studio interface...");

    // 5. Run Studio Desktop UI on main thread
    let db_path_str = db_path.to_string_lossy().to_string();
    let ui_res = poom_ui::run_studio(storage, db_path_str, Some(broadcaster));

    daemon_arc.shutdown();

    if socket_path.exists() {
        let _ = std::fs::remove_file(socket_path);
    }

    ui_res.map_err(|e| CliError::Gui(e.to_string()))
}

fn seed_sample_traces(db: &redb::Database) -> Result<(), Box<dyn std::error::Error>> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_nanos() as u64;

    // Trace 1: Multi-Agent Financial Research Workflow
    let trace_1_id = TraceId::generate();
    let root_1_id = SpanId::generate();
    let t1_start = now - 180_000_000_000;

    let tool_span_id = SpanId::generate();
    let llm_span_id = SpanId::generate();

    let mut t1_attrs = BTreeMap::new();
    t1_attrs.insert("workflow.type".into(), AttributeValue::from("financial_analyst_v2"));
    t1_attrs.insert("user.id".into(), AttributeValue::from("usr_4920"));
    t1_attrs.insert("environment".into(), AttributeValue::from("production"));

    let root_1 = SpanRecord {
        trace_id: trace_1_id,
        span_id: root_1_id,
        parent_span_id: None,
        name: "FinancialAnalystAgent".into(),
        kind: SpanKind::Agent,
        start_time_unix_nanos: t1_start,
        end_time_unix_nanos: Some(t1_start + 1_820_000_000),
        status: SpanStatus::Ok,
        attributes: t1_attrs,
        events: vec![EventRecord::with_timestamp(t1_start + 12_000_000, "workflow_started")],
        metrics: SpanMetrics::new().with_tokens(1450, 320).with_cost(0.0215),
    };

    let mut tool_attrs = BTreeMap::new();
    tool_attrs.insert("tool.name".into(), AttributeValue::from("sec_edgar_retriever"));
    tool_attrs.insert("query".into(), AttributeValue::from("AAPL 10-K Q3 2024 Revenue Breakdown"));
    tool_attrs.insert("output".into(), AttributeValue::from("{\"ticker\": \"AAPL\", \"q3_revenue\": \"$85.8B\", \"services_growth\": \"+12.1% YoY\"}"));

    let tool_span = SpanRecord {
        trace_id: trace_1_id,
        span_id: tool_span_id,
        parent_span_id: Some(root_1_id),
        name: "RetrieveSECFilings".into(),
        kind: SpanKind::Tool,
        start_time_unix_nanos: t1_start + 80_000_000,
        end_time_unix_nanos: Some(t1_start + 240_000_000),
        status: SpanStatus::Ok,
        attributes: tool_attrs,
        events: vec![],
        metrics: SpanMetrics::new(),
    };

    let mut llm_attrs = BTreeMap::new();
    llm_attrs.insert("gen_ai.system".into(), AttributeValue::from("openai"));
    llm_attrs.insert("gen_ai.request.model".into(), AttributeValue::from("gpt-4o-2024-08-06"));
    llm_attrs.insert("prompt".into(), AttributeValue::from("Analyze the following Apple Q3 10-K data: Revenue: $85.8B, Services: +12.1% YoY."));
    llm_attrs.insert("completion".into(), AttributeValue::from("Executive Summary: Apple's Q3 results demonstrate robust operating leverage."));

    let llm_span = SpanRecord {
        trace_id: trace_1_id,
        span_id: llm_span_id,
        parent_span_id: Some(root_1_id),
        name: "LlmCall:gpt-4o".into(),
        kind: SpanKind::Llm,
        start_time_unix_nanos: t1_start + 260_000_000,
        end_time_unix_nanos: Some(t1_start + 1_780_000_000),
        status: SpanStatus::Ok,
        attributes: llm_attrs,
        events: vec![],
        metrics: SpanMetrics::new().with_tokens(1450, 320).with_cost(0.0215),
    };

    let eval_1 = EvaluationRecord::new(
        trace_1_id,
        Some(llm_span_id),
        "factual_accuracy",
        EvaluationValue::Numeric(0.98),
    )
    .with_comment("All figures accurately matched official SEC 10-K filing tables.");

    commit_batch(db, &[root_1, tool_span, llm_span], &[eval_1])?;
    Ok(())
}
