//! The view behind `uniform_list`: ten thousand rows, of which a handful exist.
//!
//! The list is the point: a display list has a fixed command ceiling, so a row that nobody can
//! see must not be built. Scrolling changes which rows exist, not how many.

use vlib::vui::prelude::*;
use vlib::vui::{div, text, uniform_list};

pub const ROWS: usize = 10_000;
pub const ROW_HEIGHT: f32 = 24.;

pub struct Rows {
    pub list: ListState,
}

impl Default for Rows {
    fn default() -> Self {
        Self {
            list: ListState::new(ROWS, ROW_HEIGHT),
        }
    }
}

impl Render for Rows {
    fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_col()
            .gap(8.)
            .p(16.)
            .size(320., 260.)
            .bg(rgb(0x101018))
            .child(
                text(format!("{ROWS} rows, a dozen of them real"))
                    .size(12.)
                    .color(rgb(0x8080a0)),
            )
            .child(
                uniform_list(&self.list, |rows| {
                    rows.map(|index| {
                        div()
                            .flex()
                            .items_center()
                            .px(8.)
                            .bg(if index % 2 == 0 {
                                rgb(0x1a1a24)
                            } else {
                                rgb(0x14141c)
                            })
                            .child(text(format!("Row {index}")).size(13.).color(rgb(0xd0d0e0)))
                            .into_node()
                    })
                    .collect()
                })
                .id("list")
                .w_full()
                .flex_1()
                .rounded(6.)
                .on_wheel(cx.listener(|state: &mut Self, event, cx| {
                    if let UiEvent::Wheel { delta, .. } = event {
                        let height = cx.bounds_of("list").map_or(0., |box_| box_.height().get());
                        state.list.scroll_by(delta.y.get(), height);
                        cx.notify();
                    }
                })),
            )
    }
}
