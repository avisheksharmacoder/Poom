use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Alignment, Color, Element, Length, Padding};

use poom_types::{AttributeValue, SpanRecord};

use crate::state::Message;
use crate::theme::*;

/// Inspects span attributes for LLM prompts, tool inputs, and model completions.
pub fn view_prompts<'a>(span: &'a SpanRecord) -> Element<'a, Message> {
    let mut cards = column![].spacing(14).padding(Padding::from(12.0));

    let mut found_prompts = false;

    // Check common input keys
    for key in ["system_prompt", "prompt", "user_prompt", "messages", "input", "arguments", "query"] {
        if let Some(val) = span.attributes.get(key) {
            found_prompts = true;
            cards = cards.push(render_prompt_card(key, val, "INPUT / PROMPT", KIND_AGENT));
        }
    }

    // Check common output keys
    for key in ["completion", "response", "output", "result", "content", "tool_calls"] {
        if let Some(val) = span.attributes.get(key) {
            found_prompts = true;
            cards = cards.push(render_prompt_card(key, val, "OUTPUT / COMPLETION", KIND_LLM));
        }
    }

    if !found_prompts {
        return container(
            column![
                text("No prompt or completion attributes recorded on this span.")
                    .size(FONT_SIZE_BASE)
                    .color(TEXT_MUTED),
                text("Spans recorded with @trace_tool or LLM wrappers will display prompts and model outputs here.")
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

    scrollable(cards).width(Length::Fill).height(Length::Fill).into()
}

fn render_prompt_card<'a>(
    key: &'a str,
    val: &'a AttributeValue,
    category: &'static str,
    accent_color: Color,
) -> Element<'a, Message> {
    let val_str = format_attribute_value(val);

    let header = row![
        container(
            text(category)
                .size(FONT_SIZE_XS)
                .color(accent_color),
        )
        .padding(Padding::from([2.0, 6.0]))
        .style(badge_container(Color::from_rgba(0.2, 0.3, 0.5, 0.15), accent_color)),
        text(format!("Key: {key}"))
            .size(FONT_SIZE_SM)
            .color(TEXT_SECONDARY),
        iced::widget::horizontal_space(),
        button(text("Copy").size(FONT_SIZE_XS).color(TEXT_SECONDARY))
            .on_press(Message::CopyToClipboard(val_str.clone()))
            .style(nav_button),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let content_text = text(val_str)
        .size(FONT_SIZE_SM)
        .color(TEXT_PRIMARY);

    let body = container(scrollable(content_text).height(Length::Shrink))
        .padding(10.0)
        .style(surface_card);

    container(column![header, body].spacing(8))
        .width(Length::Fill)
        .padding(10.0)
        .style(surface_card)
        .into()
}

fn format_attribute_value(val: &AttributeValue) -> String {
    match val {
        AttributeValue::String(s) => s.clone(),
        AttributeValue::Smol(s) => s.to_string(),
        AttributeValue::Bool(b) => b.to_string(),
        AttributeValue::Int(i) => i.to_string(),
        AttributeValue::Float(f) => f.to_string(),
        AttributeValue::Bytes(b) => format!("Binary payload ({} bytes)", b.len()),
        AttributeValue::Null => "null".to_string(),
        AttributeValue::Array(arr) => {
            serde_json::to_string_pretty(arr).unwrap_or_else(|_| format!("{arr:?}"))
        }
        AttributeValue::Map(map) => {
            serde_json::to_string_pretty(map).unwrap_or_else(|_| format!("{map:?}"))
        }
    }
}
