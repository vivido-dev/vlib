//! The view behind `window_shadow`: a frame drawn around the window, with resize edges.
//!
//! The edges are hit regions whose role is `Resize`, so the *host* performs the resize: the
//! window grows or shrinks natively as the edge is dragged, and the frame redraws at the new
//! size. Nothing here reimplements window resizing.

use vivid_protocol::vector::CursorShape;
use vlib::vui::prelude::*;
use vlib::vui::{Anchor, div, text};

/// One resizable edge: which edges the host may move, as the protocol's bitmask — 1 left,
/// 2 top, 4 right, 8 bottom — along with the shape the pointer takes there and where the
/// region sits.
const EDGES: [(u8, CursorShape, Anchor, f32, f32, f32, f32); 4] = [
    (
        1,
        CursorShape::ResizeLeft,
        Anchor::TopLeft,
        0.,
        0.,
        6.,
        236.,
    ),
    (
        4,
        CursorShape::ResizeRight,
        Anchor::TopRight,
        0.,
        0.,
        6.,
        236.,
    ),
    (2, CursorShape::ResizeUp, Anchor::TopLeft, 0., 0., 336., 6.),
    (
        8,
        CursorShape::ResizeDown,
        Anchor::BottomLeft,
        0.,
        0.,
        336.,
        6.,
    ),
];
pub struct WindowFrame;

impl Render for WindowFrame {
    fn render(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        div().p(12.).size(360., 260.).bg(rgb(0x0c0c14)).child(
            div()
                .relative()
                .size(336., 236.)
                .rounded(8.)
                .bg(rgb(0x1a1a28))
                .border(1., rgb(0x3a3a52))
                .child(
                    text("Drag an edge to resize")
                        .size(13.)
                        .color(rgb(0xc0c0d0))
                        .id("label"),
                )
                .children(
                    EDGES.map(|(direction, cursor, anchor, dx, dy, width, height)| {
                        div()
                            .id(format!("edge-{direction}"))
                            .deferred()
                            .anchor(anchor, Point::new(dx, dy))
                            .resize_region(direction)
                            .size(width, height)
                            .cursor(cursor)
                    }),
                ),
        )
    }
}
