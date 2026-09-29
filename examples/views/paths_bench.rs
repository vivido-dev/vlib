//! The view behind `paths_bench`: how many paths one frame can hold.
//!
//! A display list has three ceilings and they are not the same size. A path costs one *command*
//! (4096 a frame), eleven *segments* (65536 a frame) and about two hundred *bytes* once encoded
//! — and it is the bytes that run out first, because the scene a host grants is capped well
//! below the profile's own ceiling. A window 640 by 420 is granted at most 256 KiB, which is
//! where this example's own default comes from: a thousand stars fit, and twelve hundred do not.
//!
//! So the shape of the limit is not how complex a *drawing* is, but how many *shapes* it is made
//! of. Adding a curve to an existing path costs bytes; adding a path costs all three.
//!
//! GPUI's `paths_bench` is a command-line benchmark, because a GPU renderer's cost is mostly
//! per-pixel. What is interesting here is per-command, per-segment and per-byte, two of which
//! this window prints.

use vivid_protocol::vector::{Brush, Path};
use vlib::vui::canvas::canvas;
use vlib::vui::prelude::*;
use vlib::vui::{CursorShape, div, text};

/// A star comes out as eleven segments: a move, nine lines, and a close.
pub const SEGMENTS_PER_STAR: usize = 11;

/// What the frame is asked for when the example opens.
///
/// Chosen against the encoded size rather than either count: a thousand eleven-segment paths is
/// about 220 KB of the 256 KiB a window this size is granted, so it is a real load with room to
/// spare. See the module documentation.
pub const DEFAULT_STARS: usize = 1000;

/// The counts a click steps down through, so the frame can be seen shrinking.
pub const STEPS: [usize; 4] = [1000, 500, 250, 125];

/// The view behind `paths_bench`.
pub struct PathsBench {
    pub stars: usize,
    /// Which of [`STEPS`] is showing.
    step: usize,
}

impl Default for PathsBench {
    fn default() -> Self {
        Self {
            stars: DEFAULT_STARS,
            step: 0,
        }
    }
}

impl PathsBench {
    /// The segments this frame's drawing asks for, in total.
    pub fn segments(&self) -> usize {
        self.stars * SEGMENTS_PER_STAR
    }

    /// Roughly what this frame's drawing weighs once encoded, in kilobytes.
    ///
    /// The view cannot ask: the granted scene size is negotiated below the toolkit, and what it
    /// gets is not visible from here. A star's worth of coordinates encodes to about 220 bytes,
    /// which is close enough to show what the ceiling is doing.
    pub fn kilobytes(&self) -> usize {
        self.stars * 220 / 1024
    }

    /// Where the `index`th star sits, in a grid that covers the box.
    fn place(bounds: Bounds, index: usize, stars: usize) -> (f32, f32, f32) {
        let columns = (stars as f32).sqrt().ceil().max(1.) as usize;
        let rows = stars.div_ceil(columns);
        let cell = Bounds::new(
            bounds.origin,
            Size::new(
                bounds.width().0 / columns as f32,
                bounds.height().0 / rows as f32,
            ),
        );
        let column = index % columns;
        let row = index / columns;
        (
            cell.origin.x.0 + cell.width().0 * (column as f32 + 0.5),
            cell.origin.y.0 + cell.height().0 * (row as f32 + 0.5),
            cell.width().0.min(cell.height().0) * 0.44,
        )
    }
}

impl Render for PathsBench {
    fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let stars = self.stars;
        div()
            .flex_col()
            .size(640., 420.)
            .bg(rgb(0x0c0c12))
            .child(
                canvas(move |draw, bounds| {
                    for index in 0..stars {
                        let (cx, cy, radius) = PathsBench::place(bounds, index, stars);
                        let mut path = Path::builder();
                        for corner in 0..10 {
                            // Ten corners alternating between the two radii, starting upward.
                            let reach = if corner % 2 == 0 { radius } else { radius * 0.45 };
                            let angle = -std::f32::consts::FRAC_PI_2
                                + corner as f32 * std::f32::consts::PI / 5.;
                            let (x, y) = (cx + reach * angle.cos(), cy + reach * angle.sin());
                            path = if corner == 0 {
                                path.move_to(x as f64, y as f64)
                            } else {
                                path.line_to(x as f64, y as f64)
                            };
                        }
                        draw.fill(path.close(), Brush::Solid(rgb(0xd0a860)));
                    }
                })
                .id("bench")
                .flex_1()
                .cursor(CursorShape::Pointer)
                .on_click(cx.listener(|state: &mut Self, _event, cx| {
                    state.step = (state.step + 1) % STEPS.len();
                    state.stars = STEPS[state.step];
                    cx.notify();
                })),
            )
            .child(
                div().p(8.).child(
                    text(format!(
                        "{} star paths · {} of 65536 segments · {} of 4096 commands · {} of 256 KiB · click to halve",
                        self.stars,
                        self.segments(),
                        self.stars + 1,
                        self.kilobytes()
                    ))
                    .size(11.)
                    .color(rgb(0x8080a0)),
                ),
            )
    }
}
