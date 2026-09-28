//! Freeform drawing: a closure that paints paths into the box it was given.
//!
//! Everything else in the toolkit paints a box, a run of text, or a picture, and that covers most
//! of a user interface. A canvas is the escape hatch — a gauge, a plot, a diagram, a freehand
//! annotation — and it is the only element whose drawing is code rather than data.
//!
//! It draws in **window coordinates** and is handed its own [`Bounds`], exactly as GPUI's paint
//! callback is. That is not only a matter of familiarity: a path is quantized to the wire's fixed
//! point once, on the way out, so a drawing done in some other space would either need a transform
//! command per canvas or be translated after quantization, which rounds twice. Everything here is
//! placed while it is still `f32`, and `f32` only.

use std::fmt;
use std::io;
use std::rc::Rc;

use vivid_protocol::vector::{
    Brush, Canvas, ColorSpace, InvalidScene, MAX_TOTAL_SEGMENTS, Path, PathBuilder, StrokeStyle,
};

use crate::vui::geometry::Bounds;
use crate::vui::node::{ElementId, Node};
use crate::vui::style::{Colors, GradientStopSpec, Style};

/// What a drawing call accepts as geometry: a finished [`Path`], or a [`PathBuilder`] whose
/// pending error is carried into the drawing rather than unwrapped where it was written.
///
/// This is what lets a drawing statement read as one — no `?`, and no `unwrap` on a coordinate the
/// caller had no way to check.
pub trait IntoPath {
    /// The path, or the reason there is not one.
    fn resolve(self) -> Result<Path, InvalidScene>;
}

impl IntoPath for Path {
    fn resolve(self) -> Result<Path, InvalidScene> {
        self.validate().map(|()| self)
    }
}

impl IntoPath for PathBuilder {
    fn resolve(self) -> Result<Path, InvalidScene> {
        self.build()
    }
}

/// The drawing a canvas closure is handed.
///
/// Every call is a drawing statement and none of them fails at the call site. A coordinate the
/// wire cannot carry, a path past a ceiling, or a stroke style it would reject is remembered, and
/// the frame fails once, when the closure returns, naming the first thing that went wrong. A
/// canvas is content, not decoration: painting truncates decoration when a frame runs out of room
/// and fails loudly rather than truncating structure, and this is structure.
pub struct Drawing<'a> {
    canvas: &'a mut Canvas,
    bounds: Bounds,
    /// Segments drawn so far.
    ///
    /// Charged here because `Canvas::push` counts commands and nothing else: without this the
    /// segment ceiling would not be reached until the frame was encoded, which is nowhere near the
    /// statement that overran it.
    segments: usize,
    failed: Option<io::Error>,
}

impl Drawing<'_> {
    /// The box this canvas was given, in window coordinates.
    pub fn bounds(&self) -> Bounds {
        self.bounds
    }

    /// Fill a path.
    pub fn fill(&mut self, path: impl IntoPath, brush: Brush) {
        let Some(path) = self.geometry(path) else {
            return;
        };
        if let Err(error) = self.canvas.fill(path, brush).map(|_| ()) {
            self.refused(error);
        }
    }

    /// Stroke a path at `width`, in logical pixels.
    pub fn stroke(&mut self, path: impl IntoPath, brush: Brush, width: f32) {
        let Some(path) = self.geometry(path) else {
            return;
        };
        if let Err(error) = self.canvas.stroke(path, brush, width as f64).map(|_| ()) {
            self.refused(error);
        }
    }

    /// Stroke a path with caps, joins, and an optional dash pattern.
    pub fn stroke_styled(&mut self, path: impl IntoPath, brush: Brush, style: StrokeStyle) {
        // Checked here rather than left to the frame's encode step, which is where it would
        // otherwise be caught: a style the wire refuses is a mistake in this drawing, and the
        // drawing is what should say so.
        if let Err(error) = style.validate() {
            self.refused(error);
            return;
        }
        let Some(path) = self.geometry(path) else {
            return;
        };
        if let Err(error) = self.canvas.stroke_styled(path, brush, style).map(|_| ()) {
            self.refused(error);
        }
    }

    /// A linear gradient across this canvas's box, at `angle` degrees — the CSS convention, so 0
    /// is upward and 90 rightward.
    ///
    /// The endpoints come from the box, so a gradient is written where the box is known rather
    /// than reconstructed by hand at every call. A box the wire cannot describe records the error
    /// and yields a brush nothing goes on to draw with, because that drawing has already failed.
    pub fn gradient(&mut self, angle: f32, stops: Vec<GradientStopSpec>) -> Brush {
        let (start, end) = Style::gradient_line(self.bounds, angle);
        match (start.to_wire(), end.to_wire()) {
            (Ok(start), Ok(end)) => Brush::Linear {
                start,
                end,
                stops: stops.iter().map(GradientStopSpec::wire).collect(),
                color_space: ColorSpace::Srgb,
            },
            (Err(error), _) | (_, Err(error)) => {
                self.fail(error);
                Brush::Solid(Colors::TRANSPARENT)
            }
        }
    }

    /// The path a call asked for, once it is one the wire can carry and there is room for it.
    fn geometry(&mut self, path: impl IntoPath) -> Option<Path> {
        let path = match path.resolve() {
            Ok(path) => path,
            Err(error) => {
                self.refused(error);
                return None;
            }
        };
        self.segments = self.segments.saturating_add(path.segments.len());
        if self.segments > MAX_TOTAL_SEGMENTS {
            self.refused(InvalidScene("scene path budget exceeded"));
            return None;
        }
        Some(path)
    }

    fn refused(&mut self, error: InvalidScene) {
        self.fail(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("the canvas cannot be drawn ({error})"),
        ));
    }

    /// Keep the first failure: a drawing that went wrong once usually goes wrong everywhere after.
    fn fail(&mut self, error: io::Error) {
        if self.failed.is_none() {
            self.failed = Some(error);
        }
    }
}

/// One canvas's drawing, rebuilt with its element every frame.
pub(crate) struct CanvasSpec {
    pub draw: Draw,
}

impl CanvasSpec {
    /// Run the drawing into the frame's display list.
    pub(crate) fn paint(&self, canvas: &mut Canvas, bounds: Bounds) -> io::Result<()> {
        let mut drawing = Drawing {
            canvas,
            bounds,
            segments: 0,
            failed: None,
        };
        (self.draw.0)(&mut drawing, bounds);
        match drawing.failed {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl PartialEq for CanvasSpec {
    fn eq(&self, other: &Self) -> bool {
        self.draw == other.draw
    }
}

impl fmt::Debug for CanvasSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CanvasSpec")
    }
}

impl Clone for CanvasSpec {
    fn clone(&self) -> Self {
        Self {
            draw: self.draw.clone(),
        }
    }
}

/// A boxed drawing closure.
///
/// Compared by identity, like [`crate::vui::list::RowBuilder`]: nothing can compare closures, and a
/// view that rebuilds its element tree every frame can only produce a fresh one. That is harmless,
/// because what decides whether a frame is submitted is the display list the closures produced,
/// not the tree they were held in.
pub(crate) type DrawFn = dyn Fn(&mut Drawing, Bounds);

pub(crate) struct Draw(Rc<DrawFn>);

impl PartialEq for Draw {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for Draw {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Draw")
    }
}

impl Clone for Draw {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

/// A drawing of your own, painted into whatever box the element is given.
///
/// A canvas has no size of its own — the closure draws whatever the box turned out to be — so one
/// has to be given: `.size(..)`, `.w_full()`, `.h_full()` or `.flex_1()`. An unsized canvas is not
/// an error, it is a canvas of nothing, which is why this is worth saying twice.
///
/// ```no_run
/// # use vlib::vui::prelude::*;
/// # use vlib::vui::canvas;
/// # use vivid_protocol::vector::{Brush, Path};
/// canvas(|draw, bounds| {
///     let centre = bounds.center();
///     draw.fill(
///         Path::builder()
///             .move_to(centre.x.get() as f64, bounds.origin.y.get() as f64)
///             .line_to(bounds.right().get() as f64, bounds.bottom().get() as f64)
///             .line_to(bounds.origin.x.get() as f64, bounds.bottom().get() as f64)
///             .close(),
///         Brush::Solid(rgb(0x4a5fd0)),
///     );
/// })
/// .size(120., 120.);
/// ```
pub fn canvas(draw: impl Fn(&mut Drawing, Bounds) + 'static) -> CanvasEl {
    CanvasEl {
        node: Node::canvas(CanvasSpec {
            draw: Draw(Rc::new(draw)),
        }),
    }
}

/// A drawing an element paints itself.
#[derive(Debug)]
pub struct CanvasEl {
    node: Node,
}

impl CanvasEl {
    /// Give the canvas an identity.
    ///
    /// Anything that takes a pointer event needs one, including a drawing that follows the
    /// pointer: the host reports what was hit by region number, and an element without an id has
    /// no number to report.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.node.id = Some(id.into());
        self
    }

    pub fn w(mut self, width: f32) -> Self {
        self.node.layout.width = crate::vui::layout::Length::px(width);
        self
    }

    pub fn h(mut self, height: f32) -> Self {
        self.node.layout.height = crate::vui::layout::Length::px(height);
        self
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.node.layout.width = crate::vui::layout::Length::px(width);
        self.node.layout.height = crate::vui::layout::Length::px(height);
        self
    }

    pub fn w_full(mut self) -> Self {
        self.node.layout.width = crate::vui::layout::Length::full();
        self
    }

    pub fn h_full(mut self) -> Self {
        self.node.layout.height = crate::vui::layout::Length::full();
        self
    }

    pub fn flex_1(mut self) -> Self {
        self.node.layout.flex_grow = 1.;
        self.node.layout.flex_shrink = 1.;
        self.node.layout.flex_basis = crate::vui::layout::Length::px(0.);
        self
    }

    /// Clip the drawing to its box, which a canvas that draws past its own edges wants.
    pub fn clipped(mut self) -> Self {
        self.node.layout.overflow = crate::vui::layout::OverflowSpec::Hidden;
        self
    }

    pub fn bg(mut self, color: vivid_protocol::vector::Color) -> Self {
        self.node.style.background = Some(crate::vui::style::Background::Solid(color));
        self
    }

    pub fn rounded(mut self, radius: f32) -> Self {
        self.node.style.rounded = crate::vui::geometry::Corners::uniform(radius);
        self
    }

    pub fn border(mut self, width: f32, color: vivid_protocol::vector::Color) -> Self {
        self.node.style.border = Some(crate::vui::style::Border::solid(
            crate::vui::geometry::Pixels::new(width),
            color,
        ));
        self
    }

    pub fn cursor(mut self, cursor: vivid_protocol::vector::CursorShape) -> Self {
        self.node.interactions.set_cursor(Some(cursor));
        self
    }

    pub fn on_click(mut self, handler: crate::vui::interactive::Handler) -> Self {
        self.node.interactions.handlers.click = Some(handler);
        self
    }

    pub fn on_mouse_down(mut self, handler: crate::vui::interactive::Handler) -> Self {
        self.node.interactions.handlers.mouse_down = Some(handler);
        self
    }

    pub fn on_mouse_up(mut self, handler: crate::vui::interactive::Handler) -> Self {
        self.node.interactions.handlers.mouse_up = Some(handler);
        self
    }

    pub fn on_mouse_move(mut self, handler: crate::vui::interactive::Handler) -> Self {
        self.node.interactions.handlers.mouse_move = Some(handler);
        self
    }

    /// A stylus or trackpad pressed with force, which is what a brush width can follow.
    pub fn on_pressure(mut self, handler: crate::vui::interactive::Handler) -> Self {
        self.node.interactions.handlers.pressure = Some(handler);
        self
    }

    pub fn on_wheel(mut self, handler: crate::vui::interactive::Handler) -> Self {
        self.node.interactions.handlers.wheel = Some(handler);
        self
    }

    /// Name the region the host should treat as a drag handle, which is what moves a window.
    pub fn drag_region(mut self) -> Self {
        self.node
            .interactions
            .set_role(vivid_protocol::vector::HitRole::Drag);
        self
    }

    /// Name the region the host should treat as a resize handle on `direction`'s edge.
    pub fn resize_region(mut self, direction: u8) -> Self {
        self.node
            .interactions
            .set_role(vivid_protocol::vector::HitRole::Resize(direction));
        self
    }
}

impl crate::vui::a11y::Semantic for CanvasEl {
    fn accessibility_mut(&mut self) -> &mut crate::vui::a11y::Accessibility {
        &mut self.node.accessibility
    }
}

impl crate::vui::element::IntoElement for CanvasEl {
    fn into_node(self) -> Node {
        self.node
    }
}
