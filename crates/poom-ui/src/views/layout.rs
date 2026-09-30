use iced::widget::{column, container, row};
use iced::{Element, Length};

use poom_types::{EvaluationRecord, SpanId, SpanRecord, TraceId};

use crate::state::{CanvasViewState, InspectorTab, Message, StatusFilter, TraceSummary};
use crate::theme::*;
use crate::widgets::waterfall::{SpanLayoutItem, TraceBounds};
use crate::widgets::{view_inspector, view_trace_table, view_waterfall};

pub fn view_main_layout<'a>(
    traces: &'a [TraceSummary],
    selected_trace_id: Option<TraceId>,
    search_query: &str,
    status_filter: StatusFilter,
    layout_items: &'a [SpanLayoutItem],
    trace_bounds: Option<TraceBounds>,
    selected_span_id: Option<SpanId>,
    selected_span: Option<&'a SpanRecord>,
    evaluations: &'a [EvaluationRecord],
    canvas_view: &'a CanvasViewState,
    inspector_tab: InspectorTab,
    attribute_filter: &'a str,
) -> Element<'a, Message> {
    // 1. Left Trace Table Column
    let left_pane = container(view_trace_table(
        traces,
        selected_trace_id,
        search_query,
        status_filter,
    ))
    .width(Length::FillPortion(3))
    .height(Length::Fill)
    .style(surface_card);

    // 2. Right Pane: Upper Waterfall Canvas + Lower Inspector
    let waterfall_pane = container(view_waterfall(
        layout_items,
        trace_bounds,
        selected_span_id,
        canvas_view,
    ))
    .width(Length::Fill)
    .height(Length::FillPortion(5))
    .style(surface_card);

    let inspector_pane = container(view_inspector(
        selected_span,
        evaluations,
        inspector_tab,
        attribute_filter,
    ))
    .width(Length::Fill)
    .height(Length::FillPortion(4))
    .style(surface_card);

    let right_pane = column![waterfall_pane, inspector_pane]
        .spacing(6)
        .width(Length::FillPortion(7))
        .height(Length::Fill);

    row![left_pane, right_pane]
        .spacing(6)
        .padding(6.0)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
