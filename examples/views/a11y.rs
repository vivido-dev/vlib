//! The view behind `a11y`: a panel that says what it is.
//!
//! A Canvas window is opaque — the host sees a display list, and a display list does not say
//! which rectangle is a switch. So this panel describes itself: roles, labels, values, states,
//! and the actions it will answer. Assistive technology reads that description, and what it asks
//! for comes back as an event like any other.
//!
//! Nothing is inferred from drawing. The switch below is announced as a switch because it says it
//! is one, not because it looks like one.

use vlib::vui::prelude::*;
use vlib::vui::{CursorShape, div, text};

pub const ITEMS: [&str; 3] = ["Appearance", "Keyboard", "Network"];
pub const VOLUME_STEP: i32 = 5;

pub struct Panel {
    pub notifications: bool,
    pub volume: i32,
    pub selected: usize,
    /// How many times the panel has been applied, so a test can see an action arrive.
    pub applied: usize,
}

impl Default for Panel {
    fn default() -> Self {
        Self {
            notifications: true,
            volume: 40,
            selected: 0,
            applied: 0,
        }
    }
}

impl Panel {
    pub fn adjust(&mut self, by: i32) {
        self.volume = (self.volume + by).clamp(0, 100);
    }
}

impl Render for Panel {
    fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = ITEMS.iter().enumerate().map(|(index, label)| {
            let chosen = self.selected == index;
            div()
                .id(format!("item-{index}"))
                .flex()
                .items_center()
                .px(8.)
                .h(26.)
                .rounded(4.)
                .bg(if chosen { rgb(0x24304a) } else { rgb(0x181824) })
                .cursor(CursorShape::Pointer)
                .hover(|style| style.bg(rgb(0x2a2a3c)))
                // "The second of three", which is what a screen reader reads out.
                .role(SemanticRole::ListItem)
                .label(*label)
                .accessible_position(index as u16 + 1, ITEMS.len() as u16)
                .accessible_actions([AccessibleAction::Default, AccessibleAction::Click])
                .on_click(cx.listener(move |state: &mut Self, _event, cx| {
                    state.selected = index;
                    cx.notify();
                }))
                .on_accessibility(cx.listener(move |state: &mut Self, _event, cx| {
                    state.selected = index;
                    cx.notify();
                }))
                .child(text(*label).size(13.).color(rgb(0xd0d0e0)))
        });

        div()
            // The panel as a whole, which is what the tree hangs from.
            .role(SemanticRole::Application)
            .label("Settings")
            .flex_col()
            .gap(10.)
            .p(20.)
            .size(360., 340.)
            .bg(rgb(0x101018))
            .child(
                text("Settings")
                    .size(18.)
                    .color(rgb(0xe6e6f0))
                    .role(SemanticRole::Heading)
                    .label("Settings")
                    .accessible_level(1),
            )
            // A switch: a state, not a value.
            .child(
                div()
                    .id("notifications")
                    .flex()
                    .items_center()
                    .justify_between()
                    .w_full()
                    .h(32.)
                    .px(10.)
                    .rounded(6.)
                    .bg(rgb(0x1a1a24))
                    .cursor(CursorShape::Pointer)
                    .role(SemanticRole::Switch)
                    .label("Notifications")
                    .toggled(if self.notifications {
                        Toggled::On
                    } else {
                        Toggled::Off
                    })
                    .accessible_actions([AccessibleAction::Default, AccessibleAction::Click])
                    .on_click(cx.listener(|state: &mut Self, _event, cx| {
                        state.notifications = !state.notifications;
                        cx.notify();
                    }))
                    .on_accessibility(cx.listener(|state: &mut Self, _event, cx| {
                        state.notifications = !state.notifications;
                        cx.notify();
                    }))
                    .child(text("Notifications").size(13.).color(rgb(0xd0d0e0)))
                    .child(div().size(36., 18.).rounded(9.).bg(if self.notifications {
                        rgb(0x4a5fd0)
                    } else {
                        rgb(0x33334a)
                    })),
            )
            // A spin button: a value in a range, which increments and decrements.
            .child(
                div()
                    .id("volume")
                    .flex()
                    .items_center()
                    .justify_between()
                    .w_full()
                    .h(32.)
                    .px(10.)
                    .rounded(6.)
                    .bg(rgb(0x1a1a24))
                    .role(SemanticRole::SpinButton)
                    .label("Volume")
                    .accessible_value(self.volume as f32, 0., 100.)
                    .accessible_actions([AccessibleAction::Increment, AccessibleAction::Decrement])
                    .on_accessibility(cx.listener(|state: &mut Self, event, cx| {
                        // The action says which way; the step is the application's to choose.
                        if let UiEvent::Accessibility { action } = event {
                            match action {
                                AccessibleAction::Increment => state.adjust(VOLUME_STEP),
                                AccessibleAction::Decrement => state.adjust(-VOLUME_STEP),
                                _ => {}
                            }
                            cx.notify();
                        }
                    }))
                    .child(text("Volume").size(13.).color(rgb(0xd0d0e0)))
                    .child(
                        text(format!("{}", self.volume))
                            .size(13.)
                            .color(rgb(0x8ecbff)),
                    ),
            )
            // A list, whose items say which of how many they are.
            .child(
                div()
                    .id("sections")
                    .flex_col()
                    .gap(4.)
                    .w_full()
                    .role(SemanticRole::List)
                    .label("Sections")
                    .children(rows),
            )
            .child(
                div()
                    .id("apply")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w_full()
                    .h(32.)
                    .rounded(6.)
                    .bg(rgb(0x4a5fd0))
                    .cursor(CursorShape::Pointer)
                    .hover(|style| style.bg(rgb(0x5a6fe0)))
                    .role(SemanticRole::Button)
                    .label("Apply settings")
                    .accessible_actions([AccessibleAction::Default, AccessibleAction::Click])
                    .on_click(cx.listener(|state: &mut Self, _event, cx| {
                        state.applied += 1;
                        cx.notify();
                    }))
                    .on_accessibility(cx.listener(|state: &mut Self, _event, cx| {
                        state.applied += 1;
                        cx.notify();
                    }))
                    .child(text("Apply").size(13.).color(rgb(0xffffff))),
            )
    }
}
