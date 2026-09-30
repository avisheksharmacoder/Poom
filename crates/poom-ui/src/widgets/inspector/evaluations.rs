use iced::widget::{column, container, row, scrollable, text};
use iced::{Alignment, Color, Element, Length, Padding};

use poom_types::{EvaluationRecord, EvaluationValue};

use crate::state::Message;
use crate::theme::*;

pub fn view_evaluations<'a>(evaluations: &'a [EvaluationRecord]) -> Element<'a, Message> {
    if evaluations.is_empty() {
        return container(
            column![
                text("No evaluations recorded for this trace.")
                    .size(FONT_SIZE_BASE)
                    .color(TEXT_MUTED),
                text("Record evaluation scores (hallucination metrics, user feedback) via poom.record_evaluation().")
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

    let mut eval_list = column![].spacing(8).padding(Padding::from(12.0));

    for eval in evaluations {
        let (val_badge_text, val_badge_color) = match &eval.value {
            EvaluationValue::Numeric(f) => (format!("{f:.2}"), ACCENT_SUCCESS),
            EvaluationValue::Boolean(true) => ("PASS".to_string(), ACCENT_SUCCESS),
            EvaluationValue::Boolean(false) => ("FAIL".to_string(), ACCENT_ERROR),
            EvaluationValue::Categorical(c) => (c.to_string(), ACCENT_WARNING),
        };

        let badge = container(
            text(val_badge_text)
                .size(FONT_SIZE_XS)
                .color(val_badge_color),
        )
        .padding(Padding::from([2.0, 8.0]))
        .style(badge_container(Color::from_rgba(0.2, 0.5, 0.4, 0.15), val_badge_color));

        let header = row![
            text(eval.name.as_str())
                .size(FONT_SIZE_SM)
                .color(TEXT_PRIMARY),
            badge,
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let mut card = column![header].spacing(6);

        if let Some(comment) = &eval.comment {
            card = card.push(
                text(comment)
                    .size(FONT_SIZE_SM)
                    .color(TEXT_SECONDARY),
            );
        }

        eval_list = eval_list.push(
            container(card)
                .width(Length::Fill)
                .padding(10.0)
                .style(surface_card),
        );
    }

    scrollable(eval_list).width(Length::Fill).height(Length::Fill).into()
}
