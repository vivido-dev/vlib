//! The view behind `list_example`: a message log pinned to its newest entry, with a scrollbar.

use vlib::vui::prelude::*;
use vlib::vui::{div, text, uniform_list};

actions!(log_example, [Append]);

pub const ROW_HEIGHT: f32 = 22.;
/// The element the list is, so its measured box can be asked for.
pub const LIST_ID: &str = "log";

pub const VIEWPORT: f32 = 200.;

pub struct Log {
    pub entries: Vec<String>,
    pub list: ListState,
    /// Stay at the bottom as entries arrive, until the reader scrolls away from it.
    pub follow: bool,
}

impl Default for Log {
    fn default() -> Self {
        let entries: Vec<String> = (0..200)
            .map(|index| format!("[{index:04}] the log said something worth keeping"))
            .collect();
        let mut list = ListState::new(entries.len(), ROW_HEIGHT);
        list.scroll_to(list.maximum_offset(VIEWPORT), VIEWPORT);
        Self {
            entries,
            list,
            follow: true,
        }
    }
}

impl Log {
    /// Append an entry, following the tail if the reader has not scrolled away from it.
    ///
    /// `viewport` is the height the list was last given: where its end is depends on how much of
    /// it is on screen.
    pub fn push(&mut self, entry: impl Into<String>, viewport: f32) {
        self.entries.push(entry.into());
        self.list.set_count(self.entries.len(), viewport);
        if self.follow {
            self.list
                .scroll_to(self.list.maximum_offset(viewport), viewport);
        }
    }

    /// Whether the list is at its end, which is what "following" means.
    pub fn at_tail(&self, viewport: f32) -> bool {
        (self.list.offset() - self.list.maximum_offset(viewport)).abs() < 1.
    }
}

impl Render for Log {
    fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let entries = self.entries.clone();
        // The height the list was actually given, which is not the height it asked for: a
        // column that does not quite fit shrinks its children. `bounds_of` is last frame's box,
        // and on the first frame the declared height is the best estimate there is.
        let viewport = cx
            .bounds_of(LIST_ID)
            .map_or(VIEWPORT, |box_| box_.height().get());
        let thumb = self.list.thumb(viewport);
        cx.bind_keys([KeyBinding::parse("enter", ActionId::of::<Append>()).unwrap()]);
        div()
            .id("surface")
            .flex_col()
            .gap(8.)
            .p(16.)
            .size(420., 260.)
            .bg(rgb(0x101018))
            .on_action::<Append>(cx.listener(|state: &mut Self, _event, cx| {
                let viewport = cx
                    .bounds_of(LIST_ID)
                    .map_or(VIEWPORT, |box_| box_.height().get());
                let next = state.entries.len();
                state.push(format!("[{next:04}] appended by hand"), viewport);
                cx.notify();
            }))
            .child(
                text(if self.follow {
                    "following the tail"
                } else {
                    "scrolled back"
                })
                .size(12.)
                .color(rgb(0x8080a0)),
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .w_full()
                    .h(VIEWPORT)
                    .child(
                        uniform_list(&self.list, move |rows| {
                            rows.map(|index| {
                                div()
                                    .flex()
                                    .items_center()
                                    .px(8.)
                                    .child(
                                        text(entries.get(index).cloned().unwrap_or_default())
                                            .size(12.)
                                            .color(rgb(0xc0c0d0)),
                                    )
                                    .into_node()
                            })
                            .collect()
                        })
                        .id("log")
                        .flex_1()
                        .h_full()
                        .rounded(6.)
                        .bg(rgb(0x16161e))
                        .on_wheel(cx.listener(
                            |state: &mut Self, event, cx| {
                                if let UiEvent::Wheel { delta, .. } = event {
                                    let viewport = cx
                                        .bounds_of(LIST_ID)
                                        .map_or(VIEWPORT, |box_| box_.height().get());
                                    state.list.scroll_by(delta.y.get(), viewport);
                                    // Letting go of the tail is a scroll away from it; coming back
                                    // takes it up again.
                                    state.follow = state.at_tail(viewport);
                                    cx.notify();
                                }
                            },
                        )),
                    )
                    // The scrollbar is elements, not a special case: a track, and a thumb whose
                    // length is the fraction of the content on screen.
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
