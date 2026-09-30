use iced::widget::{button, container, row, text, text_input};
use iced::{Alignment, Element, Length, Padding};

use crate::state::{Message, StatusFilter, TimeWindowPreset, TraceSummary};
use crate::theme::*;

pub fn view_top_bar<'a>(
    search_query: &'a str,
    time_preset: TimeWindowPreset,
    status_filter: StatusFilter,
    auto_scroll: bool,
    traces: &'a [TraceSummary],
) -> Element<'a, Message> {
    // 1. Brand Logo
    let brand = row![
        text("POOM")
            .size(FONT_SIZE_MD)
            .color(KIND_AGENT),
        container(
            text("STUDIO")
                .size(FONT_SIZE_XS)
                .color(TEXT_MUTED),
        )
        .padding(Padding::from([2.0, 5.0]))
        .style(badge_container(BG_CARD, BORDER_SUBTLE)),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    // 2. Search Box
    let search = text_input("Search traces, models, or tags...", search_query)
        .on_input(Message::SearchQueryChanged)
        .padding(6.0)
        .size(FONT_SIZE_SM)
        .width(Length::Fixed(240.0))
        .style(dark_text_input);

    // 3. Time Preset Buttons
    let mut time_buttons = row![].spacing(3);
    for preset in TimeWindowPreset::ALL {
        let is_selected = preset == time_preset;
        let btn_style = if is_selected {
            active_nav_button
        } else {
            nav_button
        };

        time_buttons = time_buttons.push(
            button(
                text(preset.as_str())
                    .size(FONT_SIZE_XS)
                    .color(if is_selected { TEXT_PRIMARY } else { TEXT_SECONDARY }),
            )
            .on_press(Message::TimeWindowChanged(preset))
            .style(btn_style)
            .padding(Padding::from([4.0, 8.0])),
        );
    }

    // 4. Status Filter Buttons
    let mut status_buttons = row![].spacing(3);
    for filter in StatusFilter::ALL {
        let is_selected = filter == status_filter;
        let btn_style = if is_selected {
            active_nav_button
        } else {
            nav_button
        };

        status_buttons = status_buttons.push(
            button(
                text(filter.as_str())
                    .size(FONT_SIZE_XS)
                    .color(if is_selected { TEXT_PRIMARY } else { TEXT_SECONDARY }),
            )
            .on_press(Message::StatusFilterChanged(filter))
            .style(btn_style)
            .padding(Padding::from([4.0, 8.0])),
        );
    }

    // 5. Aggregate Stats Banner
    let total_traces = traces.len();
    let error_traces = traces.iter().filter(|t| t.status.is_error()).count();
    let total_cost: f64 = traces.iter().map(|t| t.total_cost_usd).sum();

    let stats_text = if total_traces > 0 {
        format!("{total_traces} traces · {error_traces} err · ${total_cost:.4}")
    } else {
        "0 traces".to_string()
    };

    let stats_badge = container(
        text(stats_text)
            .size(FONT_SIZE_XS)
            .color(TEXT_SECONDARY),
    )
    .padding(Padding::from([4.0, 8.0]))
    .style(badge_container(BG_PRIMARY, BORDER_SUBTLE));

    // 6. Action controls (Auto-scroll & Refresh)
    let auto_scroll_btn = button(
        text(if auto_scroll { "Auto-Scroll: ON" } else { "Auto-Scroll: OFF" })
            .size(FONT_SIZE_XS)
            .color(if auto_scroll { ACCENT_SUCCESS } else { TEXT_MUTED }),
    )
    .on_press(Message::ToggleAutoScroll(!auto_scroll))
    .style(nav_button)
    .padding(Padding::from([4.0, 8.0]));

    let refresh_btn = button(text("Refresh").size(FONT_SIZE_XS).color(TEXT_SECONDARY))
        .on_press(Message::RefreshTraces)
        .style(nav_button)
        .padding(Padding::from([4.0, 8.0]));

    let top_bar_row = row![
        brand,
        search,
        time_buttons,
        status_buttons,
        iced::widget::horizontal_space(),
        stats_badge,
        auto_scroll_btn,
        refresh_btn,
    ]
    .spacing(12)
    .padding(Padding::from([8.0, 14.0]))
    .align_y(Alignment::Center);

    container(top_bar_row)
        .width(Length::Fill)
        .style(header_container)
        .into()
}
