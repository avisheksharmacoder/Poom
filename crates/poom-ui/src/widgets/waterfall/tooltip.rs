use iced::widget::canvas::{Frame, Path, Stroke};
use iced::{Point, Rectangle, Size};

use poom_types::{SpanRecord, SpanStatus};
use crate::theme::*;

/// Draws a rich hover tooltip overlay directly into the canvas frame.
pub fn draw_hover_tooltip(
    frame: &mut Frame,
    span: &SpanRecord,
    cursor: Point,
    bounds: Rectangle,
) {
    let tooltip_width = 240.0;
    let tooltip_height = 80.0;

    // Anchor tooltip avoiding screen edges
    let mut x = cursor.x + 12.0;
    let mut y = cursor.y + 12.0;

    if x + tooltip_width > bounds.width {
        x = cursor.x - tooltip_width - 12.0;
    }
    if y + tooltip_height > bounds.height {
        y = cursor.y - tooltip_height - 12.0;
    }

    let tooltip_rect = Path::rectangle(Point::new(x, y), Size::new(tooltip_width, tooltip_height));

    // Background & Border
    frame.fill(&tooltip_rect, BG_SURFACE);
    frame.stroke(
        &tooltip_rect,
        Stroke::default().with_color(BORDER_ACCENT).with_width(1.0),
    );

    // Text details
    let mut text_y = y + 10.0;

    // Name + Kind
    let kind_color = color_for_kind(span.kind);
    frame.fill_text(iced::widget::canvas::Text {
        content: format!("[{}] {}", span.kind.as_str().to_uppercase(), span.name),
        position: Point::new(x + 10.0, text_y),
        color: kind_color,
        size: iced::Pixels(FONT_SIZE_SM),
        font: FONT_DEFAULT,
        ..Default::default()
    });

    text_y += 20.0;

    // Duration & Status
    let dur_text = span.duration_millis()
        .map(|ms| if ms >= 1000.0 { format!("{:.2}s", ms / 1000.0) } else { format!("{:.1}ms", ms) })
        .unwrap_or_else(|| "running".to_string());

    let status_str = match span.status {
        SpanStatus::Ok => "Status: Ok",
        SpanStatus::Error { .. } => "Status: Error",
    };

    frame.fill_text(iced::widget::canvas::Text {
        content: format!("Duration: {dur_text} · {status_str}"),
        position: Point::new(x + 10.0, text_y),
        color: TEXT_PRIMARY,
        size: iced::Pixels(FONT_SIZE_XS),
        font: FONT_DEFAULT,
        ..Default::default()
    });

    text_y += 18.0;

    // Tokens & Cost
    let tok_info = if let Some(tot) = span.metrics.total_tokens {
        let cost = span.metrics.estimated_cost_usd.unwrap_or(0.0);
        format!("Tokens: {tot} · ${cost:.4}")
    } else {
        format!("Span ID: {:.8}...", span.span_id.to_string())
    };

    frame.fill_text(iced::widget::canvas::Text {
        content: tok_info,
        position: Point::new(x + 10.0, text_y),
        color: TEXT_MUTED,
        size: iced::Pixels(FONT_SIZE_XS),
        font: FONT_DEFAULT,
        ..Default::default()
    });
}
