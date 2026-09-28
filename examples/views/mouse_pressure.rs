//! The view behind `mouse_pressure`: a stroke that is as wide as it was pressed hard.
//!
//! Pressure is only sent by hardware that reports it. A host with no such device sends nothing,
//! so the stroke falls back to its base width rather than to a value nobody measured.

use vlib::vui::prelude::*;
use vlib::vui::{Anchor, CursorShape, div, text};

#[derive(Default)]
pub struct Pressure {
    /// Each mark is a position and the pressure the device reported with it.
    pub points: Vec<(Point, f32)>,
    pub saw_pressure: bool,
}

impl Pressure {
    pub const BASE_WIDTH: f32 = 6.;

    /// How wide a stroke is at this pressure. A host that reports none gives the base width.
    pub fn width_for(pressure: f32) -> f32 {
        Self::BASE_WIDTH * (0.4 + pressure.clamp(0., 1.) * 1.6)
    }

    pub fn push(&mut self, position: Point, pressure: f32) {
        self.points.push((position, pressure));
        if pressure > 0. {
            self.saw_pressure = true;
        }
    }
}

impl Render for Pressure {
    fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut surface = div()
            .id("pad")
            .size(380., 200.)
            .rounded(8.)
            .bg(rgb(0x14141c))
            .cursor(CursorShape::Crosshair)
            .on_mouse_down(cx.listener(|state: &mut Self, event, cx| {
                if let Some(position) = event.position() {
                    state.push(position, 0.5);
                    cx.notify();
                }
            }))
            .on_pressure(cx.listener(|state: &mut Self, event, cx| {
                if let UiEvent::Pressure { position, pressure } = event {
                    state.push(*position, *pressure);
                    cx.notify();
                }
            }));

        // Each point is drawn as a square whose size is its pressure, deferred so the stroke
        // sits over the pad it was drawn on.
        for (position, pressure) in &self.points {
            let width = Self::width_for(*pressure);
            surface = surface.child(
                div()
                    .deferred()
                    .anchor(
                        Anchor::TopLeft,
                        Point::new(position.x.0 - width / 2., position.y.0 - width / 2.),
                    )
                    .size(width, width)
                    .rounded(width / 2.)
                    .bg(rgb(0x8ecbff)),
            );
        }

        div()
            .flex_col()
            .gap(10.)
            .p(20.)
            .size(420., 280.)
            .bg(rgb(0x101018))
            .child(
                text(if self.saw_pressure {
                    "pressure device detected"
                } else {
                    "press and draw"
                })
                .size(12.)
                .color(rgb(0x8080a0)),
            )
            .child(surface)
    }
}
