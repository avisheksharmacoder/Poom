use iced::widget::{button, container, row, text};
use iced::{Alignment, Color, Element, Length, Padding};

use crate::state::Message;
use crate::theme::*;

pub fn view_status_bar<'a>(
    db_path: &'a str,
    live_event_count: usize,
    canvas_zoom: f32,
    show_settings: bool,
    fastapi_port: &'a str,
) -> Element<'a, Message> {
    // 1. Settings text button at the bottom left
    let settings_btn = button(text("Settings").size(FONT_SIZE_XS))
        .on_press(Message::ToggleSettings)
        .style(if show_settings {
            active_nav_button
        } else {
            nav_button
        })
        .padding(Padding::from([3.0, 10.0]));

    // 2. Pre-configured FastAPI Uvicorn Port indicator badge
    let fastapi_badge = container(
        row![
            text("FastAPI:").size(FONT_SIZE_XS).color(TEXT_MUTED),
            text(fastapi_port).size(FONT_SIZE_XS).color(ACCENT_SUCCESS),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([2.0, 7.0]))
    .style(badge_container(Color::from_rgba(0.063, 0.725, 0.506, 0.12), BORDER_SUBTLE));

    // 3. Storage and Live ingestion status
    let storage_status = row![
        container(text("")).width(6.0).height(6.0).style(badge_container(ACCENT_SUCCESS, ACCENT_SUCCESS)),
        text(format!("Storage: redb ({db_path})"))
            .size(FONT_SIZE_XS)
            .color(TEXT_MUTED),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let stream_status = row![
        container(text("")).width(6.0).height(6.0).style(badge_container(ACCENT_INFO, ACCENT_INFO)),
        text(format!("Live Events: {live_event_count}"))
            .size(FONT_SIZE_XS)
            .color(TEXT_MUTED),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let zoom_info = text(format!("Zoom: {:.1}x", canvas_zoom))
        .size(FONT_SIZE_XS)
        .color(TEXT_MUTED);

    let reset_view_btn = button(text("Reset View").size(FONT_SIZE_XS).color(TEXT_MUTED))
        .on_press(Message::ResetCanvasView)
        .style(nav_button)
        .padding(Padding::from([2.0, 6.0]));

    let status_row = row![
        settings_btn,
        fastapi_badge,
        text("·").size(FONT_SIZE_XS).color(TEXT_MUTED),
        storage_status,
        text("·").size(FONT_SIZE_XS).color(TEXT_MUTED),
        stream_status,
        iced::widget::horizontal_space(),
        zoom_info,
        reset_view_btn,
    ]
    .spacing(10)
    .padding(Padding::from([4.0, 10.0]))
    .align_y(Alignment::Center);

    container(status_row)
        .width(Length::Fill)
        .style(header_container)
        .into()
}
