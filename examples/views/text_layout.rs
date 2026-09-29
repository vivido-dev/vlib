//! The view behind `text_layout`: alignment, weight, slant, and decorations.
//!
//! Everything here is one paragraph per block, so the alignment is the paragraph's own: a line
//! that is centered inside a box it fills, not a box that was moved.

use vlib::vui::prelude::*;
use vlib::vui::{TextAlign, TextEl, TextRunSpec, div, paragraph, text};

pub struct TextLayout;

const BLOCK: f32 = 360.;

fn block(label: &str, aligned: TextEl) -> impl IntoElement {
    div()
        .flex_col()
        .gap(4.)
        .child(text(label).size(11.).color(rgb(0x8080a0)))
        .child(
            div()
                .w(BLOCK)
                .p(6.)
                .rounded(4.)
                .bg(rgb(0x20202c))
                .child(aligned),
        )
}

impl Render for TextLayout {
    fn render(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_col()
            .gap(12.)
            .p(20.)
            .size(420., 460.)
            .bg(rgb(0x101018))
            .child(block(
                "start",
                text("Aligned to the start edge")
                    .size(15.)
                    .max_width(BLOCK)
                    .align(TextAlign::Start),
            ))
            .child(block(
                "center",
                text("Aligned to the center")
                    .size(15.)
                    .max_width(BLOCK)
                    .align(TextAlign::Center),
            ))
            .child(block(
                "end",
                text("Aligned to the end edge")
                    .size(15.)
                    .max_width(BLOCK)
                    .align(TextAlign::End),
            ))
            .child(block(
                "decorations",
                text("underlined")
                    .size(15.)
                    .color(rgb(0x8ecbff))
                    .underline()
                    .max_width(BLOCK),
            ))
            .child(block(
                "weights",
                paragraph(vec![
                    TextRunSpec {
                        text: "regular ".into(),
                        weight: 400,
                        ..TextRunSpec::default()
                    },
                    TextRunSpec {
                        text: "bold ".into(),
                        weight: 700,
                        ..TextRunSpec::default()
                    },
                    TextRunSpec {
                        text: "italic".into(),
                        italic: true,
                        ..TextRunSpec::default()
                    },
                ])
                .size(16.)
                .color(rgb(0xe6e6f0))
                .max_width(BLOCK),
            ))
            .child(block(
                "struck through",
                text("a line that was wrong")
                    .size(15.)
                    .strikethrough()
                    .max_width(BLOCK),
            ))
    }
}
