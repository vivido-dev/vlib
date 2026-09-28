//! The view behind `input`: a text field you can type into.
//!
//! Typing, selection, arrow keys, and copy/cut all come from the protocol's own keys: the field
//! answers HID usages, so the same code types the same way on every host. Paste arrives as
//! committed text through the host's paste policy — a terminal-hosted editor never reads the
//! clipboard, it only writes one.

use vlib::vui::prelude::*;
use vlib::vui::{CursorShape, TextInput, div, text, text_input};

actions!(input_example, [FocusField, Copy, Cut]);

pub struct Editor {
    pub field: TextInput,
    pub handle: FocusHandle,
    pub copies: usize,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            field: TextInput::new(""),
            handle: FocusHandle::new("editor"),
            copies: 0,
        }
    }
}

impl Render for Editor {
    fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        cx.bind_keys([
            KeyBinding::parse("tab", ActionId::of::<FocusField>()).unwrap(),
            KeyBinding::parse("cmd-c", ActionId::of::<Copy>()).unwrap(),
            KeyBinding::parse("cmd-x", ActionId::of::<Cut>()).unwrap(),
        ]);
        let handle = self.handle.clone();
        div()
            .id("surface")
            .flex_col()
            .gap(12.)
            .p(20.)
            .size(420., 240.)
            .bg(rgb(0x101018))
            .on_key(cx.listener(|state: &mut Self, event, cx| {
                if state.field.handle(event) {
                    cx.notify();
                }
            }))
            .on_text(cx.listener(|state: &mut Self, event, cx| {
                if state.field.handle(event) {
                    cx.notify();
                }
            }))
            .on_action::<Copy>(cx.listener(|state: &mut Self, _event, cx| {
                let selected = state.field.selected_text().to_owned();
                if !selected.is_empty() && cx.copy(&selected).is_ok() {
                    state.copies += 1;
                }
                cx.notify();
            }))
            .on_action::<Cut>(cx.listener(|state: &mut Self, _event, cx| {
                let selected = state.field.selected_text().to_owned();
                if !selected.is_empty() && cx.copy(&selected).is_ok() {
                    state.field.cut();
                    state.copies += 1;
                }
                cx.notify();
            }))
            .on_action::<FocusField>(cx.listener(|state: &mut Self, _event, cx| {
                cx.focus(state.handle.clone());
            }))
            .child(text("Type here").size(12.).color(rgb(0x8080a0)))
            .child(
                div()
                    .id("field")
                    .w_full()
                    .p(10.)
                    .rounded(6.)
                    .bg(rgb(0x1c1c28))
                    .border(1., rgb(0x30304a))
                    .focus_ring(|style| style.border(2., rgb(0x8ecbff)))
                    .cursor(CursorShape::Text)
                    .on_click(cx.listener(move |state: &mut Self, _event, cx| {
                        cx.focus(state.handle.clone());
                    }))
                    .child(
                        text_input(&self.field)
                            .size(16.)
                            .color(rgb(0xe6e6f0))
                            .placeholder("a line of text")
                            .track_focus(&handle)
                            .cursor(CursorShape::Text),
                    ),
            )
            .child(
                text(format!(
                    "{} characters, {} copied",
                    self.field.text().chars().count(),
                    self.copies
                ))
                .size(12.)
                .color(rgb(0x8080a0)),
            )
    }
}
