use iced::widget::{column, container, text};
use iced::{Alignment, Element, Length};

use crate::state::Message;
use crate::theme::*;

pub fn view_empty_canvas<'a>() -> Element<'a, Message> {
    container(
        column![
            text("Select a trace to visualize execution").size(FONT_SIZE_LG).color(TEXT_SECONDARY),
            text("The waterfall canvas will render hierarchical timeline bars, tool invocations, and agent loops.")
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
    .into()
}
