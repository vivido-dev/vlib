//! The canvas: drawing statements, the ceilings they run into, and the boxes they are drawn in.
//!
//! A canvas is the one element whose drawing is code, so most of what there is to check is what
//! happens when that code asks for something the wire cannot carry. Every failure here has to be
//! loud and early: painting truncates decoration when a frame runs out of room, and a canvas is
//! content, so an overrun is an error rather than a partial picture.

#![cfg(feature = "testing")]

use vivid_protocol::vector::{
    Brush, Canvas, Color, Command, MAX_PATH_SEGMENTS, Path, Scalar, StrokeStyle,
};
use vlib::vui::prelude::*;
use vlib::vui::testing::TestUi;
use vlib::vui::{ImageSource, canvas, div, img};

/// The color every canvas in this file draws in, so its commands can be told from the boxes
/// around them.
const INK: Color = rgb(0x00ff00);

fn filled(canvas: &Canvas, color: Color) -> Vec<&Path> {
    canvas
        .commands()
        .iter()
        .filter_map(|command| match command {
            Command::Fill(path, Brush::Solid(brush)) if *brush == color => Some(path),
            _ => None,
        })
        .collect()
}

/// The width of every stroke in the frame, plain or styled.
fn stroked(canvas: &Canvas) -> Vec<f64> {
    canvas
        .commands()
        .iter()
        .filter_map(|command| match command {
            Command::Stroke(_, _, width) => Some(width.get()),
            Command::StyledStroke(_, _, style) => Some(style.width.get()),
            _ => None,
        })
        .collect()
}

/// The point a path starts at, as `(x, y)`.
fn start_of(path: &Path) -> (f64, f64) {
    match path.segments.first().expect("a path begins with a segment") {
        vivid_protocol::vector::Segment::Move(point) => (point.x.get(), point.y.get()),
        other => panic!("a path starts with a move, not {other:?}"),
    }
}

/// One square at the box's own origin, so where it lands is where the box is.
struct Square;

impl Render for Square {
    fn render(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        div().p(24.).size(240., 160.).bg(rgb(0x101018)).child(
            canvas(|draw, bounds| {
                draw.fill(
                    Path::builder()
                        .move_to(bounds.origin.x.get() as f64, bounds.origin.y.get() as f64)
                        .line_to(bounds.right().get() as f64, bounds.origin.y.get() as f64)
                        .line_to(bounds.right().get() as f64, bounds.bottom().get() as f64)
                        .close(),
                    Brush::Solid(INK),
                );
            })
            .id("square")
            .size(100., 60.),
        )
    }
}

#[test]
fn a_canvas_draws_in_window_coordinates() {
    let ui = TestUi::start(Square, 240., 160.).unwrap();
    let box_of = ui.bounds("square").expect("the canvas was laid out");
    assert_eq!((box_of.origin.x.get(), box_of.origin.y.get()), (24., 24.));

    // The path is where the box is, not where the box is relative to itself: the drawing is in
    // window coordinates, and the box it was handed is the whole of the conversion.
    let canvas = ui.canvas();
    let drawn = filled(&canvas, INK);
    assert_eq!(drawn.len(), 1, "one shape was filled");
    assert_eq!(start_of(drawn[0]), (24., 24.));
}

/// A canvas that was never given a size.
struct Unsized;

impl Render for Unsized {
    fn render(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        let dot = |draw: &mut vlib::vui::Drawing, bounds: Bounds| {
            draw.fill(
                Path::builder()
                    .move_to(bounds.origin.x.get() as f64, bounds.origin.y.get() as f64)
                    .line_to(bounds.right().get() as f64, bounds.bottom().get() as f64)
                    .close(),
                Brush::Solid(INK),
            );
        };
        div()
            .flex_col()
            .items_start()
            .size(240., 160.)
            .bg(rgb(0x101018))
            .child(canvas(dot).id("unsized"))
            .child(canvas(dot).id("sized").size(40., 30.))
    }
}

#[test]
fn a_canvas_is_sized_by_its_box_and_not_by_its_drawing() {
    let ui = TestUi::start(Unsized, 240., 160.).unwrap();

    // Nothing in the closure says how big the drawing is, so a canvas nobody sized is nothing.
    let never_sized = ui.bounds("unsized").expect("the canvas exists");
    assert_eq!(never_sized.size.height.get(), 0.);
    let sized = ui.bounds("sized").expect("the canvas was laid out");
    assert_eq!(
        (sized.size.width.get(), sized.size.height.get()),
        (40., 30.)
    );

    // And an unsized one is not drawn at all rather than drawn into nothing: the closure is
    // skipped, because a drawing that paints a thousand paths should not be paid for by a box
    // with no room in it.
    assert_eq!(filled(&ui.canvas(), INK).len(), 1, "only the sized one");
}

/// A drawing that keeps going until the frame refuses it.
struct TooMany {
    paths: usize,
}

impl Render for TooMany {
    fn render(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        let paths = self.paths;
        div().size(200., 120.).child(
            canvas(move |draw, bounds| {
                // The longest path the protocol carries, drawn over and over: the command ceiling
                // is nowhere near, so what stops this is the segment budget.
                for _ in 0..paths {
                    let mut path = Path::builder()
                        .move_to(bounds.origin.x.get() as f64, bounds.origin.y.get() as f64);
                    for step in 1..MAX_PATH_SEGMENTS {
                        path = path.line_to(bounds.right().get() as f64, step as f64 % 8.);
                    }
                    draw.stroke(path, Brush::Solid(INK), 1.);
                }
            })
            .size(200., 120.),
        )
    }
}

#[test]
fn a_drawing_past_the_segment_budget_fails_the_frame_rather_than_truncating_it() {
    // Sixteen of these is the whole scene budget; the seventeenth is over it.
    let error = match TestUi::start(TooMany { paths: 17 }, 200., 120.) {
        Ok(_) => panic!("a canvas past the segment budget must not be accepted"),
        Err(error) => error,
    };
    let message = error.to_string();
    assert!(
        message.contains("canvas"),
        "the diagnosis names what could not be drawn: {message}"
    );
    assert!(
        message.contains("budget"),
        "and what it ran out of: {message}"
    );

    // Below it, it draws: the refusal is the budget rather than the shape of the test. Two paths
    // is not the ceiling — the host grants a record limit below the profile ceiling, and a scene
    // of eight thousand segments reaches it — but it is the same code path without the host
    // standing in front of it.
    let ui = TestUi::start(TooMany { paths: 2 }, 200., 120.).unwrap();
    assert_eq!(stroked(&ui.canvas()), vec![1., 1.]);
}

/// A closure that asks for three things the wire will not carry.
struct Bad {
    which: u8,
}

impl Render for Bad {
    fn render(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        let which = self.which;
        div().size(200., 120.).child(
            canvas(move |draw, bounds| {
                let x = bounds.origin.x.get() as f64;
                let y = bounds.origin.y.get() as f64;
                match which {
                    // A coordinate that is not a number.
                    0 => draw.fill(
                        Path::builder()
                            .move_to(f64::NAN, y)
                            .line_to(x + 10., y + 10.),
                        Brush::Solid(INK),
                    ),
                    // A stroke style the protocol's own validation refuses.
                    1 => draw.stroke_styled(
                        Path::builder().move_to(x, y).line_to(x + 10., y + 10.),
                        Brush::Solid(INK),
                        StrokeStyle {
                            miter_limit: Scalar::ZERO,
                            ..StrokeStyle::default()
                        },
                    ),
                    // A path the wire would refuse however it was built.
                    _ => draw.fill(Path::builder().line_to(x + 10., y + 10.), Brush::Solid(INK)),
                }
            })
            .size(200., 120.),
        )
    }
}

#[test]
fn a_drawing_statement_the_wire_refuses_is_reported_once_and_loudly() {
    for (which, expected) in [
        (0, "coordinate"),
        (1, "miter limit"),
        (2, "path must begin with move"),
    ] {
        let error = match TestUi::start(Bad { which }, 200., 120.) {
            Ok(_) => panic!("a drawing the wire refuses must not be accepted: {expected}"),
            Err(error) => error,
        };
        let message = error.to_string();
        assert!(message.contains("canvas"), "the canvas is named: {message}");
        assert!(
            message.contains(expected),
            "and so is the reason, which should be {expected}: {message}"
        );
    }
}

#[test]
fn a_bad_statement_does_not_stop_the_ones_before_it_from_being_checked() {
    // Both statements are wrong, and the first is the one reported — the same sticky-error rule
    // the protocol's own builder follows.
    struct Two;
    impl Render for Two {
        fn render(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
            div().size(200., 120.).child(
                canvas(|draw, _bounds| {
                    draw.fill(
                        Path::builder().move_to(f64::NAN, 0.).line_to(1., 1.),
                        Brush::Solid(INK),
                    );
                    draw.stroke_styled(
                        Path::builder().move_to(0., 0.).line_to(10., 10.),
                        Brush::Solid(INK),
                        StrokeStyle {
                            miter_limit: Scalar::ZERO,
                            ..StrokeStyle::default()
                        },
                    );
                })
                .size(200., 120.),
            )
        }
    }

    let error = match TestUi::start(Two, 200., 120.) {
        Ok(_) => panic!("both statements are wrong, so the frame must be refused"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("coordinate"),
        "the first thing that went wrong is the one reported: {error}"
    );
}

/// A canvas and an image, each styled like a box.
struct Styled;

impl Render for Styled {
    fn render(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_col()
            .gap(8.)
            .size(240., 200.)
            .bg(rgb(0x101018))
            .child(
                canvas(|draw, bounds| {
                    draw.fill(
                        Path::builder()
                            .move_to(bounds.origin.x.get() as f64, bounds.origin.y.get() as f64)
                            .line_to(bounds.right().get() as f64, bounds.bottom().get() as f64)
                            .close(),
                        Brush::Solid(INK),
                    );
                })
                .size(80., 40.)
                .bg(rgb(0x203040))
                .border(2., rgb(0x506070))
                .rounded(4.),
            )
            .child(
                img(pixels())
                    .id("picture")
                    .size(80., 40.)
                    .bg(rgb(0x203040))
                    .border(2., rgb(0x506070)),
            )
    }
}

/// Two pixels of red, which need no decoder to mean something.
fn pixels() -> ImageSource {
    ImageSource::rgba("red", 2, 2, [0xd0, 0x40, 0x40, 0xff].repeat(4))
}

#[test]
fn an_element_that_has_a_box_paints_its_own_box() {
    let ui = TestUi::start(Styled, 240., 200.).unwrap();
    let canvas = ui.canvas();

    // Both the background and the border of both elements reached the host. This is where a
    // picture's own styling used to be dropped on the floor: the image arm painted the picture
    // and nothing else, so `.bg(..)` and `.border(..)` on an `img(..)` did nothing at all.
    let backgrounds = filled(&canvas, rgb(0x203040));
    assert_eq!(
        backgrounds.len(),
        2,
        "one for the canvas, one for the image"
    );
    let borders = canvas
        .commands()
        .iter()
        .filter(|command| matches!(command, Command::Stroke(_, Brush::Solid(color), _) if *color == rgb(0x506070)))
        .count();
    assert_eq!(borders, 2, "and a border around each");
}

#[test]
fn a_dashed_border_thinner_than_half_a_pixel_is_still_a_border() {
    // A dashed border used to be stroked with the miter limit set to the dash length, which is
    // below the limit the wire accepts for anything thinner than half a pixel — so this frame was
    // refused, and the window showed nothing at all.
    struct Hairline;
    impl Render for Hairline {
        fn render(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
            div().size(120., 80.).child(
                div()
                    .size(60., 40.)
                    .bg(rgb(0x203040))
                    .border_dashed(0.4, rgb(0x506070)),
            )
        }
    }

    let ui = TestUi::start(Hairline, 120., 80.).unwrap();
    let dashed = ui
        .canvas()
        .commands()
        .iter()
        .filter(|command| matches!(command, Command::StyledStroke(..)))
        .count();
    assert_eq!(dashed, 1, "the dashed border was drawn");
}

#[test]
fn a_canvas_says_what_it_is_to_a_screen_reader() {
    // A drawing is content, but nothing about it is inferable: a canvas describes itself only
    // when the view says so, which is what `Semantic` is for.
    struct Named;
    impl Render for Named {
        fn render(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
            div().size(200., 120.).child(
                canvas(|draw, bounds| {
                    draw.fill(
                        Path::builder()
                            .move_to(bounds.origin.x.get() as f64, bounds.origin.y.get() as f64)
                            .line_to(bounds.right().get() as f64, bounds.bottom().get() as f64)
                            .close(),
                        Brush::Solid(INK),
                    );
                })
                .id("chart")
                .role(SemanticRole::Image)
                .label("a rising line")
                .size(200., 120.),
            )
        }
    }

    let ui = TestUi::start(Named, 200., 120.).unwrap();
    let node = ui.semantic_node("a rising line").expect("described");
    assert_eq!(node.role, SemanticRole::Image);
    assert_eq!(
        Some(node.id),
        ui.region("chart"),
        "an interactive node's identity is the one a click uses"
    );
}
