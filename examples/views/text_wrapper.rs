//! The view behind `text_wrapper`: wrapping, truncation, and the line ceiling.

use vlib::vui::prelude::*;
use vlib::vui::{div, text};

pub struct TextWrapper;

const PARAGRAPH: &str = "The quick brown fox jumps over the lazy dog, and then keeps going for long enough that the line it is on runs out of room and has to be broken somewhere sensible.";
const WIDTH: f32 = 150.;

fn sample(id: &str, label: &str, wrapped: impl IntoElement) -> impl IntoElement {
    div()
        .flex_col()
        .gap(4.)
        .child(text(label).size(11.).color(rgb(0x8080a0)))
        .child(div().id(id).w(WIDTH).overflow_hidden().child(wrapped))
}

impl Render for TextWrapper {
    fn render(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .gap(16.)
            .p(20.)
            .size(560., 320.)
            .bg(rgb(0x101018))
            .child(sample(
                "wrapped",
                "wrapped",
                text(PARAGRAPH)
                    .size(13.)
                    .color(rgb(0xd0d0e0))
                    .max_width(WIDTH)
                    .wrap(),
            ))
            .child(sample(
                "ellipsized",
                "one line, ellipsized",
                text(PARAGRAPH)
                    .size(13.)
                    .color(rgb(0xd0d0e0))
                    .max_width(WIDTH)
                    .ellipsis(1),
            ))
            .child(sample(
                "clamped",
                "two lines, ellipsized",
                text(PARAGRAPH)
                    .size(13.)
                    .color(rgb(0xd0d0e0))
                    .max_width(WIDTH)
                    .ellipsis(2),
            ))
    }
}
