use iced::widget::{column, container, row, scrollable, text};
use iced::{Alignment, Color, Element, Length, Padding};

use poom_types::SpanRecord;

use crate::state::Message;
use crate::theme::*;

pub fn view_events<'a>(span: &'a SpanRecord) -> Element<'a, Message> {
    if span.events.is_empty() {
        return container(
            column![
                text("No point-in-time events recorded for this span.")
                    .size(FONT_SIZE_BASE)
                    .color(TEXT_MUTED),
                text("Events record fine-grained internal milestones (e.g. first token stream arrival, tool retries).")
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

    let mut event_list = column![].spacing(8).padding(Padding::from(12.0));

    for event in &span.events {
        let offset_nanos = event.timestamp_unix_nanos.saturating_sub(span.start_time_unix_nanos);
        let offset_ms = offset_nanos as f64 / 1_000_000.0;

        let dot = container(text(""))
            .width(6.0)
            .height(6.0)
            .style(badge_container(ACCENT_INFO, ACCENT_INFO));

        let offset_tag = container(
            text(format!("+{offset_ms:.1}ms"))
                .size(FONT_SIZE_XS)
                .color(ACCENT_INFO),
        )
        .padding(Padding::from([2.0, 6.0]))
        .style(badge_container(Color::from_rgba(0.2, 0.4, 0.8, 0.15), ACCENT_INFO));

        let event_row = row![
            dot,
            offset_tag,
            text(event.name.as_str())
                .size(FONT_SIZE_SM)
                .color(TEXT_PRIMARY),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let mut event_card = column![event_row].spacing(6);

        if !event.attributes.is_empty() {
            let attr_summary = event.attributes
                .iter()
                .map(|(k, v)| format!("{k}: {:?}", v))
                .collect::<Vec<_>>()
                .join(" · ");

            event_card = event_card.push(
                text(attr_summary)
                    .size(FONT_SIZE_XS)
                    .color(TEXT_MUTED),
            );
        }

        event_list = event_list.push(
            container(event_card)
                .width(Length::Fill)
                .padding(10.0)
                .style(surface_card),
        );
    }

    scrollable(event_list).width(Length::Fill).height(Length::Fill).into()
}
