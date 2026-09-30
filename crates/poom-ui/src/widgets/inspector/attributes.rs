use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Element, Length, Padding};

use poom_types::{AttributeValue, SpanRecord};

use crate::state::Message;
use crate::theme::*;

pub fn view_attributes<'a>(
    span: &'a SpanRecord,
    filter_query: &'a str,
) -> Element<'a, Message> {
    let search_bar = text_input("Filter attributes by key...", filter_query)
        .on_input(Message::AttributeFilterChanged)
        .padding(8.0)
        .size(FONT_SIZE_SM)
        .style(dark_text_input);

    let mut rows = column![].spacing(6);
    let mut match_count = 0;

    for (k, v) in &span.attributes {
        if !filter_query.is_empty() && !k.to_ascii_lowercase().contains(&filter_query.to_ascii_lowercase()) {
            continue;
        }

        match_count += 1;
        let val_formatted = format_attr(v);

        let row_item = row![
            text(k.as_str())
                .size(FONT_SIZE_SM)
                .color(KIND_CHAIN)
                .width(Length::Fixed(180.0)),
            text(val_formatted.clone())
                .size(FONT_SIZE_SM)
                .color(TEXT_PRIMARY)
                .width(Length::Fill),
            button(text("Copy").size(FONT_SIZE_XS).color(TEXT_MUTED))
                .on_press(Message::CopyToClipboard(val_formatted))
                .style(nav_button),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        rows = rows.push(
            container(row_item)
                .width(Length::Fill)
                .padding(Padding::from([6.0, 10.0]))
                .style(surface_card),
        );
    }

    if match_count == 0 {
        rows = rows.push(
            container(
                text("No attributes match your filter.")
                    .size(FONT_SIZE_SM)
                    .color(TEXT_MUTED),
            )
            .padding(16.0),
        );
    }

    let body = column![
        search_bar,
        scrollable(rows).width(Length::Fill).height(Length::Fill),
    ]
    .spacing(10)
    .padding(Padding::from(12.0));

    body.into()
}

fn format_attr(val: &AttributeValue) -> String {
    match val {
        AttributeValue::String(s) => s.clone(),
        AttributeValue::Smol(s) => s.to_string(),
        AttributeValue::Bool(b) => b.to_string(),
        AttributeValue::Int(i) => i.to_string(),
        AttributeValue::Float(f) => format!("{f:.4}"),
        AttributeValue::Bytes(b) => format!("[{} bytes]", b.len()),
        AttributeValue::Null => "null".to_string(),
        AttributeValue::Array(a) => format!("[{} items]", a.len()),
        AttributeValue::Map(m) => format!("{{{}}}", m.keys().cloned().collect::<Vec<_>>().join(", ")),
    }
}
