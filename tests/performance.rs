//! What a frame costs to build.
//!
//! This measures the producer's half only: render, measure, lay out, paint, and put a display
//! list on the wire. The host's half — compiling the list and drawing it — is not here and
//! cannot be, because there is no GPU in this test. So a number from this file is a floor, not a
//! frame time.
//!
//! It is still worth having. Layout is where this toolkit has already been bitten once: a chain
//! of auto-sized containers took most of a second at sixteen levels, and nothing but a
//! measurement would have said so.
//!
//! What dominates the number below is round trips to the host for text it has not measured
//! before — a table that scrolls is a table whose every run is new. So the properties worth
//! asserting are the ones that hold whatever a host's own timings are: the cost does not grow
//! with the number of rows, the requests are batched, and a frame that did not change is nearly
//! free.

#![cfg(feature = "testing")]

use std::time::{Duration, Instant};

use vivid_protocol::vector::Command;

use vlib::vui::prelude::*;
use vlib::vui::testing::TestUi;
use vlib::vui::{ListState, div, text, uniform_list};

/// A frame with everything expensive in it: a virtualized list, a header, plain cells, and two
/// columns of ellipsized text — which is what a table actually looks like, since most cells are
/// short and only some need truncating.
///
/// Every cell's text changes with the scroll, so nothing is reused between frames. That is the
/// expensive case: a table that scrolls is a table whose every paragraph is new.
struct Heavy {
    list: ListState,
    rows: usize,
}

const ROW_HEIGHT: f32 = 24.;
const VIEWPORT: f32 = 400.;
const COLUMNS: usize = 6;
/// Which columns truncate. Shaping is what costs a round trip; a plain run does not need one.
const ELLIPSIZED: [usize; 2] = [0, 3];

impl Default for Heavy {
    fn default() -> Self {
        Self {
            list: ListState::new(20_000, ROW_HEIGHT),
            rows: 20_000,
        }
    }
}

impl Render for Heavy {
    fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let _ = self.rows;
        div()
            .flex_col()
            .size(720., VIEWPORT + 40.)
            .bg(rgb(0x101018))
            .child(
                div()
                    .flex()
                    .h(28.)
                    .w_full()
                    .children((0..COLUMNS).map(|column| {
                        div().w(110.).child(
                            text(format!("column {column}"))
                                .size(12.)
                                .color(rgb(0x9090b0))
                                .max_width(100.),
                        )
                    })),
            )
            .child(
                uniform_list(&self.list, |rows| {
                    rows.map(|index| {
                        div()
                            .flex()
                            .items_center()
                            .bg(if index.is_multiple_of(2) {
                                rgb(0x16161e)
                            } else {
                                rgb(0x1a1a24)
                            })
                            .children((0..COLUMNS).map(|column| {
                                let cell = text(format!("r{index} c{column} value"))
                                    .size(12.)
                                    .color(rgb(0xd0d0e0))
                                    .max_width(100.);
                                div().w(110.).child(if ELLIPSIZED.contains(&column) {
                                    cell.ellipsis(1)
                                } else {
                                    cell
                                })
                            }))
                            .into_node()
                    })
                    .collect()
                })
                .id("rows")
                .w_full()
                .h(VIEWPORT)
                .on_wheel(cx.listener(|state: &mut Self, event, cx| {
                    if let UiEvent::Wheel { delta, .. } = event {
                        state.list.scroll_by(delta.y.get(), VIEWPORT);
                        cx.notify();
                    }
                })),
            )
    }
}

/// Build and submit `count` frames, each at a different scroll offset so nothing is reused, and
/// answer how long each took.
fn frame_times(count: usize) -> Vec<Duration> {
    let mut ui =
        TestUi::start(Heavy::default(), 720., f64::from(VIEWPORT) + 40.).expect("a window");
    let mut times = Vec::with_capacity(count);
    for step in 0..count {
        // A different offset every frame: the same frame twice would measure the submit-if-
        // unchanged path rather than the work.
        ui.update(|view| view.list.scroll_to(step as f32 * ROW_HEIGHT * 3., VIEWPORT));
        ui.app_mut().notify(0);
        let started = Instant::now();
        ui.app_mut().paint_once(0).expect("a frame");
        times.push(started.elapsed());
        ui.settle().expect("the host caught up");
    }
    times
}

fn percentile(sorted: &[Duration], fraction: f64) -> Duration {
    let index = ((sorted.len() as f64 - 1.) * fraction).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}

#[test]
fn a_heavy_frame_is_built_in_a_fraction_of_a_frame_budget() {
    let mut times = frame_times(60);
    times.sort_unstable();

    let p50 = percentile(&times, 0.5);
    let p95 = percentile(&times, 0.95);

    // Generous on purpose: this is a regression guard, not a benchmark, and it runs on whatever
    // machine happens to be building. What it catches is the kind of mistake that has already
    // happened here — a layout whose cost compounds — where the number is not slightly worse but
    // hundreds of times worse.
    assert!(
        p95 < Duration::from_millis(50),
        "building a frame should not cost a frame: p50 {p50:?}, p95 {p95:?}"
    );

    // And the work is bounded by what is on screen, not by what exists: twenty thousand rows
    // and the frame is still a few hundred commands.
    let ui = TestUi::start(Heavy::default(), 720., f64::from(VIEWPORT) + 40.).expect("a window");
    assert!(
        ui.canvas().commands().len() < 1000,
        "a virtualized frame stays small: {}",
        ui.canvas().commands().len()
    );
}

#[test]
fn scrolling_does_not_make_a_frame_grow() {
    let mut ui =
        TestUi::start(Heavy::default(), 720., f64::from(VIEWPORT) + 40.).expect("a window");
    let first = ui.canvas().commands().len();

    let list = ui.bounds("rows").expect("the list was laid out");
    let (x, y) = (list.center().x.get() as f64, list.center().y.get() as f64);
    ui.move_pointer(x, y).unwrap();
    ui.wheel(x, y, 0., f64::from(ROW_HEIGHT) * 5_000.).unwrap();

    let deep = ui.canvas().commands().len();
    assert!(
        (first as i64 - deep as i64).abs() < 30,
        "the frame is the same size five thousand rows down: {first} then {deep}"
    );

    // And the shapes the host is holding are bounded too: a row that scrolled away stops being
    // drawn, and what is not drawn is released.
    assert!(
        ui.retained_layouts() <= 256,
        "retained shapes stay inside the host's ceiling: {}",
        ui.retained_layouts()
    );
}

#[test]
#[ignore = "records frame-construction percentiles for this machine"]
fn frame_construction_percentiles() {
    let mut times = frame_times(240);
    times.sort_unstable();
    println!(
        "frame construction over {} frames: p50 {:?}, p95 {:?}, p99 {:?}, max {:?}",
        times.len(),
        percentile(&times, 0.5),
        percentile(&times, 0.95),
        percentile(&times, 0.99),
        times.last().copied().unwrap_or_default(),
    );
}

#[test]
fn a_frame_that_did_not_change_costs_almost_nothing() {
    let mut ui =
        TestUi::start(Heavy::default(), 720., f64::from(VIEWPORT) + 40.).expect("a window");

    // The expensive frame: every run is new, so every one is measured.
    ui.update(|view| view.list.scroll_to(4_000., VIEWPORT));
    ui.app_mut().notify(0);
    let cold = {
        let started = Instant::now();
        ui.app_mut().paint_once(0).expect("a frame");
        started.elapsed()
    };
    ui.settle().expect("the host caught up");

    // The same frame again: nothing changed, so nothing is measured and nothing is submitted.
    let warm = {
        let started = Instant::now();
        ui.app_mut().notify(0);
        ui.app_mut().paint_once(0).expect("a frame");
        started.elapsed()
    };

    // What is left in a still frame is layout: the element tree is rebuilt every frame, so the
    // taffy tree behind it is too. That is the price of an ephemeral tree, and it is the one
    // thing here that would repay a persistent one.
    assert!(
        warm * 2 < cold,
        "a still frame should be much cheaper than a new one: {warm:?} against {cold:?}"
    );
}

#[test]
fn a_frame_wanting_more_shapes_than_the_host_holds_still_draws_all_of_it() {
    // The host retains a bounded number of shaped paragraphs per owner, and the scene on screen
    // keeps its own charged until it is replaced — so a frame can shape about half the ceiling.
    // Past that a paragraph is drawn plainly: it loses the styling that needed shaping and keeps
    // its place in the frame.
    let mut ui = TestUi::start(Crowded, 400., 400.).expect("a window");

    let shaped = ui
        .canvas()
        .commands()
        .iter()
        .filter(|command| matches!(command, Command::TextLayout { .. }))
        .count();
    let plain = ui
        .canvas()
        .commands()
        .iter()
        .filter(|command| matches!(command, Command::Text(..)))
        .count();

    assert!(shaped > 0, "what fits is shaped");
    assert!(plain > 0, "and what does not is still drawn");
    assert_eq!(
        shaped + plain,
        Crowded::COUNT,
        "every paragraph reached the host somehow"
    );
    assert!(
        ui.retained_layouts() <= 128,
        "inside the host's ceiling: {}",
        ui.retained_layouts()
    );

    // And it is still a frame, not an error.
    ui.settle().expect("the host accepted it");
}

struct Crowded;

impl Crowded {
    const COUNT: usize = 200;
}

impl Render for Crowded {
    fn render(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_col()
            .size(400., 400.)
            .children((0..Self::COUNT).map(|index| {
                // Every one of them needs shaping, which is more than the host will hold.
                text(format!("paragraph number {index} of many"))
                    .size(10.)
                    .max_width(200.)
                    .ellipsis(1)
            }))
    }
}
