use iced::{Point, Vector};
use smol_str::SmolStr;

use poom_storage::SpanNode;
use poom_types::{EvaluationRecord, SpanId, SpanKind, SpanRecord, SpanStatus, TraceId};

/// Lightweight summary representation of a trace for high-density tabular browsing.
#[derive(Debug, Clone, PartialEq)]
pub struct TraceSummary {
    pub trace_id: TraceId,
    pub root_name: SmolStr,
    pub root_kind: SpanKind,
    pub start_time_unix_nanos: u64,
    pub duration_nanos: Option<u64>,
    pub status: SpanStatus,
    pub span_count: usize,
    pub total_tokens: u32,
    pub total_cost_usd: f64,
}

impl TraceSummary {
    /// Formats the duration into a human-readable string (e.g. "142ms", "1.24s").
    pub fn formatted_duration(&self) -> String {
        match self.duration_nanos {
            Some(nanos) => {
                let ms = nanos as f64 / 1_000_000.0;
                if ms >= 1000.0 {
                    format!("{:.2}s", ms / 1000.0)
                } else if ms >= 1.0 {
                    format!("{:.1}ms", ms)
                } else {
                    format!("{}μs", nanos / 1000)
                }
            }
            None => "running...".to_string(),
        }
    }

    /// Formats the relative start time (e.g. "12s ago", "5m ago").
    pub fn formatted_time_ago(&self) -> String {
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        let diff_nanos = now_nanos.saturating_sub(self.start_time_unix_nanos);
        let secs = diff_nanos / 1_000_000_000;

        if secs < 5 {
            "just now".to_string()
        } else if secs < 60 {
            format!("{secs}s ago")
        } else if secs < 3600 {
            format!("{}m ago", secs / 60)
        } else if secs < 86400 {
            format!("{}h ago", secs / 3600)
        } else {
            format!("{}d ago", secs / 86400)
        }
    }
}

/// Time window filter preset options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimeWindowPreset {
    #[default]
    AllTime,
    Last15Minutes,
    Last1Hour,
    Last6Hours,
    Last24Hours,
    Last7Days,
}

impl TimeWindowPreset {
    pub const ALL: [TimeWindowPreset; 6] = [
        TimeWindowPreset::AllTime,
        TimeWindowPreset::Last15Minutes,
        TimeWindowPreset::Last1Hour,
        TimeWindowPreset::Last6Hours,
        TimeWindowPreset::Last24Hours,
        TimeWindowPreset::Last7Days,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AllTime => "All Time",
            Self::Last15Minutes => "15m",
            Self::Last1Hour => "1h",
            Self::Last6Hours => "6h",
            Self::Last24Hours => "24h",
            Self::Last7Days => "7d",
        }
    }

    pub fn duration_nanos(&self) -> Option<u64> {
        match self {
            Self::AllTime => None,
            Self::Last15Minutes => Some(15 * 60 * 1_000_000_000),
            Self::Last1Hour => Some(60 * 60 * 1_000_000_000),
            Self::Last6Hours => Some(6 * 60 * 60 * 1_000_000_000),
            Self::Last24Hours => Some(24 * 60 * 60 * 1_000_000_000),
            Self::Last7Days => Some(7 * 24 * 60 * 60 * 1_000_000_000),
        }
    }
}

/// Status filter choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StatusFilter {
    #[default]
    All,
    SuccessOnly,
    ErrorOnly,
}

impl StatusFilter {
    pub const ALL: [StatusFilter; 3] = [
        StatusFilter::All,
        StatusFilter::SuccessOnly,
        StatusFilter::ErrorOnly,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::All => "All Status",
            Self::SuccessOnly => "Ok Only",
            Self::ErrorOnly => "Errors Only",
        }
    }
}

/// Tabs for the Span Inspector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InspectorTab {
    #[default]
    Overview,
    Prompts,
    Attributes,
    Events,
    Evaluations,
}

impl InspectorTab {
    pub const ALL: [InspectorTab; 5] = [
        InspectorTab::Overview,
        InspectorTab::Prompts,
        InspectorTab::Attributes,
        InspectorTab::Events,
        InspectorTab::Evaluations,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Prompts => "Prompts & I/O",
            Self::Attributes => "Attributes",
            Self::Events => "Events",
            Self::Evaluations => "Evaluations",
        }
    }
}

/// Canvas interaction and viewport state.
#[derive(Debug, Clone)]
pub struct CanvasViewState {
    pub pan_x: f32,
    pub pan_y: f32,
    pub zoom: f32,
    pub is_dragging: bool,
    pub drag_start: Option<Point>,
    pub hovered_span_id: Option<SpanId>,
    pub hovered_pos: Option<Point>,
}

impl Default for CanvasViewState {
    fn default() -> Self {
        Self {
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
            is_dragging: false,
            drag_start: None,
            hovered_span_id: None,
            hovered_pos: None,
        }
    }
}

/// Exhaustive UI event / message taxonomy.
#[derive(Debug, Clone)]
pub enum Message {
    // Navigation & Selection
    SelectTrace(TraceId),
    SelectSpan(SpanId),
    HoverSpan(Option<SpanId>, Option<Point>),
    DeselectSpan,

    // Filtering & Querying
    SearchQueryChanged(String),
    TimeWindowChanged(TimeWindowPreset),
    StatusFilterChanged(StatusFilter),
    ToggleAutoScroll(bool),
    RefreshTraces,

    // Async Data Responses
    TracesLoaded(Result<Vec<TraceSummary>, String>),
    TraceTreeLoaded(Result<Option<SpanNode>, String>),
    EvaluationsLoaded(Result<Vec<EvaluationRecord>, String>),

    // Live Ingestion Stream
    LiveSpanReceived(SpanRecord),

    // Canvas Interactions
    CanvasScrolled(f32, f32),
    CanvasZoomed(f32),
    CanvasDragStarted(Point),
    CanvasDragged(Vector),
    CanvasDragEnded,
    ResetCanvasView,

    // Inspector Tabs & Controls
    InspectorTabSelected(InspectorTab),
    AttributeFilterChanged(String),
    CopyToClipboard(String),

    // Layout
    SplitRatioChanged(f32),

    // Settings & Display Customization
    ToggleSettings,
    CloseSettings,
    IncreaseFontSize,
    DecreaseFontSize,
    ResetFontSize,
    SetTheme(ThemeMode),
    FastApiPortChanged(String),
}

/// Application appearance mode (Dark slate vs. Light).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
}

impl ThemeMode {
    pub const ALL: [ThemeMode; 2] = [ThemeMode::Dark, ThemeMode::Light];

    pub fn is_dark(&self) -> bool {
        matches!(self, Self::Dark)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
        }
    }
}
