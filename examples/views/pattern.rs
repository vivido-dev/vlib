//! The view behind `pattern`: a tiled background, next to a gradient.
//!
//! GPUI fills this with `pattern_slash`, a built-in hatch. There is no hatch primitive in the
//! protocol, so the tile here is an image the producer uploads once and repeats: the same
//! appearance, through the path a sprite or a texture atlas would use.

use vivid_sdk::overlay::RetainedImage;
use vlib::vui::prelude::*;
use vlib::vui::{GradientStopSpec, div, text};

#[derive(Default)]
pub struct Patterned {
    tile: Option<RetainedImage>,
}

impl Patterned {
    /// An 8 by 8 diagonal hatch, as RGBA8.
    pub fn tile_pixels() -> Vec<u8> {
        const SIZE: usize = 8;
        let mut pixels = Vec::with_capacity(SIZE * SIZE * 4);
        for y in 0..SIZE {
            for x in 0..SIZE {
                // Every other diagonal is lighter, which reads as a slash pattern.
                let lit = (x + y) % 4 < 2;
                let (r, g, b, a) = if lit {
                    (0x50u8, 0x60u8, 0xe0u8, 0xffu8)
                } else {
                    (0x1cu8, 0x1cu8, 0x2cu8, 0xffu8)
                };
                pixels.extend_from_slice(&[r, g, b, a]);
            }
        }
        pixels
    }
}

impl Render for Patterned {
    fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        if self.tile.is_none()
            && let Ok(tile) = cx.upload_rgba(8, 8, &Self::tile_pixels())
        {
            self.tile = Some(tile);
        }
        let mut root = div()
            .flex_col()
            .gap(12.)
            .p(20.)
            .size(400., 300.)
            .bg(rgb(0x101018));
        if let Some(tile) = &self.tile {
            root = root.child(div().size(360., 120.).rounded(6.).bg_image(tile.id()));
        }
        root.child(
            div()
                .size(360., 120.)
                .rounded(6.)
                .bg_gradient(
                    135.,
                    vec![
                        GradientStopSpec::new(0., rgb(0x203050)),
                        GradientStopSpec::new(1., rgb(0x50a0d0)),
                    ],
                )
                .child(text("gradient").size(13.).color(rgb(0xffffff))),
        )
    }
}
