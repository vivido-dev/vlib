//! The view behind `data_table`: a table of generated quotes, virtualized by row.
//!
//! A table is a list whose rows happen to be columns. The header is outside the list so it does
//! not scroll, and the columns are the same track widths in both, which is what keeps them lined
//! up without a measuring pass.

use vlib::vui::prelude::*;
use vlib::vui::{TextAlign, div, text, uniform_list};

pub const ROWS: usize = 5_000;
pub const ROW_HEIGHT: f32 = 26.;
/// The element the list is, so its measured box can be asked for.
pub const LIST_ID: &str = "table";

pub const VIEWPORT: f32 = 220.;

/// The columns, and how wide each one is.
pub const COLUMNS: [(&str, f32); 4] = [
    ("symbol", 90.),
    ("last", 90.),
    ("change", 90.),
    ("volume", 110.),
];

/// One generated row. Deterministic, so a test and a screenshot agree.
pub fn quote(index: usize) -> (String, f64, f64, u64) {
    let seed = index as u64;
    let symbol = format!(
        "{}{}{}",
        (b'A' + (seed % 26) as u8) as char,
        (b'A' + ((seed / 26) % 26) as u8) as char,
        (b'A' + ((seed / 676) % 26) as u8) as char,
    );
    let last = 10. + (seed % 9000) as f64 / 100.;
    let change = ((seed % 401) as f64 - 200.) / 100.;
    let volume = 1_000 + seed * 37 % 900_000;
    (symbol, last, change, volume)
}

pub struct Table {
    pub list: ListState,
}

impl Default for Table {
    fn default() -> Self {
        Self {
            list: ListState::new(ROWS, ROW_HEIGHT),
        }
    }
}

fn cell(
    content: String,
    width: f32,
    color: vlib::vui::Color,
    align: TextAlign,
) -> impl IntoElement {
    div().w(width).flex().items_center().px(6.).child(
        text(content)
            .size(12.)
            .color(color)
            .max_width(width - 12.)
            .align(align)
            .ellipsis(1),
    )
}

impl Render for Table {
    fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        // The height the list was actually given, which is not the height it asked for: a
        // column that does not quite fit shrinks its children. `bounds_of` is last frame's box,
        // and on the first frame the declared height is the best estimate there is.
        let viewport = cx
            .bounds_of(LIST_ID)
            .map_or(VIEWPORT, |box_| box_.height().get());
        let thumb = self.list.thumb(viewport);
        div()
            .flex_col()
            .gap(6.)
            .p(16.)
            .size(460., 300.)
            .bg(rgb(0x101018))
            .child(
                text(format!("{ROWS} quotes"))
                    .size(12.)
                    .color(rgb(0x8080a0)),
            )
            // The header does not scroll, so it is not in the list.
            .child(
                div()
                    .id("header")
                    .flex()
                    .h(24.)
                    .w_full()
                    .rounded(4.)
                    .bg(rgb(0x22222e))
                    .children(COLUMNS.map(|(label, width)| {
                        cell(label.to_owned(), width, rgb(0x9090b0), TextAlign::Start)
                    })),
            )
            .child(
                div()
                    .flex()
                    .w_full()
                    .h(VIEWPORT)
                    .child(
                        uniform_list(&self.list, |rows| {
                            rows.map(|index| {
                                let (symbol, last, change, volume) = quote(index);
                                let tint = if change >= 0. {
                                    rgb(0x70d0a0)
                                } else {
                                    rgb(0xe08080)
                                };
                                div()
                                    .flex()
                                    .items_center()
                                    .bg(if index % 2 == 0 {
                                        rgb(0x16161e)
                                    } else {
                                        rgb(0x1a1a24)
                                    })
                                    .child(cell(
                                        symbol,
                                        COLUMNS[0].1,
                                        rgb(0xe6e6f0),
                                        TextAlign::Start,
                                    ))
                                    .child(cell(
                                        format!("{last:.2}"),
                                        COLUMNS[1].1,
                                        rgb(0xd0d0e0),
                                        TextAlign::End,
                                    ))
                                    .child(cell(
                                        format!("{change:+.2}"),
                                        COLUMNS[2].1,
                                        tint,
                                        TextAlign::End,
                                    ))
                                    .child(cell(
                                        format!("{volume}"),
                                        COLUMNS[3].1,
                                        rgb(0xa0a0c0),
                                        TextAlign::End,
                                    ))
                                    .into_node()
                            })
                            .collect()
                        })
                        .id("table")
                        .flex_1()
                        .h_full()
                        .rounded(4.)
                        .on_wheel(cx.listener(
                            |state: &mut Self, event, cx| {
                                if let UiEvent::Wheel { delta, .. } = event {
                                    let viewport = cx
                                        .bounds_of(LIST_ID)
                                        .map_or(VIEWPORT, |box_| box_.height().get());
                                    state.list.scroll_by(delta.y.get(), viewport);
                                    cx.notify();
                                }
                            },
                        )),
                    )
                    .child(
                        div()
                            .id("scrollbar")
                            .relative()
                            .w(8.)
                            .h_full()
                            .ml(4.)
                            .rounded(4.)
                            .bg(rgb(0x1c1c28))
                            .child(thumb.map(|(top, length)| {
                                div()
                                    .id("thumb")
                                    .absolute()
                                    .top(top)
                                    .left(0.)
                                    .w(8.)
                                    .h(length)
                                    .rounded(4.)
                                    .bg(rgb(0x4a5fd0))
                            })),
                    ),
            )
    }
}
