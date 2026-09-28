//! The view behind `grid_layout`: a page laid out on grid tracks.
//!
//! GPUI collapses this page with a container query — a re-layout driven by the container's own
//! measured size. This version decides from the window it was given, which is known before
//! layout runs, and is what a container query compiles down to anyway.

use vlib::vui::prelude::*;
use vlib::vui::{Track, div, text};

pub struct Page {
    pub width: f32,
}

impl Default for Page {
    fn default() -> Self {
        Self { width: 640. }
    }
}

impl Page {
    pub fn wide(&self) -> bool {
        self.width >= 600.
    }

    fn cell(label: &str, color: u32) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_center()
            .p(12.)
            .rounded(6.)
            .bg(rgb(color))
            .child(text(label).size(13.).color(rgb(0xf0f0f8)))
    }
}

impl Render for Page {
    fn render(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        let page = div()
            .grid()
            .grid_columns([
                Track::Fixed(vlib::vui::Pixels::new(140.)),
                Track::Fraction(1.),
            ])
            .grid_rows([Track::Auto, Track::Fraction(1.), Track::Auto])
            .gap(10.)
            .p(16.)
            .size(self.width, 360.)
            .bg(rgb(0x101018))
            .child(Self::cell("header", 0x30364c))
            .child(Self::cell("header", 0x30364c));
        let body = if self.wide() {
            page.child(Self::cell("sidebar", 0x2a4a3a))
                .child(Self::cell("main", 0x1e2a44))
        } else {
            // Narrow: the sidebar collapses into a strip above the content.
            page.child(Self::cell("side", 0x2a4a3a))
                .child(Self::cell("main", 0x1e2a44))
        };
        body.child(Self::cell("footer", 0x3a2a3a))
            .child(Self::cell("footer", 0x3a2a3a))
    }
}
