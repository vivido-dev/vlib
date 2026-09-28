//! The view behind `anchor`: one panel placed against each corner of its parent.

use vlib::vui::prelude::*;
use vlib::vui::{Anchor, Point, div, text};

pub struct Anchors;

const CORNERS: [(&str, Anchor); 6] = [
    ("top left", Anchor::TopLeft),
    ("top center", Anchor::TopCenter),
    ("top right", Anchor::TopRight),
    ("bottom left", Anchor::BottomLeft),
    ("bottom center", Anchor::BottomCenter),
    ("bottom right", Anchor::BottomRight),
];

fn panel(label: &str, anchor: Anchor) -> impl IntoElement {
    div()
        .deferred()
        .anchor(anchor, Point::new(8., 8.))
        .p(6.)
        .rounded(4.)
        .bg(rgb(0x4a5fd0))
        .child(text(label).size(11.).color(rgb(0xffffff)))
}

impl Render for Anchors {
    fn render(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        div().p(16.).size(420., 260.).bg(rgb(0x101018)).child(
            div()
                .id("field")
                .relative()
                .size(380., 220.)
                .rounded(8.)
                .bg(rgb(0x1c1c2c))
                .border(1., rgb(0x30304a))
                .child(text("the anchor's parent").size(12.).color(rgb(0x8080a0)))
                .children(CORNERS.map(|(label, anchor)| panel(label, anchor))),
        )
    }
}
