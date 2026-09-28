//! The view behind `shadow`: the token scale, and the parts of a shadow.

use vlib::vui::prelude::*;
use vlib::vui::{BoxShadow, div, text};

pub struct Shadows {
    pub custom: bool,
}

impl Default for Shadows {
    fn default() -> Self {
        Self { custom: true }
    }
}

pub const TOKENS: [(&str, BoxShadow); 6] = [
    ("xs", shadows::XS),
    ("sm", shadows::SM),
    ("base", shadows::BASE),
    ("md", shadows::MD),
    ("lg", shadows::LG),
    ("xl", shadows::XL),
];

fn card(label: &str, shadow: BoxShadow) -> impl IntoElement {
    div()
        .flex_col()
        .justify_center()
        .items_center()
        .size(120., 70.)
        .rounded(8.)
        .bg(rgb(0xffffff))
        .shadow(shadow)
        .child(text(label).size(12.).color(rgb(0x333344)))
}

impl Render for Shadows {
    fn render(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        let tokens = TOKENS.iter().map(|(label, shadow)| card(label, *shadow));
        let mut column = div()
            .flex_col()
            .gap(16.)
            .p(24.)
            .size(440., 420.)
            .bg(rgb(0xededf2))
            .child(div().flex().gap(16.).items_center().children(tokens));
        if self.custom {
            column = column.child(
                div()
                    .flex()
                    .gap(16.)
                    .items_center()
                    .child(card(
                        "offset",
                        BoxShadow::new(
                            vlib::vui::Point::new(6., 6.),
                            vlib::vui::Pixels::new(0.),
                            rgb(0x203050),
                        ),
                    ))
                    .child(card(
                        "spread",
                        BoxShadow::new(
                            vlib::vui::Point::new(0., 2.),
                            vlib::vui::Pixels::new(6.),
                            rgba(0x802020, 0x80),
                        )
                        .with_spread(vlib::vui::Pixels::new(4.)),
                    ))
                    .child(card(
                        "inset",
                        BoxShadow::new(
                            vlib::vui::Point::new(0., 3.),
                            vlib::vui::Pixels::new(8.),
                            rgba(0x000000, 0x60),
                        )
                        .inset(),
                    )),
            );
        }
        column
    }
}
