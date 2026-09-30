pub mod attributes;
pub mod evaluations;
pub mod events;
pub mod overview;
pub mod prompts;

use iced::widget::{button, column, container, row, text};
use iced::{Element, Length, Padding};

use poom_types::{EvaluationRecord, SpanRecord};

use crate::state::{InspectorTab, Message};
use crate::theme::*;

pub fn view_inspector<'a>(
    selected_span: Option<&'a SpanRecord>,
    evaluations: &'a [EvaluationRecord],
    active_tab: InspectorTab,
    attribute_filter: &'a str,
) -> Element<'a, Message> {
    let Some(span) = selected_span else {
        return container(
            column![
                text("No span selected").size(FONT_SIZE_BASE).color(TEXT_MUTED),
                text("Click any execution bar in the waterfall canvas above to inspect details.")
                    .size(FONT_SIZE_SM)
                    .color(TEXT_MUTED),
            ]
            .spacing(6)
            .align_x(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into();
    };

    // Tab Bar
    let mut tab_buttons = row![].spacing(6);

    for tab in InspectorTab::ALL {
        let is_active = tab == active_tab;
        let btn_style = if is_active {
            active_nav_button
        } else {
            nav_button
        };

        let label = match tab {
            InspectorTab::Overview => "Overview".to_string(),
            InspectorTab::Prompts => "Prompts & I/O".to_string(),
            InspectorTab::Attributes => format!("Attributes ({})", span.attributes.len()),
            InspectorTab::Events => format!("Events ({})", span.events.len()),
            InspectorTab::Evaluations => format!("Evaluations ({})", evaluations.len()),
        };

        tab_buttons = tab_buttons.push(
            button(
                text(label)
                    .size(FONT_SIZE_XS)
                    .color(if is_active { TEXT_PRIMARY } else { TEXT_SECONDARY }),
            )
            .on_press(Message::InspectorTabSelected(tab))
            .style(btn_style)
            .padding(Padding::from([5.0, 10.0])),
        );
    }

    let tab_bar = container(tab_buttons)
        .width(Length::Fill)
        .padding(Padding::from([8.0, 12.0]))
        .style(header_container);

    // Active tab body
    let body: Element<'a, Message> = match active_tab {
        InspectorTab::Overview => overview::view_overview(span),
        InspectorTab::Prompts => prompts::view_prompts(span),
        InspectorTab::Attributes => attributes::view_attributes(span, attribute_filter),
        InspectorTab::Events => events::view_events(span),
        InspectorTab::Evaluations => evaluations::view_evaluations(evaluations),
    };

    column![tab_bar, body]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
