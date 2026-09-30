use iced::widget::{button, column, container, horizontal_space, row, text, text_input};
use iced::{Alignment, Color, Element, Length, Padding};

use crate::state::{Message, ThemeMode};
use crate::theme::*;

/// Renders the Studio Settings modal panel for font scaling, light/dark mode, and FastAPI configuration.
pub fn view_settings_modal<'a>(
    ui_scale: f64,
    theme_mode: ThemeMode,
    fastapi_port: &'a str,
) -> Element<'a, Message> {
    // 1. Header
    let title_row = row![
        text("Studio Settings").size(FONT_SIZE_MD),
        horizontal_space(),
        button(text("Close").size(FONT_SIZE_XS))
            .on_press(Message::ToggleSettings)
            .style(nav_button)
            .padding(Padding::from([3.0, 10.0])),
    ]
    .align_y(Alignment::Center);

    // 2. UI & Font Scale Section
    let scale_pct = format!("{:.0}%", ui_scale * 100.0);
    let scale_controls = row![
        button(text("-").size(FONT_SIZE_SM))
            .on_press(Message::DecreaseFontSize)
            .style(nav_button)
            .padding(Padding::from([4.0, 12.0])),
        container(
            text(scale_pct)
                .size(FONT_SIZE_SM)
        )
        .padding(Padding::from([4.0, 14.0]))
        .style(surface_card),
        button(text("+").size(FONT_SIZE_SM))
            .on_press(Message::IncreaseFontSize)
            .style(nav_button)
            .padding(Padding::from([4.0, 12.0])),
        button(text("Reset").size(FONT_SIZE_XS))
            .on_press(Message::ResetFontSize)
            .style(nav_button)
            .padding(Padding::from([4.0, 10.0])),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let scale_section = column![
        text("UI & Font Scale").size(FONT_SIZE_SM),
        text("Scale the typography and interface dimensions across all panels")
            .size(FONT_SIZE_XS)
            .color(TEXT_MUTED),
        scale_controls,
    ]
    .spacing(6);

    // 3. Theme Mode Section
    let is_dark = theme_mode == ThemeMode::Dark;
    let is_light = theme_mode == ThemeMode::Light;

    let dark_btn = button(text("Dark Mode").size(FONT_SIZE_SM))
        .on_press(Message::SetTheme(ThemeMode::Dark))
        .style(if is_dark { active_nav_button } else { nav_button })
        .padding(Padding::from([6.0, 14.0]));

    let light_btn = button(text("Light Mode").size(FONT_SIZE_SM))
        .on_press(Message::SetTheme(ThemeMode::Light))
        .style(if is_light { active_nav_button } else { nav_button })
        .padding(Padding::from([6.0, 14.0]));

    let theme_section = column![
        text("Appearance Theme").size(FONT_SIZE_SM),
        text("Toggle between dark slate telemetry mode and high-contrast light mode")
            .size(FONT_SIZE_XS)
            .color(TEXT_MUTED),
        row![dark_btn, light_btn].spacing(8),
    ]
    .spacing(6);

    // 4. FastAPI / Uvicorn Integration Section
    let port_input = text_input("8000", fastapi_port)
        .on_input(Message::FastApiPortChanged)
        .padding(Padding::from([4.0, 8.0]))
        .width(Length::Fixed(80.0))
        .style(dark_text_input);

    let port_badge = container(
        row![
            container(text("")).width(6.0).height(6.0).style(badge_container(ACCENT_SUCCESS, ACCENT_SUCCESS)),
            text("Port Ready").size(FONT_SIZE_XS).color(ACCENT_SUCCESS),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([3.0, 8.0]))
    .style(badge_container(Color::from_rgba(0.063, 0.725, 0.506, 0.12), ACCENT_SUCCESS));

    let snippet_code = format!(
r#"# FastAPI ASGI Native Integration
from fastapi import FastAPI
from poom.middleware import PoomMiddleware

app = FastAPI()
# Auto-instruments HTTP boundaries, headers & errors
app.add_middleware(PoomMiddleware, app_name="fastapi_service")

# Run natively via Uvicorn:
# uvicorn main:app --port {fastapi_port}"#
    );

    let copy_snippet_btn = button(text("Copy FastAPI Code").size(FONT_SIZE_XS))
        .on_press(Message::CopyToClipboard(snippet_code.clone()))
        .style(active_nav_button)
        .padding(Padding::from([4.0, 10.0]));

    let fastapi_section = column![
        row![
            text("FastAPI / Uvicorn Service").size(FONT_SIZE_SM),
            horizontal_space(),
            port_badge,
        ]
        .align_y(Alignment::Center),
        text(format!("Native tracing configured for FastAPI apps running on port {fastapi_port}."))
            .size(FONT_SIZE_XS)
            .color(TEXT_MUTED),
        row![
            text("Uvicorn Port:").size(FONT_SIZE_XS).color(TEXT_MUTED),
            port_input,
            horizontal_space(),
            copy_snippet_btn,
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        container(
            text(snippet_code)
                .size(FONT_SIZE_XS)
        )
        .padding(10.0)
        .width(Length::Fill)
        .style(code_box_container),
    ]
    .spacing(8);

    // Modal Card Content
    let content = column![
        title_row,
        container(text("")).height(1.0).width(Length::Fill).style(badge_container(BORDER_SUBTLE, BORDER_SUBTLE)),
        scale_section,
        theme_section,
        fastapi_section,
    ]
    .spacing(14)
    .padding(16.0);

    container(content)
        .width(Length::Fixed(490.0))
        .style(modal_container)
        .into()
}
