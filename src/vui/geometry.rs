//! Logical-pixel geometry.
//!
//! Layout is computed in floating point and quantized to the wire's Q32.32 fixed point exactly
//! once, on the way out to a display list. Everything between layout and that boundary stays
//! `f32`, so a rounding error cannot compound through nested layout.

use std::fmt;
use std::io;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub};

use vivid_protocol::vector::{
    Corners as WireCorners, Point as WirePoint, Rect as WireRect, Scalar,
};

/// A length, coordinate, or size in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Pixels(pub f32);

impl Pixels {
    pub const ZERO: Self = Self(0.);

    pub const fn new(value: f32) -> Self {
        Self(value)
    }

    pub fn get(self) -> f32 {
        self.0
    }

    /// A value the wire can carry: finite and inside the protocol's logical extent ceiling.
    pub fn is_paintable(self) -> bool {
        self.0.is_finite() && self.0.abs() <= 1_000_000.
    }

    pub fn clamp(self, minimum: Self, maximum: Self) -> Self {
        Self(self.0.clamp(minimum.0, maximum.0))
    }

    pub fn max(self, other: Self) -> Self {
        Self(self.0.max(other.0))
    }

    pub fn min(self, other: Self) -> Self {
        Self(self.0.min(other.0))
    }

    /// Round to whole logical pixels, which is what a crisp one-pixel border wants.
    pub fn round(self) -> Self {
        Self(self.0.round())
    }

    pub fn floor(self) -> Self {
        Self(self.0.floor())
    }

    pub fn ceil(self) -> Self {
        Self(self.0.ceil())
    }

    pub(crate) fn to_scalar(self) -> io::Result<Scalar> {
        Scalar::new(self.0 as f64).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("coordinate rejected: {error}"),
            )
        })
    }
}

impl Add for Pixels {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

impl AddAssign for Pixels {
    fn add_assign(&mut self, other: Self) {
        self.0 += other.0;
    }
}

impl Sub for Pixels {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self(self.0 - other.0)
    }
}

impl Mul<f32> for Pixels {
    type Output = Self;
    fn mul(self, factor: f32) -> Self {
        Self(self.0 * factor)
    }
}

impl Div<f32> for Pixels {
    type Output = Self;
    fn div(self, divisor: f32) -> Self {
        Self(self.0 / divisor)
    }
}

impl Neg for Pixels {
    type Output = Self;
    fn neg(self) -> Self {
        Self(-self.0)
    }
}

impl From<f32> for Pixels {
    fn from(value: f32) -> Self {
        Self(value)
    }
}

impl From<Pixels> for f32 {
    fn from(value: Pixels) -> Self {
        value.0
    }
}

impl fmt::Display for Pixels {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}px", self.0)
    }
}

/// A position in a window's own logical pixels, measured from its top-left corner.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: Pixels,
    pub y: Pixels,
}

impl Point {
    pub const ZERO: Self = Self {
        x: Pixels::ZERO,
        y: Pixels::ZERO,
    };

    pub const fn new(x: f32, y: f32) -> Self {
        Self {
            x: Pixels(x),
            y: Pixels(y),
        }
    }

    /// Read a wire point back, which is how an event's position arrives.
    pub fn from_wire(point: WirePoint) -> Self {
        Self::new(point.x.get() as f32, point.y.get() as f32)
    }

    pub(crate) fn to_wire(self) -> io::Result<WirePoint> {
        Point::wire_extent(self.x, self.y)
    }

    fn wire_extent(x: Pixels, y: Pixels) -> io::Result<WirePoint> {
        WirePoint::new(x.0 as f64, y.0 as f64).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("point rejected: {error}"),
            )
        })
    }
}

impl Add for Point {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
}

impl Sub for Point {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }
}

impl AddAssign for Point {
    fn add_assign(&mut self, other: Self) {
        self.x += other.x;
        self.y += other.y;
    }
}

/// A width and a height.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Size {
    pub width: Pixels,
    pub height: Pixels,
}

impl Size {
    pub const ZERO: Self = Self {
        width: Pixels::ZERO,
        height: Pixels::ZERO,
    };

    pub const fn new(width: f32, height: f32) -> Self {
        Self {
            width: Pixels(width),
            height: Pixels(height),
        }
    }

    pub(crate) fn to_wire(self, origin: Point) -> io::Result<WireRect> {
        WireRect::new(
            origin.x.0 as f64,
            origin.y.0 as f64,
            self.width.0 as f64,
            self.height.0 as f64,
        )
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("rect rejected: {error}"),
            )
        })
    }
}

/// A rectangle: an origin and a size. Layout produces one per node.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Bounds {
    pub origin: Point,
    pub size: Size,
}

impl Bounds {
    pub const ZERO: Self = Self {
        origin: Point::ZERO,
        size: Size::ZERO,
    };

    pub const fn new(origin: Point, size: Size) -> Self {
        Self { origin, size }
    }

    pub const fn from_xywh(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        }
    }

    pub fn width(self) -> Pixels {
        self.size.width
    }

    pub fn height(self) -> Pixels {
        self.size.height
    }

    pub fn right(self) -> Pixels {
        self.origin.x + self.size.width
    }

    pub fn bottom(self) -> Pixels {
        self.origin.y + self.size.height
    }

    pub fn center(self) -> Point {
        Point::new(
            self.origin.x.0 + self.size.width.0 / 2.,
            self.origin.y.0 + self.size.height.0 / 2.,
        )
    }

    /// Shrink by an inset, never past nothing.
    pub fn inset(self, edges: Edges) -> Self {
        Self {
            origin: Point::new(
                self.origin.x.0 + edges.left.0,
                self.origin.y.0 + edges.top.0,
            ),
            size: Size::new(
                (self.size.width.0 - edges.horizontal().0).max(0.),
                (self.size.height.0 - edges.vertical().0).max(0.),
            ),
        }
    }

    pub fn contains(self, point: Point) -> bool {
        point.x >= self.origin.x
            && point.y >= self.origin.y
            && point.x < self.right()
            && point.y < self.bottom()
    }

    pub(crate) fn to_wire(self) -> io::Result<WireRect> {
        self.size.to_wire(self.origin)
    }

    /// Read a wire rectangle back, which is how a window learns its own geometry.
    pub fn from_wire(rect: WireRect) -> Self {
        Self {
            origin: Point::new(rect.origin.x.get() as f32, rect.origin.y.get() as f32),
            size: Size::new(rect.width.get() as f32, rect.height.get() as f32),
        }
    }
}

/// Per-side lengths, used for padding, margin, and border widths.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Edges {
    pub top: Pixels,
    pub right: Pixels,
    pub bottom: Pixels,
    pub left: Pixels,
}

impl Edges {
    pub const ZERO: Self = Self {
        top: Pixels::ZERO,
        right: Pixels::ZERO,
        bottom: Pixels::ZERO,
        left: Pixels::ZERO,
    };

    pub const fn uniform(value: f32) -> Self {
        Self {
            top: Pixels(value),
            right: Pixels(value),
            bottom: Pixels(value),
            left: Pixels(value),
        }
    }

    pub const fn all(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top: Pixels(top),
            right: Pixels(right),
            bottom: Pixels(bottom),
            left: Pixels(left),
        }
    }

    pub fn horizontal(self) -> Pixels {
        self.left + self.right
    }

    pub fn vertical(self) -> Pixels {
        self.top + self.bottom
    }

    pub fn is_zero(self) -> bool {
        self == Self::ZERO
    }
}

/// Four corner radii, clockwise from the top left.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Corners {
    pub top_left: Pixels,
    pub top_right: Pixels,
    pub bottom_right: Pixels,
    pub bottom_left: Pixels,
}

impl Corners {
    pub const ZERO: Self = Self {
        top_left: Pixels::ZERO,
        top_right: Pixels::ZERO,
        bottom_right: Pixels::ZERO,
        bottom_left: Pixels::ZERO,
    };

    pub const fn uniform(radius: f32) -> Self {
        Self {
            top_left: Pixels(radius),
            top_right: Pixels(radius),
            bottom_right: Pixels(radius),
            bottom_left: Pixels(radius),
        }
    }

    pub fn is_zero(self) -> bool {
        self == Self::ZERO
    }

    /// No corner radius may exceed half of the shorter side, or the shape inverts.
    pub fn clamp_to(self, bounds: Bounds) -> Self {
        let limit = bounds.width().0.min(bounds.height().0) / 2.;
        let clamp = |radius: Pixels| Pixels(radius.0.clamp(0., limit));
        Self {
            top_left: clamp(self.top_left),
            top_right: clamp(self.top_right),
            bottom_right: clamp(self.bottom_right),
            bottom_left: clamp(self.bottom_left),
        }
    }

    pub(crate) fn to_wire(self) -> io::Result<WireCorners> {
        WireCorners::new([
            self.top_left.0 as f64,
            self.top_right.0 as f64,
            self.bottom_right.0 as f64,
            self.bottom_left.0 as f64,
        ])
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("corner radius rejected: {error}"),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_is_the_obvious_thing() {
        let a = Pixels::new(10.);
        let b = Pixels::new(4.);
        assert_eq!((a + b).get(), 14.);
        assert_eq!((a - b).get(), 6.);
        assert_eq!((a * 2.).get(), 20.);
        assert_eq!((a / 4.).get(), 2.5);
        assert_eq!((-a).get(), -10.);
        assert_eq!(Pixels::new(2.4).round().get(), 2.);
        assert_eq!(Pixels::new(2.6).round().get(), 3.);
    }

    #[test]
    fn a_not_finite_length_is_not_paintable() {
        assert!(Pixels::new(0.).is_paintable());
        assert!(Pixels::new(1_000_000.).is_paintable());
        assert!(!Pixels::new(f32::NAN).is_paintable());
        assert!(!Pixels::new(f32::INFINITY).is_paintable());
        assert!(!Pixels::new(1_000_001.).is_paintable());
        // And the wire agrees: the boundary is the protocol's, not ours.
        assert!(Pixels::new(1_000_000.).to_scalar().is_ok());
        assert!(Pixels::new(f32::NAN).to_scalar().is_err());
    }

    #[test]
    fn bounds_inset_never_goes_negative() {
        let bounds = Bounds::from_xywh(10., 10., 20., 20.);
        let inset = bounds.inset(Edges::uniform(30.));
        assert_eq!(inset.size.width.get(), 0.);
        assert_eq!(inset.size.height.get(), 0.);
        assert_eq!(inset.origin.x.get(), 40.);

        let padded = bounds.inset(Edges::all(1., 2., 3., 4.));
        assert_eq!(padded.origin.x.get(), 14.);
        assert_eq!(padded.origin.y.get(), 11.);
        assert_eq!(padded.size.width.get(), 14.);
        assert_eq!(padded.size.height.get(), 16.);
    }

    #[test]
    fn contains_is_half_open() {
        let bounds = Bounds::from_xywh(10., 10., 20., 20.);
        assert!(bounds.contains(Point::new(10., 10.)));
        assert!(bounds.contains(Point::new(29.9, 29.9)));
        assert!(!bounds.contains(Point::new(30., 20.)));
        assert!(!bounds.contains(Point::new(20., 30.)));
    }

    #[test]
    fn radii_clamp_to_half_the_shorter_side() {
        let bounds = Bounds::from_xywh(0., 0., 100., 40.);
        let clamped = Corners::uniform(80.).clamp_to(bounds);
        assert_eq!(clamped.top_left.get(), 20.);
        // A radius that already fits is left alone.
        assert_eq!(Corners::uniform(4.).clamp_to(bounds).bottom_right.get(), 4.);
    }
}
