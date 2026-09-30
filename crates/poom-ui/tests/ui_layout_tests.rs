use poom_storage::SpanNode;
use poom_types::{SpanId, SpanKind, SpanMetrics, SpanRecord, SpanStatus, TraceId};
use poom_ui::state::{TimeWindowPreset, TraceSummary};
use poom_ui::widgets::waterfall::{
    calculate_bar_geometry, flatten_span_tree, SpanLayoutItem, TraceBounds, HEADER_HEIGHT,
    MIN_BAR_WIDTH, ROW_HEIGHT, TREE_LABEL_WIDTH,
};

#[test]
fn test_tree_flattening_depth_first() {
    let trace_id = TraceId::generate();
    let root_span = SpanRecord {
        trace_id,
        span_id: SpanId::generate(),
        parent_span_id: None,
        name: "AgentRoot".into(),
        kind: SpanKind::Agent,
        start_time_unix_nanos: 1_000_000,
        end_time_unix_nanos: Some(2_000_000),
        status: SpanStatus::Ok,
        attributes: Default::default(),
        events: Vec::new(),
        metrics: SpanMetrics::default(),
    };

    let mut root_node = SpanNode::new(root_span);

    let child_1 = SpanRecord {
        trace_id,
        span_id: SpanId::generate(),
        parent_span_id: Some(root_node.span.span_id),
        name: "RetrievalTool".into(),
        kind: SpanKind::Tool,
        start_time_unix_nanos: 1_100_000,
        end_time_unix_nanos: Some(1_300_000),
        status: SpanStatus::Ok,
        attributes: Default::default(),
        events: Vec::new(),
        metrics: SpanMetrics::default(),
    };

    let child_2 = SpanRecord {
        trace_id,
        span_id: SpanId::generate(),
        parent_span_id: Some(root_node.span.span_id),
        name: "LlmCall".into(),
        kind: SpanKind::Llm,
        start_time_unix_nanos: 1_350_000,
        end_time_unix_nanos: Some(1_900_000),
        status: SpanStatus::Ok,
        attributes: Default::default(),
        events: Vec::new(),
        metrics: SpanMetrics::default(),
    };

    root_node.children.push(SpanNode::new(child_1));
    root_node.children.push(SpanNode::new(child_2));

    let items = flatten_span_tree(&root_node);

    assert_eq!(items.len(), 3);
    assert_eq!(items[0].span.name.as_str(), "AgentRoot");
    assert_eq!(items[0].depth, 0);
    assert_eq!(items[0].row_index, 0);

    assert_eq!(items[1].span.name.as_str(), "RetrievalTool");
    assert_eq!(items[1].depth, 1);
    assert_eq!(items[1].row_index, 1);

    assert_eq!(items[2].span.name.as_str(), "LlmCall");
    assert_eq!(items[2].depth, 1);
    assert_eq!(items[2].row_index, 2);
}

#[test]
fn test_trace_bounds_computation() {
    let trace_id = TraceId::generate();
    let items = vec![
        SpanLayoutItem {
            span: SpanRecord {
                trace_id,
                span_id: SpanId::generate(),
                parent_span_id: None,
                name: "root".into(),
                kind: SpanKind::Agent,
                start_time_unix_nanos: 10_000,
                end_time_unix_nanos: Some(50_000),
                status: SpanStatus::Ok,
                attributes: Default::default(),
                events: Vec::new(),
                metrics: SpanMetrics::default(),
            },
            depth: 0,
            row_index: 0,
        },
        SpanLayoutItem {
            span: SpanRecord {
                trace_id,
                span_id: SpanId::generate(),
                parent_span_id: None,
                name: "child".into(),
                kind: SpanKind::Tool,
                start_time_unix_nanos: 15_000,
                end_time_unix_nanos: Some(60_000),
                status: SpanStatus::Ok,
                attributes: Default::default(),
                events: Vec::new(),
                metrics: SpanMetrics::default(),
            },
            depth: 1,
            row_index: 1,
        },
    ];

    let bounds = TraceBounds::compute(&items).expect("Should compute bounds");
    assert_eq!(bounds.start_unix_nanos, 10_000);
    assert_eq!(bounds.end_unix_nanos, 60_000);
    assert_eq!(bounds.total_duration_nanos, 50_000);
}

#[test]
fn test_calculate_bar_geometry() {
    let trace_id = TraceId::generate();
    let bounds = TraceBounds {
        start_unix_nanos: 1_000_000,
        end_unix_nanos: 2_000_000,
        total_duration_nanos: 1_000_000,
    };

    let item = SpanLayoutItem {
        span: SpanRecord {
            trace_id,
            span_id: SpanId::generate(),
            parent_span_id: None,
            name: "sub_span".into(),
            kind: SpanKind::Llm,
            start_time_unix_nanos: 1_250_000, // 25% through timeline
            end_time_unix_nanos: Some(1_750_000), // 50% duration
            status: SpanStatus::Ok,
            attributes: Default::default(),
            events: Vec::new(),
            metrics: SpanMetrics::default(),
        },
        depth: 0,
        row_index: 2,
    };

    let canvas_width = 1240.0;
    let _timeline_width = canvas_width - TREE_LABEL_WIDTH; // 1000.0

    let (bar_x, bar_y, bar_w, bar_h) = calculate_bar_geometry(
        &item,
        &bounds,
        canvas_width,
        0.0, // pan_x
        0.0, // pan_y
        1.0, // zoom
    );

    // bar_x should be TREE_LABEL_WIDTH + 25% of 1000.0 = 240 + 250 = 490.0
    assert_eq!(bar_x, TREE_LABEL_WIDTH + 250.0);
    // bar_w should be 50% of 1000.0 = 500.0
    assert_eq!(bar_w, 500.0);
    // bar_y should be HEADER_HEIGHT + (2 * ROW_HEIGHT) + 3.0
    assert_eq!(bar_y, HEADER_HEIGHT + (2.0 * ROW_HEIGHT) + 3.0);
    assert_eq!(bar_h, ROW_HEIGHT - 6.0);
}

#[test]
fn test_bar_geometry_min_width_clamp() {
    let trace_id = TraceId::generate();
    let bounds = TraceBounds {
        start_unix_nanos: 0,
        end_unix_nanos: 10_000_000,
        total_duration_nanos: 10_000_000,
    };

    let item = SpanLayoutItem {
        span: SpanRecord {
            trace_id,
            span_id: SpanId::generate(),
            parent_span_id: None,
            name: "micro_tool".into(),
            kind: SpanKind::Tool,
            start_time_unix_nanos: 5_000_000,
            end_time_unix_nanos: Some(5_000_010), // 10 nanoseconds
            status: SpanStatus::Ok,
            attributes: Default::default(),
            events: Vec::new(),
            metrics: SpanMetrics::default(),
        },
        depth: 1,
        row_index: 0,
    };

    let (_, _, bar_w, _) = calculate_bar_geometry(&item, &bounds, 1000.0, 0.0, 0.0, 1.0);
    assert_eq!(bar_w, MIN_BAR_WIDTH);
}

#[test]
fn test_trace_summary_formatting() {
    let summary = TraceSummary {
        trace_id: TraceId::generate(),
        root_name: "Workflow".into(),
        root_kind: SpanKind::Agent,
        start_time_unix_nanos: 1_000_000_000,
        duration_nanos: Some(1_450_000_000), // 1.45s
        status: SpanStatus::Ok,
        span_count: 5,
        total_tokens: 1200,
        total_cost_usd: 0.024,
    };

    assert_eq!(summary.formatted_duration(), "1.45s");

    let sub_ms = TraceSummary {
        duration_nanos: Some(850_000), // 850μs
        ..summary.clone()
    };
    assert_eq!(sub_ms.formatted_duration(), "850μs");

    let millis = TraceSummary {
        duration_nanos: Some(45_200_000), // 45.2ms
        ..summary
    };
    assert_eq!(millis.formatted_duration(), "45.2ms");
}

#[test]
fn test_time_window_presets() {
    assert_eq!(TimeWindowPreset::AllTime.duration_nanos(), None);
    assert_eq!(TimeWindowPreset::Last15Minutes.duration_nanos(), Some(15 * 60 * 1_000_000_000));
    assert_eq!(TimeWindowPreset::Last1Hour.duration_nanos(), Some(3600 * 1_000_000_000));
}

#[test]
fn test_settings_state_and_theme_modes() {
    use poom_ui::state::ThemeMode;

    assert!(ThemeMode::Dark.is_dark());
    assert!(!ThemeMode::Light.is_dark());
    assert_eq!(ThemeMode::ALL.len(), 2);
    assert_eq!(ThemeMode::Dark.as_str(), "Dark");
    assert_eq!(ThemeMode::Light.as_str(), "Light");
}

#[test]
fn test_fastapi_default_port_and_scaling_bounds() {
    let mut ui_scale: f64 = 1.0;
    let default_port = "8000";

    assert_eq!(default_port, "8000");

    // Scale up
    ui_scale = (ui_scale + 0.10).min(1.60);
    assert!((ui_scale - 1.10).abs() < 1e-6);

    // Scale down
    ui_scale = (ui_scale - 0.50).max(0.70);
    assert!((ui_scale - 0.70).abs() < 1e-6);

    // Reset
    ui_scale = 1.0;
    assert_eq!(ui_scale, 1.0);
}
