//! The view behind `image_loading`: a picture that arrives, and one that never will.
//!
//! Pixels come from somewhere else — a fetch, a file, a side channel — so the bytes are not
//! always what they were promised to be. A picture that cannot be decoded leaves the box it
//! would have been drawn in, styled and empty, and the frame keeps the rest of its content.
//!
//! That is the whole distinction: a *missing* picture is ordinary and degrades, while a frame
//! too complex to draw is an error and says so. This window has both kinds in it.

use std::time::Duration;

use vlib::vui::prelude::*;
use vlib::vui::{ImageSource, div, img, text};

/// A small picture, generated here so the example carries it rather than a blob on disk.
///
/// It goes through a real PNG encoder and comes back through a real decoder, which is the path
/// a picture from anywhere else takes.
pub fn checkerboard(width: u32, height: u32, square: u32) -> Vec<u8> {
    let picture = image::RgbaImage::from_fn(width, height, |x, y| {
        if ((x / square.max(1)) + (y / square.max(1))).is_multiple_of(2) {
            image::Rgba([0x30, 0x38, 0x60, 0xff])
        } else {
            image::Rgba([0x80, 0x90, 0xe0, 0xff])
        }
    });
    let mut bytes = std::io::Cursor::new(Vec::new());
    picture
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("a generated picture encodes");
    bytes.into_inner()
}

/// The bytes that are not a picture, which is what a truncated download or an error page is.
pub fn not_a_picture() -> Vec<u8> {
    b"<html><body>404 Not Found</body></html>".to_vec()
}

/// The view behind `image_loading`.
pub struct Loading {
    /// The picture that arrives.
    pub good: ImageSource,
    /// The bytes that are not one.
    pub bad: ImageSource,
    /// How long the good one takes to "arrive", so the wait is visible rather than instant.
    pub arriving: Duration,
    /// Time since the window opened.
    pub elapsed: Duration,
}

impl Default for Loading {
    fn default() -> Self {
        Self {
            good: ImageSource::bytes("checkerboard", checkerboard(96, 96, 12)),
            bad: ImageSource::bytes("not-a-picture", not_a_picture()),
            arriving: Duration::from_millis(600),
            elapsed: Duration::ZERO,
        }
    }
}

impl Loading {
    /// Whether the slow picture has arrived yet.
    pub fn has_arrived(&self) -> bool {
        self.elapsed >= self.arriving
    }
}

impl Render for Loading {
    fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        // The clock is the window's, so a test can stop it and watch the wait without waiting.
        self.elapsed = cx.elapsed();

        // A picture that has not arrived is not an error — it is a box with nothing in it yet.
        let arriving = slot(
            div()
                .flex()
                .items_center()
                .justify_center()
                .size(120., 96.)
                .when(
                    self.has_arrived(),
                    img(self.good.clone()).id("arrived").w_full().h_full(),
                )
                .when(
                    !self.has_arrived(),
                    text("loading…").size(11.).color(rgb(0x606078)),
                ),
        );
        if !self.has_arrived() {
            // The window is asked for the frame that will show it, at the moment it arrives.
            // There is no timer anywhere: a wait is a deadline on the window's own clock.
            cx.request_frame_after(self.arriving - self.elapsed);
        }

        div()
            .flex_col()
            .gap(12.)
            .p(20.)
            .size(440., 240.)
            .bg(rgb(0x101018))
            .child(
                div()
                    .flex()
                    .gap(16.)
                    .child(labelled("arriving", arriving))
                    // The bytes here are an error page. The box is drawn, the picture is not,
                    // and the two pictures beside it are unaffected.
                    .child(labelled(
                        "not a picture",
                        slot(img(self.bad.clone()).id("broken").w_full().h_full()),
                    ))
                    .child(labelled(
                        "undamaged",
                        slot(img(self.good.clone()).id("intact").w_full().h_full()),
                    )),
            )
            .child(
                text("a picture that cannot be decoded leaves its box behind")
                    .size(11.)
                    .color(rgb(0x606078)),
            )
    }
}

/// A box the size a picture is drawn at, whether or not a picture arrives.
fn slot(content: impl IntoElement) -> impl IntoElement {
    div()
        .size(120., 96.)
        .bg(rgb(0x1a1a24))
        .rounded(6.)
        .border(1., rgb(0x303048))
        .child(content)
}

fn labelled(label: &str, content: impl IntoElement) -> impl IntoElement {
    div()
        .flex_col()
        .gap(4.)
        .child(text(label).size(10.).color(rgb(0x606078)))
        .child(content)
}
