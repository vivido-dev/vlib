//! The view behind `text`: a type specimen.
//!
//! The plain text command carries a size, a weight, a slant, a family and the two decorations.
//! Everything else typography is — letter and word spacing, a line height, the font's own
//! features — rides `overlay-typography-v1`, and a paragraph that asks for any of them is shaped
//! through a retained layout rather than drawn as plain text.
//!
//! That distinction is invisible on screen and matters a great deal underneath: a run drawn
//! plainly is one command; a run shaped is a layout the host holds until it is replaced, drawn
//! from a fixed budget. This window shows both, and the paragraph at the bottom is the one that
//! has to be shaped.

use vlib::vui::prelude::*;
use vlib::vui::{div, text};

/// The families the specimen names. Empty means whatever the host chooses, which is what a
/// terminal-friendly default is.
pub const FAMILIES: [&str; 3] = ["", "Menlo", "Georgia"];

/// The sizes the ramp climbs.
pub const RAMP: [f32; 5] = [11., 14., 18., 24., 32.];

impl Render for Specimen {
    fn render(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        let mut column = div().flex_col().gap(10.);

        // The size ramp: one line per size, which is what a font scale is.
        let mut ramp = div().flex().items_end().gap(14.);
        for size in RAMP {
            ramp = ramp.child(text(format!("{size:.0}")).size(size).color(rgb(0xd8d8e8)));
        }
        column = column.child(section("size", ramp));

        // Weight and slant, which the plain text command already carried.
        column = column.child(section(
            "weight and slant",
            div()
                .flex()
                .items_end()
                .gap(14.)
                .child(text("regular").size(16.).color(rgb(0xd8d8e8)))
                .child(text("bold").size(16.).weight(700).color(rgb(0xd8d8e8)))
                .child(text("italic").size(16.).italic().color(rgb(0xd8d8e8)))
                .child(text("under").size(16.).underline().color(rgb(0xd8d8e8)))
                .child(
                    text("struck")
                        .size(16.)
                        .strikethrough()
                        .color(rgb(0xd8d8e8)),
                ),
        ));

        // Families. A host without the named family substitutes its own, which is why the
        // specimen names the family rather than promising the glyphs.
        let mut families = div().flex().items_end().gap(14.);
        for family in FAMILIES {
            families = families.child(
                text(if family.is_empty() { "default" } else { family })
                    .size(16.)
                    .family(family)
                    .color(rgb(0xd8d8e8)),
            );
        }
        column = column.child(section("family", families));

        // The typography the plain command cannot carry. Each of these is shaped.
        let tracked = text("loose tracking, wide word spacing")
            .size(15.)
            .letter_spacing(2.5)
            .word_spacing(6.)
            .color(rgb(0x9ecbff));
        // A line height is a *measurement* as well as a drawing instruction, so this paragraph
        // occupies the taller box the layout was told about, not only the taller box on screen.
        let leading = text("one line\nset on a taller rhythm\nthan the font would choose")
            .size(14.)
            .line_height(28.)
            .color(rgb(0x9ecbff));
        let spacing = div().flex_col().gap(6.).child(tracked).child(leading);
        column = column.child(section("spacing and leading", spacing));

        div()
            .flex_col()
            .gap(12.)
            .p(20.)
            .size(520., 400.)
            .bg(rgb(0x12121a))
            .child(column)
    }
}

/// A labelled row, so each part of the specimen says what it is showing.
fn section(label: &str, content: impl IntoElement) -> impl IntoElement {
    div()
        .flex_col()
        .gap(4.)
        .child(text(label).size(10.).color(rgb(0x606078)))
        .child(content)
}

/// The view behind `text`.
#[derive(Default)]
pub struct Specimen;
