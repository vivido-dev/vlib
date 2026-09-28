//! What an element looks like: fills, borders, radii, shadows, opacity.
//!
//! This is the paint half of styling. Layout is [`crate::vui::layout::LayoutStyle`], which is a
//! different vocabulary for a different engine, and the two are set through one builder so an
//! element reads as a single declaration.

use vivid_protocol::vector::{Color, ColorSpace, GradientStop};

use crate::vui::geometry::{Bounds, Corners, Edges, Pixels, Point};

/// An opaque `0xRRGGBB` color.
pub const fn rgb(hex: u32) -> Color {
    rgba(hex, 0xff)
}

/// An `0xRRGGBB` color with an explicit alpha byte.
pub const fn rgba(hex: u32, alpha: u8) -> Color {
    Color((hex & 0x00ff_ffff) << 8 | alpha as u32)
}

pub struct Colors;
impl Colors {
    pub const WHITE: Color = rgba(0xffffff, 0xff);
    pub const BLACK: Color = rgba(0x000000, 0xff);
    pub const TRANSPARENT: Color = rgba(0x000000, 0x00);
}

/// How an element's background is painted.
#[derive(Debug, Clone, PartialEq)]
pub enum Background {
    Solid(Color),
    /// A linear gradient at `angle` degrees, where 0 runs left to right and 90 runs top to
    /// bottom — the CSS convention, so a ported value keeps its meaning.
    Linear {
        angle: f32,
        stops: Vec<GradientStopSpec>,
    },
    /// A retained image, drawn at its natural size and tiled across the box.
    ///
    /// The asset must still be retained by whoever uploaded it: an id outlives nothing on its
    /// own.
    Image {
        asset: u64,
    },
}

/// One gradient stop. `offset` is 0..=1; the wire's 16-bit units are an encoding detail.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradientStopSpec {
    pub offset: f32,
    pub color: Color,
}

impl GradientStopSpec {
    pub const fn new(offset: f32, color: Color) -> Self {
        Self { offset, color }
    }

    pub(crate) fn wire(&self) -> GradientStop {
        let offset = (self.offset.clamp(0., 1.) * 65535.).round() as u16;
        GradientStop {
            offset,
            color: self.color,
        }
    }
}

/// A border around an element. Widths are per side, which is what a divider or an underline
/// needs; a uniform border strokes the rounded outline so corners stay true.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Border {
    pub width: Edges,
    pub color: Color,
    /// Dash length in logical pixels. `None` is a solid line.
    pub dash: Option<Pixels>,
}

impl Border {
    pub fn solid(width: Pixels, color: Color) -> Self {
        Self {
            width: Edges::uniform(width.0),
            color,
            dash: None,
        }
    }

    pub fn dashed(width: Pixels, color: Color) -> Self {
        Self {
            width: Edges::uniform(width.0),
            color,
            dash: Some(Pixels(width.0 * 2.)),
        }
    }

    /// True when every side has the same width, which is what lets a border follow the outline.
    pub fn is_uniform(&self) -> bool {
        self.width.top == self.width.right
            && self.width.right == self.width.bottom
            && self.width.bottom == self.width.left
    }
}

/// A shadow cast behind an element, or inside it when `inset`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxShadow {
    pub offset: Point,
    pub blur: Pixels,
    pub spread: Pixels,
    pub color: Color,
    pub inset: bool,
}

impl BoxShadow {
    pub const fn new(offset: Point, blur: Pixels, color: Color) -> Self {
        Self {
            offset,
            blur,
            spread: Pixels::ZERO,
            color,
            inset: false,
        }
    }

    pub const fn with_spread(mut self, spread: Pixels) -> Self {
        self.spread = spread;
        self
    }

    pub const fn inset(mut self) -> Self {
        self.inset = true;
        self
    }
}

/// The paint style of one element.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Style {
    pub background: Option<Background>,
    pub border: Option<Border>,
    pub rounded: Corners,
    pub shadows: Vec<BoxShadow>,
    /// `0..=1`. Applied to the element and everything it draws, as one group.
    pub opacity: Option<f32>,
}

impl Style {
    /// How `BoxShadow` composes: shadows stack behind the element, nearest first.
    pub fn push_shadow(mut self, shadow: BoxShadow) -> Self {
        self.shadows.push(shadow);
        self
    }

    /// Replace the background with one color.
    pub fn bg(mut self, color: Color) -> Self {
        self.background = Some(Background::Solid(color));
        self
    }

    /// Replace the background with a gradient at `angle` degrees: 0 is upward, 90 rightward.
    pub fn bg_gradient(mut self, angle: f32, stops: Vec<GradientStopSpec>) -> Self {
        self.background = Some(Background::Linear { angle, stops });
        self
    }

    pub fn bg_image(mut self, asset: u64) -> Self {
        self.background = Some(Background::Image { asset });
        self
    }

    pub fn rounded(mut self, radius: f32) -> Self {
        self.rounded = Corners::uniform(radius);
        self
    }

    pub fn border(mut self, width: f32, color: Color) -> Self {
        self.border = Some(Border::solid(Pixels::new(width), color));
        self
    }

    pub fn border_1(self) -> Self {
        self.border(1., rgba(0x000000, 0x40))
    }

    pub fn shadow(mut self, shadow: BoxShadow) -> Self {
        self.shadows.push(shadow);
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = Some(opacity);
        self
    }

    /// The gradient's endpoints for a box of `bounds`, following the CSS angle convention.
    pub(crate) fn gradient_line(bounds: Bounds, angle: f32) -> (Point, Point) {
        // CSS measures the angle clockwise from "to top", and screen y grows downward, so the
        // direction is (sin, -cos). The line is scaled to span the box along its own direction.
        let (sin, cos) = angle.to_radians().sin_cos();
        let direction = (sin, -cos);
        let extent = direction.0.abs() * (bounds.width().0 / 2.)
            + direction.1.abs() * (bounds.height().0 / 2.);
        let center = bounds.center();
        (
            Point::new(
                center.x.0 - direction.0 * extent,
                center.y.0 - direction.1 * extent,
            ),
            Point::new(
                center.x.0 + direction.0 * extent,
                center.y.0 + direction.1 * extent,
            ),
        )
    }
}

/// The shadow scale, in the token names a stylesheet uses.
pub mod shadows {
    use super::{BoxShadow, rgba};
    use crate::vui::geometry::{Pixels, Point};

    pub const XS: BoxShadow = token(Point::new(0., 1.), 2., 0., 0x05);
    pub const SM: BoxShadow = token(Point::new(0., 1.), 3., 0., 0x0a);
    pub const BASE: BoxShadow = token(Point::new(0., 4.), 6., -1., 0x0a);
    pub const MD: BoxShadow = token(Point::new(0., 6.), 12., -2., 0x0a);
    pub const LG: BoxShadow = token(Point::new(0., 10.), 15., -3., 0x0a);
    pub const XL: BoxShadow = token(Point::new(0., 20.), 25., -5., 0x0a);
    pub const XL2: BoxShadow = token(Point::new(0., 25.), 50., -12., 0x19);

    const fn token(offset: Point, blur: f32, spread: f32, alpha: u8) -> BoxShadow {
        BoxShadow::new(offset, Pixels(blur), rgba(0x000000, alpha)).with_spread(Pixels(spread))
    }
}

/// The color space gradients interpolate in.
pub(crate) const GRADIENT_COLOR_SPACE: ColorSpace = ColorSpace::Srgb;

/// How a background image samples outside its own extent: it tiles.
pub(crate) const TILED: vivid_protocol::vector::Extend = vivid_protocol::vector::Extend::Repeat;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_pack_as_rgba() {
        assert_eq!(rgb(0x203050).0, 0x2030_50ff);
        assert_eq!(rgba(0x203050, 0x80).0, 0x2030_5080);
        assert_eq!(Colors::TRANSPARENT.0, 0);
        // A value wider than 24 bits cannot bleed into the alpha byte.
        assert_eq!(rgb(0xff20_3050).0, 0x2030_50ff);
    }

    #[test]
    fn gradient_offsets_quantize_to_the_wire_range() {
        assert_eq!(GradientStopSpec::new(0., Colors::BLACK).wire().offset, 0);
        assert_eq!(
            GradientStopSpec::new(1., Colors::BLACK).wire().offset,
            65535
        );
        assert_eq!(
            GradientStopSpec::new(0.5, Colors::BLACK).wire().offset,
            32768
        );
        // Out of range clamps rather than wrapping into a different color stop.
        assert_eq!(GradientStopSpec::new(-1., Colors::BLACK).wire().offset, 0);
        assert_eq!(
            GradientStopSpec::new(9., Colors::BLACK).wire().offset,
            65535
        );
    }

    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 0.001
    }

    #[test]
    fn a_gradient_line_follows_the_css_angle_convention() {
        let bounds = Bounds::from_xywh(0., 0., 100., 50.);

        // Zero degrees runs upward, so the line is vertical and ends at the top edge.
        let (start, end) = Style::gradient_line(bounds, 0.);
        assert!(close(start.x.get(), 50.) && close(start.y.get(), 50.));
        assert!(close(end.x.get(), 50.) && close(end.y.get(), 0.));

        // Ninety degrees runs left to right.
        let (start, end) = Style::gradient_line(bounds, 90.);
        assert!(close(start.x.get(), 0.) && close(start.y.get(), 25.));
        assert!(close(end.x.get(), 100.) && close(end.y.get(), 25.));

        // A hundred and eighty degrees runs downward, across the shorter side.
        let (start, end) = Style::gradient_line(bounds, 180.);
        assert!(close(start.x.get(), 50.) && close(start.y.get(), 0.));
        assert!(close(end.x.get(), 50.) && close(end.y.get(), 50.));

        // Diagonal: the line spans the box along its own direction, so it is longer than either
        // side.
        let (start, end) = Style::gradient_line(bounds, 45.);
        let length = ((end.x.0 - start.x.0).powi(2) + (end.y.0 - start.y.0).powi(2)).sqrt();
        assert!(
            length > 50.,
            "a diagonal gradient covers the whole box: {length}"
        );
    }

    #[test]
    fn a_uniform_border_is_recognized_and_a_ring_is_not() {
        let solid = Border::solid(Pixels::new(1.), Colors::BLACK);
        assert!(solid.is_uniform());
        let mut lopsided = solid;
        lopsided.width.bottom = Pixels::ZERO;
        assert!(!lopsided.is_uniform());
    }
}
