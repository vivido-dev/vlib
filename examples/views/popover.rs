//! The view behind `popover`: a floating panel anchored to the button that opened it.
//!
//! The panel is deferred, so it paints over everything it overlaps, and a scrim behind it
//! swallows clicks aimed at the rest of the window — which is how clicking outside closes it.

use vlib::vui::prelude::*;
use vlib::vui::{Anchor, CursorShape, div, text};

#[derive(Default)]
pub struct Popover {
    pub open: bool,
}

impl Render for Popover {
    fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut root = div()
            .id("surface")
            .flex_col()
            .gap(10.)
            .p(20.)
            .size(420., 300.)
            .bg(rgb(0x101018));

        // The scrim covers the window and takes the clicks meant for what is under it. It is a
        // normal region rather than a blocking one: it has to *hear* the click that closes the
        // panel, and a blocking region reports nothing by design.
        if self.open {
            root = root.child(
                div()
                    .id("scrim")
                    .absolute()
                    .anchor(Anchor::TopLeft, Point::new(0., 0.))
                    .size(420., 300.)
                    .deferred()
                    .on_click(cx.listener(|state: &mut Self, _event, cx| {
                        state.open = false;
                        cx.notify();
                    })),
            );
        }

        let mut column = root
            .child(
                text("Anchored to the button above")
                    .size(12.)
                    .color(rgb(0x8080a0)),
            )
            .child(
                div()
                    .id("menu-button")
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(140., 36.)
                    .rounded(6.)
                    .bg(rgb(0x4a5fd0))
                    .cursor(CursorShape::Pointer)
                    .hover(|style| style.bg(rgb(0x5a6fe0)))
                    .on_click(cx.listener(|state: &mut Self, _event, cx| {
                        state.open = !state.open;
                        cx.notify();
                    }))
                    .child(text("Open menu").color(rgb(0xffffff))),
            );

        if self.open {
            column = column.child(
                div()
                    .id("menu")
                    .deferred()
                    .anchor(Anchor::TopLeft, Point::new(0., 46.))
                    .w(200.)
                    .flex_col()
                    .rounded(8.)
                    .p(6.)
                    .bg(rgb(0x24243a))
                    .shadow_lg()
                    .child(
                        text("Copy")
                            .size(14.)
                            .color(rgb(0xe6e6f0))
                            .on_click(cx.listener(|state: &mut Self, _event, cx| {
                                state.open = false;
                                cx.notify();
                            })),
                    )
                    .child(text("Paste").size(14.).color(rgb(0xe6e6f0)))
                    .child(text("Select all").size(14.).color(rgb(0xe6e6f0))),
            );
        }
        column
    }
}
