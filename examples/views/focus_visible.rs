//! The view behind `focus_visible`: a focus ring that a keyboard shows and a click does not.
//!
//! Focus that arrived from the keyboard is drawn with a ring; focus that arrived from a click is
//! not. That is the whole difference between `focus` and `focus_from_pointer` in this toolkit.

use vlib::vui::prelude::*;
use vlib::vui::{CursorShape, div, text};

actions!(focus_visible_example, [FocusNext]);

pub struct FocusRings {
    pub handles: Vec<FocusHandle>,
    pub focused: usize,
}

impl Default for FocusRings {
    fn default() -> Self {
        Self {
            handles: (0..3)
                .map(|index| FocusHandle::new(format!("ring-{index}")))
                .collect(),
            focused: 0,
        }
    }
}

impl Render for FocusRings {
    fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        cx.bind_keys([KeyBinding::parse("tab", ActionId::of::<FocusNext>()).unwrap()]);
        let buttons = (0..self.handles.len()).map(|index| {
            let handle = self.handles[index].clone();
            div()
                .id(format!("button-{index}"))
                .flex()
                .items_center()
                .justify_center()
                .size(96., 36.)
                .rounded(6.)
                .bg(rgb(0x2a2a3c))
                .cursor(CursorShape::Pointer)
                .hover(|style| style.bg(rgb(0x36364c)))
                .focus_ring(|style| style.border(2., rgb(0x8ecbff)))
                .track_focus(&handle)
                .on_click(cx.listener(move |state: &mut Self, _event, cx| {
                    // A click focuses without claiming the keyboard put it there.
                    state.focused = index;
                    cx.focus_from_pointer(state.handles[index].clone());
                    cx.notify();
                }))
                .child(text(format!("{}", index + 1)).color(rgb(0xffffff)))
        });
        div()
            .id("surface")
            .flex_col()
            .gap(12.)
            .p(20.)
            .size(360., 220.)
            .bg(rgb(0x101018))
            .on_action::<FocusNext>(cx.listener(|state: &mut Self, _event, cx| {
                state.focused = (state.focused + 1) % state.handles.len().max(1);
                cx.focus(state.handles[state.focused].clone());
                cx.notify();
            }))
            .child(
                text("Tab shows a ring; clicking does not")
                    .size(12.)
                    .color(rgb(0x8080a0)),
            )
            .child(div().flex().gap(10.).children(buttons))
    }
}
