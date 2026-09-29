//! The view behind `drag_drop`: tiles dragged onto a tray.
//!
//! A drag starts on a press, follows the pointer outside the tile it started on — which is what
//! pointer capture is for — and ends on the release. The payload lives in the view's own state,
//! so what is being dragged is whatever the application says it is.

use vlib::vui::prelude::*;
use vlib::vui::{CursorShape, div, text};

pub const TILES: [&str; 3] = ["alpha", "beta", "gamma"];

pub struct DragDrop {
    /// The tile being dragged, and where the pointer has it.
    pub dragging: Option<(usize, Point)>,
    /// Which tiles made it into the tray.
    pub dropped: Vec<usize>,
    pub handle: FocusHandle,
}

impl Default for DragDrop {
    fn default() -> Self {
        Self {
            dragging: None,
            dropped: Vec::new(),
            handle: FocusHandle::new("drag-surface"),
        }
    }
}

impl DragDrop {
    /// Where the tile being dragged is drawn, if any: it follows the pointer.
    pub fn drag_position(&self) -> Option<(usize, Point)> {
        self.dragging
    }

    pub fn is_dropped(&self, index: usize) -> bool {
        self.dropped.contains(&index)
    }
}

impl Render for DragDrop {
    fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let tiles = TILES.iter().enumerate().map(|(index, label)| {
            let handle = self.handle.clone();
            div()
                .id(format!("tile-{index}"))
                .flex()
                .items_center()
                .justify_center()
                .size(64., 40.)
                .rounded(6.)
                .bg(if self.dragging.map(|(index, _)| index) == Some(index) {
                    rgb(0x8a6a20)
                } else {
                    rgb(0x3a4a6a)
                })
                .cursor(CursorShape::Grab)
                .on_mouse_down(cx.listener(move |state: &mut Self, event, cx| {
                    state.dragging = event.position().map(|position| (index, position));
                    // Capture keeps the moves coming after the pointer leaves the tile, so the
                    // release is heard wherever it happens.
                    let _ = cx.capture_pointer(true);
                    cx.focus(handle.clone());
                    cx.notify();
                }))
                .child(text(*label).size(12.).color(rgb(0xffffff)))
        });

        div()
            .id("drag-surface")
            .flex_col()
            .gap(12.)
            .p(20.)
            .size(420., 260.)
            .bg(rgb(0x101018))
            .on_mouse_move(cx.listener(|state: &mut Self, event, cx| {
                if let Some((dragged, _)) = state.dragging
                    && let Some(position) = event.position()
                {
                    state.dragging = Some((dragged, position));
                    cx.notify();
                }
            }))
            .on_mouse_up(cx.listener(|state: &mut Self, _event, cx| {
                if let Some((dragged, position)) = state.dragging.take() {
                    // Dropping on the tray is dropping inside it; anywhere else puts it back.
                    let tray = cx.bounds_of("tray");
                    if tray.is_some_and(|tray| tray.contains(position))
                        && !state.is_dropped(dragged)
                    {
                        state.dropped.push(dragged);
                    }
                    let _ = cx.capture_pointer(false);
                    cx.notify();
                }
            }))
            .child(
                text("Drag a tile into the tray")
                    .size(12.)
                    .color(rgb(0x8080a0)),
            )
            .child(div().flex().gap(10.).children(tiles))
            .child(
                div()
                    .id("tray")
                    .flex()
                    .items_center()
                    .gap(8.)
                    .p(10.)
                    .w_full()
                    .h(64.)
                    .rounded(8.)
                    .border_dashed(1., rgb(0x50506a))
                    .children(self.dropped.iter().map(|index| {
                        div()
                            .size(48., 28.)
                            .rounded(4.)
                            .bg(rgb(0x2a4a3a))
                            .child(text(TILES[*index]).size(11.).color(rgb(0xc0e0d0)))
                    })),
            )
            .child(
                // The tile being dragged follows the pointer, drawn over everything else.
                self.drag_position().map(|(index, position)| {
                    div()
                        .deferred()
                        .absolute()
                        .anchor(vlib::vui::Anchor::TopLeft, position)
                        .size(64., 40.)
                        .rounded(6.)
                        .bg(rgb(0x6a7ab0))
                        .child(text(TILES[index]).size(12.).color(rgb(0xffffff)))
                }),
            )
    }
}
