//! The view behind `scrollable`: content taller than its box, scrolled by the wheel.

use vlib::vui::prelude::*;
use vlib::vui::{div, text};

pub const ROW_HEIGHT: f32 = 28.;
pub const VISIBLE_ROWS: usize = 6;
pub const ROWS: usize = 40;

pub struct Scrollable {
    pub offset: f32,
}

impl Default for Scrollable {
    fn default() -> Self {
        Self { offset: 0. }
    }
}

impl Scrollable {
    /// The largest offset that still leaves content on screen.
    pub fn maximum_offset(&self) -> f32 {
        (ROWS as f32 * ROW_HEIGHT - VISIBLE_ROWS as f32 * ROW_HEIGHT).max(0.)
    }

    /// Move the content by a wheel delta. A positive `dy` is a scroll down, so the content
    /// moves up: the offset from the top of the content grows.
    pub fn scroll_by(&mut self, dy: f32) {
        self.offset = (self.offset + dy).clamp(0., self.maximum_offset());
    }
}

impl Render for Scrollable {
    fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = (0..ROWS).map(|index| {
            div()
                .id(format!("row-{index}"))
                .h(ROW_HEIGHT)
                .w_full()
                .bg(if index % 2 == 0 {
                    rgb(0x1a1a24)
                } else {
                    rgb(0x14141c)
                })
                .child(text(format!("Row {index}")).size(14.).color(rgb(0xd0d0e0)))
        });
        div()
            .id("viewport")
            .flex_col()
            .w(300.)
            .h(ROW_HEIGHT * VISIBLE_ROWS as f32)
            .overflow_hidden()
            .bg(rgb(0x101018))
            .on_wheel(cx.listener(|state: &mut Self, event, cx| {
                if let UiEvent::Wheel { delta, .. } = event {
                    state.scroll_by(delta.y.get());
                    cx.notify();
                }
            }))
            // The content is out of flow so its offset does not move the box that clips it.
            .child(
                div()
                    .absolute()
                    .top(-self.offset)
                    .left(0.)
                    .w_full()
                    .flex_col()
                    .children(rows),
            )
    }
}
