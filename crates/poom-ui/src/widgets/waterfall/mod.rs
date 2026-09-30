pub mod colors;
pub mod layout;
pub mod program;
pub mod tooltip;

use iced::widget::canvas::Canvas;
use iced::{Element, Length};

pub use layout::{
    calculate_bar_geometry, flatten_span_tree, SpanLayoutItem, TraceBounds, HEADER_HEIGHT,
    MIN_BAR_WIDTH, ROW_HEIGHT, TREE_LABEL_WIDTH,
};
pub use program::WaterfallProgram;

use crate::state::{CanvasViewState, Message};
use poom_types::SpanId;

/// Constructs the interactive 2D Waterfall canvas element.
pub fn view_waterfall<'a>(
    items: &'a [SpanLayoutItem],
    bounds: Option<TraceBounds>,
    selected_span_id: Option<SpanId>,
    view_state: &'a CanvasViewState,
) -> Element<'a, Message> {
    Canvas::new(WaterfallProgram {
        items,
        bounds,
        selected_span_id,
        view_state,
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
