//! The view behind `image`: one picture, drawn four ways.
//!
//! Decoding happens here rather than in the terminal — the protocol carries raw pixels, so a
//! producer that wants a PNG on screen is the one that turns it into pixels. What reaches the
//! host is an upload and a rectangle.
//!
//! Remote images are not here: fetching one is a network request, which is the application's
//! business and not the toolkit's. Hand `ImageSource::bytes` whatever the fetch returned.

use vlib::vui::prelude::*;
use vlib::vui::{ImageSource, div, img, text};

pub struct Images {
    pub picture: ImageSource,
}

impl Default for Images {
    fn default() -> Self {
        Self {
            // A checkerboard, because how a picture is fitted into a box is obvious in one and
            // invisible in a photograph.
            picture: ImageSource::bytes("checkerboard", checkerboard(64, 32, 8)),
        }
    }
}

/// A checkerboard PNG, generated so the example carries its own picture instead of a blob.
/// It goes through a real encoder, and the toolkit through a real decoder.
fn checkerboard(width: u32, height: u32, square: u32) -> Vec<u8> {
    let picture = image::RgbaImage::from_fn(width, height, |x, y| {
        if ((x / square.max(1)) + (y / square.max(1))).is_multiple_of(2) {
            image::Rgba([0x20, 0x20, 0x30, 0xff])
        } else {
            image::Rgba([0x70, 0x80, 0xd0, 0xff])
        }
    });
    let mut bytes = std::io::Cursor::new(Vec::new());
    picture
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("a generated picture encodes");
    bytes.into_inner()
}

fn labelled(label: &str, content: impl IntoElement) -> impl IntoElement {
    div()
        .flex_col()
        .gap(4.)
        .child(text(label).size(11.).color(rgb(0x8080a0)))
        .child(content)
}

impl Render for Images {
    fn render(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        // The box every fitted variant is given, so the difference between them is the fit.
        let slot = |content: vlib::vui::ImgEl| {
            div()
                .size(96., 72.)
                .bg(rgb(0x1a1a24))
                .rounded(4.)
                .child(content.w_full().h_full())
        };
        div()
            .flex_col()
            .gap(12.)
            .p(20.)
            .size(460., 280.)
            .bg(rgb(0x101018))
            .child(
                div()
                    .flex()
                    .gap(12.)
                    .items_end()
                    .child(labelled(
                        "natural size",
                        img(self.picture.clone()).id("natural"),
                    ))
                    .child(labelled(
                        "contain",
                        slot(img(self.picture.clone()).id("contain").contain()),
                    ))
                    .child(labelled(
                        "cover",
                        slot(img(self.picture.clone()).id("cover").cover().clipped()),
                    )),
            )
            .child(
                div()
                    .flex()
                    .gap(12.)
                    .items_end()
                    .child(labelled(
                        "fill",
                        slot(img(self.picture.clone()).id("fill").fill()),
                    ))
                    .child(labelled(
                        "half opacity",
                        slot(img(self.picture.clone()).id("faded").contain().opacity(0.5)),
                    ))
                    .child(labelled(
                        "rounded",
                        slot(img(self.picture.clone()).id("rounded").cover().rounded(12.)),
                    )),
            )
    }
}
