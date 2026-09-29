//! The view behind `opacity`: a subtree painted as one translucent group.
//!
//! GPUI's example fades on a timer; without an animation clock yet, this one takes its opacity
//! from where the pointer is, which is the same amount of the feature and one less moving part.

use vlib::vui::prelude::*;
use vlib::vui::{div, text};

pub struct Fade {
    pub opacity: f32,
}

impl Default for Fade {
    fn default() -> Self {
        Self { opacity: 1. }
    }
}

impl Fade {
    /// Opacity from a pointer position across the window: left is transparent, right is solid.
    pub fn from_pointer(x: f32, width: f32) -> f32 {
        (x / width).clamp(0., 1.)
    }
}

impl Render for Fade {
    fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("surface")
            .flex_col()
            .gap(10.)
            .p(20.)
            .size(360., 260.)
            .bg(rgb(0x141420))
            .on_mouse_move(cx.listener(|state: &mut Self, event, cx| {
                if let UiEvent::MouseMove { position } = event {
                    let next = Self::from_pointer(position.x.get(), 360.);
                    if (next - state.opacity).abs() > 0.001 {
                        state.opacity = next;
                        cx.notify();
                    }
                }
            }))
            .child(
                text("Move the pointer left and right")
                    .size(13.)
                    .color(rgb(0xa0a0b8)),
            )
            .child(
                div()
                    .flex_col()
                    .gap(6.)
                    .p(12.)
                    .rounded(8.)
                    .bg(rgb(0x4a5fd0))
                    .opacity(self.opacity)
                    .child(text("Fading group").size(20.).color(rgb(0xffffff)))
                    .child(
                        text("Everything in here is one group")
                            .size(13.)
                            .color(rgb(0xd0d0e0)),
                    )
                    .child(div().size(120., 24.).rounded(4.).bg(rgb(0x9ecbff))),
            )
    }
}
