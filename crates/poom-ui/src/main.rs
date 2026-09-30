use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use poom_daemon::{DaemonConfig, PoomDaemon};
use poom_storage::{commit_batch, StorageReader};
use poom_types::{
    AttributeValue, EvaluationRecord, EvaluationValue, EventRecord, SpanId, SpanKind, SpanMetrics,
    SpanRecord, SpanStatus, TraceId,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    // 1. Setup default directory ~/.poom
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let poom_dir = PathBuf::from(home).join(".poom");
    let _ = fs::create_dir_all(&poom_dir);
    let db_path = poom_dir.join("data.redb");

    println!("Starting Poom Observability Studio...");
    println!("Database: {}", db_path.display());

    let config = DaemonConfig::default();
    let socket_display = config.socket_path.display().to_string();

    // 2. Start background Tokio runtime
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;

    let _guard = rt.enter();

    // 3. Start Daemon (which creates and manages the single shared redb instance)
    let (daemon, broadcaster, storage) = rt.block_on(async {
        let daemon = PoomDaemon::start(config).await?;
        let broadcaster = daemon.broadcaster().clone();
        let storage = daemon.storage().clone();
        Ok::<_, poom_daemon::DaemonError>((daemon, broadcaster, storage))
    })?;

    // 4. Seed demo traces if database is currently empty
    {
        let reader = StorageReader::new(storage.database());
        let recent = reader.list_recent_traces(1, 0)?;
        if recent.is_empty() {
            println!("Empty database detected. Seeding realistic sample AI agent traces...");
            seed_demo_traces(storage.database())?;
        }
    }

    // 5. Spawn accept loop in background Tokio task
    let daemon_arc = Arc::new(daemon);
    let d = Arc::clone(&daemon_arc);
    rt.spawn(async move {
        if let Err(e) = d.run().await {
            eprintln!("[DAEMON ERROR]: {e}");
        }
    });

    println!("IPC Socket listening on: {socket_display}");
    println!("Launching native Iced studio interface...");

    // 6. Run Studio Desktop UI
    let db_path_str = db_path.to_string_lossy().to_string();
    poom_ui::run_studio(storage, db_path_str, Some(broadcaster))?;

    daemon_arc.shutdown();
    Ok(())
}

fn seed_demo_traces(db: &redb::Database) -> Result<(), Box<dyn std::error::Error>> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_nanos() as u64;

    // -----------------------------------------------------------------------
    // Trace 1: Multi-Agent Financial Research Workflow (Ok, 1.82s)
    // -----------------------------------------------------------------------
    let trace_1_id = TraceId::generate();
    let root_1_id = SpanId::generate();
    let t1_start = now - 180_000_000_000; // 3 mins ago

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
        end_time_unix_nanos: Some(t1_start + 1_820_000_000), // 1.82s
        status: SpanStatus::Ok,
        attributes: t1_attrs,
        events: vec![EventRecord::with_timestamp(t1_start + 12_000_000, "workflow_started")],
        metrics: SpanMetrics::new().with_tokens(1450, 320).with_cost(0.0215),
    };

    let mut tool_attrs = BTreeMap::new();
    tool_attrs.insert("tool.name".into(), AttributeValue::from("sec_edgar_retriever"));
    tool_attrs.insert("query".into(), AttributeValue::from("AAPL 10-K Q3 2024 Revenue Breakdown"));
    tool_attrs.insert("output".into(), AttributeValue::from("{\n  \"ticker\": \"AAPL\",\n  \"q3_revenue\": \"$85.8B\",\n  \"services_growth\": \"+12.1% YoY\",\n  \"operating_margin\": \"30.2%\"\n}"));

    let tool_span = SpanRecord {
        trace_id: trace_1_id,
        span_id: tool_span_id,
        parent_span_id: Some(root_1_id),
        name: "RetrieveSECFilings".into(),
        kind: SpanKind::Tool,
        start_time_unix_nanos: t1_start + 80_000_000,
        end_time_unix_nanos: Some(t1_start + 240_000_000), // 160ms
        status: SpanStatus::Ok,
        attributes: tool_attrs,
        events: vec![],
        metrics: SpanMetrics::new(),
    };

    let mut llm_attrs = BTreeMap::new();
    llm_attrs.insert("gen_ai.system".into(), AttributeValue::from("openai"));
    llm_attrs.insert("gen_ai.request.model".into(), AttributeValue::from("gpt-4o-2024-08-06"));
    llm_attrs.insert("gen_ai.request.temperature".into(), AttributeValue::from(0.2));
    llm_attrs.insert("prompt".into(), AttributeValue::from("You are an expert equity research analyst. Analyze the following Apple Q3 10-K data:\n\nRevenue: $85.8B\nServices: +12.1% YoY\nOperating Margin: 30.2%\n\nProvide an executive summary of margin expansion and risks."));
    llm_attrs.insert("completion".into(), AttributeValue::from("Executive Summary: Apple's Q3 results demonstrate robust operating leverage driven by acceleration in high-margin Services revenue (+12.1% YoY). Operating margins expanded to 30.2%, offsetting modest hardware headwinds. Key risks include regulatory scrutiny in European markets and FX volatility."));

    let llm_span = SpanRecord {
        trace_id: trace_1_id,
        span_id: llm_span_id,
        parent_span_id: Some(root_1_id),
        name: "LlmCall:gpt-4o".into(),
        kind: SpanKind::Llm,
        start_time_unix_nanos: t1_start + 260_000_000,
        end_time_unix_nanos: Some(t1_start + 1_780_000_000), // 1.52s
        status: SpanStatus::Ok,
        attributes: llm_attrs,
        events: vec![
            EventRecord::with_timestamp(t1_start + 270_000_000, "stream_first_chunk"),
            EventRecord::with_timestamp(t1_start + 1_775_000_000, "stream_completed"),
        ],
        metrics: SpanMetrics::new().with_tokens(1450, 320).with_cost(0.0215),
    };

    let eval_1 = EvaluationRecord::new(
        trace_1_id,
        Some(llm_span_id),
        "factual_accuracy",
        EvaluationValue::Numeric(0.98),
    )
    .with_comment("All figures accurately matched official SEC 10-K filing tables.");

    // -----------------------------------------------------------------------
    // Trace 2: FastAPI HTTP Request with Rate Limit Error (310ms)
    // -----------------------------------------------------------------------
    let trace_2_id = TraceId::generate();
    let root_2_id = SpanId::generate();
    let t2_start = now - 60_000_000_000; // 1 min ago
    let error_tool_id = SpanId::generate();

    let mut http_attrs = BTreeMap::new();
    http_attrs.insert("http.method".into(), AttributeValue::from("POST"));
    http_attrs.insert("http.route".into(), AttributeValue::from("/api/v1/chat/completions"));
    http_attrs.insert("http.status_code".into(), AttributeValue::from(429));
    http_attrs.insert("client.ip".into(), AttributeValue::from("192.168.1.104"));

    let root_2 = SpanRecord {
        trace_id: trace_2_id,
        span_id: root_2_id,
        parent_span_id: None,
        name: "POST /api/v1/chat/completions".into(),
        kind: SpanKind::Http,
        start_time_unix_nanos: t2_start,
        end_time_unix_nanos: Some(t2_start + 310_000_000),
        status: SpanStatus::error("RateLimitError", "HTTP 429: Too many model inference requests."),
        attributes: http_attrs,
        events: vec![],
        metrics: SpanMetrics::new(),
    };

    let mut err_tool_attrs = BTreeMap::new();
    err_tool_attrs.insert("provider".into(), AttributeValue::from("anthropic"));
    err_tool_attrs.insert("model".into(), AttributeValue::from("claude-3-5-sonnet-20241022"));

    let error_span = SpanRecord {
        trace_id: trace_2_id,
        span_id: error_tool_id,
        parent_span_id: Some(root_2_id),
        name: "AnthropicProviderCall".into(),
        kind: SpanKind::Tool,
        start_time_unix_nanos: t2_start + 25_000_000,
        end_time_unix_nanos: Some(t2_start + 295_000_000),
        status: SpanStatus::error_with_backtrace(
            "anthropic.RateLimitError",
            "Error code 429: Number of concurrent requests exceeds your organization quota.",
            "Traceback (most recent call last):\n  File \"/app/services/ai.py\", line 84, in dispatch_claude\n    response = client.messages.create(...)\n  File \"/usr/local/lib/python3.11/anthropic/resources/messages.py\", line 621, in create\n    return self._post(...)\nanthropic.RateLimitError: Error code 429",
        ),
        attributes: err_tool_attrs,
        events: vec![],
        metrics: SpanMetrics::new(),
    };

    // -----------------------------------------------------------------------
    // Trace 3: Customer Support Router Chain (420ms)
    // -----------------------------------------------------------------------
    let trace_3_id = TraceId::generate();
    let root_3_id = SpanId::generate();
    let t3_start = now - 15_000_000_000; // 15 secs ago

    let root_3 = SpanRecord {
        trace_id: trace_3_id,
        span_id: root_3_id,
        parent_span_id: None,
        name: "CustomerSupportRouter".into(),
        kind: SpanKind::Chain,
        start_time_unix_nanos: t3_start,
        end_time_unix_nanos: Some(t3_start + 420_000_000),
        status: SpanStatus::Ok,
        attributes: BTreeMap::new(),
        events: vec![],
        metrics: SpanMetrics::new().with_tokens(480, 110).with_cost(0.0035),
    };

    // Commit synchronous batch directly to redb
    commit_batch(
        db,
        &[root_1, tool_span, llm_span, root_2, error_span, root_3],
        &[eval_1],
    )?;

    println!("Seeded 3 sample execution traces successfully.");
    Ok(())
}
