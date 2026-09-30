use std::sync::Arc;
use iced::{clipboard, Element, Length, Subscription, Task, Theme};
use poom_daemon::EventBroadcaster;
use poom_storage::{SpanNode, StorageEngine, StorageReader};
use poom_types::{EvaluationRecord, SpanId, TraceId};

use crate::state::*;
use crate::subscription::live_spans_subscription;
use crate::views::{view_main_layout, view_settings_modal, view_status_bar, view_top_bar};
use crate::widgets::waterfall::{flatten_span_tree, SpanLayoutItem, TraceBounds};

/// The primary application state model for Poom Observability Studio.
pub struct AppState {
    pub storage: Arc<StorageEngine>,
    pub db_path: String,
    pub broadcaster: Option<EventBroadcaster>,

    // Search and Filters
    pub search_query: String,
    pub time_preset: TimeWindowPreset,
    pub status_filter: StatusFilter,
    pub auto_scroll: bool,

    // Data Cache
    pub traces: Vec<TraceSummary>,
    pub selected_trace_id: Option<TraceId>,
    pub active_tree: Option<SpanNode>,
    pub layout_items: Vec<SpanLayoutItem>,
    pub trace_bounds: Option<TraceBounds>,
    pub selected_span_id: Option<SpanId>,
    pub evaluations: Vec<EvaluationRecord>,

    // Canvas & Inspector state
    pub canvas_view: CanvasViewState,
    pub inspector_tab: InspectorTab,
    pub attribute_filter: String,
    pub live_event_count: usize,

    // Display & Settings
    pub show_settings: bool,
    pub ui_scale: f64,
    pub theme_mode: ThemeMode,
    pub fastapi_port: String,
}

impl AppState {
    pub fn new(
        storage: StorageEngine,
        db_path: String,
        broadcaster: Option<EventBroadcaster>,
    ) -> (Self, Task<Message>) {
        let storage_arc = Arc::new(storage);
        let db_clone = storage_arc.database().clone();

        let initial_state = Self {
            storage: storage_arc,
            db_path,
            broadcaster,
            search_query: String::new(),
            time_preset: TimeWindowPreset::AllTime,
            status_filter: StatusFilter::All,
            auto_scroll: true,
            traces: Vec::new(),
            selected_trace_id: None,
            active_tree: None,
            layout_items: Vec::new(),
            trace_bounds: None,
            selected_span_id: None,
            evaluations: Vec::new(),
            canvas_view: CanvasViewState::default(),
            inspector_tab: InspectorTab::Overview,
            attribute_filter: String::new(),
            live_event_count: 0,
            show_settings: false,
            ui_scale: 1.0,
            theme_mode: ThemeMode::Dark,
            fastapi_port: "8000".to_string(),
        };

        // Trigger initial background load of traces
        let initial_task = Task::perform(
            async move { query_recent_traces(db_clone, 100) },
            Message::TracesLoaded,
        );

        (initial_state, initial_task)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SelectTrace(trace_id) => {
                self.selected_trace_id = Some(trace_id);
                self.selected_span_id = None;
                self.canvas_view = CanvasViewState::default();

                let db = self.storage.database().clone();
                let tree_task = Task::perform(
                    async move { query_trace_tree(db, trace_id) },
                    Message::TraceTreeLoaded,
                );

                let db_eval = self.storage.database().clone();
                let eval_task = Task::perform(
                    async move { query_evaluations(db_eval, trace_id) },
                    Message::EvaluationsLoaded,
                );

                return Task::batch([tree_task, eval_task]);
            }

            Message::SelectSpan(span_id) => {
                self.selected_span_id = Some(span_id);
            }

            Message::HoverSpan(span_id, pos) => {
                self.canvas_view.hovered_span_id = span_id;
                self.canvas_view.hovered_pos = pos;
            }

            Message::DeselectSpan => {
                self.selected_span_id = None;
            }

            Message::SearchQueryChanged(q) => {
                self.search_query = q;
            }

            Message::TimeWindowChanged(preset) => {
                self.time_preset = preset;
                return self.refresh_traces();
            }

            Message::StatusFilterChanged(status) => {
                self.status_filter = status;
            }

            Message::ToggleAutoScroll(val) => {
                self.auto_scroll = val;
            }

            Message::RefreshTraces => {
                return self.refresh_traces();
            }

            Message::TracesLoaded(res) => {
                if let Ok(loaded) = res {
                    // Automatically select first trace if none selected
                    if self.selected_trace_id.is_none() && !loaded.is_empty() {
                        let first_id = loaded[0].trace_id;
                        self.traces = loaded;
                        return self.update(Message::SelectTrace(first_id));
                    }
                    self.traces = loaded;
                }
            }

            Message::TraceTreeLoaded(res) => {
                if let Ok(Some(tree)) = res {
                    let items = flatten_span_tree(&tree);
                    let bounds = TraceBounds::compute(&items);

                    // Auto-select root span if none selected
                    if self.selected_span_id.is_none() {
                        self.selected_span_id = Some(tree.span.span_id);
                    }

                    self.active_tree = Some(tree);
                    self.layout_items = items;
                    self.trace_bounds = bounds;
                }
            }

            Message::EvaluationsLoaded(res) => {
                if let Ok(evals) = res {
                    self.evaluations = evals;
                }
            }

            Message::LiveSpanReceived(span) => {
                self.live_event_count += 1;

                // If currently viewing this trace, update the tree
                if self.selected_trace_id == Some(span.trace_id) {
                    let db = self.storage.database().clone();
                    let trace_id = span.trace_id;
                    return Task::perform(
                        async move { query_trace_tree(db, trace_id) },
                        Message::TraceTreeLoaded,
                    );
                }

                // If auto-scroll is enabled and it's a new root span, refresh traces
                if self.auto_scroll && span.is_root() {
                    return self.refresh_traces();
                }
            }

            Message::CanvasScrolled(dx, dy) => {
                self.canvas_view.pan_x = (self.canvas_view.pan_x + dx).min(500.0);
                self.canvas_view.pan_y = (self.canvas_view.pan_y - dy).min(50.0);
            }

            Message::CanvasZoomed(zoom_delta) => {
                self.canvas_view.zoom = (self.canvas_view.zoom + zoom_delta).clamp(0.1, 50.0);
            }

            Message::CanvasDragStarted(pt) => {
                self.canvas_view.is_dragging = true;
                self.canvas_view.drag_start = Some(pt);
            }

            Message::CanvasDragged(delta) => {
                self.canvas_view.pan_x += delta.x;
                self.canvas_view.pan_y += delta.y;
            }

            Message::CanvasDragEnded => {
                self.canvas_view.is_dragging = false;
                self.canvas_view.drag_start = None;
            }

            Message::ResetCanvasView => {
                self.canvas_view.pan_x = 0.0;
                self.canvas_view.pan_y = 0.0;
                self.canvas_view.zoom = 1.0;
            }

            Message::InspectorTabSelected(tab) => {
                self.inspector_tab = tab;
            }

            Message::AttributeFilterChanged(q) => {
                self.attribute_filter = q;
            }

            Message::CopyToClipboard(text) => {
                return clipboard::write(text);
            }

            Message::SplitRatioChanged(_) => {}

            // Settings & Display Customization
            Message::ToggleSettings => {
                self.show_settings = !self.show_settings;
            }

            Message::CloseSettings => {
                self.show_settings = false;
            }

            Message::IncreaseFontSize => {
                self.ui_scale = (self.ui_scale + 0.10).min(1.60);
            }

            Message::DecreaseFontSize => {
                self.ui_scale = (self.ui_scale - 0.10).max(0.70);
            }

            Message::ResetFontSize => {
                self.ui_scale = 1.0;
            }

            Message::SetTheme(mode) => {
                self.theme_mode = mode;
            }

            Message::FastApiPortChanged(port) => {
                self.fastapi_port = port;
            }
        }

        Task::none()
    }

    fn refresh_traces(&self) -> Task<Message> {
        let db = self.storage.database().clone();
        Task::perform(
            async move { query_recent_traces(db, 100) },
            Message::TracesLoaded,
        )
    }

    pub fn view(&self) -> Element<'_, Message> {
        let selected_span = self.selected_span_id.and_then(|id| {
            self.layout_items.iter().find(|item| item.span.span_id == id).map(|item| &item.span)
        });

        let top_bar = view_top_bar(
            &self.search_query,
            self.time_preset,
            self.status_filter,
            self.auto_scroll,
            &self.traces,
        );

        let main_layout = view_main_layout(
            &self.traces,
            self.selected_trace_id,
            &self.search_query,
            self.status_filter,
            &self.layout_items,
            self.trace_bounds,
            self.selected_span_id,
            selected_span,
            &self.evaluations,
            &self.canvas_view,
            self.inspector_tab,
            &self.attribute_filter,
        );

        let status_bar = view_status_bar(
            &self.db_path,
            self.live_event_count,
            self.canvas_view.zoom,
            self.show_settings,
            &self.fastapi_port,
        );

        let base_layout = iced::widget::column![top_bar, main_layout, status_bar]
            .width(Length::Fill)
            .height(Length::Fill);

        if self.show_settings {
            let backdrop = iced::widget::mouse_area(
                iced::widget::container(iced::widget::Space::new(Length::Fill, Length::Fill))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(|_t: &Theme| iced::widget::container::Style {
                        background: Some(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.45).into()),
                        ..Default::default()
                    }),
            )
            .on_press(Message::CloseSettings);

            let modal = view_settings_modal(
                self.ui_scale,
                self.theme_mode,
                &self.fastapi_port,
            );

            let modal_positioned = iced::widget::container(
                iced::widget::column![
                    iced::widget::vertical_space(),
                    iced::widget::row![
                        modal,
                        iced::widget::horizontal_space(),
                    ]
                    .padding(iced::Padding {
                        top: 0.0,
                        right: 0.0,
                        bottom: 34.0,
                        left: 10.0,
                    }),
                ]
            )
            .width(Length::Fill)
            .height(Length::Fill);

            iced::widget::stack![
                base_layout,
                backdrop,
                modal_positioned,
            ]
            .into()
        } else {
            base_layout.into()
        }
    }

    pub fn theme(&self) -> Theme {
        match self.theme_mode {
            ThemeMode::Dark => Theme::Dark,
            ThemeMode::Light => Theme::Light,
        }
    }

    pub fn scale_factor(&self) -> f64 {
        self.ui_scale
    }

    pub fn subscription(&self) -> Subscription<Message> {
        live_spans_subscription(self.broadcaster.clone())
    }
}

// ---------------------------------------------------------------------------
// Background Database Query Functions
// ---------------------------------------------------------------------------

fn query_recent_traces(db: Arc<redb::Database>, limit: usize) -> Result<Vec<TraceSummary>, String> {
    let reader = StorageReader::new(&db);
    let recent = reader.list_recent_traces(limit, 0).map_err(|e| e.to_string())?;

    let mut summaries = Vec::with_capacity(recent.len());

    for (_, trace_id) in recent {
        if let Ok(spans) = reader.get_trace_spans(trace_id) {
            if spans.is_empty() {
                continue;
            }

            let root_span = spans.iter().find(|s| s.is_root()).unwrap_or(&spans[0]);

            let mut total_tokens = 0u32;
            let mut total_cost = 0.0f64;
            let mut has_error = false;

            for s in &spans {
                if let Some(t) = s.metrics.total_tokens {
                    total_tokens = total_tokens.saturating_add(t);
                }
                if let Some(c) = s.metrics.estimated_cost_usd {
                    total_cost += c;
                }
                if s.status.is_error() {
                    has_error = true;
                }
            }

            let status = if has_error {
                root_span.status.clone()
            } else {
                poom_types::SpanStatus::Ok
            };

            summaries.push(TraceSummary {
                trace_id,
                root_name: root_span.name.clone(),
                root_kind: root_span.kind,
                start_time_unix_nanos: root_span.start_time_unix_nanos,
                duration_nanos: root_span.duration_nanos(),
                status,
                span_count: spans.len(),
                total_tokens,
                total_cost_usd: total_cost,
            });
        }
    }

    Ok(summaries)
}

fn query_trace_tree(db: Arc<redb::Database>, trace_id: TraceId) -> Result<Option<SpanNode>, String> {
    let reader = StorageReader::new(&db);
    reader.get_trace_tree(trace_id).map_err(|e| e.to_string())
}

fn query_evaluations(db: Arc<redb::Database>, trace_id: TraceId) -> Result<Vec<EvaluationRecord>, String> {
    let reader = StorageReader::new(&db);
    reader.get_trace_evaluations(trace_id).map_err(|e| e.to_string())
}
