//! The view behind `window_movable`: a title bar, and the edges that resize.
//!
//! Both are the same mechanism. A region's *role* tells the host what a drag on it means — move
//! the window, or resize it from that edge — and the host does the work natively, at full speed,
//! without a frame in between. Nothing here moves a window; this names the handle.
//!
//! This is the pane-local counterpart of GPUI's `window_movable`, which is about OS titlebar
//! flags. An overlay window has no titlebar of its own — the terminal behind it does — so what is
//! movable here is the part of the pane the application nominates.

use vivid_protocol::vector::CursorShape;
use vlib::vui::prelude::*;
use vlib::vui::{Anchor, div, text};

/// The four edges the host may resize from, as the protocol numbers them: 1 left, 2 top,
/// 4 right, 8 bottom. Each is a thin region just inside the window's own frame.
const EDGES: [(u8, CursorShape, Anchor, f32, f32, f32, f32); 4] = [
    (
        1,
        CursorShape::ResizeLeft,
        Anchor::TopLeft,
        0.,
        0.,
        6.,
        240.,
    ),
    (
        4,
        CursorShape::ResizeRight,
        Anchor::TopRight,
        0.,
        0.,
        6.,
        240.,
    ),
    (2, CursorShape::ResizeUp, Anchor::TopLeft, 0., 0., 360., 6.),
    (
        8,
        CursorShape::ResizeDown,
        Anchor::BottomLeft,
        0.,
        0.,
        360.,
        6.,
    ),
];

/// The height of the title bar, which is the moving region.
pub const TITLE_BAR: f32 = 30.;

/// The view behind `window_movable`.
#[derive(Default)]
pub struct Movable {
    /// How many times the title bar has been dragged, which is what a view does with the
    /// notification rather than with the movement itself.
    pub drags: usize,
}

impl Render for Movable {
    fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        div().p(12.).size(384., 264.).bg(rgb(0x0c0c14)).child(
            div()
                .relative()
                .size(360., 240.)
                .rounded(8.)
                .bg(rgb(0x1a1a28))
                .border(1., rgb(0x3a3a52))
                .child(
                    // The bar is a drag handle first and an element second. It carries no click
                    // handler for the same reason: the host is already doing the moving, and a
                    // region that both moved the window and reported a click would do both.
                    div()
                        .id("title-bar")
                        .drag_region()
                        .flex()
                        .items_center()
                        .justify_center()
                        .h(TITLE_BAR)
                        .w_full()
                        .rounded_top(8.)
                        .bg(rgb(0x26263c))
                        .cursor(CursorShape::Pointer)
                        .on_mouse_up(cx.listener(|state: &mut Self, _event, cx| {
                            // A drag ends with a release the view can hear about, which is what
                            // "remember where the window was put" would hang off.
                            state.drags += 1;
                            cx.notify();
                        }))
                        .child(
                            text("drag this bar to move the window")
                                .size(12.)
                                .color(rgb(0xc0c0d0)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(6.)
                        .p(14.)
                        .child(
                            text("the four edges resize, the bar moves")
                                .size(12.)
                                .color(rgb(0x8080a0)),
                        )
                        .child(
                            text(format!("{} drags so far", self.drags))
                                .size(12.)
                                .color(rgb(0x606078)),
                        ),
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
