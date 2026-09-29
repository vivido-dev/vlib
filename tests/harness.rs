//! How to test a `vlib::vui` application, in one file.
//!
//! Everything here runs against the SDK's in-process presenter: a real authentication transcript,
//! real record framing, a real display list at the other end. No GPU, no terminal, no waiting.
//! What a test asserts is what a host would have received.
//!
//! This is the counterpart to GPUI's `testing` example. It is an integration test rather than an
//! example binary because that is what it is — the point is the assertions, not a window.

#![cfg(feature = "testing")]

use std::time::Duration;

use vivid_protocol::vector::{Brush, Command};
use vlib::vui::prelude::*;
use vlib::vui::testing::TestUi;
use vlib::vui::{CursorShape, div, text};

actions!(harness, [Reset]);

/// A view small enough to read and complete enough to exercise: a counter with a button, a
/// label, and something to say about itself.
#[derive(Default)]
struct Counter {
    count: i32,
    hovered_hint: bool,
}

impl Render for Counter {
    fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        cx.bind_keys([KeyBinding::parse("cmd-r", ActionId::of::<Reset>()).unwrap()]);
        div()
            .id("surface")
            .flex_col()
            .gap(10.)
            .p(16.)
            .size(240., 140.)
            .bg(rgb(0x101018))
            .role(SemanticRole::Application)
            .label("Counter")
            .on_action::<Reset>(cx.listener(|state: &mut Self, _event, cx| {
                state.count = 0;
                cx.notify();
            }))
            .child(
                text(format!("Count: {}", self.count))
                    .size(14.)
                    .color(rgb(0xe6e6f0)),
            )
            .child(
                div()
                    .id("increment")
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(120., 32.)
                    .rounded(6.)
                    .bg(rgb(0x4a5fd0))
                    .cursor(CursorShape::Pointer)
                    .hover(|style| style.bg(rgb(0x5a6fe0)))
                    .role(SemanticRole::Button)
                    .label("Increment")
                    .accessible_actions([AccessibleAction::Default])
                    .on_click(cx.listener(|state: &mut Self, _event, cx| {
                        state.count += 1;
                        cx.notify();
                    }))
                    .on_accessibility(cx.listener(|state: &mut Self, _event, cx| {
                        state.count += 1;
                        cx.notify();
                    }))
                    .on_hover(cx.listener(|state: &mut Self, event, cx| {
                        if let UiEvent::Hover { entered } = event {
                            state.hovered_hint = *entered;
                            cx.notify();
                        }
                    }))
                    .child(text("Increment").size(13.).color(rgb(0xffffff))),
            )
    }
}

/// Starting a test is one line: a view, and the window it goes in.
fn started() -> TestUi<Counter> {
    TestUi::start(Counter::default(), 240., 140.).expect("a window against the test presenter")
}

#[test]
fn a_test_asserts_on_what_was_laid_out() {
    let ui = started();

    // Where an element ended up, by the identity it was given.
    let button = ui.bounds("increment").expect("the button was laid out");
    assert_eq!(button.width().get(), 120.);
    assert_eq!(button.height().get(), 32.);
    // Inside the window's padding, under the label above it.
    assert!(button.origin.x.get() >= 16.);
    assert!(button.origin.y.get() > 16.);

    // An element that does not exist says so rather than defaulting to somewhere.
    assert_eq!(ui.bounds("nothing-here"), None);
}

#[test]
fn a_test_asserts_on_what_the_host_received() {
    let ui = started();
    let canvas = ui.canvas();

    // The display list itself: this is the wire, not a rendering of it.
    assert!(canvas.commands().iter().any(|command| matches!(
        command,
        Command::Fill(_, Brush::Solid(color)) if color.0 == rgb(0x4a5fd0).0
    )));

    // A region is how the host knows the button is there at all.
    let region = ui
        .region("increment")
        .expect("the button asked for a region");
    assert!(canvas.commands().iter().any(|command| matches!(
        command,
        Command::Hit { id, cursor: Some(CursorShape::Pointer), .. } if *id == region
    )));

    // Text is read from the frame, because a shaped paragraph reaches the host as an identity
    // rather than as its words.
    assert!(ui.texts().iter().any(|text| text == "Count: 0"));
}

#[test]
fn a_test_drives_the_input_a_person_would() {
    let mut ui = started();
    let region = ui.region("increment").expect("a region");

    // Hovering is hovering: the host hit-tests it and the element hears about it.
    ui.hover("increment").unwrap();
    assert!(ui.state_at(region).hovered);
    assert!(ui.read(|view| view.hovered_hint));
    assert_eq!(
        ui.cursor(),
        Some(CursorShape::Pointer),
        "and the pointer took its shape"
    );

    // Clicking runs the handler and the next frame follows.
    ui.click("increment").unwrap();
    assert_eq!(ui.read(|view| view.count), 1);
    assert!(ui.texts().iter().any(|text| text == "Count: 1"));

    // Keys reach a window once something in it has been pressed, which the click above did.
    ui.key(vlib::vui::keymap::usage_of("r").unwrap(), true, modifiers())
        .unwrap();
    ui.key(
        vlib::vui::keymap::usage_of("r").unwrap(),
        false,
        modifiers(),
    )
    .unwrap();
    assert_eq!(ui.read(|view| view.count), 0, "the binding fired");
}

/// The platform's command modifier, as the protocol names it.
fn modifiers() -> u32 {
    vivid_protocol::overlay::modifiers::SUPER
}

#[test]
fn a_test_owns_the_clock() {
    let mut ui = started();

    // Nothing in this view moves, so nothing is scheduled and the loop would sleep.
    assert_eq!(ui.wake_in(), None);
    assert_eq!(ui.elapsed(), Duration::ZERO, "a test starts at zero");

    // Time passes when the test says so, and never otherwise: an animation is a function of the
    // clock, so a test that owns the clock owns the animation.
    ui.advance(Duration::from_millis(500)).unwrap();
    assert_eq!(ui.elapsed(), Duration::from_millis(500));
}

#[test]
fn a_test_reads_what_the_application_says_about_itself() {
    let mut ui = started();

    let semantics = ui.semantics().expect("the counter described itself");
    semantics.validate().expect("a tree the protocol accepts");
    assert_eq!(semantics.nodes[0].label, "Counter");

    let button = ui.semantic_node("Increment").expect("a described button");
    assert_eq!(button.role, SemanticRole::Button);

    // And what assistive technology would ask for arrives as an event.
    assert!(
        ui.accessibility_action("Increment", AccessibleAction::Default)
            .unwrap()
    );
    assert_eq!(ui.read(|view| view.count), 1);
}

#[test]
fn a_test_sees_the_budgets_the_host_enforces() {
    let ui = started();

    // A frame is commands, and they are finite. Everything else is bounded the same way.
    assert!(ui.canvas().commands().len() < 4096);
    assert_eq!(ui.retained_layouts(), 0, "nothing here needed shaping");
    assert_eq!(ui.retained_images(), 0, "nor any pixels");
    assert_eq!(
        ui.dropped_semantics(),
        0,
        "nothing was left out of the description"
    );
}

#[test]
fn a_test_changes_the_state_directly_when_that_is_the_point() {
    let mut ui = started();

    // Not every test is about the input that would have caused a change. Setting the state and
    // repainting asks the narrower question: given this state, what does the frame look like?
    ui.update(|view| view.count = 41);
    ui.app_mut().notify(0);
    ui.paint().unwrap();
    assert!(ui.texts().iter().any(|text| text == "Count: 41"));
}
