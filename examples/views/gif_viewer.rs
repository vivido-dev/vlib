//! The view behind `gif_viewer`: an animation playing at its own frame rate.
//!
//! Nothing in the view drives the animation. The frame it is showing is a function of how long
//! the window has been up, and the toolkit asks the loop to wake it when the next frame is due —
//! so a still window sleeps, and a playing one wakes exactly as often as the file asks for and no
//! more.

use vlib::vui::prelude::*;
use vlib::vui::{ImageSource, div, img, text};

actions!(gif_example, [TogglePlay]);

pub struct Gif {
    pub animation: ImageSource,
    pub playing: bool,
}

impl Default for Gif {
    fn default() -> Self {
        Self {
            animation: ImageSource::bytes("walker", animated_gif(96)),
            playing: true,
        }
    }
}

/// How many frames the generated animation has, and how long each one lasts.
pub const FRAMES: u32 = 8;
pub const FRAME_MS: u32 = 80;

/// An animated GIF: a dot walking around a ring, one frame per step. Generated so the example
/// carries its own animation, through a real encoder and back through a real decoder.
fn animated_gif(size: u32) -> Vec<u8> {
    use image::codecs::gif::{GifEncoder, Repeat};

    let mut bytes = Vec::new();
    {
        let mut encoder = GifEncoder::new(std::io::Cursor::new(&mut bytes));
        encoder
            .set_repeat(Repeat::Infinite)
            .expect("a generated animation loops");
        for step in 0..FRAMES {
            let angle = step as f32 / FRAMES as f32 * std::f32::consts::TAU;
            let centre = size as f32 / 2.;
            let radius = size as f32 * 0.3;
            let (cx, cy) = (centre + angle.cos() * radius, centre + angle.sin() * radius);
            let buffer = image::RgbaImage::from_fn(size, size, |x, y| {
                let near =
                    ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt() < size as f32 * 0.16;
                if near {
                    image::Rgba([0x8e, 0xcb, 0xff, 0xff])
                } else {
                    image::Rgba([0x14, 0x14, 0x1c, 0xff])
                }
            });
            encoder
                .encode_frame(image::Frame::from_parts(
                    buffer,
                    0,
                    0,
                    image::Delay::from_numer_denom_ms(FRAME_MS, 1),
                ))
                .expect("a generated frame encodes");
        }
    }
    bytes
}

impl Render for Gif {
    fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        cx.bind_keys([KeyBinding::parse("space", ActionId::of::<TogglePlay>()).unwrap()]);
        let picture = img(self.animation.clone()).id("gif").size(160., 160.);
        let picture = if self.playing {
            picture
        } else {
            picture.paused()
        };
        div()
            .id("surface")
            .flex_col()
            .gap(12.)
            .p(20.)
            .size(320., 260.)
            .bg(rgb(0x101018))
            .on_action::<TogglePlay>(cx.listener(|state: &mut Self, _event, cx| {
                state.playing = !state.playing;
                cx.notify();
            }))
            .child(
                text(if self.playing {
                    "playing — space to hold"
                } else {
                    "held — space to play"
                })
                .size(12.)
                .color(rgb(0x8080a0)),
            )
            .child(picture)
            .child(
                text(format!("{} frames at {}ms", FRAMES, FRAME_MS))
                    .size(11.)
                    .color(rgb(0x606080)),
            )
    }
}
