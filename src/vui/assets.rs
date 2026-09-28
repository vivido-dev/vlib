//! Images: decoding them here, and keeping what the host holds bounded.
//!
//! The protocol carries raw RGBA and nothing else, and that is deliberate — a PNG on the wire
//! would be a PNG decoder in the terminal, running on bytes a producer chose. So decoding is the
//! producer's job: PNG, JPEG, GIF and WebP through the `image` crate, SVG rasterized through
//! `resvg` at the size it will actually be drawn.
//!
//! What the host keeps is an upload, and uploads are bounded: 256 of them, 64 MiB in total. This
//! module is the cache in front of that, evicting whatever was drawn least recently.

use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use vivid_protocol::vector::{MAX_RETAINED_ASSET_BYTES, MAX_RETAINED_ASSETS};
use vivid_sdk::overlay::{OverlayWindow, RetainedImage};

/// The most pixels one decoded image may have, before it is anything the host sees.
///
/// A decoder is being handed bytes; a header claiming a 60000 by 60000 image would otherwise ask
/// for fourteen gigabytes before anyone could refuse it.
pub const MAX_PIXELS: u64 = 64 << 20;

/// One decoded frame: premultiplied-agnostic RGBA8, and how long it is shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<Vec<u8>>,
    /// How long this frame is shown. Zero for a still image.
    pub delay: Duration,
}

impl Frame {
    pub fn bytes(&self) -> usize {
        self.rgba.len()
    }
}

/// A decoded image: one frame, or several with their delays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    pub frames: Vec<Frame>,
}

impl Decoded {
    pub fn is_animated(&self) -> bool {
        self.frames.len() > 1
    }

    pub fn width(&self) -> u32 {
        self.frames.first().map_or(0, |frame| frame.width)
    }

    pub fn height(&self) -> u32 {
        self.frames.first().map_or(0, |frame| frame.height)
    }

    /// How long one loop takes. Zero when nothing animates.
    pub fn duration(&self) -> Duration {
        self.frames.iter().map(|frame| frame.delay).sum()
    }

    /// Which frame is showing `elapsed` into the loop.
    ///
    /// A frame with no delay would otherwise stall the loop on it forever, so a zero-length loop
    /// simply shows its first frame.
    pub fn frame_at(&self, elapsed: Duration) -> usize {
        let total = self.duration();
        if self.frames.len() < 2 || total.is_zero() {
            return 0;
        }
        let mut remaining = Duration::from_nanos((elapsed.as_nanos() % total.as_nanos()) as u64);
        for (index, frame) in self.frames.iter().enumerate() {
            if remaining < frame.delay {
                return index;
            }
            remaining -= frame.delay;
        }
        self.frames.len() - 1
    }

    /// How long until the frame after the one showing at `elapsed`.
    pub fn until_next_frame(&self, elapsed: Duration) -> Option<Duration> {
        let total = self.duration();
        if self.frames.len() < 2 || total.is_zero() {
            return None;
        }
        let mut remaining = Duration::from_nanos((elapsed.as_nanos() % total.as_nanos()) as u64);
        for frame in &self.frames {
            if remaining < frame.delay {
                return Some(frame.delay - remaining);
            }
            remaining -= frame.delay;
        }
        self.frames.first().map(|frame| frame.delay)
    }
}

/// Decode encoded bytes into frames.
///
/// A GIF becomes its frames with their delays; everything else becomes one.
pub fn decode(bytes: &[u8]) -> io::Result<Decoded> {
    let format = image::guess_format(bytes).map_err(invalid)?;
    if format == image::ImageFormat::Gif {
        return decode_animation(bytes);
    }
    let decoded = image::load_from_memory_with_format(bytes, format).map_err(invalid)?;
    let (width, height) = (decoded.width(), decoded.height());
    refuse_enormous(width, height)?;
    Ok(Decoded {
        frames: vec![Frame {
            width,
            height,
            rgba: Arc::new(decoded.to_rgba8().into_raw()),
            delay: Duration::ZERO,
        }],
    })
}

/// Decode an animation, frame by frame with its delays.
fn decode_animation(bytes: &[u8]) -> io::Result<Decoded> {
    use image::AnimationDecoder;

    let decoder =
        image::codecs::gif::GifDecoder::new(std::io::Cursor::new(bytes)).map_err(invalid)?;
    let mut frames = Vec::new();
    let mut pixels: u64 = 0;
    for frame in decoder.into_frames() {
        let frame = frame.map_err(invalid)?;
        let (numerator, denominator) = frame.delay().numer_denom_ms();
        let delay = if denominator == 0 {
            Duration::ZERO
        } else {
            Duration::from_micros(u64::from(numerator) * 1000 / u64::from(denominator))
        };
        let buffer = frame.into_buffer();
        let (width, height) = (buffer.width(), buffer.height());
        refuse_enormous(width, height)?;
        // Every frame counts against the ceiling: a thousand small frames is a large image.
        pixels = pixels.saturating_add(u64::from(width) * u64::from(height));
        if pixels > MAX_PIXELS {
            return Err(invalid_message(
                "animation exceeds the decoded pixel ceiling",
            ));
        }
        frames.push(Frame {
            width,
            height,
            rgba: Arc::new(buffer.into_raw()),
            // A GIF that asks for no delay at all is played at the speed browsers settled on
            // rather than as fast as the machine can go.
            delay: if delay.is_zero() {
                Duration::from_millis(100)
            } else {
                delay
            },
        });
    }
    if frames.is_empty() {
        return Err(invalid_message("animation has no frames"));
    }
    Ok(Decoded { frames })
}

/// Rasterize an SVG at a pixel size.
///
/// The size is the box it will be drawn in, so the result is sharp there rather than a scaled
/// bitmap: that is the whole reason a vector source is worth keeping as one.
pub fn rasterize_svg(bytes: &[u8], width: u32, height: u32) -> io::Result<Decoded> {
    refuse_enormous(width, height)?;
    if width == 0 || height == 0 {
        return Err(invalid_message(
            "an SVG cannot be rasterized into no pixels",
        ));
    }
    let tree = usvg::Tree::from_data(bytes, &usvg::Options::default()).map_err(invalid)?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| invalid_message("SVG raster size rejected"))?;

    // Scale the document to fit the box, keeping its proportions and centring what is left over.
    let size = tree.size();
    let scale = (width as f32 / size.width()).min(height as f32 / size.height());
    let transform = resvg::tiny_skia::Transform::from_translate(
        (width as f32 - size.width() * scale) / 2.,
        (height as f32 - size.height() * scale) / 2.,
    )
    .pre_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    Ok(Decoded {
        frames: vec![Frame {
            width,
            height,
            // tiny-skia hands back premultiplied RGBA, which is what the protocol's raster
            // alpha mode expects.
            rgba: Arc::new(pixmap.take()),
            delay: Duration::ZERO,
        }],
    })
}

fn refuse_enormous(width: u32, height: u32) -> io::Result<()> {
    let pixels = u64::from(width).saturating_mul(u64::from(height));
    if pixels > MAX_PIXELS {
        return Err(invalid_message("image exceeds the decoded pixel ceiling"));
    }
    Ok(())
}

fn invalid(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("image rejected: {error}"),
    )
}

fn invalid_message(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_owned())
}

/// What an image element draws, and how it is named.
///
/// The key is the caller's: a cache needs an identity, and the bytes are the wrong one — hashing
/// a megabyte every frame to discover it has not changed is worse than being told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageSource {
    pub key: String,
    pub data: SourceData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceData {
    /// Encoded bytes: PNG, JPEG, GIF, or WebP.
    Encoded(Arc<Vec<u8>>),
    /// An SVG document, rasterized at the size it is drawn.
    Svg(Arc<Vec<u8>>),
    /// Pixels somebody already has.
    Rgba {
        width: u32,
        height: u32,
        pixels: Arc<Vec<u8>>,
    },
}

impl ImageSource {
    /// Encoded bytes under a name of the caller's choosing.
    pub fn bytes(key: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            key: key.into(),
            data: SourceData::Encoded(Arc::new(bytes.into())),
        }
    }

    /// An SVG document under a name of the caller's choosing.
    pub fn svg(key: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            key: key.into(),
            data: SourceData::Svg(Arc::new(bytes.into())),
        }
    }

    /// Pixels somebody already has.
    pub fn rgba(
        key: impl Into<String>,
        width: u32,
        height: u32,
        pixels: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            key: key.into(),
            data: SourceData::Rgba {
                width,
                height,
                pixels: Arc::new(pixels.into()),
            },
        }
    }

    /// A file on disk, named by its path.
    ///
    /// The file is read now, so a path that cannot be read is an error here rather than a blank
    /// space in a frame.
    pub fn path(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path)?;
        let key = path.to_string_lossy().into_owned();
        let svg = path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"));
        Ok(if svg {
            Self::svg(key, bytes)
        } else {
            Self::bytes(key, bytes)
        })
    }

    /// Whether this source must be rasterized for the box it is drawn in, rather than decoded
    /// once at its own size.
    fn is_vector(&self) -> bool {
        matches!(self.data, SourceData::Svg(_))
    }
}

/// How an image fills the box it is given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ObjectFit {
    /// Keep the proportions, fitting inside the box. What a photo in a slot wants.
    #[default]
    Contain,
    /// Keep the proportions, covering the box and overflowing it.
    Cover,
    /// Stretch to the box, proportions be damned.
    Fill,
}

impl ObjectFit {
    /// Where an image of `natural` size is drawn inside `bounds`.
    pub fn place(
        self,
        natural: (f32, f32),
        bounds: crate::vui::geometry::Bounds,
    ) -> crate::vui::geometry::Bounds {
        use crate::vui::geometry::{Bounds, Point, Size};
        let (width, height) = (bounds.size.width.0, bounds.size.height.0);
        if natural.0 <= 0. || natural.1 <= 0. || width <= 0. || height <= 0. {
            return bounds;
        }
        let scale = match self {
            Self::Fill => return bounds,
            Self::Contain => (width / natural.0).min(height / natural.1),
            Self::Cover => (width / natural.0).max(height / natural.1),
        };
        let drawn = Size::new(natural.0 * scale, natural.1 * scale);
        Bounds::new(
            Point::new(
                bounds.origin.x.0 + (width - drawn.width.0) / 2.,
                bounds.origin.y.0 + (height - drawn.height.0) / 2.,
            ),
            drawn,
        )
    }
}

/// What identifies a decoded picture: the source's name, and — for a vector — the size it was
/// rasterized at, since the same document at two sizes is two bitmaps.
type DecodedKey = (String, Option<(u32, u32)>);

/// What identifies an upload: a decoded picture, and which of its frames.
type UploadKey = (String, Option<(u32, u32)>, usize);

/// An upload the host is holding, and what it cost.
struct Held {
    image: RetainedImage,
    bytes: usize,
    /// The frame this was last drawn in, for eviction.
    used: u64,
}

/// The cache in front of the host's uploads.
#[derive(Default)]
pub(crate) struct Assets {
    /// Decoded pixels, by source key and — for a vector — the size it was rasterized at.
    decoded: HashMap<DecodedKey, Decoded>,
    /// Uploads, by the key and frame index they hold.
    uploaded: HashMap<UploadKey, Held>,
    bytes: usize,
    /// Rises once per painted frame, which is what "least recently drawn" is measured in.
    clock: u64,
}

impl Assets {
    /// A new frame is being painted.
    pub(crate) fn begin_frame(&mut self) {
        self.clock = self.clock.wrapping_add(1);
    }

    /// The decoded form of a source, decoding it if this is the first sight of it.
    ///
    /// `raster` is the size a vector source should be rasterized at; it is ignored for everything
    /// else, whose size is its own.
    pub(crate) fn decoded(
        &mut self,
        source: &ImageSource,
        raster: (u32, u32),
    ) -> io::Result<&Decoded> {
        let size = source.is_vector().then_some(raster);
        let key = (source.key.clone(), size);
        if !self.decoded.contains_key(&key) {
            let decoded = match &source.data {
                SourceData::Encoded(bytes) => decode(bytes)?,
                SourceData::Svg(bytes) => rasterize_svg(bytes, raster.0, raster.1)?,
                SourceData::Rgba {
                    width,
                    height,
                    pixels,
                } => {
                    refuse_enormous(*width, *height)?;
                    let wanted = u64::from(*width) * u64::from(*height) * 4;
                    if pixels.len() as u64 != wanted {
                        return Err(invalid_message(
                            "pixel buffer does not match the size it claims",
                        ));
                    }
                    Decoded {
                        frames: vec![Frame {
                            width: *width,
                            height: *height,
                            rgba: Arc::clone(pixels),
                            delay: Duration::ZERO,
                        }],
                    }
                }
            };
            self.decoded.insert(key.clone(), decoded);
        }
        Ok(&self.decoded[&key])
    }

    /// The identity a scene draws one frame of a source through, uploading it if the host is not
    /// already holding it.
    pub(crate) fn upload(
        &mut self,
        overlay: &OverlayWindow,
        source: &ImageSource,
        raster: (u32, u32),
        frame: usize,
    ) -> io::Result<u64> {
        let size = source.is_vector().then_some(raster);
        let key = (source.key.clone(), size, frame);
        if let Some(held) = self.uploaded.get_mut(&key) {
            held.used = self.clock;
            return Ok(held.image.id());
        }

        let (width, height, rgba) = {
            let decoded = self.decoded(source, raster)?;
            let frame = decoded
                .frames
                .get(frame)
                .or_else(|| decoded.frames.first())
                .ok_or_else(|| invalid_message("image has no frames"))?;
            (frame.width, frame.height, Arc::clone(&frame.rgba))
        };

        // Make room before asking, so the host is never the one to refuse.
        self.evict_for(overlay, rgba.len());
        let image = overlay.upload_rgba(width, height, &rgba)?;
        let id = image.id();
        self.bytes = self.bytes.saturating_add(rgba.len());
        self.uploaded.insert(
            key,
            Held {
                image,
                bytes: rgba.len(),
                used: self.clock,
            },
        );
        Ok(id)
    }

    /// Release uploads, least recently drawn first, until one more of `bytes` fits.
    ///
    /// The ceilings are the protocol's: a producer that ignored them would have the host refuse
    /// its next upload instead, which is the same outcome arrived at less gracefully.
    fn evict_for(&mut self, overlay: &OverlayWindow, bytes: usize) {
        while !self.uploaded.is_empty()
            && (self.uploaded.len() >= MAX_RETAINED_ASSETS
                || self.bytes.saturating_add(bytes) > MAX_RETAINED_ASSET_BYTES)
        {
            let Some(oldest) = self
                .uploaded
                .iter()
                .min_by_key(|(_, held)| held.used)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            if let Some(held) = self.uploaded.remove(&oldest) {
                self.bytes = self.bytes.saturating_sub(held.bytes);
                // A host that has already forgotten it is not an error: what matters is that we
                // stop counting it.
                let _ = overlay.release_image(&held.image);
            }
        }
    }

    /// Decode every raster source a frame is about to draw, so layout knows how big they are.
    ///
    /// A vector is not decoded here: it has no size of its own to offer, because it is
    /// rasterized for the box it lands in, and that box is what layout is about to decide.
    pub(crate) fn prepare(&mut self, sources: &[ImageSource]) {
        for source in sources {
            if source.is_vector() {
                continue;
            }
            // A source that cannot be decoded is simply not drawn: one broken image does not
            // cost the frame it is in. The error surfaces at paint, where it names the source.
            let _ = self.decoded(source, (0, 0));
        }
    }

    /// The size each decoded raster source is, by key.
    pub(crate) fn natural_sizes(&self) -> HashMap<String, crate::vui::geometry::Size> {
        self.decoded
            .iter()
            .filter(|((_, raster), _)| raster.is_none())
            .map(|((key, _), decoded)| {
                (
                    key.clone(),
                    crate::vui::geometry::Size::new(
                        decoded.width() as f32,
                        decoded.height() as f32,
                    ),
                )
            })
            .collect()
    }

    /// How many uploads the host is holding for this window.
    #[cfg_attr(not(feature = "testing"), allow(dead_code))]
    pub(crate) fn retained(&self) -> usize {
        self.uploaded.len()
    }

    /// Let go of everything, which a window does when it closes.
    pub(crate) fn clear(&mut self, overlay: &OverlayWindow) {
        for (_, held) in self.uploaded.drain() {
            let _ = overlay.release_image(&held.image);
        }
        self.decoded.clear();
        self.bytes = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny PNG, built rather than embedded so the test carries its own fixture.
    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut buffer = std::io::Cursor::new(Vec::new());
        let image = image::RgbaImage::from_fn(width, height, |x, y| {
            image::Rgba([(x * 8) as u8, (y * 8) as u8, 0x80, 0xff])
        });
        image
            .write_to(&mut buffer, image::ImageFormat::Png)
            .expect("encode");
        buffer.into_inner()
    }

    #[test]
    fn a_png_decodes_to_one_frame_of_its_own_size() {
        let decoded = decode(&png(8, 4)).unwrap();
        assert_eq!(decoded.frames.len(), 1);
        assert_eq!((decoded.width(), decoded.height()), (8, 4));
        assert_eq!(decoded.frames[0].rgba.len(), 8 * 4 * 4);
        assert!(!decoded.is_animated());
        assert_eq!(decoded.duration(), Duration::ZERO);
        // A still image has no next frame to wait for.
        assert_eq!(decoded.until_next_frame(Duration::ZERO), None);
    }

    #[test]
    fn bytes_that_are_not_an_image_are_refused_rather_than_drawn_blank() {
        let error = decode(b"this is not a picture").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("image rejected"));
    }

    #[test]
    fn an_svg_rasterizes_to_the_box_it_will_be_drawn_in() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">
            <rect width="10" height="10" fill="#4a5fd0"/></svg>"##;
        let decoded = rasterize_svg(svg, 32, 32).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (32, 32));
        assert_eq!(decoded.frames[0].rgba.len(), 32 * 32 * 4);

        // Rasterized at a different size it is a different bitmap, which is the point of keeping
        // the vector rather than scaling one.
        let larger = rasterize_svg(svg, 64, 64).unwrap();
        assert_eq!((larger.width(), larger.height()), (64, 64));

        // A box with no pixels in it is refused rather than rasterized into nothing.
        assert!(rasterize_svg(svg, 0, 32).is_err());
    }

    #[test]
    fn an_animation_advances_through_its_frames_and_loops() {
        let decoded = Decoded {
            frames: vec![
                Frame {
                    width: 1,
                    height: 1,
                    rgba: Arc::new(vec![0; 4]),
                    delay: Duration::from_millis(100),
                },
                Frame {
                    width: 1,
                    height: 1,
                    rgba: Arc::new(vec![0; 4]),
                    delay: Duration::from_millis(50),
                },
            ],
        };
        assert!(decoded.is_animated());
        assert_eq!(decoded.duration(), Duration::from_millis(150));

        assert_eq!(decoded.frame_at(Duration::ZERO), 0);
        assert_eq!(decoded.frame_at(Duration::from_millis(99)), 0);
        assert_eq!(decoded.frame_at(Duration::from_millis(100)), 1);
        assert_eq!(decoded.frame_at(Duration::from_millis(149)), 1);
        // And round again.
        assert_eq!(decoded.frame_at(Duration::from_millis(150)), 0);
        assert_eq!(decoded.frame_at(Duration::from_millis(250)), 1);

        // The wait is until the frame changes, not a fixed tick.
        assert_eq!(
            decoded.until_next_frame(Duration::from_millis(60)),
            Some(Duration::from_millis(40))
        );
        assert_eq!(
            decoded.until_next_frame(Duration::from_millis(100)),
            Some(Duration::from_millis(50))
        );
    }

    #[test]
    fn an_animation_whose_frames_have_no_duration_does_not_spin() {
        let decoded = Decoded {
            frames: vec![
                Frame {
                    width: 1,
                    height: 1,
                    rgba: Arc::new(vec![0; 4]),
                    delay: Duration::ZERO,
                },
                Frame {
                    width: 1,
                    height: 1,
                    rgba: Arc::new(vec![0; 4]),
                    delay: Duration::ZERO,
                },
            ],
        };
        // A zero-length loop shows its first frame and asks for no wake-up, rather than dividing
        // by a duration of nothing.
        assert_eq!(decoded.frame_at(Duration::from_secs(5)), 0);
        assert_eq!(decoded.until_next_frame(Duration::ZERO), None);
    }

    #[test]
    fn object_fit_keeps_proportions_except_where_it_is_told_not_to() {
        use crate::vui::geometry::Bounds;
        let box_ = Bounds::from_xywh(0., 0., 100., 50.);

        // Contain: the whole image fits, centred, with the box's spare width either side.
        let contained = ObjectFit::Contain.place((100., 100.), box_);
        assert_eq!(contained.size.width.get(), 50.);
        assert_eq!(contained.size.height.get(), 50.);
        assert_eq!(contained.origin.x.get(), 25.);

        // Cover: the box is filled and the image overflows.
        let covered = ObjectFit::Cover.place((100., 100.), box_);
        assert_eq!(covered.size.width.get(), 100.);
        assert_eq!(covered.size.height.get(), 100.);
        assert_eq!(covered.origin.y.get(), -25.);

        // Fill: the box, exactly.
        assert_eq!(ObjectFit::Fill.place((100., 100.), box_), box_);

        // An image with no size is not divided by zero.
        assert_eq!(ObjectFit::Contain.place((0., 0.), box_), box_);
    }
}
