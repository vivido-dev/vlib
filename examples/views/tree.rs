//! The view behind `tree`: one box inside another, deeper than anything a UI needs.

use vlib::vui::prelude::*;
use vlib::vui::{div, text};

/// A column of nested boxes, one per level.
pub struct DeepTree {
    pub depth: usize,
}

impl Default for DeepTree {
    fn default() -> Self {
        Self { depth: 64 }
    }
}

fn level(depth: usize) -> impl IntoElement {
    let shade = 0x20 + (depth as u32 % 6) * 0x04;
    let mut node = div()
        .flex_col()
        // Each level is told how wide it is. An auto-sized box asks the layout engine for its
        // content size at every level, and that question costs more the deeper it goes.
        .w_full()
        .p(2.)
        .bg(rgb(0x101018 + shade * 0x000101))
        .border_1()
        .child(text(format!("{depth}")).size(11.).color(rgb(0xc0c0d0)));
    if depth > 0 {
        node = node.child(level(depth - 1));
    }
    node
}

impl Render for DeepTree {
    fn render(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        div().p(8.).bg(rgb(0x0a0a12)).child(level(self.depth))
    }
}
