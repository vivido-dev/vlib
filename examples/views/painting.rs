//! The view behind `painting`: freeform paths, drawn rather than described.
//!
//! Everything else the toolkit paints is a box, a run of text, or a picture, and those are
//! values — an element says what it is and the toolkit works out the commands. A canvas is the
//! other half: a closure that runs at paint time and issues drawing statements, so a shape nobody
//! has a command for is written rather than requested.
//!
//! The figures here are placed against the box each canvas was given, which is in window
//! coordinates. That is the whole of the coordinate story: there is no local space to convert
//! out of, and no transform command to pay for.

use vivid_protocol::vector::{Brush, Cap, Join, Path, PathBuilder, Scalar, StrokeStyle};
use vlib::vui::canvas::canvas;
use vlib::vui::prelude::*;
use vlib::vui::{CursorShape, div, text};

/// The circle constant `Path::ellipse` uses, written out because a ring needs two subpaths in one
/// builder and a finished `Path` cannot be added to one.
const KAPPA: f64 = 0.552_284_749_830_793_6;

/// A point at a fraction across a box, which is how every figure here places itself.
fn at(bounds: Bounds, fx: f32, fy: f32) -> (f64, f64) {
    (
        (bounds.origin.x.0 + bounds.width().0 * fx) as f64,
        (bounds.origin.y.0 + bounds.height().0 * fy) as f64,
    )
}

/// A circle as four cubics, appended to a path that may already have subpaths.
fn circle(builder: PathBuilder, cx: f64, cy: f64, radius: f64) -> PathBuilder {
    let k = radius * KAPPA;
    builder
        .move_to(cx, cy - radius)
        .cubic_to(cx + k, cy - radius, cx + radius, cy - k, cx + radius, cy)
        .cubic_to(cx + radius, cy + k, cx + k, cy + radius, cx, cy + radius)
        .cubic_to(cx - k, cy + radius, cx - radius, cy + k, cx - radius, cy)
        .cubic_to(cx - radius, cy - k, cx - k, cy - radius, cx, cy - radius)
        .close()
}

/// A round-capped stroke, which is what a freehand line wants at both ends.
fn round_stroke(width: f64) -> StrokeStyle {
    let mut style = StrokeStyle::new(width).expect("a width the wire can carry");
    style.cap = Cap::Round;
    style.join = Join::Round;
    style
}

/// The view behind `painting`.
#[derive(Default)]
pub struct Painting {
    /// One stroke per press: the points the pointer visited while it was held down.
    pub strokes: Vec<Vec<Point>>,
    /// Whether a press is currently being drawn.
    pub drawing: bool,
}

impl Painting {
    /// How many strokes have been drawn on the freehand pad.
    pub fn stroke_count(&self) -> usize {
        self.strokes.len()
    }

    /// A star, filled. Ten corners alternating between two radii, walked with `line_to`.
    fn polygon(&self) -> impl IntoElement {
        canvas(|draw, bounds| {
            let mut path = Path::builder();
            for corner in 0..10 {
                // Upward first, so the star has a point on top rather than a flat edge.
                let angle =
                    -std::f32::consts::FRAC_PI_2 + corner as f32 * std::f32::consts::PI / 5.;
                let radius = if corner % 2 == 0 { 0.44 } else { 0.18 };
                let (x, y) = at(
                    bounds,
                    0.5 + radius * angle.cos(),
                    0.5 + radius * angle.sin(),
                );
                path = if corner == 0 {
                    path.move_to(x, y)
                } else {
                    path.line_to(x, y)
                };
            }
            draw.fill(path.close(), Brush::Solid(rgb(0xe0b050)));
        })
        .size(104., 104.)
        .bg(rgb(0x1a1a24))
        .rounded(6.)
    }

    /// One cubic, stroked with round caps and a round join.
    fn curve(&self) -> impl IntoElement {
        canvas(|draw, bounds| {
            let (start, c1, c2, end) = (
                at(bounds, 0.12, 0.82),
                at(bounds, 0.3, 0.02),
                at(bounds, 0.7, 0.98),
                at(bounds, 0.88, 0.18),
            );
            let path = Path::builder()
                .move_to(start.0, start.1)
                .cubic_to(c1.0, c1.1, c2.0, c2.1, end.0, end.1);
            draw.stroke_styled(path, Brush::Solid(rgb(0x8ecbff)), round_stroke(5.));
        })
        .size(104., 104.)
        .bg(rgb(0x1a1a24))
        .rounded(6.)
    }

    /// Two nested circles filled by the even-odd rule: a shape with a hole in it, which is also a
    /// hole the host will not deliver a click to, because it hit tests the same rule it fills by.
    fn ring(&self) -> impl IntoElement {
        canvas(|draw, bounds| {
            let (cx, cy) = at(bounds, 0.5, 0.5);
            let radius = bounds.width().0 as f64;
            // Outward first, then the hole: two subpaths, and the even-odd rule makes the second
            // one a hole rather than a second disc.
            let ring = circle(Path::builder(), cx, cy, radius * 0.44);
            let ring = circle(ring, cx, cy, radius * 0.2).even_odd();
            draw.fill(ring, Brush::Solid(rgb(0x70d090)));
        })
        .size(104., 104.)
        .bg(rgb(0x1a1a24))
        .rounded(6.)
    }

    /// A dashed stroke: dashes are the style's, not the path's, so the path is one straight line.
    fn dashed(&self) -> impl IntoElement {
        canvas(|draw, bounds| {
            let path = Path::builder()
                .move_to(at(bounds, 0.1, 0.5).0, at(bounds, 0.1, 0.5).1)
                .line_to(at(bounds, 0.9, 0.5).0, at(bounds, 0.9, 0.5).1);
            let mut style = round_stroke(4.);
            style.dashes = vec![
                Scalar::new(9.).expect("a dash the wire can carry"),
                Scalar::new(6.).expect("a gap the wire can carry"),
            ];
            draw.stroke_styled(path, Brush::Solid(rgb(0xff8ea0)), style);
        })
        .size(104., 104.)
        .bg(rgb(0x1a1a24))
        .rounded(6.)
    }

    /// A pad the pointer draws on. Each stroke is a path built from the points the host reported,
    /// in the same window coordinates the canvas is painted in — so what is drawn is exactly
    /// where the pointer was, with nothing to convert.
    fn freehand(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let strokes = self.strokes.clone();
        canvas(move |draw, _bounds| {
            for points in &strokes {
                if points.len() < 2 {
                    // A press with no movement is a dot. It is left out rather than drawn as a
                    // round cap nobody asked for: the pad reports where the pointer went, and it
                    // has not gone anywhere yet.
                    continue;
                }
                let mut path = Path::builder();
                for (index, point) in points.iter().enumerate() {
                    let (x, y) = (point.x.0 as f64, point.y.0 as f64);
                    path = if index == 0 {
                        path.move_to(x, y)
                    } else {
                        path.line_to(x, y)
                    };
                }
                draw.stroke_styled(path, Brush::Solid(rgb(0xffd070)), round_stroke(3.));
            }
        })
        .id("freehand")
        .flex_1()
        .clipped()
        .bg(rgb(0x14141c))
        .rounded(8.)
        .cursor(CursorShape::Crosshair)
        .on_mouse_down(cx.listener(|state: &mut Self, event, cx| {
            if let Some(position) = event.position() {
                state.strokes.push(vec![position]);
                state.drawing = true;
                cx.notify();
            }
        }))
        .on_mouse_move(cx.listener(|state: &mut Self, event, cx| {
            if !state.drawing {
                return;
            }
            if let Some(position) = event.position() {
                if let Some(stroke) = state.strokes.last_mut() {
                    stroke.push(position);
                }
                cx.notify();
            }
        }))
        .on_mouse_up(cx.listener(|state: &mut Self, _event, cx| {
            state.drawing = false;
            cx.notify();
        }))
    }
}

impl Render for Painting {
    fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let row = div()
            .flex()
            .gap(10.)
            .child(self.polygon())
            .child(self.curve())
            .child(self.ring())
            .child(self.dashed());
        let pad = self.freehand(cx);

        div()
            .flex_col()
            .gap(10.)
            .p(16.)
            .size(472., 300.)
            .bg(rgb(0x101018))
            .child(
                text(format!(
                    "filled, stroked, even-odd, dashed — {} strokes on the pad",
                    self.stroke_count()
                ))
                .size(11.)
                .color(rgb(0x8080a0)),
            )
            .child(row)
            .child(pad)
    }
}
