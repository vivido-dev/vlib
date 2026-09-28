//! The view behind `svg`: one document, rasterized at each size it is drawn.
//!
//! This is the reason a vector is worth keeping as one. The same SVG at 32 and at 160 pixels is
//! not one bitmap scaled twice — it is rasterized for each box, in the display's own pixels, so
//! the thin strokes stay thin.

use vlib::vui::prelude::*;
use vlib::vui::{div, svg, text};

pub struct Svgs;

/// A document worth rasterizing per size: thin strokes and a curve, both of which a scaled
/// bitmap would blur.
pub const DRAGON: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
  <circle cx="50" cy="50" r="46" fill="#1c2438" stroke="#4a5fd0" stroke-width="2"/>
  <path d="M20 70 Q50 10 80 70" fill="none" stroke="#8ecbff" stroke-width="3"/>
  <path d="M30 78 L50 44 L70 78 Z" fill="#70d0a0" opacity="0.85"/>
  <circle cx="50" cy="34" r="6" fill="#ffe080"/>
</svg>"##;

/// A second document, so the cache holds more than one.
pub const BADGE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">
  <rect x="4" y="4" width="56" height="56" rx="12" fill="#2a4a3a"/>
  <path d="M18 34 L28 44 L46 22" fill="none" stroke="#9ef0c0" stroke-width="6"
        stroke-linecap="round" stroke-linejoin="round"/>
</svg>"##;

pub const SIZES: [f32; 4] = [32., 56., 96., 160.];

impl Render for Svgs {
    fn render(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_col()
            .gap(12.)
            .p(20.)
            .size(460., 280.)
            .bg(rgb(0x101018))
            .child(
                text("one document, rasterized per box")
                    .size(12.)
                    .color(rgb(0x8080a0)),
            )
            .child(
                div()
                    .flex()
                    .gap(12.)
                    .items_center()
                    .children(SIZES.map(|size| {
                        svg("dragon", DRAGON)
                            .id(format!("dragon-{size}"))
                            .size(size, size)
                    })),
            )
            .child(
                div()
                    .flex()
                    .gap(12.)
                    .items_center()
                    .child(svg("badge", BADGE).id("badge").size(48., 48.))
                    .child(
                        text("a second document, so the cache holds two")
                            .size(11.)
                            .color(rgb(0x8080a0)),
                    ),
            )
    }
}
