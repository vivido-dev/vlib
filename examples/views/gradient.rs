//! The view behind `gradient`: linear fills at several angles and stop sets.

use vlib::vui::prelude::*;
use vlib::vui::{GradientStopSpec, div, text};

pub struct Gradients;

fn panel(label: &str, angle: f32, stops: Vec<GradientStopSpec>) -> impl IntoElement {
    div()
        .flex_col()
        .justify_end()
        .size(180., 110.)
        .rounded(8.)
        .bg_gradient(angle, stops)
        .p(8.)
        .child(text(label).size(12.).color(rgb(0xffffff)))
}

impl Render for Gradients {
    fn render(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .grid_columns([
                vlib::vui::Track::Fraction(1.),
                vlib::vui::Track::Fraction(1.),
            ])
            .gap(12.)
            .p(20.)
            .size(420., 300.)
            .bg(rgb(0x101018))
            .child(panel(
                "to right",
                90.,
                vec![
                    GradientStopSpec::new(0., rgb(0xff5060)),
                    GradientStopSpec::new(1., rgb(0x5060ff)),
                ],
            ))
            .child(panel(
                "to bottom",
                180.,
                vec![
                    GradientStopSpec::new(0., rgb(0x50d0a0)),
                    GradientStopSpec::new(1., rgb(0x104060)),
                ],
            ))
            .child(panel(
                "diagonal",
                45.,
                vec![
                    GradientStopSpec::new(0., rgb(0xffe080)),
                    GradientStopSpec::new(0.5, rgb(0xff8060)),
                    GradientStopSpec::new(1., rgb(0x603060)),
                ],
            ))
            .child(panel(
                "hard stop",
                90.,
                vec![
                    GradientStopSpec::new(0., rgb(0x202030)),
                    GradientStopSpec::new(0.5, rgb(0x202030)),
                    GradientStopSpec::new(0.5, rgb(0x8080ff)),
                    GradientStopSpec::new(1., rgb(0x8080ff)),
                ],
            ))
    }
}
