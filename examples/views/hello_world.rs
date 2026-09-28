//! The view behind `hello_world`: a counter, a button, and a line of text that follows it.

use vlib::vui::prelude::*;
use vlib::vui::{CursorShape, div, text};

#[derive(Default)]
pub struct Counter {
    pub count: i32,
}

impl Counter {
    fn background(&self) -> vivid_protocol::vector::Color {
        if self.count % 2 == 0 {
            rgb(0x1e1e2e)
        } else {
            rgb(0x2a2a3c)
        }
    }
}

impl Render for Counter {
    fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_col()
            .gap(12.)
            .p(20.)
            .size(420., 240.)
            .bg(self.background())
            .child(
                text(format!("Clicked {} times", self.count))
                    .size(24.)
                    .color(rgb(0xe6e6f0)),
            )
            .child(
                div()
                    .id("increment")
                    .px(14.)
                    .py(8.)
                    .rounded(6.)
                    .bg(rgb(0x4a5fd0))
                    .cursor(CursorShape::Pointer)
                    .hover(|style| style.bg(rgb(0x5a6fe0)))
                    .active(|style| style.bg(rgb(0x3a4fc0)))
                    .on_click(cx.listener(|state: &mut Self, _event, cx| {
                        state.count += 1;
                        cx.notify();
                    }))
                    .child(text("Increment").color(rgb(0xffffff))),
            )
    }
}
