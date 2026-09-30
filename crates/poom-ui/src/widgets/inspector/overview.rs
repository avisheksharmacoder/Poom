use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Alignment, Color, Element, Length, Padding};

use poom_types::{SpanRecord, SpanStatus};

use crate::state::Message;
use crate::theme::*;

pub fn view_overview<'a>(span: &'a SpanRecord) -> Element<'a, Message> {
    let mut content = column![].spacing(12).padding(Padding::from(12.0));

    // 1. Header Card: Name, Kind, Status
    let kind_color = color_for_kind(span.kind);
    let kind_badge = container(
        text(span.kind.as_str().to_uppercase())
            .size(FONT_SIZE_XS)
            .color(kind_color),
    )
    .padding(Padding::from([3.0, 8.0]))
    .style(badge_container(bg_tint_for_kind(span.kind), kind_color));

    let status_badge = match &span.status {
        SpanStatus::Ok => container(
            text("OK")
                .size(FONT_SIZE_XS)
                .color(ACCENT_SUCCESS),
        )
        .padding(Padding::from([3.0, 8.0]))
        .style(badge_container(Color::from_rgba(0.06, 0.72, 0.50, 0.15), ACCENT_SUCCESS)),

        SpanStatus::Error { error_type, .. } => container(
            text(format!("ERROR: {error_type}"))
                .size(FONT_SIZE_XS)
                .color(ACCENT_ERROR),
        )
        .padding(Padding::from([3.0, 8.0]))
        .style(badge_container(Color::from_rgba(0.93, 0.26, 0.26, 0.15), ACCENT_ERROR)),
    };

    let title_row = row![
        text(span.name.as_str())
            .size(FONT_SIZE_MD)
            .color(TEXT_PRIMARY),
        kind_badge,
        status_badge,
        iced::widget::horizontal_space(),
        button(text("Copy Span ID").size(FONT_SIZE_XS).color(TEXT_SECONDARY))
            .on_press(Message::CopyToClipboard(span.span_id.to_string()))
            .style(nav_button),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    content = content.push(container(title_row).width(Length::Fill).padding(10.0).style(surface_card));

    // 2. Error Diagnostic Details (if error)
    if let SpanStatus::Error { error_type, message, backtrace } = &span.status {
        let mut err_col = column![
            text(format!("Error Type: {error_type}")).size(FONT_SIZE_SM).color(ACCENT_ERROR),
            text(format!("Message: {message}")).size(FONT_SIZE_SM).color(TEXT_PRIMARY),
        ].spacing(6);

        if let Some(bt) = backtrace {
            err_col = err_col.push(
                container(
                    scrollable(
                        text(bt).size(FONT_SIZE_XS).color(TEXT_SECONDARY),
                    )
                    .height(Length::Fixed(120.0)),
                )
                .padding(8.0)
                .style(surface_card),
            );
        }

        content = content.push(
            container(err_col)
                .width(Length::Fill)
                .padding(12.0)
                .style(badge_container(Color::from_rgba(0.93, 0.26, 0.26, 0.1), ACCENT_ERROR)),
        );
    }

    // 3. Execution Metrics & Timing Cards
    let dur_str = span.duration_millis()
        .map(|ms| if ms >= 1000.0 { format!("{:.3} s", ms / 1000.0) } else { format!("{:.2} ms", ms) })
        .unwrap_or_else(|| "In-flight...".to_string());

    let timing_card = container(
        column![
            text("TIMING & DURATION").size(FONT_SIZE_XS).color(TEXT_MUTED),
            text(dur_str).size(FONT_SIZE_LG).color(TEXT_PRIMARY),
            text(format!("Start Nanos: {}", span.start_time_unix_nanos))
                .size(FONT_SIZE_XS)
                .color(TEXT_MUTED),
        ]
        .spacing(4),
    )
    .width(Length::FillPortion(1))
    .padding(12.0)
    .style(surface_card);

    let metrics_card = container(
        column![
            text("TOKEN CONSUMPTION").size(FONT_SIZE_XS).color(TEXT_MUTED),
            text(format!("Total: {} tokens", span.metrics.total_tokens.unwrap_or(0)))
                .size(FONT_SIZE_BASE)
                .color(TEXT_PRIMARY),
            text(format!(
                "In: {} · Out: {} · Cached: {}",
                span.metrics.input_tokens.unwrap_or(0),
                span.metrics.output_tokens.unwrap_or(0),
                span.metrics.cached_tokens.unwrap_or(0),
            ))
            .size(FONT_SIZE_XS)
            .color(TEXT_SECONDARY),
        ]
        .spacing(4),
    )
    .width(Length::FillPortion(1))
    .padding(12.0)
    .style(surface_card);

    let cost_card = container(
        column![
            text("FINANCIAL COST").size(FONT_SIZE_XS).color(TEXT_MUTED),
            text(format!("${:.5} USD", span.metrics.estimated_cost_usd.unwrap_or(0.0)))
                .size(FONT_SIZE_LG)
                .color(ACCENT_SUCCESS),
            text(format!("Reasoning: {} tokens", span.metrics.reasoning_tokens.unwrap_or(0)))
                .size(FONT_SIZE_XS)
                .color(TEXT_MUTED),
        ]
        .spacing(4),
    )
    .width(Length::FillPortion(1))
    .padding(12.0)
    .style(surface_card);

    content = content.push(row![timing_card, metrics_card, cost_card].spacing(10).width(Length::Fill));

    // 4. Identifiers & Hierarchy Info
    let id_info = column![
        row![
            text("Trace ID:").size(FONT_SIZE_XS).color(TEXT_MUTED).width(Length::Fixed(90.0)),
            text(span.trace_id.to_string()).size(FONT_SIZE_XS).color(TEXT_SECONDARY),
        ].spacing(8),
        row![
            text("Span ID:").size(FONT_SIZE_XS).color(TEXT_MUTED).width(Length::Fixed(90.0)),
            text(span.span_id.to_string()).size(FONT_SIZE_XS).color(TEXT_SECONDARY),
        ].spacing(8),
        row![
            text("Parent ID:").size(FONT_SIZE_XS).color(TEXT_MUTED).width(Length::Fixed(90.0)),
            text(span.parent_span_id.map(|id| id.to_string()).unwrap_or_else(|| "Root Span (None)".to_string()))
                .size(FONT_SIZE_XS)
                .color(TEXT_SECONDARY),
        ].spacing(8),
    ].spacing(6);

    content = content.push(
        container(id_info)
            .width(Length::Fill)
            .padding(12.0)
            .style(surface_card),
    );

    scrollable(content).width(Length::Fill).height(Length::Fill).into()
}
