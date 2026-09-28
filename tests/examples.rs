//! Each C1 example, driven a frame at a time against the headless presenter.
//!
//! A test here asserts what the host received and what the view's state became. It is the same
//! code path a live pane runs: render, measure, lay out, paint, submit, route input.

#![cfg(feature = "testing")]

#[path = "../examples/views/a11y.rs"]
mod a11y_view;
#[path = "../examples/views/active_state_bug.rs"]
mod active_state_bug;
#[path = "../examples/views/anchor.rs"]
mod anchor;
#[path = "../examples/views/animation.rs"]
mod animation_view;
#[path = "../examples/views/data_table.rs"]
mod data_table;
#[path = "../examples/views/drag_drop.rs"]
mod drag_drop;
#[path = "../examples/views/focus_visible.rs"]
mod focus_visible;
#[path = "../examples/views/gif_viewer.rs"]
mod gif_viewer;
#[path = "../examples/views/gradient.rs"]
mod gradient;
#[path = "../examples/views/grid_layout.rs"]
mod grid_layout;
#[path = "../examples/views/hello_world.rs"]
mod hello_world;
#[path = "../examples/views/image_gallery.rs"]
mod image_gallery;
#[path = "../examples/views/image_loading.rs"]
mod image_loading;
#[path = "../examples/views/image.rs"]
mod image_view;
#[path = "../examples/views/input.rs"]
mod input;
#[path = "../examples/views/list_example.rs"]
mod list_example;
#[path = "../examples/views/mouse_pressure.rs"]
mod mouse_pressure;
#[path = "../examples/views/opacity.rs"]
mod opacity;
#[path = "../examples/views/painting.rs"]
mod painting;
#[path = "../examples/views/paths_bench.rs"]
mod paths_bench;
#[path = "../examples/views/pattern.rs"]
mod pattern;
#[path = "../examples/views/popover.rs"]
mod popover;
#[path = "../examples/views/scrollable.rs"]
mod scrollable;
#[path = "../examples/views/shadow.rs"]
mod shadow;
#[path = "../examples/views/svg.rs"]
mod svg_view;
#[path = "../examples/views/tab_stop.rs"]
mod tab_stop;
#[path = "../examples/views/text_layout.rs"]
mod text_layout;
#[path = "../examples/views/text.rs"]
mod text_view;
#[path = "../examples/views/text_wrapper.rs"]
mod text_wrapper;
#[path = "../examples/views/tree.rs"]
mod tree;
#[path = "../examples/views/uniform_list.rs"]
mod uniform_list;
#[path = "../examples/views/window_movable.rs"]
mod window_movable;
#[path = "../examples/views/window_shadow.rs"]
mod window_shadow;

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use vivid_protocol::overlay::modifiers;
use vivid_protocol::vector::{Brush, Command, Extend, HitRole};
use vlib::vui::keymap::usage_of;
use vlib::vui::prelude::*;
use vlib::vui::testing::TestUi;
use vlib::vui::{BoxShadow, GradientStopSpec, Pixels, Point, Track, div, text};

use a11y_view::Panel;
use active_state_bug::Toggle;
use anchor::Anchors;
use animation_view::Animated;
use data_table::Table;
use drag_drop::DragDrop;
use focus_visible::FocusRings;
use gif_viewer::Gif;
use gradient::Gradients;
use grid_layout::Page;
use image_gallery::Gallery;
use image_loading::Loading;
use image_view::Images;
use input::Editor;
use list_example::Log;
use mouse_pressure::Pressure;
use opacity::Fade;
use painting::Painting;
use paths_bench::PathsBench;
use pattern::Patterned;
use popover::Popover;
use scrollable::{ROW_HEIGHT, Scrollable};
use shadow::Shadows;
use svg_view::Svgs;
use tab_stop::TabStops;
use text_layout::TextLayout;
use text_view::Specimen;
use text_wrapper::TextWrapper;
use tree::DeepTree;
use uniform_list::Rows;
use window_movable::Movable;
use window_shadow::WindowFrame;

fn labels(canvas: &vivid_protocol::vector::Canvas) -> Vec<String> {
    canvas
        .commands()
        .iter()
        .filter_map(|command| match command {
            Command::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect()
}

fn opacity_of(canvas: &vivid_protocol::vector::Canvas) -> Option<u16> {
    canvas.commands().iter().find_map(|command| match command {
        Command::Opacity(value) => Some(*value),
        _ => None,
    })
}

fn fills(canvas: &vivid_protocol::vector::Canvas) -> usize {
    canvas
        .commands()
        .iter()
        .filter(|command| matches!(command, Command::Fill(..)))
        .count()
}

fn shadows(canvas: &vivid_protocol::vector::Canvas) -> Vec<vivid_protocol::vector::Shadow> {
    canvas
        .commands()
        .iter()
        .filter_map(|command| match command {
            Command::Shadow(shadow) => Some(*shadow),
            _ => None,
        })
        .collect()
}

#[test]
fn tree_nests_deeper_than_a_frame_should_but_still_fits() {
    let ui = TestUi::start(DeepTree { depth: 64 }, 500., 500.).unwrap();

    // Every level painted its text, so the whole chain laid out.
    let canvas = ui.canvas();
    let labels = canvas
        .commands()
        .iter()
        .filter(|command| matches!(command, Command::Text(text) if text.text.chars().all(|c| c.is_ascii_digit())))
        .count();
    assert_eq!(labels, 65, "one label per level plus the root");
    assert!(fills(&canvas) >= 65, "every level drew its background");

    // Depth costs commands, and the budget is generous but not unlimited.
    assert!(
        canvas.commands().len() < 4096,
        "a 64 level tree fits: {}",
        canvas.commands().len()
    );
}

#[test]
fn a_frame_too_complex_to_draw_says_so_instead_of_dropping_content() {
    // Each child is its own region and its own fill, so this is the command ceiling by
    // construction.
    let error = match TestUi::start(Bulk { children: 3000 }, 400., 400.) {
        Ok(_) => panic!("a frame past the command ceiling must not be accepted"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("too complex"),
        "the diagnosis names the budget: {error}"
    );
}

struct Bulk {
    children: usize,
}

impl Render for Bulk {
    fn render(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_col()
            .size(400., 400.)
            .children((0..self.children).map(|index| {
                div()
                    .id(format!("cell-{index}"))
                    .flex_none()
                    .size(400., 1.)
                    .bg(rgb(0x303040))
            }))
    }
}

#[test]
fn scrollable_moves_its_content_and_clamps_at_both_ends() {
    let mut ui = TestUi::start(Scrollable::default(), 300., 168.).unwrap();

    let viewport = ui.bounds("viewport").expect("the viewport was laid out");
    let (x, y) = (
        viewport.center().x.get() as f64,
        viewport.center().y.get() as f64,
    );
    // A wheel goes to what the pointer is over, so the pointer has to be there first.
    ui.move_pointer(x, y).unwrap();
    let first_row = ui.bounds("row-0").expect("the first row was laid out");
    assert_eq!(first_row.origin.y.get(), 0.);

    ui.wheel(x, y, 0., 3. * ROW_HEIGHT as f64).unwrap();
    assert_eq!(ui.read(|view| view.offset), 3. * ROW_HEIGHT);
    let scrolled = ui.bounds("row-0").expect("the first row is still laid out");
    assert!(
        scrolled.origin.y.get() < 0.,
        "the content moved up: {scrolled:?}"
    );

    // A row scrolled out of the box is not painted and has no region: the host could not reach
    // it, and a region it could reach would be a hit region for something invisible.
    assert!(ui.region("row-0").is_none());

    // The clamp holds at both ends.
    ui.wheel(x, y, 0., -10_000.).unwrap();
    assert_eq!(ui.read(|view| view.offset), 0.);
    ui.wheel(x, y, 0., 10_000.).unwrap();
    let maximum = ui.read(|view| view.maximum_offset());
    assert_eq!(ui.read(|view| view.offset), maximum);
    assert!(maximum > 0.);
}

#[test]
fn opacity_groups_a_subtree_and_follows_the_pointer() {
    let mut ui = TestUi::start(Fade::default(), 360., 260.).unwrap();
    assert_eq!(ui.read(|view| view.opacity), 1.);
    assert!(
        !opacity_of(&ui.canvas()).is_some(),
        "a fully opaque group needs no opacity command"
    );

    ui.move_pointer(90., 130.).unwrap();
    assert!(
        ui.read(|view| view.opacity) < 0.3,
        "the pointer set the opacity"
    );

    // The group is painted inside one save/opacity/restore, not faded element by element.
    let canvas = ui.canvas();
    let commands = canvas.commands();
    let save = commands
        .iter()
        .position(|command| matches!(command, Command::Save));
    let opacity = commands
        .iter()
        .position(|command| matches!(command, Command::Opacity(..)));
    let restore = commands
        .iter()
        .position(|command| matches!(command, Command::Restore));
    assert!(
        save < opacity && opacity < restore,
        "the group is bracketed: {save:?} {opacity:?} {restore:?}"
    );

    // And the value the host holds is the one the view has.
    let faded = opacity_of(&canvas).expect("the group is bracketed now");
    assert!(faded < 65535, "the hosted opacity follows the view");
}

#[test]
fn the_shadow_scale_reaches_the_host_with_its_parts_intact() {
    let ui = TestUi::start(Shadows::default(), 440., 420.).unwrap();
    let canvas = ui.canvas();
    let drawn = shadows(&canvas);
    assert_eq!(
        drawn.len(),
        shadow::TOKENS.len() + 3,
        "six tokens and three custom shadows"
    );

    // A token carries its blur and offset; the parts are what a host renders from.
    let xs = drawn[0];
    assert!(xs.blur.get() > 0.);
    assert_eq!(xs.offset.y.get(), 1.);
    assert!(!xs.inset);

    // The custom ones are the three forms the tokens do not cover.
    let custom = &drawn[shadow::TOKENS.len()..];
    assert_eq!(custom[0].offset.x.get(), 6.);
    assert_eq!(custom[0].blur.get(), 0.);
    assert_eq!(custom[1].spread.get(), 4.);
    assert!(custom[2].inset, "the inset shadow says so");
}

#[test]
fn gradients_reach_the_host_as_linear_brushes_at_their_angles() {
    let ui = TestUi::start(Gradients, 420., 300.).unwrap();
    let canvas = ui.canvas();
    let linear = canvas
        .commands()
        .iter()
        .filter_map(|command| match command {
            Command::Fill(
                _,
                Brush::Linear {
                    start, end, stops, ..
                },
            ) => Some((*start, *end, stops.len())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(linear.len(), 4, "one per panel");
    assert!(linear.iter().all(|(_, _, stops)| *stops >= 2));

    // The first panel is "to the right": its line runs across, not down.
    let (start, end, _) = linear[0];
    assert!(end.x.get() > start.x.get());
    assert!(
        (start.y.get() - end.y.get()).abs() < 0.001,
        "a horizontal gradient keeps one row: {start:?} {end:?}"
    );

    // The second runs downward.
    let (start, end, _) = linear[1];
    assert!(end.y.get() > start.y.get());
    assert!(
        (start.x.get() - end.x.get()).abs() < 0.001,
        "a vertical gradient keeps one column: {start:?} {end:?}"
    );
}

#[test]
fn grid_layout_places_cells_on_its_tracks() {
    let mut ui = TestUi::start(Page { width: 640. }, 640., 360.).unwrap();
    assert!(ui.read(|view| view.wide()));

    // Six cells were painted: two header, sidebar, main, two footer.
    let canvas = ui.canvas();
    assert!(fills(&canvas) >= 6, "every cell drew its background");

    // Narrowing the window changes which branch the view takes, and the frame follows.
    let wide = labels(&ui.canvas());
    ui.update(|view| view.width = 480.);
    ui.app_mut().notify(0);
    ui.paint().unwrap();
    assert!(!ui.read(|view| view.wide()));
    let narrow = labels(&ui.canvas());
    assert_ne!(
        wide, narrow,
        "the collapsed layout painted a different frame"
    );
    assert!(wide.contains(&"sidebar".to_owned()));
    assert!(narrow.contains(&"side".to_owned()));
}

#[test]
fn a_press_clears_when_the_pointer_leaves_before_the_release() {
    let mut ui = TestUi::start(Toggle::default(), 320., 160.).unwrap();

    let button = ui.bounds("press").expect("the button was laid out");
    let region = ui.region("press").expect("the button has a region");
    let center = (
        button.center().x.get() as f64,
        button.center().y.get() as f64,
    );

    // Press: the button takes the active style, and stays active while it is held.
    ui.presenter()
        .overlay_pointer(center.0, center.1, Some((0, true)), 0)
        .unwrap();
    ui.settle().unwrap();
    assert!(ui.pressed(region), "the press is active");

    // Move away and release there: the press must not survive it. This is the bug the GPUI
    // example is a reproduction of.
    ui.presenter()
        .overlay_pointer(8., 8., Some((0, false)), 0)
        .unwrap();
    ui.settle().unwrap();
    assert!(!ui.pressed(region), "a release elsewhere ends the press");
    assert_eq!(ui.read(|view| view.presses), 0, "and it is not a click");

    // Press and release on the button: that is a click.
    ui.click("press").unwrap();
    assert_eq!(ui.read(|view| view.presses), 1);
    assert!(!ui.pressed(region));
}

#[test]
fn pattern_tiles_an_uploaded_image_next_to_a_gradient() {
    let ui = TestUi::start(Patterned::default(), 400., 300.).unwrap();
    let canvas = ui.canvas();

    let tiled = canvas.commands().iter().find_map(|command| match command {
        Command::Fill(
            _,
            Brush::Image {
                asset,
                extend,
                transform,
            },
        ) => Some((*asset, *extend, *transform)),
        _ => None,
    });
    let (asset, extend, _) = tiled.expect("the tile filled its box as an image brush");
    assert_ne!(asset, 0, "the asset is the uploaded tile");
    assert_eq!(extend, Extend::Repeat, "a pattern is a repeated tile");

    assert!(
        canvas
            .commands()
            .iter()
            .any(|command| matches!(command, Command::Fill(_, Brush::Linear { .. }))),
        "the gradient panel is a linear fill"
    );
}

#[test]
fn an_element_without_an_id_cannot_receive_input() {
    // The rule the toolkit enforces by construction: a click needs a region, a region needs an
    // identity.
    let ui = TestUi::start(Anonymous, 200., 100.).unwrap();
    let canvas = ui.canvas();
    assert!(
        !canvas
            .commands()
            .iter()
            .any(|command| matches!(command, Command::Hit { .. })),
        "nothing anonymous asked for a region"
    );
}

struct Anonymous;

impl Render for Anonymous {
    fn render(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size(200., 100.)
            .bg(rgb(0x202030))
            .child(text("plain").color(rgb(0xffffff)))
    }
}

#[test]
fn two_elements_sharing_an_id_path_are_refused_rather_than_merged() {
    // Two children with the same id would share hover and press state, and the host could not
    // tell them apart, so the frame is refused where the mistake is.
    let error = match TestUi::start(Twins, 200., 100.) {
        Ok(_) => panic!("two elements sharing an id path must not be accepted"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("share the id path"),
        "diagnosis: {error}"
    );
}

struct Twins;

impl Render for Twins {
    fn render(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_col()
            .size(200., 100.)
            .child(div().id("twin").size(200., 20.).bg(rgb(0x303040)))
            .child(div().id("twin").size(200., 20.).bg(rgb(0x404050)))
    }
}

/// A view that counts how many times it rendered, for the frame-pacing tests.
pub struct Counter {
    pub renders: Rc<Cell<usize>>,
}

impl Render for Counter {
    fn render(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        div()
            .size(120., 60.)
            .bg(rgb(0x202030))
            .child(text("counted").color(rgb(0xffffff)))
    }
}

#[test]
fn an_idle_frame_is_not_rebuilt_and_an_unchanged_frame_is_not_resubmitted() {
    let renders = Rc::new(Cell::new(0));
    let mut ui = TestUi::start(
        Counter {
            renders: Rc::clone(&renders),
        },
        120.,
        60.,
    )
    .unwrap();
    let after_start = renders.get();
    assert!(after_start >= 1);

    // Nothing asked to repaint, so nothing was rendered again.
    for _ in 0..3 {
        ui.settle().unwrap();
    }
    assert_eq!(
        renders.get(),
        after_start,
        "an idle application does not re-render"
    );

    // A repaint that produces the same display list is not sent to the host again.
    let canvas = ui.canvas();
    ui.app_mut().notify(0);
    ui.app_mut().paint_once(0).unwrap();
    assert_eq!(renders.get(), after_start + 1, "a notified frame renders");
    assert_eq!(
        canvas,
        ui.canvas(),
        "and an identical frame is not resubmitted"
    );
}

#[test]
fn the_style_a_closure_restyles_is_the_same_style_an_element_declares() {
    // `.hover(|style| style.bg(..))` and `.bg(..)` are one vocabulary: a hover state is not a
    // second way to say the same property.
    let declared = Style::default()
        .bg(rgb(0x102030))
        .rounded(4.)
        .border(1., rgb(0xffffff));
    let restyled = Style::default()
        .bg(rgb(0x102030))
        .rounded(4.)
        .border(1., rgb(0xffffff));
    assert_eq!(declared, restyled);

    // Shadows stack rather than replace.
    let shadow = BoxShadow::new(Point::new(1., 2.), Pixels::new(3.), rgb(0x000000));
    let stacked = Style::default().shadow(shadow).shadow(shadow);
    assert_eq!(stacked.shadows.len(), 2);

    let _ = (
        GradientStopSpec::new(0., rgb(0xffffff)),
        Track::Fraction(1.),
    );
}

#[test]
fn a_paragraph_is_shaped_by_the_host_and_drawn_through_its_layout() {
    let ui = TestUi::start(TextLayout, 420., 460.).unwrap();
    let canvas = ui.canvas();
    let shaped = canvas
        .commands()
        .iter()
        .filter(|command| matches!(command, Command::TextLayout { .. }))
        .count();
    // Centered, end aligned, underlined, several runs, and struck through are all shapes; a
    // start-aligned run with a width is still plain text, because nothing about it needs one.
    assert_eq!(shaped, 5, "each paragraph that needs shaping was shaped");
    assert_eq!(ui.retained_layouts(), 5);

    // What the host shaped is the paragraph itself, styling included.
    let plain = canvas
        .commands()
        .iter()
        .filter(|command| matches!(command, Command::Text(..)))
        .count();
    assert_eq!(
        plain, 7,
        "six labels and the start-aligned sample are plain text"
    );
}

#[test]
fn a_text_run_that_is_plain_stays_plain() {
    let ui = TestUi::start(PlainText, 200., 60.).unwrap();
    let canvas = ui.canvas();
    assert!(
        canvas
            .commands()
            .iter()
            .any(|command| matches!(command, Command::Text(..))),
        "a plain run draws as text, without asking the host to retain anything"
    );
    assert_eq!(ui.retained_layouts(), 0);
}

struct PlainText;

impl Render for PlainText {
    fn render(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size(200., 60.)
            .bg(rgb(0x101018))
            .child(text("plain").color(rgb(0xffffff)))
    }
}

#[test]
fn a_paragraph_the_frame_stops_drawing_stops_being_retained() {
    let mut ui = TestUi::start(Swap { shaped: true }, 200., 120.).unwrap();
    assert_eq!(ui.retained_layouts(), 1);

    // The next frame drops the paragraph, so the host is told to let go of it once the scene
    // that drew it has been replaced.
    ui.update(|view| view.shaped = false);
    ui.app_mut().notify(0);
    ui.paint().unwrap();
    assert_eq!(ui.retained_layouts(), 0);
    assert!(
        ui.canvas()
            .commands()
            .iter()
            .any(|command| matches!(command, Command::Text(..)))
    );
}

struct Swap {
    shaped: bool,
}

impl Render for Swap {
    fn render(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        let line = if self.shaped {
            text("wrapped line that is certainly wider than the box")
                .size(13.)
                .max_width(120.)
                .wrap()
        } else {
            text("plain").size(13.)
        };
        div()
            .flex_col()
            .size(200., 120.)
            .bg(rgb(0x101018))
            .child(line)
    }
}

#[test]
fn wrapping_and_truncation_reach_the_host_as_paragraphs_with_widths() {
    let ui = TestUi::start(TextWrapper, 560., 320.).unwrap();
    let canvas = ui.canvas();
    assert_eq!(
        canvas
            .commands()
            .iter()
            .filter(|command| matches!(command, Command::TextLayout { .. }))
            .count(),
        3,
        "wrapped, one-line, and two-line samples are all shaped"
    );

    // A wrapped run is taller than one line, and the box it was measured into is the width it was
    // given rather than the width of its longest line.
    assert_eq!(ui.retained_layouts(), 3);
    let wrapped = ui
        .bounds("wrapped")
        .expect("the wrapped sample was laid out");
    assert!(
        (wrapped.width().get() - 150.).abs() < 1.,
        "the box is the wrap width: {wrapped:?}"
    );

    // The wrapped run is several lines tall, and the one clipped to a single line is one.
    let one_line = ui
        .bounds("ellipsized")
        .expect("the ellipsized sample was laid out");
    let clamped = ui
        .bounds("clamped")
        .expect("the clamped sample was laid out");
    assert!(
        wrapped.height().get() > one_line.height().get() * 2.,
        "wrapping made the paragraph taller: {wrapped:?} {one_line:?}"
    );
    assert!(
        clamped.height().get() > one_line.height().get(),
        "two lines are taller than one: {clamped:?} {one_line:?}"
    );
}

/// A floating window has no dismissal of its own, so the loop gives it one: escape always, and
/// `q` where nothing in the window takes typed text. The window asks for focus when it opens, so
/// neither waits for a click first.
#[test]
fn escape_or_q_ends_the_loop_and_a_field_keeps_q_for_typing() {
    let mut ui = TestUi::start(hello_world::Counter::default(), 420., 240.).unwrap();
    ui.focus_window().unwrap();
    let q = usage_of("q").unwrap();
    ui.key(q, true, modifiers::CONTROL).unwrap();
    assert!(!ui.quitting(), "a chord is somebody else's shortcut");
    ui.key(q, true, 0).unwrap();
    assert!(ui.quitting(), "q ends a window with no field");

    let mut ui = TestUi::start(Editor::default(), 420., 240.).unwrap();
    ui.focus_window().unwrap();
    ui.key(q, true, 0).unwrap();
    assert!(!ui.quitting(), "a window with a field spends q on typing");
    ui.key(usage_of("escape").unwrap(), true, 0).unwrap();
    assert!(ui.quitting(), "escape ends any window");
}

#[test]
fn typing_reaches_the_field_and_the_frame_follows() {
    let mut ui = TestUi::start(Editor::default(), 420., 240.).unwrap();
    ui.focus_window().unwrap();

    let field = ui.bounds("field").expect("the field was laid out");
    // The field's own element carries the focus handle, so that is the region that goes focused.
    let region = ui.region("editor").expect("the field's text has a region");
    assert!(!ui.state_at(region).focused, "the field starts unfocused");

    // Clicking it focuses the field, which is what makes keys land here. The click is also what
    // asks the host for focus: keys reach a window only once the user has pressed in it.
    ui.click("field").unwrap();
    assert!(ui.state_at(region).focused, "the click focused the field");

    for chunk in ["he", "llo"] {
        ui.text(chunk).unwrap();
    }
    assert_eq!(ui.read(|view| view.field.text().to_owned()), "hello");

    // A caret is drawn for the focused field, and its box is what the host places the input
    // method's candidate window from.
    let caret = ui.presenter().overlay_editor_caret();
    let (revision, caret) = caret.expect("the host was told where the caret is");
    assert!(revision > 0);
    let caret = caret.expect("a focused field has a caret");
    assert!(caret.height.get() > 0.);

    // The caret is at the end of the text, and the text starts at the field's left padding.
    assert!(
        caret.origin.x.get() as f32 > field.origin.x.get(),
        "the caret followed the text"
    );

    // Selection and clipboard: select everything, then copy it out.
    ui.key(usage_of("a").unwrap(), true, modifiers::SUPER)
        .unwrap();
    ui.key(usage_of("a").unwrap(), false, modifiers::SUPER)
        .unwrap();
    assert_eq!(ui.read(|view| view.field.selection()), Some((0, 5)));
    ui.key(usage_of("c").unwrap(), true, modifiers::SUPER)
        .unwrap();
    ui.key(usage_of("c").unwrap(), false, modifiers::SUPER)
        .unwrap();
    assert_eq!(ui.presenter().overlay_clipboard(), vec!["hello".to_owned()]);
    assert_eq!(ui.read(|view| view.copies), 1);

    // Cut removes it, and the field says so.
    ui.key(usage_of("x").unwrap(), true, modifiers::SUPER)
        .unwrap();
    ui.key(usage_of("x").unwrap(), false, modifiers::SUPER)
        .unwrap();
    assert_eq!(ui.read(|view| view.field.text().to_owned()), "");
}

#[test]
fn a_composition_is_drawn_where_it_is_being_typed_without_being_in_the_buffer() {
    let mut ui = TestUi::start(Editor::default(), 420., 240.).unwrap();
    ui.focus_window().unwrap();
    ui.click("field").unwrap();
    ui.text("ok ").unwrap();

    ui.presenter()
        .overlay_key(usage_of("space").unwrap(), true, 0)
        .unwrap();

    // The composition reaches the view rather than the buffer.
    ui.presenter().overlay_ime("にほん", None).unwrap();
    ui.settle().unwrap();
    assert_eq!(ui.read(|view| view.field.text().to_owned()), "ok ");
    assert_eq!(
        ui.read(|view| view.field.preedit().map(|p| p.text.clone())),
        Some("にほん".to_owned())
    );

    // It is drawn: the shaped paragraph carries the composition as an underlined run.
    let canvas = ui.canvas();
    assert!(
        canvas
            .commands()
            .iter()
            .any(|command| matches!(command, Command::TextLayout { .. })),
        "a composition is drawn as part of the paragraph"
    );

    // Committing it puts the text in the buffer and ends the composition.
    ui.text("日本").unwrap();
    assert_eq!(ui.read(|view| view.field.text().to_owned()), "ok 日本");
    assert!(ui.read(|view| view.field.preedit().is_none()));
}

#[test]
fn tab_moves_focus_between_fields_and_typing_follows_it() {
    let mut ui = TestUi::start(TabStops::default(), 360., 240.).unwrap();
    ui.focus_window().unwrap();
    assert_eq!(ui.read(|view| view.focused), 0);

    // A click in the window is what makes keys arrive at all.
    ui.click("row-0").unwrap();
    assert_eq!(ui.read(|view| view.focused), 0);

    // Tab is a bound action, so it moves focus rather than reaching the field.
    ui.key(usage_of("tab").unwrap(), true, 0).unwrap();
    ui.key(usage_of("tab").unwrap(), false, 0).unwrap();
    assert_eq!(
        ui.read(|view| view.focused),
        1,
        "tab moved to the second field"
    );

    // The second field is where typing goes now.
    ui.text("X").unwrap();
    assert_eq!(ui.read(|view| view.fields[1].text().to_owned()), "field 2X");
    assert_eq!(ui.read(|view| view.fields[0].text().to_owned()), "field 1");

    // Shift-tab goes back.
    ui.key(usage_of("tab").unwrap(), true, modifiers::SHIFT)
        .unwrap();
    ui.key(usage_of("tab").unwrap(), false, modifiers::SHIFT)
        .unwrap();
    assert_eq!(ui.read(|view| view.focused), 0);
    ui.text("Y").unwrap();
    assert_eq!(ui.read(|view| view.fields[0].text().to_owned()), "field 1Y");
}

#[test]
fn a_focus_ring_follows_the_keyboard_and_not_the_pointer() {
    let mut ui = TestUi::start(FocusRings::default(), 360., 220.).unwrap();
    ui.focus_window().unwrap();
    let second = ui
        .region("button-1")
        .expect("the second button has a region");

    // A click focuses it without claiming the keyboard put it there.
    ui.click("button-1").unwrap();
    assert!(ui.state_at(second).focused);
    assert!(!ui.state_at(second).focus_visible, "a click shows no ring");

    // Tab focuses the next one, and that one is marked as keyboard-reached.
    ui.key(usage_of("tab").unwrap(), true, 0).unwrap();
    ui.key(usage_of("tab").unwrap(), false, 0).unwrap();
    let third = ui
        .region("button-2")
        .expect("the third button has a region");
    assert!(ui.state_at(third).focused);
    assert!(
        ui.state_at(third).focus_visible,
        "the keyboard shows a ring"
    );
    assert!(
        !ui.state_at(second).focus_visible,
        "and the ring left the second"
    );
}

#[test]
fn a_popover_paints_over_the_button_that_opened_it_and_a_scrim_closes_it() {
    let mut ui = TestUi::start(Popover::default(), 420., 300.).unwrap();
    assert!(!ui.read(|view| view.open));

    ui.click("menu-button").unwrap();
    assert!(ui.read(|view| view.open));

    // The panel is deferred, so it is painted after the button: it is the last thing drawn.
    let canvas = ui.canvas();
    let last_text = canvas
        .commands()
        .iter()
        .rev()
        .find_map(|command| match command {
            Command::Text(text) => Some(text.text.clone()),
            _ => None,
        });
    assert_eq!(
        last_text.as_deref(),
        Some("Select all"),
        "the panel painted last"
    );

    // It is anchored under the button rather than laid out in the column.
    let button = ui.bounds("menu-button").expect("the button was laid out");
    let menu = ui.bounds("menu").expect("the panel was laid out");
    assert!(
        menu.origin.y.get() > button.origin.y.get(),
        "the panel hangs below the button"
    );

    // The scrim covers the window and swallows the click that closes the panel.
    ui.click("scrim").unwrap();
    assert!(
        !ui.read(|view| view.open),
        "clicking outside closed the panel"
    );
}

#[test]
fn anchored_panels_sit_against_the_edges_they_name() {
    let ui = TestUi::start(Anchors, 420., 260.).unwrap();
    let field = ui.bounds("field").expect("the parent was laid out");
    let canvas = ui.canvas();

    // Each panel is a fill the parent never draws: it is the anchored colour and nothing else
    // is. Where they are is what the anchoring decided, so the assertion is that they took up
    // the corners they named rather than stacking in the flow.
    let panels = canvas
        .commands()
        .iter()
        .filter(|command| {
            matches!(command, Command::Fill(_, Brush::Solid(color)) if color.0 == rgb(0x4a5fd0).0)
        })
        .count();
    assert_eq!(panels, 6, "one panel per corner");
    assert!(field.height().get() > 0.);

    // And the panels did not grow the box they are anchored to.
    assert!(
        field.height().get() <= 220. + 0.01,
        "a floating child does not make its parent taller: {field:?}"
    );
}

#[test]
fn a_drag_follows_the_pointer_and_drops_where_it_is_let_go() {
    let mut ui = TestUi::start(DragDrop::default(), 420., 260.).unwrap();
    ui.focus_window().unwrap();

    let tile = ui.bounds("tile-0").expect("the first tile was laid out");
    let tray = ui.bounds("tray").expect("the tray was laid out");
    let start = (tile.center().x.get() as f64, tile.center().y.get() as f64);

    // Press the tile: it starts being dragged, and it follows the pointer.
    ui.presenter()
        .overlay_pointer(start.0, start.1, Some((0, true)), 1)
        .unwrap();
    ui.settle().unwrap();
    assert!(
        ui.read(|view| view.dragging.is_some()),
        "the press started a drag"
    );

    let over_tray = (tray.center().x.get() as f64, tray.center().y.get() as f64);
    ui.move_pointer(over_tray.0, over_tray.1).unwrap();
    let followed = ui.read(|view| view.dragging.map(|(_, position)| position));
    assert!(followed.is_some_and(|position| (position.x.get() as f64 - over_tray.0).abs() < 2.));

    // Releasing over the tray drops it there, and the drag is over.
    ui.presenter()
        .overlay_pointer(over_tray.0, over_tray.1, Some((0, false)), 0)
        .unwrap();
    ui.settle().unwrap();
    assert_eq!(ui.read(|view| view.dropped.clone()), vec![0]);
    assert!(ui.read(|view| view.dragging.is_none()));

    // A second tile let go outside the tray goes back where it was.
    let second = ui.bounds("tile-1").expect("the second tile was laid out");
    ui.presenter()
        .overlay_pointer(
            second.center().x.get() as f64,
            second.center().y.get() as f64,
            Some((0, true)),
            1,
        )
        .unwrap();
    ui.settle().unwrap();
    ui.presenter()
        .overlay_pointer(4., 4., Some((0, false)), 0)
        .unwrap();
    ui.settle().unwrap();
    assert_eq!(
        ui.read(|view| view.dropped.clone()),
        vec![0],
        "a drop outside the tray is not a drop"
    );
}

#[test]
fn a_resize_edge_declares_the_direction_the_host_should_resize_in() {
    let ui = TestUi::start(WindowFrame, 360., 260.).unwrap();
    let canvas = ui.canvas();
    let edges: Vec<u8> = canvas
        .commands()
        .iter()
        .filter_map(|command| match command {
            Command::Hit {
                role: vivid_protocol::vector::HitRole::Resize(direction),
                ..
            } => Some(*direction),
            _ => None,
        })
        .collect();
    assert_eq!(edges.len(), 4, "four edges are resizable");
    // The role is a bitmask of the edges the host may move.
    assert!(
        edges.contains(&1) && edges.contains(&4),
        "left and right are among them"
    );
    assert!(
        edges.contains(&2) && edges.contains(&8),
        "and so are top and bottom"
    );
}

#[test]
fn pressure_widens_a_mark_and_hardware_without_it_still_draws() {
    assert_eq!(Pressure::width_for(0.), Pressure::BASE_WIDTH * 0.4);
    assert!(Pressure::width_for(1.) > Pressure::width_for(0.5));

    let mut ui = TestUi::start(Pressure::default(), 420., 280.).unwrap();
    let pad = ui.bounds("pad").expect("the pad was laid out");
    let (x, y) = (pad.center().x.get() as f64, pad.center().y.get() as f64);

    // A device that reports pressure: the mark is as wide as it was pressed.
    ui.presenter().overlay_pressure(x, y, 0.9).unwrap();
    ui.settle().unwrap();
    assert_eq!(ui.read(|view| view.points.len()), 1);
    assert!(ui.read(|view| view.saw_pressure));

    // One that reports none says nothing rather than drawing a guessed value.
    let without = Pressure::default();
    assert!(!without.saw_pressure);
    assert_eq!(without.points.len(), 0);
}

#[test]
fn a_ten_thousand_row_list_builds_only_the_rows_it_can_show() {
    let mut ui = TestUi::start(Rows::default(), 320., 260.).unwrap();

    // Ten thousand rows, and a frame that fits inside a budget built for a few thousand
    // commands. That is the whole reason a list exists here.
    let canvas = ui.canvas();
    assert!(
        canvas.commands().len() < 200,
        "a virtualized list is a handful of commands: {}",
        canvas.commands().len()
    );

    // The rows that exist are the ones on screen, and they say which they are.
    let visible = labels(&canvas);
    assert!(
        visible.iter().any(|label| label == "Row 0"),
        "the first row is there: {visible:?}"
    );
    assert!(
        !visible.iter().any(|label| label == "Row 500"),
        "and a far one is not"
    );

    // Scrolling changes which rows exist, not how many.
    let before = canvas.commands().len();
    let list = ui.bounds("list").expect("the list was laid out");
    ui.move_pointer(list.center().x.get() as f64, list.center().y.get() as f64)
        .unwrap();
    ui.wheel(
        list.center().x.get() as f64,
        list.center().y.get() as f64,
        0.,
        uniform_list::ROW_HEIGHT as f64 * 500.,
    )
    .unwrap();

    let scrolled = labels(&ui.canvas());
    assert!(
        scrolled.iter().any(|label| label == "Row 500"),
        "the far row exists now: {scrolled:?}"
    );
    assert!(
        !scrolled.iter().any(|label| label == "Row 0"),
        "and the first one no longer does"
    );
    assert!(
        (ui.canvas().commands().len() as i64 - before as i64).abs() < 20,
        "the frame is the same size wherever it is scrolled"
    );
}

#[test]
fn a_row_scrolled_out_of_a_list_is_neither_drawn_nor_reachable() {
    let mut ui = TestUi::start(Rows::default(), 320., 260.).unwrap();
    let list = ui.bounds("list").expect("the list was laid out");
    let (x, y) = (list.center().x.get() as f64, list.center().y.get() as f64);
    ui.move_pointer(x, y).unwrap();

    // Halfway down, the rows on screen are the ones around the offset.
    ui.wheel(x, y, 0., uniform_list::ROW_HEIGHT as f64 * 100.)
        .unwrap();
    assert_eq!(
        ui.read(|view| view.list.offset()),
        uniform_list::ROW_HEIGHT * 100.
    );
    let showing = labels(&ui.canvas());
    assert!(showing.iter().any(|label| label == "Row 100"));

    // And the clamp holds at the end: a list cannot be scrolled past its last row.
    ui.wheel(x, y, 0., 1_000_000.).unwrap();
    let offset = ui.read(|view| view.list.offset());
    let expected = ui.read(|view| view.list.maximum_offset(list.height().get()));
    assert!((offset - expected).abs() < 1., "{offset} vs {expected}");
    let last = labels(&ui.canvas());
    assert!(
        last.iter()
            .any(|label| label == &format!("Row {}", uniform_list::ROWS - 1)),
        "the last row is on screen at the end: {last:?}"
    );
}

#[test]
fn a_log_follows_its_tail_until_the_reader_scrolls_away() {
    let mut ui = TestUi::start(Log::default(), 420., 260.).unwrap();
    assert!(ui.read(|view| view.follow));
    let viewport = ui
        .bounds("log")
        .expect("the log was laid out")
        .height()
        .get();
    assert!(ui.read(|view| view.at_tail(viewport)));

    // A new entry keeps the tail in view.
    ui.update(|view| view.push("[9999] arrived", viewport));
    ui.app_mut().notify(0);
    ui.paint().unwrap();
    assert!(ui.texts().iter().any(|text| text.contains("9999")));

    // Scrolling back lets go of the tail, and a later entry does not drag the view down.
    let log = ui.bounds("log").expect("the log was laid out");
    let (x, y) = (log.center().x.get() as f64, log.center().y.get() as f64);
    ui.move_pointer(x, y).unwrap();
    ui.wheel(x, y, 0., -400.).unwrap();
    assert!(
        !ui.read(|view| view.follow),
        "scrolling back let go of the tail"
    );

    let offset = ui.read(|view| view.list.offset());
    ui.update(|view| view.push("[10000] arrived while scrolled back", viewport));
    ui.app_mut().notify(0);
    ui.paint().unwrap();
    assert_eq!(
        ui.read(|view| view.list.offset()),
        offset,
        "the view stayed where it was"
    );
}

#[test]
fn a_scrollbar_thumb_is_a_fraction_of_the_track_and_moves_with_the_content() {
    let mut ui = TestUi::start(Table::default(), 460., 300.).unwrap();
    let thumb = ui.bounds("thumb").expect("the thumb was laid out");
    let bar = ui.bounds("scrollbar").expect("the track was laid out");

    // Five thousand rows in a 220 pixel viewport: the thumb is a sliver, floored so it can be
    // seen at all.
    assert!(thumb.height().get() >= 16.);
    assert!(thumb.height().get() < bar.height().get() / 4.);
    assert!(
        (thumb.origin.y.get() - bar.origin.y.get()).abs() < 1.,
        "it starts at the top"
    );

    // Scrolling to the end puts the thumb at the end of the track.
    let table = ui.bounds("table").expect("the table was laid out");
    let (x, y) = (table.center().x.get() as f64, table.center().y.get() as f64);
    ui.move_pointer(x, y).unwrap();
    ui.wheel(x, y, 0., 1_000_000.).unwrap();
    let thumb = ui.bounds("thumb").expect("the thumb is still laid out");
    assert!(
        (thumb.bottom().get() - bar.bottom().get()).abs() < 2.,
        "the thumb ended at the track's end: {thumb:?} in {bar:?}"
    );
}

#[test]
fn a_table_keeps_its_header_still_while_its_rows_scroll() {
    let mut ui = TestUi::start(Table::default(), 460., 300.).unwrap();
    let header = ui.bounds("header").expect("the header was laid out");

    let first = ui.texts();
    assert!(
        first.iter().any(|label| label == "symbol"),
        "the header names its columns"
    );

    let table = ui.bounds("table").expect("the table was laid out");
    let (x, y) = (table.center().x.get() as f64, table.center().y.get() as f64);
    ui.move_pointer(x, y).unwrap();
    ui.wheel(x, y, 0., 2_000.).unwrap();

    assert_eq!(ui.bounds("header"), Some(header), "the header did not move");
    let scrolled = ui.texts();
    assert!(
        scrolled.iter().any(|label| label == "symbol"),
        "and it is still drawn"
    );
    assert_ne!(first, scrolled, "but the rows under it changed");
}

/// Every image the frame drew: the asset it used and the box it went in.
fn drawn_images(
    canvas: &vivid_protocol::vector::Canvas,
) -> Vec<(u64, vivid_protocol::vector::Rect)> {
    canvas
        .commands()
        .iter()
        .filter_map(|command| match command {
            Command::Image { asset, rect, .. } => Some((*asset, *rect)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_picture_is_decoded_here_and_reaches_the_host_as_an_upload() {
    let ui = TestUi::start(Images::default(), 460., 280.).unwrap();
    let drawn = drawn_images(&ui.canvas());
    assert_eq!(drawn.len(), 6, "six variants of the same picture");

    // One picture, one upload: the same bytes drawn six ways are uploaded once.
    assert_eq!(ui.retained_images(), 1, "the same source is uploaded once");
    let assets: std::collections::BTreeSet<u64> = drawn.iter().map(|(asset, _)| *asset).collect();
    assert_eq!(assets.len(), 1);
    assert!(!assets.contains(&0), "an asset identity is never zero");

    // The picture is 64 by 32, so at its natural size that is the box it takes.
    let natural = ui.bounds("natural").expect("laid out");
    assert_eq!(natural.width().get(), 64.);
    assert_eq!(natural.height().get(), 32.);
}

#[test]
fn how_a_picture_fits_its_box_is_the_difference_between_contain_cover_and_fill() {
    let ui = TestUi::start(Images::default(), 460., 280.).unwrap();
    let canvas = ui.canvas();
    let boxes: std::collections::HashMap<&str, vlib::vui::Bounds> = ["contain", "cover", "fill"]
        .iter()
        .map(|id| (*id, ui.bounds(id).expect("laid out")))
        .collect();

    // Every variant got the same box.
    assert_eq!(boxes["contain"].size, boxes["cover"].size);
    assert_eq!(boxes["contain"].size, boxes["fill"].size);

    // The picture is 2:1 in a 96 by 72 box.
    let drawn = drawn_images(&canvas);
    let placed = |box_: vlib::vui::Bounds| {
        drawn
            .iter()
            .map(|(_, rect)| *rect)
            .find(|rect| {
                rect.origin.x.get() as f32 >= box_.origin.x.get() - 30.
                    && rect.origin.x.get() as f32 <= box_.right().get() + 30.
                    && rect.origin.y.get() as f32 >= box_.origin.y.get() - 30.
                    && rect.origin.y.get() as f32 <= box_.bottom().get() + 30.
            })
            .expect("the picture went somewhere in its box")
    };

    // Contain fits inside: as wide as the box, half as tall as it could be.
    let contained = placed(boxes["contain"]);
    assert!((contained.width.get() - 96.).abs() < 0.5, "{contained:?}");
    assert!((contained.height.get() - 48.).abs() < 0.5, "{contained:?}");

    // Fill takes the box exactly, proportions be damned.
    let filled = placed(boxes["fill"]);
    assert!((filled.width.get() - 96.).abs() < 0.5);
    assert!((filled.height.get() - 72.).abs() < 0.5);

    // Cover overflows: wider than the box to cover its height.
    let covered = placed(boxes["cover"]);
    assert!(covered.width.get() > 96., "cover overflows: {covered:?}");
    assert!((covered.height.get() - 72.).abs() < 0.5);
}

#[test]
fn an_svg_is_rasterized_once_per_size_it_is_drawn_at() {
    let ui = TestUi::start(Svgs, 460., 280.).unwrap();
    let drawn = drawn_images(&ui.canvas());
    assert_eq!(
        drawn.len(),
        svg_view::SIZES.len() + 1,
        "four dragons and a badge"
    );

    // Each size is its own raster, because that is the whole point of keeping the vector: the
    // same document at 32 and at 160 is not one bitmap scaled twice.
    let assets: std::collections::BTreeSet<u64> = drawn.iter().map(|(asset, _)| *asset).collect();
    assert_eq!(
        assets.len(),
        svg_view::SIZES.len() + 1,
        "one upload per size: {assets:?}"
    );
    assert_eq!(ui.retained_images(), svg_view::SIZES.len() + 1);

    // And the boxes really are the sizes that were asked for.
    for size in svg_view::SIZES {
        let box_ = ui.bounds(&format!("dragon-{size}")).expect("laid out");
        assert_eq!(box_.width().get(), size);
        assert_eq!(box_.height().get(), size);
    }
}

#[test]
fn a_picture_that_cannot_be_decoded_is_not_drawn_blank() {
    // This used to fail the frame, on the reasoning that an SVG the application embedded is an
    // asset it got wrong and should hear about. That reasoning does not survive the toolkit
    // having exactly one kind of image: `svg(..)` and `img(..)` bytes look identical here, and
    // the second one routinely arrives over a network. So there is one rule for both — the box
    // stays, the picture does not, and the frame is counted rather than killed.
    let ui = TestUi::start(BrokenSvg, 200., 200.).unwrap();

    assert_eq!(ui.dropped_images(), 1, "the document was not drawn");
    assert!(
        ui.canvas()
            .commands()
            .iter()
            .all(|command| !matches!(command, Command::Image { .. })),
        "and nothing was put in its place"
    );

    // What it must not do is disappear silently: the element is still laid out, so the space it
    // occupies is still its own and a view can put something else there.
    let broken = ui.bounds("broken").expect("the box was laid out");
    assert_eq!(
        (broken.size.width.get(), broken.size.height.get()),
        (64., 64.)
    );
}

struct BrokenSvg;

impl Render for BrokenSvg {
    fn render(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size(200., 200.).child(
            vlib::vui::svg("broken", "<not-an-svg")
                .id("broken")
                .size(64., 64.),
        )
    }
}

#[test]
fn an_animation_advances_on_the_clock_and_asks_to_be_woken_for_the_next_frame() {
    let mut ui = TestUi::start(Gif::default(), 320., 260.).unwrap();
    let first = drawn_images(&ui.canvas());
    assert_eq!(first.len(), 1, "one picture");

    // It is playing, so it asked to be drawn again when the frame changes — and not sooner.
    let wake = ui
        .wake_in()
        .expect("a playing animation asks for the next frame");
    assert!(
        wake <= Duration::from_millis(u64::from(gif_viewer::FRAME_MS)),
        "the wait is the frame's, not a fixed tick: {wake:?}"
    );

    // Moving the clock past a frame boundary shows the next frame, which is a different upload.
    ui.advance(Duration::from_millis(u64::from(gif_viewer::FRAME_MS) + 10))
        .unwrap();
    let second = drawn_images(&ui.canvas());
    assert_ne!(first[0].0, second[0].0, "the frame advanced");

    // And it loops: a whole cycle later it is back where it started.
    let cycle = u64::from(gif_viewer::FRAME_MS) * gif_viewer::FRAMES as usize as u64;
    ui.advance(Duration::from_millis(cycle)).unwrap();
    let looped = drawn_images(&ui.canvas());
    assert_eq!(
        second[0].0, looped[0].0,
        "a whole cycle later, the same frame"
    );

    // Every frame it has shown is held, and no more than the file has.
    assert!(ui.retained_images() <= gif_viewer::FRAMES as usize);
}

#[test]
fn a_held_animation_shows_its_first_frame_and_lets_the_loop_sleep() {
    let mut ui = TestUi::start(Gif::default(), 320., 260.).unwrap();
    ui.focus_window().unwrap();
    // A press is what asks the host for focus, and keys reach a window only once it has.
    ui.click("surface").unwrap();
    let playing = drawn_images(&ui.canvas())[0].0;

    // Space holds it.
    ui.key(usage_of("space").unwrap(), true, 0).unwrap();
    ui.key(usage_of("space").unwrap(), false, 0).unwrap();
    assert!(!ui.read(|view| view.playing));

    // A held animation asks for nothing, so the loop has no reason to wake.
    assert_eq!(
        ui.wake_in(),
        None,
        "nothing is moving, so nothing is scheduled"
    );

    // And time passing does not move it.
    let held = drawn_images(&ui.canvas())[0].0;
    ui.advance(Duration::from_millis(1000)).unwrap();
    assert_eq!(
        drawn_images(&ui.canvas())[0].0,
        held,
        "a held animation does not advance"
    );
    assert_eq!(held, playing, "and it is holding the first frame");
}

#[test]
fn a_gallery_reuses_its_uploads_and_never_holds_more_than_the_host_allows() {
    let mut ui = TestUi::start(Gallery::default(), 400., 260.).unwrap();
    let first = ui.retained_images();
    assert!(
        first > 0 && first <= image_gallery::TILES,
        "only what is on screen: {first}"
    );

    // Scrolling draws different pictures. The ones now on screen are uploaded; the budget is the
    // host's, and it is never exceeded.
    let list = ui.bounds(image_gallery::LIST_ID).expect("laid out");
    let (x, y) = (list.center().x.get() as f64, list.center().y.get() as f64);
    ui.move_pointer(x, y).unwrap();
    for _ in 0..4 {
        ui.wheel(x, y, 0., image_gallery::ROW_HEIGHT as f64 * 3.)
            .unwrap();
    }
    assert!(
        ui.retained_images() <= image_gallery::TILES,
        "the cache is bounded by the pictures that exist"
    );

    // Scrolling back to the top draws the first pictures again without decoding them twice:
    // what is held is an upload, and an upload that is still held is reused.
    ui.wheel(x, y, 0., -100_000.).unwrap();
    let back = drawn_images(&ui.canvas());
    assert!(!back.is_empty(), "the first row is drawn again");
    assert!(back.iter().all(|(asset, _)| *asset != 0));
}

#[test]
fn an_animation_moves_on_the_clock_and_asks_for_the_frames_it_needs() {
    let mut ui = TestUi::start(Animated::default(), 360., 320.).unwrap();

    // At the start everything is at the beginning of its track.
    let dot = |ui: &TestUi<Animated>, label: &str| {
        let track = ui
            .bounds(&format!("track-{label}"))
            .expect("the track was laid out");
        (track, ui.canvas())
    };
    let (track, _) = dot(&ui, "linear");
    assert!(track.width().get() > 0.);

    // It is moving, so it asked to be drawn again — at the display's pace, not as fast as it can.
    let wake = ui.wake_in().expect("something moving asks for a frame");
    assert!(
        wake <= Duration::from_millis(20),
        "paced to the display: {wake:?}"
    );
    assert!(
        wake >= vlib::vui::animation::FASTEST_FRAME || wake.is_zero(),
        "{wake:?}"
    );

    // Half way through the sweep, the easings have moved apart: that is what makes them
    // different curves rather than different names for one.
    ui.advance(animation_view::SWEEP / 2).unwrap();
    let positions: Vec<f32> = animation_view::EASINGS
        .iter()
        .map(|(_, easing)| easing.apply(0.5))
        .collect();
    assert!((positions[0] - 0.5).abs() < 0.001, "linear is half way");
    assert!(positions[1] < positions[0], "ease in is behind");
    assert!(positions[2] > positions[0], "ease out is ahead");

    // And the frame really did change, rather than the model moving while the picture did not.
    let before = ui.canvas();
    ui.advance(animation_view::SWEEP / 4).unwrap();
    assert_ne!(before, ui.canvas(), "the frame followed the clock");
}

#[test]
fn a_spring_settles_and_then_the_window_stops_asking_to_be_drawn() {
    let mut ui = TestUi::start(Animated::default(), 360., 320.).unwrap();

    // Send the spring somewhere.
    ui.click("spring-track").unwrap();
    let target = ui.read(|view| view.spring.target);
    assert!(target > 0., "the click chose a target: {target}");

    // It travels: a few frames in, it is on its way but not there.
    ui.advance(Duration::from_millis(50)).unwrap();
    let moving = ui.read(|view| view.spring.value);
    assert!(
        moving > 0. && (moving - target).abs() > 0.001,
        "under way: {moving}"
    );
    assert!(ui.read(|view| !view.spring.at_rest()));

    // Given enough frames it settles exactly on its target. A bouncy spring rings for a while,
    // and "settled" here means settled, not nearly.
    for _ in 0..200 {
        ui.advance(Duration::from_millis(16)).unwrap();
    }
    assert!(ui.read(|view| view.spring.at_rest()), "it settled");
    assert_eq!(ui.read(|view| view.spring.value), target);
}

#[test]
fn a_host_that_asked_for_less_movement_is_given_the_destination_instead() {
    let presenter_environment = |reduced: bool| vivid_sdk::overlay::Environment {
        reduced_motion: Some(reduced),
        ..Default::default()
    };

    let mut ui = TestUi::start(Animated::default(), 360., 320.).unwrap();
    ui.presenter()
        .set_overlay_environment(presenter_environment(true));
    ui.settle().unwrap();
    assert!(ui.read(|_| true));

    // The sweep is at its end rather than at its beginning, and nothing is scheduled: there is
    // no movement left to draw.
    ui.app_mut().notify(0);
    ui.paint().unwrap();
    assert_eq!(
        ui.wake_in(),
        None,
        "nothing was scheduled: {:?}",
        ui.wake_in()
    );

    // A spring sent somewhere is simply there.
    ui.click("spring-track").unwrap();
    let target = ui.read(|view| view.spring.target);
    assert_eq!(
        ui.read(|view| view.spring.value),
        target,
        "it arrived at once"
    );
    assert_eq!(ui.wake_in(), None);
}

#[test]
fn a_window_with_nothing_moving_asks_for_no_frames_at_all() {
    // Every other example: no animation, so no wake-up. A toolkit that scheduled frames anyway
    // would keep a terminal busy for a still picture.
    let ui = TestUi::start(Toggle::default(), 320., 160.).unwrap();
    assert_eq!(ui.wake_in(), None);
}

#[test]
fn a_panel_describes_itself_to_the_host_for_the_scene_on_screen() {
    let ui = TestUi::start(Panel::default(), 360., 340.).unwrap();
    let semantics = ui.semantics().expect("the panel described itself");

    // The description names the scene it belongs to, which is the one presented.
    assert!(semantics.scene_revision > 0);
    semantics
        .validate()
        .expect("what was published is a tree the protocol accepts");
    assert_eq!(ui.dropped_semantics(), 0, "nothing had to be left out");

    // The panel is the root, and everything hangs from it.
    assert_eq!(semantics.nodes[0].role, SemanticRole::Application);
    assert_eq!(semantics.nodes[0].label, "Settings");

    // Each control says what kind of thing it is, not what it looks like.
    let heading = ui.semantic_node("Settings").expect("a heading");
    let switch = ui.semantic_node("Notifications").expect("a switch");
    let volume = ui.semantic_node("Volume").expect("a spin button");
    let apply = ui.semantic_node("Apply settings").expect("a button");
    assert_eq!(switch.role, SemanticRole::Switch);
    assert_eq!(volume.role, SemanticRole::SpinButton);
    assert_eq!(apply.role, SemanticRole::Button);
    let _ = heading;

    // A state, a value with its range, and a place in a set.
    assert_eq!(switch.toggled, Some(Toggled::On));
    let numeric = volume.numeric.expect("a spin button has a value");
    assert_eq!(numeric[0].get(), 40.);
    assert_eq!((numeric[1].get(), numeric[2].get()), (0., 100.));
    let second = ui.semantic_node(a11y_view::ITEMS[1]).expect("a list item");
    assert_eq!(second.set, Some([2, a11y_view::ITEMS.len() as u16]));

    // And each says what can be asked of it, from the protocol's closed set.
    assert!(volume.actions.contains(&AccessibleAction::Increment));
    assert!(volume.actions.contains(&AccessibleAction::Decrement));
    assert!(apply.actions.contains(&AccessibleAction::Default));
}

#[test]
fn the_described_structure_is_the_meaningful_one_not_the_layout() {
    let ui = TestUi::start(Panel::default(), 360., 340.).unwrap();
    let semantics = ui.semantics().expect("described");

    // The panel's boxes, spacers and rows are not in the tree: only the things worth announcing.
    let described: Vec<&str> = semantics
        .nodes
        .iter()
        .map(|node| node.label.as_str())
        .collect();
    assert!(
        semantics.nodes.len() <= 9,
        "one root, a heading, a switch, a spin button, a list and its items, a button: {described:?}"
    );

    // The list's items are its children, whatever number of boxes sit between them in the
    // layout.
    let list_index = semantics
        .nodes
        .iter()
        .position(|node| node.role == SemanticRole::List)
        .expect("a list");
    let list = &semantics.nodes[list_index];
    assert_eq!(list.children.len(), a11y_view::ITEMS.len());
    for child in &list.children {
        assert_eq!(
            semantics.nodes[*child as usize].role,
            SemanticRole::ListItem
        );
        assert!(
            *child as usize > list_index,
            "a child always follows its parent"
        );
    }
}

#[test]
fn what_assistive_technology_asks_for_reaches_the_element_that_offered_it() {
    let mut ui = TestUi::start(Panel::default(), 360., 340.).unwrap();
    assert_eq!(ui.read(|view| view.volume), 40);

    // Increment and decrement are what a spin button offers, and they arrive as themselves.
    assert!(
        ui.accessibility_action("Volume", AccessibleAction::Increment)
            .unwrap()
    );
    assert_eq!(ui.read(|view| view.volume), 40 + a11y_view::VOLUME_STEP);
    assert!(
        ui.accessibility_action("Volume", AccessibleAction::Decrement)
            .unwrap()
    );
    assert!(
        ui.accessibility_action("Volume", AccessibleAction::Decrement)
            .unwrap()
    );
    assert_eq!(ui.read(|view| view.volume), 40 - a11y_view::VOLUME_STEP);

    // And the description that follows says the new value, so what is announced is what is true.
    let volume = ui.semantic_node("Volume").expect("still described");
    assert_eq!(
        volume.numeric.expect("a value")[0].get(),
        f64::from(40 - a11y_view::VOLUME_STEP)
    );

    // A switch is toggled by being activated, and says so afterwards.
    assert_eq!(
        ui.semantic_node("Notifications").unwrap().toggled,
        Some(Toggled::On)
    );
    assert!(
        ui.accessibility_action("Notifications", AccessibleAction::Default)
            .unwrap()
    );
    assert!(!ui.read(|view| view.notifications));
    assert_eq!(
        ui.semantic_node("Notifications").unwrap().toggled,
        Some(Toggled::Off)
    );

    // A button does what a click would.
    assert!(
        ui.accessibility_action("Apply settings", AccessibleAction::Click)
            .unwrap()
    );
    assert_eq!(ui.read(|view| view.applied), 1);
}

#[test]
fn a_description_follows_the_scene_it_describes() {
    let mut ui = TestUi::start(Panel::default(), 360., 340.).unwrap();
    let first = ui.semantics().expect("described");

    // Choosing a different section is a new scene, and it carries a new description naming it.
    ui.click("item-2").unwrap();
    assert_eq!(ui.read(|view| view.selected), 2);
    let second = ui.semantics().expect("described again");
    assert!(
        second.scene_revision > first.scene_revision,
        "the description names the newer scene: {} then {}",
        first.scene_revision,
        second.scene_revision
    );
    second
        .validate()
        .expect("still a tree the protocol accepts");

    // The host refuses a description of a scene it is not showing, which is the rule that stops
    // a screen reader announcing a control that has gone.
    assert_eq!(
        second.nodes.len(),
        first.nodes.len(),
        "the same panel, described again rather than accumulated"
    );
}

#[test]
fn an_element_that_describes_nothing_is_not_announced() {
    // Every other example in this file: no roles, so no tree at all. A toolkit that guessed
    // would announce a rounded box with a word in it as a button, and be wrong about half of
    // them.
    let ui = TestUi::start(Toggle::default(), 320., 160.).unwrap();
    assert_eq!(ui.semantics(), None);
}

#[test]
fn painting_draws_freeform_paths_the_toolkit_has_no_command_for() {
    let ui = TestUi::start(Painting::default(), 472., 300.).unwrap();
    let canvas = ui.canvas();

    // Four figures, one command each: a polygon, a cubic, a ring and a dashed line. The four
    // canvases' own backgrounds are fills too, so each figure is counted by the ink it uses.
    let figure_inks = [rgb(0xe0b050), rgb(0x8ecbff), rgb(0x70d090), rgb(0xff8ea0)];
    let shapes = canvas
        .commands()
        .iter()
        .filter(|command| match command {
            Command::Fill(_, Brush::Solid(color)) => figure_inks.contains(color),
            Command::StyledStroke(_, Brush::Solid(color), _) => figure_inks.contains(color),
            _ => false,
        })
        .count();
    assert_eq!(shapes, 4, "one command per figure");

    // The ring is even-odd, which is what makes its inner circle a hole rather than a second
    // disc — and what makes the hole unclickable, since the host hit tests the rule it fills by.
    let even_odd = canvas.commands().iter().find_map(|command| match command {
        Command::Fill(path, _) if path.even_odd => Some(path),
        _ => None,
    });
    let ring = even_odd.expect("the ring is filled by the even-odd rule");
    assert_eq!(
        ring.segments
            .iter()
            .filter(|segment| matches!(segment, vivid_protocol::vector::Segment::Move(_)))
            .count(),
        2,
        "two subpaths: the disc and the hole"
    );

    // The dashes are the style's, not the path's, so the path is one straight line.
    let dashed = canvas.commands().iter().find_map(|command| match command {
        Command::StyledStroke(path, _, style) if !style.dashes.is_empty() => {
            Some((path.segments.len(), style.dashes.len()))
        }
        _ => None,
    });
    assert_eq!(
        dashed,
        Some((2, 2)),
        "a move and a line, dashed into two runs of ink"
    );
}

#[test]
fn painting_draws_the_strokes_the_pointer_made() {
    let mut ui = TestUi::start(Painting::default(), 472., 300.).unwrap();
    let pad = ui.bounds("freehand").expect("the pad was laid out");
    let ink = rgb(0xffd070);

    // Nothing has been drawn on the pad, so the only strokes in the frame are the four figures.
    let drawn_with_ink = |ui: &TestUi<Painting>| {
        ui.canvas()
            .commands()
            .iter()
            .filter_map(|command| match command {
                Command::StyledStroke(path, Brush::Solid(color), _) if *color == ink => {
                    Some(path.clone())
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert!(drawn_with_ink(&ui).is_empty());

    // A press, two moves and a release, all in window coordinates.
    let start = (pad.origin.x.0 as f64 + 40., pad.origin.y.0 as f64 + 40.);
    let middle = (start.0 + 30., start.1 + 10.);
    let end = (start.0 + 60., start.1 + 20.);
    ui.presenter()
        .overlay_pointer(start.0, start.1, Some((0, true)), 1)
        .unwrap();
    ui.settle().unwrap();
    assert_eq!(
        ui.read(|view| view.stroke_count()),
        1,
        "the press began one"
    );

    ui.move_pointer(middle.0, middle.1).unwrap();
    ui.move_pointer(end.0, end.1).unwrap();
    ui.presenter()
        .overlay_pointer(end.0, end.1, Some((0, false)), 0)
        .unwrap();
    ui.settle().unwrap();

    // One stroke, drawn through exactly the points the host reported: the drawing is in the same
    // window coordinates the events arrive in, so there is nothing between them to get wrong.
    let strokes = drawn_with_ink(&ui);
    assert_eq!(strokes.len(), 1, "one stroke on the pad");
    let points: Vec<(f64, f64)> = strokes[0]
        .segments
        .iter()
        .filter_map(|segment| match segment {
            vivid_protocol::vector::Segment::Move(point)
            | vivid_protocol::vector::Segment::Line(point) => Some((point.x.get(), point.y.get())),
            _ => None,
        })
        .collect();
    assert_eq!(points, vec![start, middle, end]);

    // A second press starts a second stroke rather than extending the first, which is what
    // "one stroke per press" means.
    ui.presenter()
        .overlay_pointer(start.0, start.1 + 30., Some((0, true)), 2)
        .unwrap();
    ui.settle().unwrap();
    ui.move_pointer(start.0 + 20., start.1 + 40.).unwrap();
    ui.presenter()
        .overlay_pointer(start.0 + 20., start.1 + 40., Some((0, false)), 0)
        .unwrap();
    ui.settle().unwrap();
    assert_eq!(ui.read(|view| view.stroke_count()), 2);
}

#[test]
fn paths_bench_fills_the_frame_it_names() {
    let ui = TestUi::start(PathsBench::default(), 640., 420.).unwrap();
    let canvas = ui.canvas();

    // Every star is its own command, and the count is the one the window prints.
    let star_ink = rgb(0xd0a860);
    let stars = canvas
        .commands()
        .iter()
        .filter(|command| matches!(command, Command::Fill(_, Brush::Solid(color)) if *color == star_ink))
        .count();
    assert_eq!(stars, paths_bench::DEFAULT_STARS);

    // All of them fit, with the numbers the view claims: the segments are what the view said
    // they were, and the frame is nowhere near the command or segment ceilings.
    assert!(stars < 4096, "{stars} commands is the command ceiling");
    let segments: usize = canvas
        .commands()
        .iter()
        .filter_map(|command| match command {
            Command::Fill(_, Brush::Solid(color)) if *color == star_ink => match command {
                Command::Fill(path, _) => Some(path.segments.len()),
                _ => None,
            },
            _ => None,
        })
        .sum();
    assert_eq!(
        segments,
        paths_bench::DEFAULT_STARS * paths_bench::SEGMENTS_PER_STAR
    );
    assert!(segments < 65536, "{segments} segments is the scene budget");
}

#[test]
fn a_click_halves_the_bench_and_the_frame_shrinks_with_it() {
    let mut ui = TestUi::start(PathsBench::default(), 640., 420.).unwrap();
    let star_ink = rgb(0xd0a860);
    let stars_now = |ui: &TestUi<PathsBench>| {
        ui.canvas()
            .commands()
            .iter()
            .filter(|command| matches!(command, Command::Fill(_, Brush::Solid(color)) if *color == star_ink))
            .count()
    };
    let full = stars_now(&ui);

    ui.click("bench").unwrap();
    assert_eq!(ui.read(|view| view.stars), paths_bench::STEPS[1]);
    let halved = stars_now(&ui);
    assert_eq!(halved, full / 2, "the frame is half the work");
}

#[test]
fn a_specimen_draws_its_typography_rather_than_dropping_it() {
    let ui = TestUi::start(Specimen, 520., 400.).unwrap();

    // Five of these are the specimen's own text; the retained layouts are the paragraphs whose
    // typography the plain command cannot carry. Without them the letter spacing, the word
    // spacing and the line height would all be silently discarded.
    assert!(
        ui.retained_layouts() > 0,
        "a paragraph with spacing or a line height is shaped"
    );

    // The plain runs are still drawn plainly, which is the cheaper path and the reason the
    // distinction is worth keeping.
    let labels = labels(&ui.canvas());
    for expected in ["size", "weight and slant", "family", "spacing and leading"] {
        assert!(
            labels.iter().any(|label| label == expected),
            "the specimen drew its {expected} label"
        );
    }
}

#[test]
fn an_undecodable_picture_leaves_its_box_and_the_rest_of_the_frame() {
    let ui = TestUi::start(Loading::default(), 440., 240.).unwrap();

    // The bytes that are not a picture did not take the frame down, and the other picture in the
    // same row is unaffected.
    assert_eq!(ui.dropped_images(), 1, "exactly one picture did not decode");
    let pictures = ui
        .canvas()
        .commands()
        .iter()
        .filter(|command| matches!(command, Command::Image { .. }))
        .count();
    assert_eq!(pictures, 1, "the one that decoded was drawn");

    // And the box it would have been drawn in is still on screen, which is what tells the user
    // there is a picture there and it is not loading.
    let broken = ui.bounds("broken").expect("the box was laid out");
    assert!(broken.size.height.get() > 0.);
}

#[test]
fn a_picture_that_has_not_arrived_is_not_a_picture_that_failed() {
    let mut ui = TestUi::start(Loading::default(), 440., 240.).unwrap();

    // Nothing has arrived yet, and nothing has failed either: the slow picture has its box and
    // a word in it, and the view asked for the frame that will replace them. The one picture that
    // did not decode is the third slot, which is meant not to — waiting is not failing, so the
    // count does not move while the first one is on its way.
    assert!(!ui.read(|view| view.has_arrived()));
    assert_eq!(ui.dropped_images(), 1, "the undecodable one, and only it");
    assert_eq!(ui.region("arrived"), None, "no picture to point at yet");
    assert!(ui.wake_in().is_some(), "the window asked to be woken");

    // The clock is the window's, so a test can arrive without waiting.
    ui.advance(Duration::from_millis(600)).unwrap();
    assert!(ui.read(|view| view.has_arrived()));
    assert!(ui.region("arrived").is_some(), "the picture is there now");
    assert_eq!(
        ui.dropped_images(),
        1,
        "and still only the one that is not a picture"
    );
}

#[test]
fn a_title_bar_reaches_the_host_as_a_drag_region() {
    let ui = TestUi::start(Movable::default(), 384., 264.).unwrap();
    let region = ui.region("title-bar").expect("the bar has a region");

    // The role is what the host reads: a drag here moves the window, and the view is not in the
    // loop at all while it happens.
    let drag = ui.canvas().commands().iter().any(
        |command| matches!(command, Command::Hit { id, role: HitRole::Drag, .. } if *id == region),
    );
    assert!(drag, "the bar is a drag handle");

    // The four edges are still resize handles, and each names its own direction.
    let resize: Vec<u8> = ui
        .canvas()
        .commands()
        .iter()
        .filter_map(|command| match command {
            Command::Hit {
                role: HitRole::Resize(direction),
                ..
            } => Some(*direction),
            _ => None,
        })
        .collect();
    assert_eq!(resize, vec![1, 4, 2, 8], "left, right, up, down");
}
