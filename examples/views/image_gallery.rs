//! The view behind `image_gallery`: a grid of pictures, and a cache with a ceiling.
//!
//! The host holds a bounded number of uploads. A gallery is the case where that matters: scroll
//! far enough and the pictures on screen are not the pictures that were uploaded first, so the
//! cache lets the oldest ones go. Nothing here does that by hand — drawing a picture is what
//! keeps it, and not drawing it is what eventually releases it.

use vlib::vui::prelude::*;
use vlib::vui::{ImageSource, div, img, text, uniform_list};

pub const TILES: usize = 60;
pub const COLUMNS: usize = 4;
pub const ROW_HEIGHT: f32 = 86.;
pub const VIEWPORT: f32 = 200.;
pub const LIST_ID: &str = "gallery";

pub struct Gallery {
    /// Every picture, made once. A gallery that re-encoded its pictures each frame would be
    /// measuring the encoder rather than the cache.
    pub tiles: Vec<ImageSource>,
    pub list: ListState,
}

impl Default for Gallery {
    fn default() -> Self {
        let tiles = (0..TILES)
            .map(|index| {
                let tint = (
                    (60 + index * 3) as u8,
                    (120 + index * 5) as u8,
                    (200 - index * 2) as u8,
                );
                ImageSource::bytes(format!("tile-{index}"), gradient(48, 48, tint))
            })
            .collect();
        let rows = TILES.div_ceil(COLUMNS);
        Self {
            tiles,
            list: ListState::new(rows, ROW_HEIGHT),
        }
    }
}

/// A small gradient PNG, generated so the gallery carries its own pictures.
fn gradient(width: u32, height: u32, tint: (u8, u8, u8)) -> Vec<u8> {
    let picture = image::RgbaImage::from_fn(width, height, |x, y| {
        let across = (x * 255 / width.max(1)) as u16;
        let down = (y * 255 / height.max(1)) as u16;
        image::Rgba([
            ((across * u16::from(tint.0)) / 255) as u8,
            ((down * u16::from(tint.1)) / 255) as u8,
            ((across + down) / 2 * u16::from(tint.2) / 255) as u8,
            0xff,
        ])
    });
    let mut bytes = std::io::Cursor::new(Vec::new());
    picture
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("a generated picture encodes");
    bytes.into_inner()
}

impl Render for Gallery {
    fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let tiles = self.tiles.clone();
        div()
            .flex_col()
            .gap(8.)
            .p(16.)
            .size(400., 260.)
            .bg(rgb(0x101018))
            .child(
                text(format!("{TILES} pictures, a row at a time"))
                    .size(12.)
                    .color(rgb(0x8080a0)),
            )
            .child(
                uniform_list(&self.list, move |rows| {
                    rows.map(|row| {
                        let first = row * COLUMNS;
                        div()
                            .flex()
                            .gap(8.)
                            .items_center()
                            .children((first..(first + COLUMNS).min(TILES)).map(|index| {
                                img(tiles[index].clone())
                                    .id(format!("tile-{index}"))
                                    .size(72., 72.)
                                    .contain()
                                    .rounded(6.)
                            }))
                            .into_node()
                    })
                    .collect()
                })
                .id(LIST_ID)
                .w_full()
                .h(VIEWPORT)
                .on_wheel(cx.listener(|state: &mut Self, event, cx| {
                    if let UiEvent::Wheel { delta, .. } = event {
                        let viewport = cx
                            .bounds_of(LIST_ID)
                            .map_or(VIEWPORT, |box_| box_.height().get());
                        state.list.scroll_by(delta.y.get(), viewport);
                        cx.notify();
                    }
                })),
            )
    }
}
