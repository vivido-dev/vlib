//! The view behind `active_state_bug`: a button that must not stay pressed.
//!
//! GPUI's example is a bug reproduction — `.active()` stuck on every other click. This one is the
//! behavior that bug was about: a press is active exactly while the button is held over the
//! element it started on, and a release anywhere ends it.

use vlib::vui::prelude::*;
use vlib::vui::{CursorShape, div, text};

#[derive(Default)]
pub struct Toggle {
    pub presses: usize,
}

impl Render for Toggle {
    fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_col()
            .gap(10.)
            .p(20.)
            .size(320., 160.)
            .bg(rgb(0x14141c))
            .child(
                text(format!("Pressed {} times", self.presses))
                    .size(14.)
                    .color(rgb(0xc0c0d0)),
            )
            .child(
                div()
                    .id("press")
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(160., 44.)
                    .rounded(6.)
                    .bg(rgb(0x3a3a52))
                    .cursor(CursorShape::Pointer)
                    .hover(|style| style.bg(rgb(0x4a4a68)))
                    .active(|style| style.bg(rgb(0x8a6a20)))
                    .on_click(cx.listener(|state: &mut Self, _event, cx| {
                        state.presses += 1;
                        cx.notify();
                    }))
                    .child(text("Hold me").color(rgb(0xffffff))),
            )
    }
}
