use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Alignment, Element, Length, Padding};

use poom_types::{SpanStatus, TraceId};

use crate::state::{Message, StatusFilter, TraceSummary};
use crate::theme::*;

/// Renders the virtualized/scrollable list of traces on the left pane.
pub fn view_trace_table<'a>(
    traces: &'a [TraceSummary],
    selected_trace_id: Option<TraceId>,
    search_query: &str,
    status_filter: StatusFilter,
) -> Element<'a, Message> {
    let mut trace_list = column![].spacing(6).padding(Padding::from([4.0, 8.0]));
    let mut visible_count = 0;

    for trace in traces {
        // Status filter
        match status_filter {
            StatusFilter::All => {}
            StatusFilter::SuccessOnly => {
                if trace.status.is_error() {
                    continue;
                }
            }
            StatusFilter::ErrorOnly => {
                if trace.status.is_ok() {
                    continue;
                }
            }
        }

        // Search query
        if !search_query.is_empty() {
            let q = search_query.to_ascii_lowercase();
            let matches_name = trace.root_name.to_ascii_lowercase().contains(&q);
            let matches_id = trace.trace_id.to_string().contains(&q);
            if !matches_name && !matches_id {
                continue;
            }
        }

        visible_count += 1;
        let is_selected = selected_trace_id == Some(trace.trace_id);
        trace_list = trace_list.push(view_trace_row(trace, is_selected));
    }

    if visible_count == 0 {
        return container(
            column![
                text("No traces found").size(FONT_SIZE_BASE).color(TEXT_MUTED),
                text("Emit spans from your Python application or adjust your active filters.")
                    .size(FONT_SIZE_SM)
                    .color(TEXT_MUTED),
            ]
            .spacing(8)
            .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into();
    }

    scrollable(trace_list)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn view_trace_row<'a>(trace: &'a TraceSummary, is_selected: bool) -> Element<'a, Message> {
    // Status color dot
    let status_color = match trace.status {
        SpanStatus::Ok => ACCENT_SUCCESS,
        SpanStatus::Error { .. } => ACCENT_ERROR,
    };

    let status_dot = container(text(""))
        .width(8.0)
        .height(8.0)
        .style(badge_container(status_color, status_color));

    // Kind badge
    let kind_color = color_for_kind(trace.root_kind);
    let kind_badge = container(
        text(trace.root_kind.as_str().to_uppercase())
            .size(FONT_SIZE_XS)
            .color(kind_color),
    )
    .padding(Padding::from([2.0, 6.0]))
    .style(badge_container(bg_tint_for_kind(trace.root_kind), kind_color));

    // Name & Kind
    let title_row = row![
        status_dot,
        text(trace.root_name.as_str())
            .size(FONT_SIZE_SM)
            .color(TEXT_PRIMARY),
        kind_badge,
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    // Duration color
    let duration_color = match trace.duration_nanos {
        Some(nanos) if nanos > 1_500_000_000 => ACCENT_ERROR,
        Some(nanos) if nanos > 250_000_000 => ACCENT_WARNING,
        _ => ACCENT_SUCCESS,
    };

    let duration_text = text(trace.formatted_duration())
        .size(FONT_SIZE_SM)
        .color(duration_color);

    let time_ago_text = text(trace.formatted_time_ago())
        .size(FONT_SIZE_XS)
        .color(TEXT_MUTED);

    let metrics_text = if trace.total_tokens > 0 {
        format!("{} spans · {} tok · ${:.4}", trace.span_count, trace.total_tokens, trace.total_cost_usd)
    } else {
        format!("{} spans", trace.span_count)
    };

    let content = column![
        row![
            title_row,
            iced::widget::horizontal_space(),
            duration_text,
        ]
        .align_y(Alignment::Center),
        row![
            text(metrics_text).size(FONT_SIZE_XS).color(TEXT_MUTED),
            iced::widget::horizontal_space(),
            time_ago_text,
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(4);

    let card_style = if is_selected {
        selected_card
    } else {
        surface_card
    };

    let trace_id = trace.trace_id;

    button(
        container(content)
            .width(Length::Fill)
            .padding(10.0)
            .style(card_style),
    )
    .on_press(Message::SelectTrace(trace_id))
    .style(iced::widget::button::text)
    .padding(0.0)
    .width(Length::Fill)
    .into()
}
