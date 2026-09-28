//! The view behind `tab_stop`: tab and shift-tab move focus through a set of fields.

use vlib::vui::prelude::*;
use vlib::vui::{CursorShape, TextInput, div, text, text_input};

actions!(tab_stop_example, [FocusNext, FocusPrevious]);

pub struct TabStops {
    pub fields: Vec<TextInput>,
    pub handles: Vec<FocusHandle>,
    pub focused: usize,
}

impl Default for TabStops {
    fn default() -> Self {
        Self {
            fields: (0..3)
                .map(|index| TextInput::new(format!("field {}", index + 1)))
                .collect(),
            handles: (0..3)
                .map(|index| FocusHandle::new(format!("stop-{index}")))
                .collect(),
            focused: 0,
        }
    }
}

impl Render for TabStops {
    fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        cx.bind_keys([
            KeyBinding::parse("tab", ActionId::of::<FocusNext>()).unwrap(),
            KeyBinding::parse("shift-tab", ActionId::of::<FocusPrevious>()).unwrap(),
        ]);
        let rows = (0..self.fields.len()).map(|index| {
            let handle = self.handles[index].clone();
            let active = self.focused == index;
            div()
                .id(format!("row-{index}"))
                .w_full()
                .p(8.)
                .rounded(6.)
                .bg(if active { rgb(0x24304a) } else { rgb(0x181824) })
                .border(1., if active { rgb(0x8ecbff) } else { rgb(0x2a2a3c) })
                .cursor(CursorShape::Text)
                .on_click(cx.listener(move |state: &mut Self, _event, cx| {
                    state.focused = index;
                    cx.focus(state.handles[index].clone());
                    cx.notify();
                }))
                .child(
                    text_input(&self.fields[index])
                        .size(14.)
                        .color(rgb(0xe6e6f0))
                        .track_focus(&handle)
                        .cursor(CursorShape::Text),
                )
        });
        div()
            .id("surface")
            .flex_col()
            .gap(8.)
            .p(20.)
            .size(360., 240.)
            .bg(rgb(0x101018))
            .on_key(cx.listener(|state: &mut Self, event, cx| {
                let changed = state.field_under_focus().is_some_and(|index| {
                    let mut field = std::mem::take(&mut state.fields[index]);
                    let changed = field.handle(event);
                    state.fields[index] = field;
                    changed
                });
                if changed {
                    cx.notify();
                }
            }))
            .on_text(cx.listener(|state: &mut Self, event, cx| {
                let changed = state.field_under_focus().is_some_and(|index| {
                    let mut field = std::mem::take(&mut state.fields[index]);
                    let changed = field.handle(event);
                    state.fields[index] = field;
                    changed
                });
                if changed {
                    cx.notify();
                }
            }))
            .on_action::<FocusNext>(cx.listener(|state: &mut Self, _event, cx| {
                state.focused = (state.focused + 1) % state.handles.len().max(1);
                cx.focus(state.handles[state.focused].clone());
                cx.notify();
            }))
            .on_action::<FocusPrevious>(cx.listener(|state: &mut Self, _event, cx| {
                let count = state.handles.len().max(1);
                state.focused = (state.focused + count - 1) % count;
                cx.focus(state.handles[state.focused].clone());
                cx.notify();
            }))
            .child(
                text("Tab moves between the fields")
                    .size(12.)
                    .color(rgb(0x8080a0)),
            )
            .children(rows)
    }
}

impl TabStops {
    /// The index of the field the window says is focused, if any.
    pub fn field_under_focus(&self) -> Option<usize> {
        Some(self.focused)
    }
}
