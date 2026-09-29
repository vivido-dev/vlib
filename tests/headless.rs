//! Whole frames, driven through a real session against the SDK's in-process presenter.
//!
//! Nothing here needs a GPU: what is asserted is the display list the host received, the boxes
//! layout produced, and what happened to the view's state when input arrived.

#![cfg(feature = "testing")]

#[path = "../examples/views/hello_world.rs"]
mod hello_world;

use vivid_protocol::vector::{Command, CursorShape};
use vlib::vui::div;
use vlib::vui::prelude::*;
use vlib::vui::testing::TestUi;

use hello_world::Counter;

#[test]
fn a_view_lays_out_draws_and_answers_a_click() {
    let mut ui = TestUi::start(Counter::default(), 420., 240.).unwrap();

    // The window's own box is what the frame is laid out against.
    let root = ui.bounds("increment").expect("the button was laid out");
    assert!(
        root.origin.y.get() > 0.,
        "the button sits under the text: {root:?}"
    );

    // The host received a display list with the button's region in it.
    let canvas = ui.canvas();
    let region = ui.region("increment").expect("the button has a region");
    assert!(canvas.commands().iter().any(|command| matches!(
        command,
        Command::Hit { id, cursor: Some(CursorShape::Pointer), .. } if *id == region
    )));

    // Moving onto the button is a hover, and the pointer shape travels with the region.
    ui.hover("increment").unwrap();
    assert!(
        ui.cursor().is_some(),
        "the host learned the pointer shape from the region"
    );

    // Clicking it runs the handler, which changes the state and asks for another frame.
    ui.click("increment").unwrap();
    assert_eq!(ui.read(|view| view.count), 1);

    // The new frame was submitted: the text moved with the count.
    let before = ui.canvas();
    ui.click("increment").unwrap();
    assert_eq!(ui.read(|view| view.count), 2);
    assert_ne!(before, ui.canvas(), "a changed frame reaches the host");
}

/// An element layout collapsed.
///
/// A container with nothing in it is ordinary — a panel that is empty, a spacer, a box whose
/// content has not arrived — and it used to take the whole frame down: the hit region was built
/// from a rectangle with no extent, which is not a shape the wire carries.
struct Collapsed;

impl Render for Collapsed {
    fn render(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_col()
            .size(200., 120.)
            .child(div().id("present").size(50., 20.))
            .child(div().id("collapsed").w_full().h(0.))
    }
}

#[test]
fn an_element_with_no_area_is_painted_and_skipped() {
    let ui = TestUi::start(Collapsed, 200., 120.).unwrap();

    // It was laid out, and it has no region, because there is nothing there to point at.
    let collapsed = ui.bounds("collapsed").expect("the element was laid out");
    assert_eq!(collapsed.size.height.get(), 0.);
    assert_eq!(ui.region("collapsed"), None);
    assert!(
        ui.region("present").is_some(),
        "and its neighbour is unaffected"
    );
}
