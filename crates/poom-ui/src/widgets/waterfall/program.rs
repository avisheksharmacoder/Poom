use iced::mouse::{self, Cursor};
use iced::widget::canvas::{self, Frame, Geometry, Path, Program, Stroke};
use iced::{Color, Point, Rectangle, Renderer, Size, Theme, Vector};

use poom_types::{SpanId, SpanStatus};

use super::layout::*;
use super::tooltip::draw_hover_tooltip;
use crate::state::{CanvasViewState, Message};
use crate::theme::*;

/// Canvas Program for high-performance immediate-mode 2D rendering of the trace waterfall.
pub struct WaterfallProgram<'a> {
    pub items: &'a [SpanLayoutItem],
    pub bounds: Option<TraceBounds>,
    pub selected_span_id: Option<SpanId>,
    pub view_state: &'a CanvasViewState,
}

#[derive(Default)]
pub struct WaterfallLocalState;

impl<'a> Program<Message, Theme, Renderer> for WaterfallProgram<'a> {
    type State = WaterfallLocalState;

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        canvas_bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, canvas_bounds.size());

        // 1. Draw canvas background
        let bg_rect = Path::rectangle(Point::ORIGIN, canvas_bounds.size());
        frame.fill(&bg_rect, bg_primary(theme));

        let Some(trace_bounds) = self.bounds else {
            frame.fill_text(canvas::Text {
                content: "No trace selected. Select a trace on the left to inspect execution.".to_string(),
                position: Point::new(30.0, 50.0),
                color: text_muted(theme),
                size: iced::Pixels(FONT_SIZE_BASE),
                ..Default::default()
            });
            return vec![frame.into_geometry()];
        };

        // 2. Draw Header Ruler Background
        let header_rect = Path::rectangle(Point::ORIGIN, Size::new(canvas_bounds.width, HEADER_HEIGHT));
        frame.fill(&header_rect, bg_surface(theme));
        frame.stroke(
            &header_rect,
            Stroke::default().with_color(border_subtle(theme)).with_width(1.0),
        );

        // 3. Draw Timeline Tick Marks and Vertical Gridlines
        let timeline_width = (canvas_bounds.width - TREE_LABEL_WIDTH).max(100.0);
        let num_ticks = 6;
        for i in 0..=num_ticks {
            let ratio = i as f32 / num_ticks as f32;
            let tick_x = TREE_LABEL_WIDTH + (ratio * timeline_width * self.view_state.zoom) + self.view_state.pan_x;

            if tick_x >= TREE_LABEL_WIDTH && tick_x <= canvas_bounds.width {
                // Vertical gridline
                let grid_path = Path::line(
                    Point::new(tick_x, HEADER_HEIGHT),
                    Point::new(tick_x, canvas_bounds.height),
                );
                frame.stroke(
                    &grid_path,
                    Stroke::default().with_color(border_subtle(theme)).with_width(1.0),
                );

                // Tick text
                let tick_nanos = (ratio as f64 * trace_bounds.total_duration_nanos as f64) as u64;
                let tick_ms = tick_nanos as f64 / 1_000_000.0;
                let label = if tick_ms >= 1000.0 {
                    format!("+{:.2}s", tick_ms / 1000.0)
                } else {
                    format!("+{:.0}ms", tick_ms)
                };

                frame.fill_text(canvas::Text {
                    content: label,
                    position: Point::new(tick_x + 4.0, 6.0),
                    color: text_muted(theme),
                    size: iced::Pixels(FONT_SIZE_XS),
                    font: FONT_DEFAULT,
                    ..Default::default()
                });
            }
        }

        // Tree label header title
        frame.fill_text(canvas::Text {
            content: "Execution Hierarchy".to_string(),
            position: Point::new(12.0, 7.0),
            color: text_secondary(theme),
            size: iced::Pixels(FONT_SIZE_XS),
            font: FONT_DEFAULT,
            ..Default::default()
        });

        // 4. Draw Rows & Waterfall Bars
        let mut hovered_record = None;

        for item in self.items {
            let row_y = HEADER_HEIGHT + (item.row_index as f32 * ROW_HEIGHT) + self.view_state.pan_y;

            // Skip off-screen rows for virtualized performance
            if row_y + ROW_HEIGHT < HEADER_HEIGHT || row_y > canvas_bounds.height {
                continue;
            }

            let is_selected = self.selected_span_id == Some(item.span.span_id);
            let is_hovered = self.view_state.hovered_span_id == Some(item.span.span_id);

            if is_hovered {
                hovered_record = Some(&item.span);
            }

            // Alternating or hover row highlight
            let row_rect = Path::rectangle(Point::new(0.0, row_y), Size::new(canvas_bounds.width, ROW_HEIGHT));
            if is_selected {
                frame.fill(&row_rect, bg_selected(theme));
            } else if is_hovered {
                frame.fill(&row_rect, bg_hover(theme));
            } else if item.row_index % 2 == 1 {
                frame.fill(&row_rect, bg_card(theme));
            }

            // A. Tree indentation and connector lines on the left
            let indent_x = 12.0 + (item.depth as f32 * 16.0);

            if item.depth > 0 {
                // Branch connector line from parent
                let branch_path = Path::line(
                    Point::new(indent_x - 8.0, row_y + ROW_HEIGHT / 2.0),
                    Point::new(indent_x - 2.0, row_y + ROW_HEIGHT / 2.0),
                );
                frame.stroke(
                    &branch_path,
                    Stroke::default().with_color(text_muted(theme)).with_width(1.0),
                );
            }

            // Status dot / Error indicator
            let status_color = match item.span.status {
                SpanStatus::Ok => color_for_kind(item.span.kind),
                SpanStatus::Error { .. } => ACCENT_ERROR,
            };

            let dot_rect = Path::circle(Point::new(indent_x + 4.0, row_y + ROW_HEIGHT / 2.0), 3.5);
            frame.fill(&dot_rect, status_color);

            // Span name label (clipped to TREE_LABEL_WIDTH)
            let span_name = if item.span.name.len() > 22 {
                format!("{}...", &item.span.name[..20])
            } else {
                item.span.name.to_string()
            };

            frame.fill_text(canvas::Text {
                content: span_name,
                position: Point::new(indent_x + 14.0, row_y + 8.0),
                color: if is_selected { text_primary(theme) } else { text_secondary(theme) },
                size: iced::Pixels(FONT_SIZE_SM),
                font: FONT_DEFAULT,
                ..Default::default()
            });

            // B. Waterfall Timeline Bar
            let (bar_x, bar_y, bar_w, bar_h) = calculate_bar_geometry(
                item,
                &trace_bounds,
                canvas_bounds.width,
                self.view_state.pan_x,
                self.view_state.pan_y,
                self.view_state.zoom,
            );

            // Only draw if within visible horizontal canvas bounds
            if bar_x + bar_w >= TREE_LABEL_WIDTH && bar_x <= canvas_bounds.width {
                let bar_rect = Path::rectangle(Point::new(bar_x, bar_y), Size::new(bar_w, bar_h));
                let kind_color = color_for_kind(item.span.kind);

                frame.fill(&bar_rect, kind_color);

                // Border highlight on selection or hover
                if is_selected {
                    frame.stroke(
                        &bar_rect,
                        Stroke::default().with_color(Color::WHITE).with_width(2.0),
                    );
                } else if is_hovered {
                    frame.stroke(
                        &bar_rect,
                        Stroke::default().with_color(BORDER_ACCENT).with_width(1.5),
                    );
                }

                // If bar is wide enough, render duration text inside; otherwise right of bar
                let dur_str = item.span.duration_millis()
                    .map(|ms| if ms >= 1000.0 { format!("{:.2}s", ms / 1000.0) } else { format!("{:.0}ms", ms) })
                    .unwrap_or_default();

                if bar_w > 45.0 {
                    frame.fill_text(canvas::Text {
                        content: dur_str,
                        position: Point::new(bar_x + 6.0, bar_y + 5.0),
                        color: Color::WHITE,
                        size: iced::Pixels(FONT_SIZE_XS),
                        font: FONT_DEFAULT,
                        ..Default::default()
                    });
                } else {
                    frame.fill_text(canvas::Text {
                        content: dur_str,
                        position: Point::new(bar_x + bar_w + 6.0, bar_y + 5.0),
                        color: text_muted(theme),
                        size: iced::Pixels(FONT_SIZE_XS),
                        font: FONT_DEFAULT,
                        ..Default::default()
                    });
                }
            }
        }

        // 5. Left boundary separator line between tree labels and timeline
        let sep_line = Path::line(
            Point::new(TREE_LABEL_WIDTH, 0.0),
            Point::new(TREE_LABEL_WIDTH, canvas_bounds.height),
        );
        frame.stroke(
            &sep_line,
            Stroke::default().with_color(border_subtle(theme)).with_width(1.5),
        );

        // 6. Draw rich hover tooltip overlay if active
        if let (Some(span), Some(pos)) = (hovered_record, self.view_state.hovered_pos) {
            draw_hover_tooltip(&mut frame, span, pos, canvas_bounds);
        }

        vec![frame.into_geometry()]
    }

    fn update(
        &self,
        _state: &mut Self::State,
        event: canvas::Event,
        canvas_bounds: Rectangle,
        cursor: Cursor,
    ) -> (canvas::event::Status, Option<Message>) {
        let Some(cursor_pos) = cursor.position_in(canvas_bounds) else {
            if self.view_state.hovered_span_id.is_some() {
                return (canvas::event::Status::Captured, Some(Message::HoverSpan(None, None)));
            }
            return (canvas::event::Status::Ignored, None);
        };

        let Some(trace_bounds) = self.bounds else {
            return (canvas::event::Status::Ignored, None);
        };

        match event {
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                // If dragging canvas
                if self.view_state.is_dragging {
                    if let Some(start) = self.view_state.drag_start {
                        let delta = Vector::new(cursor_pos.x - start.x, cursor_pos.y - start.y);
                        return (canvas::event::Status::Captured, Some(Message::CanvasDragged(delta)));
                    }
                }

                // Check hit testing across spans
                let mut hit_span_id = None;

                for item in self.items {
                    let row_y = HEADER_HEIGHT + (item.row_index as f32 * ROW_HEIGHT) + self.view_state.pan_y;
                    if cursor_pos.y >= row_y && cursor_pos.y <= row_y + ROW_HEIGHT {
                        let (bar_x, _, bar_w, _) = calculate_bar_geometry(
                            item,
                            &trace_bounds,
                            canvas_bounds.width,
                            self.view_state.pan_x,
                            self.view_state.pan_y,
                            self.view_state.zoom,
                        );

                        // If hovering anywhere on the row or bar
                        if cursor_pos.x <= TREE_LABEL_WIDTH || (cursor_pos.x >= bar_x && cursor_pos.x <= bar_x + bar_w) {
                            hit_span_id = Some(item.span.span_id);
                            break;
                        }
                    }
                }

                if hit_span_id != self.view_state.hovered_span_id {
                    return (
                        canvas::event::Status::Captured,
                        Some(Message::HoverSpan(hit_span_id, Some(cursor_pos))),
                    );
                }
            }

            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                // Check if user clicked a span bar or tree row
                for item in self.items {
                    let row_y = HEADER_HEIGHT + (item.row_index as f32 * ROW_HEIGHT) + self.view_state.pan_y;
                    if cursor_pos.y >= row_y && cursor_pos.y <= row_y + ROW_HEIGHT {
                        return (
                            canvas::event::Status::Captured,
                            Some(Message::SelectSpan(item.span.span_id)),
                        );
                    }
                }

                // Clicked on background, initiate drag panning
                return (
                    canvas::event::Status::Captured,
                    Some(Message::CanvasDragStarted(cursor_pos)),
                );
            }

            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if self.view_state.is_dragging {
                    return (canvas::event::Status::Captured, Some(Message::CanvasDragEnded));
                }
            }

            canvas::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                match delta {
                    mouse::ScrollDelta::Lines { x, y } => {
                        return (
                            canvas::event::Status::Captured,
                            Some(Message::CanvasScrolled(x * 20.0, y * 20.0)),
                        );
                    }
                    mouse::ScrollDelta::Pixels { x, y } => {
                        return (
                            canvas::event::Status::Captured,
                            Some(Message::CanvasScrolled(x, y)),
                        );
                    }
                }
            }

            _ => {}
        }

        (canvas::event::Status::Ignored, None)
    }

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        _bounds: Rectangle,
        _cursor: Cursor,
    ) -> mouse::Interaction {
        if self.view_state.is_dragging {
            mouse::Interaction::Grabbing
        } else if self.view_state.hovered_span_id.is_some() {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }
}
