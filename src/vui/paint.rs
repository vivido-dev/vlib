//! Turning a laid-out tree into one display list.
//!
//! Painting is where every floating-point length becomes a protocol coordinate, and it is the
//! only place that happens. It is also where the element tree meets the budget the host enforces:
//! a display list has a fixed command ceiling, and hit regions are charged against the same one.

use std::collections::HashMap;
use std::time::Duration;

use vivid_protocol::overlay::wire::text::styled::MAX_RETAINED_LAYOUTS;
use vivid_protocol::vector::{Brush, Canvas, Command, Path, Rect, Shadow, StrokeStyle};
use vivid_sdk::overlay::OverlayWindow;

use crate::vui::focus::FocusHandle;
use crate::vui::geometry::{Bounds, Corners as UiCorners, Pixels, Point, Size};
use crate::vui::interactive::ElementState;
use crate::vui::node::{ElementId, Node, NodeKind, region_id};
use crate::vui::style::{Background, Border, Style};

/// What one frame of painting produced.
pub(crate) struct Painted {
    pub canvas: Canvas,
    /// Region number to element, for routing what the host sends back.
    pub regions: HashMap<u64, ElementId>,
    /// Focusable elements in paint order, which is the tab order.
    pub focus_stops: Vec<(FocusHandle, u64)>,
    /// Shadows dropped to stay inside the command budget, reported rather than hidden.
    pub dropped_shadows: usize,
    /// Pictures whose bytes could not be decoded, counted for the same reason: they left a styled
    /// empty box behind, and nothing about the frame says so otherwise.
    pub dropped_images: usize,
    /// Where the focused field's caret is, in window coordinates. The host places its input
    /// method's candidate window from this.
    pub editor_caret: Option<Rect>,
    /// How soon something in this frame wants to be drawn again: the next frame of an animation,
    /// or the next step of one. `None` means nothing is moving and the loop can sleep.
    pub wake_after: Option<Duration>,
}

/// A shadow is decoration, so it is the first thing to go when a frame does not fit. Structure is
/// never dropped silently, and this ceiling bounds the work even when everything is decorated.
const MAX_DROPPED_SHADOWS: usize = 4096;

pub(crate) fn paint(
    root: &Node,
    states: &HashMap<u64, ElementState>,
    text: &mut crate::vui::text::TextSystem,
    assets: &mut crate::vui::assets::Assets,
    overlay: &OverlayWindow,
    clock: Clock,
) -> std::io::Result<Painted> {
    assets.begin_frame();
    let mut painter = Painter {
        canvas: Canvas::new(),
        regions: HashMap::new(),
        focus_stops: Vec::new(),
        dropped_shadows: 0,
        dropped_images: 0,
        editor_caret: None,
        wake_after: None,
        states,
        path: Vec::new(),
        text,
        assets,
        overlay,
        clock,
    };
    painter.node(root, None)?;
    Ok(Painted {
        canvas: painter.canvas,
        regions: painter.regions,
        focus_stops: painter.focus_stops,
        dropped_shadows: painter.dropped_shadows,
        dropped_images: painter.dropped_images,
        editor_caret: painter.editor_caret,
        wake_after: painter.wake_after,
    })
}

struct Painter<'a> {
    canvas: Canvas,
    regions: HashMap<u64, ElementId>,
    focus_stops: Vec<(FocusHandle, u64)>,
    dropped_shadows: usize,
    dropped_images: usize,
    editor_caret: Option<Rect>,
    wake_after: Option<Duration>,
    states: &'a HashMap<u64, ElementState>,
    /// The ids of the ancestors, then this node, so a region is stable wherever an element sits.
    path: Vec<&'a ElementId>,
    /// Shapes the host is holding for this frame, and the host they belong to.
    text: &'a mut crate::vui::text::TextSystem,
    assets: &'a mut crate::vui::assets::Assets,
    overlay: &'a OverlayWindow,
    clock: Clock,
}

/// What a moving thing needs to know: how long this window has been up, and how many device
/// pixels one of its logical pixels is.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Clock {
    pub elapsed: Duration,
    pub scale: f32,
}

impl<'a> Painter<'a> {
    fn node(&mut self, node: &'a Node, inherited_clip: Option<Bounds>) -> std::io::Result<()> {
        let pushed = node.id.as_ref().inspect(|id| self.path.push(id));
        let candidate = pushed.map(|_| region_id(&self.path));
        let result = self.visit(node, candidate, inherited_clip);
        if pushed.is_some() {
            self.path.pop();
        }
        result
    }

    fn visit(
        &mut self,
        node: &'a Node,
        candidate: Option<u64>,
        inherited_clip: Option<Bounds>,
    ) -> std::io::Result<()> {
        let bounds = node.bounds;

        // Clipping is per element: a scrolled list clips its children without clipping itself.
        let clip = match node.layout.overflow {
            crate::vui::layout::OverflowSpec::Visible => inherited_clip,
            crate::vui::layout::OverflowSpec::Hidden => Some(bounds),
        };
        // A node entirely outside its clip is not painted, and does not become a region: a
        // region the host could reach for a row scrolled out of sight would be a hit target for
        // something nobody can see.
        if clip.is_some_and(|clip| !overlaps(clip, bounds)) {
            return Ok(());
        }

        // An element with no area is not a hit target and cannot be drawn, so it is not a region
        // either. It is still laid out and still has an id — a collapsed panel keeps its place in
        // the tree — but nothing the host can reach, and a rectangle with no extent is not a
        // shape the wire carries in the first place.
        let mut region = None;
        if bounds.width().0 > 0.
            && bounds.height().0 > 0.
            && let (Some(candidate), Some(id)) = (candidate, &node.id)
        {
            match self.regions.get(&candidate) {
                Some(existing) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!(
                            "two elements share the id path ending in {:?}: {:?} and {:?}",
                            id.as_str(),
                            existing.as_str(),
                            id.as_str()
                        ),
                    ));
                }
                None => {
                    self.regions.insert(candidate, id.clone());
                    region = Some(candidate);
                }
            }
        }

        let state = region
            .and_then(|region| self.states.get(&region).copied())
            .unwrap_or_default();
        let style = node.interactions.resolve_style(node.style.clone(), state);

        if let (Some(handle), Some(region)) = (&node.focus, region) {
            self.focus_stops.push((handle.clone(), region));
        }

        let opacity = style.opacity.filter(|opacity| *opacity < 1.);
        let grouped = clip != inherited_clip || opacity.is_some();
        if grouped {
            self.canvas.push(Command::Save).map_err(limit)?;
            if clip != inherited_clip {
                let clip = clip.unwrap_or(bounds);
                self.canvas
                    .push(Command::Clip(path_for(clip, UiCorners::ZERO)?))
                    .map_err(limit)?;
            }
            if let Some(opacity) = opacity {
                let scaled = (opacity.clamp(0., 1.) * 65535.).round() as u16;
                self.canvas.push(Command::Opacity(scaled)).map_err(limit)?;
            }
        }

        self.body(node, &style, bounds, region, state)?;
        // Floating children paint after the ones in flow, so a popover covers what it overlaps.
        // Layout is unaffected: a deferred child is positioned absolutely anyway.
        for child in node.children.iter().filter(|child| !child.deferred) {
            self.node(child, clip)?;
        }
        for child in node.children.iter().filter(|child| child.deferred) {
            self.node(child, clip)?;
        }

        if grouped {
            self.canvas.push(Command::Restore).map_err(limit)?;
        }
        Ok(())
    }

    /// The box an element is: its shadows, its background, and its border.
    ///
    /// Every element that has a box paints it here, so styling one is styling all of them. An
    /// element whose *content* is something else — a picture, a drawing — paints this first and
    /// then its content over it, which is what a border around a picture means.
    fn box_style(&mut self, style: &Style, bounds: Bounds) -> std::io::Result<()> {
        if bounds.width().0 <= 0. || bounds.height().0 <= 0. {
            return Ok(());
        }
        let radii = style.rounded.clamp_to(bounds);
        for shadow in &style.shadows {
            if self.dropped_shadows >= MAX_DROPPED_SHADOWS {
                break;
            }
            let wire = Shadow {
                rect: bounds.to_wire()?,
                radii: radii.to_wire()?,
                color: shadow.color,
                offset: shadow.offset.to_wire()?,
                blur: shadow.blur.to_scalar()?,
                spread: shadow.spread.to_scalar()?,
                inset: shadow.inset,
            };
            if self.canvas.push(Command::Shadow(wire)).is_err() {
                // Decoration yields before structure does.
                self.dropped_shadows += 1;
            }
        }
        if let Some(background) = &style.background {
            let path = path_for(bounds, radii)?;
            self.canvas
                .push(Command::Fill(path, brush(background, bounds)?))
                .map_err(limit)?;
        }
        if let Some(border) = &style.border {
            self.border(bounds, radii, border)?;
        }
        Ok(())
    }

    /// One element's own paint, before its children.
    fn body(
        &mut self,
        node: &Node,
        style: &Style,
        bounds: Bounds,
        region: Option<u64>,
        state: ElementState,
    ) -> std::io::Result<()> {
        match &node.kind {
            NodeKind::Text(text) => {
                self.paragraph(text, bounds)?;
            }
            NodeKind::Input(field) => {
                self.field(field, bounds, region, state)?;
            }
            NodeKind::Image(image) => {
                self.box_style(style, bounds)?;
                self.image(image, bounds)?;
            }
            NodeKind::Canvas(spec) => {
                self.box_style(style, bounds)?;
                // A canvas of nothing draws nothing. The closure is skipped rather than run
                // against an empty box, because a drawing that paints a thousand paths should not
                // be paid for by a box that is collapsed, scrolled out, or not yet laid out.
                if bounds.width().0 > 0. && bounds.height().0 > 0. {
                    spec.paint(&mut self.canvas, bounds)?;
                }
            }
            // A list is a box with a clip and some rows in it: the box paints the same way.
            NodeKind::Container | NodeKind::List(_) => {
                self.box_style(style, bounds)?;
            }
        }
        if let Some(region) = region {
            // The whole box is the region, radius and all: hit testing a rounded corner by its
            // box is what every toolkit does, and the corner is a few pixels wide.
            let command = Command::Hit {
                id: region,
                path: path_for(bounds, UiCorners::ZERO)?,
                role: node.interactions.role(),
                cursor: node.interactions.cursor(),
            };
            self.canvas.push(command).map_err(limit)?;
        }
        Ok(())
    }

    /// A text field: its selection, its text, and its caret.
    fn field(
        &mut self,
        field: &crate::vui::text_input::InputSpec,
        bounds: Bounds,
        region: Option<u64>,
        state: ElementState,
    ) -> std::io::Result<()> {
        let text = field.drawn().clone();
        // A field with no box has no caret to draw in; the text still paints, so an empty field
        // is empty rather than missing.
        if bounds.width().0 <= 0. || bounds.height().0 <= 0. {
            self.paragraph(&text, bounds)?;
            return Ok(());
        }
        // The host measured this run, so the caret and the selection are read off its shape
        // rather than estimated from a character count.
        let geometry = self.text.measurement(&text).cloned();
        if let Some(measurement) = &geometry
            && let Some((start, end)) = selection_range(field)
            && let (Some(from), Some(to)) = (
                crate::vui::text_input::caret_geometry(measurement, start),
                crate::vui::text_input::caret_geometry(measurement, end),
            )
        {
            let left = bounds.origin.x.0 + from.0.min(to.0);
            let right = bounds.origin.x.0 + from.0.max(to.0);
            let height = if to.1 == from.1 {
                from.2
            } else {
                // A selection that spans lines is drawn as tall as the text it covers.
                measurement.height.get() as f32
            };
            let rect = Bounds::new(
                Point::new(left, bounds.origin.y.0 + from.1),
                Size::new((right - left).max(1.), height),
            );
            self.canvas
                .push(Command::Fill(
                    path_for(rect, UiCorners::ZERO)?,
                    Brush::Solid(field.selection_color),
                ))
                .map_err(limit)?;
        }

        self.paragraph(&text, bounds)?;

        // The caret is drawn last so it sits over the glyph it belongs to, and only while the
        // field has focus: an unfocused field shows no caret.
        if state.focused
            && let Some(measurement) = &geometry
            && let Some((x, y, line_height)) =
                crate::vui::text_input::caret_geometry(measurement, field.caret)
        {
            let caret = Bounds::new(
                Point::new(bounds.origin.x.0 + x, bounds.origin.y.0 + y),
                Size::new(1.5, line_height),
            );
            self.canvas
                .push(Command::Fill(
                    path_for(caret, UiCorners::ZERO)?,
                    Brush::Solid(field.caret_color),
                ))
                .map_err(limit)?;
            // The host places its candidate window from the caret of the field it is typing in,
            // which is the focused one.
            if self.editor_caret.is_none() && region.is_some() {
                self.editor_caret = Some(caret.to_wire()?);
            }
        }
        Ok(())
    }

    /// Pixels, in the box they were given.
    ///
    /// The background and border of an image element are painted by the container arm; this is
    /// only the picture, so a picture that fails to decode leaves a styled empty box rather than
    /// taking the frame down with it.
    fn image(
        &mut self,
        image: &crate::vui::image::ImageSpec,
        bounds: Bounds,
    ) -> std::io::Result<()> {
        if bounds.width().0 <= 0. || bounds.height().0 <= 0. {
            return Ok(());
        }
        // A vector is rasterized in device pixels, which is the whole reason to keep it a vector:
        // the same document is sharp on a scaled display and a plain one.
        let raster = (
            ((bounds.width().0 * self.clock.scale).ceil() as u32).max(1),
            ((bounds.height().0 * self.clock.scale).ceil() as u32).max(1),
        );

        let (frame, natural, wake) = {
            // A picture whose bytes are not a picture is a picture this frame does not have, and
            // the box it would have been drawn in is already painted. Bytes arriving from
            // somewhere else — a fetch, a file, a side channel — are the ordinary case here, and
            // one bad one is not a reason to end the application. Uploading those bytes is a
            // different matter: that failing is the connection, not the content, and it is still
            // an error.
            let decoded = match self.assets.decoded(&image.source, raster) {
                Ok(decoded) => decoded,
                Err(_) => {
                    self.dropped_images += 1;
                    return Ok(());
                }
            };
            let elapsed = if image.playing {
                self.clock.elapsed
            } else {
                Duration::ZERO
            };
            let frame = decoded.frame_at(elapsed);
            let natural = (decoded.width() as f32, decoded.height() as f32);
            let wake = image
                .playing
                .then(|| decoded.until_next_frame(elapsed))
                .flatten();
            (frame, natural, wake)
        };
        if let Some(wake) = wake {
            // The soonest anything wants is what the loop sleeps until.
            self.wake_after = Some(
                self.wake_after
                    .map_or(wake, |held: Duration| held.min(wake)),
            );
        }

        let asset = self
            .assets
            .upload(self.overlay, &image.source, raster, frame)?;
        let placed = image.fit.place(natural, bounds);
        let opacity = (image.opacity.clamp(0., 1.) * 65535.).round() as u16;
        self.canvas
            .push(Command::Image {
                asset,
                rect: placed.to_wire()?,
                opacity,
            })
            .map_err(limit)?;
        Ok(())
    }

    /// A paragraph: plain when the plain command carries it, shaped when it does not.
    fn paragraph(
        &mut self,
        text: &crate::vui::node::TextSpec,
        bounds: Bounds,
    ) -> std::io::Result<()> {
        let shaped = if crate::vui::text::needs_shape(text) {
            self.text.retain(self.overlay, text, MAX_RETAINED_LAYOUTS)?
        } else {
            None
        };
        if let Some(layout) = shaped {
            self.canvas
                .push(Command::TextLayout {
                    layout,
                    origin: bounds.origin.to_wire()?,
                })
                .map_err(limit)?;
            return Ok(());
        }
        let run = text.runs.first().cloned().unwrap_or_default();
        let wire = vivid_protocol::vector::Text {
            text: run.text.clone(),
            origin: bounds.origin.to_wire()?,
            size: Pixels(run.size).to_scalar()?,
            family: run.family.clone(),
            weight: run.weight,
            italic: run.italic,
            color: run.color,
            max_width: match text.max_width {
                Some(width) => Some(Pixels(width).to_scalar()?),
                None => None,
            },
        };
        self.canvas.push(Command::Text(wire)).map_err(limit)?;
        Ok(())
    }

    fn border(&mut self, bounds: Bounds, radii: UiCorners, border: &Border) -> std::io::Result<()> {
        if border.width.is_zero() {
            return Ok(());
        }
        if border.is_uniform() {
            // A uniform border follows the outline, so a rounded box keeps its corners.
            let path = path_for(bounds, radii)?;
            match border.dash {
                Some(dash) => {
                    let mut style = StrokeStyle::new(border.width.top.0 as f64).map_err(scene)?;
                    style.dashes = vec![dash.to_scalar()?];
                    // `StrokeStyle::new` carries the miter limit the wire wants. This used to
                    // overwrite it with the dash length, which for a border thinner than half a
                    // pixel was below the limit and so a frame the host would refuse.
                    self.canvas
                        .push(Command::StyledStroke(
                            path,
                            Brush::Solid(border.color),
                            style,
                        ))
                        .map_err(limit)?;
                }
                None => {
                    self.canvas
                        .push(Command::Stroke(
                            path,
                            Brush::Solid(border.color),
                            border.width.top.to_scalar()?,
                        ))
                        .map_err(limit)?;
                }
            }
            return Ok(());
        }
        // Per-side widths are four rectangles drawn inside the box: exact, and what a divider or
        // an underline is.
        let sides = [
            Bounds::new(
                bounds.origin,
                Size::new(bounds.width().0, border.width.top.0),
            ),
            Bounds::new(
                Point::new(bounds.origin.x.0, bounds.bottom().0 - border.width.bottom.0),
                Size::new(bounds.width().0, border.width.bottom.0),
            ),
            Bounds::new(
                bounds.origin,
                Size::new(border.width.left.0, bounds.height().0),
            ),
            Bounds::new(
                Point::new(bounds.right().0 - border.width.right.0, bounds.origin.y.0),
                Size::new(border.width.right.0, bounds.height().0),
            ),
        ];
        for side in sides {
            if side.width().0 > 0. && side.height().0 > 0. {
                self.canvas
                    .push(Command::Fill(
                        path_for(side, UiCorners::ZERO)?,
                        Brush::Solid(border.color),
                    ))
                    .map_err(limit)?;
            }
        }
        Ok(())
    }
}

pub(crate) fn path_for(bounds: Bounds, radii: UiCorners) -> std::io::Result<Path> {
    let wire: Rect = bounds.to_wire()?;
    if radii.is_zero() || bounds.width().0 <= 0. || bounds.height().0 <= 0. {
        return Path::rectangle(wire).map_err(scene);
    }
    Path::rounded_rectangle_corners(wire, radii.to_wire()?).map_err(scene)
}

fn brush(background: &Background, bounds: Bounds) -> std::io::Result<Brush> {
    Ok(match background {
        Background::Solid(color) => Brush::Solid(*color),
        Background::Image { asset } => Brush::Image {
            asset: *asset,
            transform: None,
            extend: crate::vui::style::TILED,
        },
        Background::Linear { angle, stops } => {
            let (start, end) = Style::gradient_line(bounds, *angle);
            Brush::Linear {
                start: start.to_wire()?,
                end: end.to_wire()?,
                stops: stops.iter().map(|stop| stop.wire()).collect(),
                color_space: crate::vui::style::GRADIENT_COLOR_SPACE,
            }
        }
    })
}

/// The byte range a field's selection covers, when it has one.
fn selection_range(field: &crate::vui::text_input::InputSpec) -> Option<(usize, usize)> {
    let (start, end) = (field.anchor.min(field.caret), field.anchor.max(field.caret));
    (start != end).then_some((start, end))
}

fn overlaps(a: Bounds, b: Bounds) -> bool {
    a.origin.x < b.right()
        && b.origin.x < a.right()
        && a.origin.y < b.bottom()
        && b.origin.y < a.bottom()
}

fn limit(error: vivid_protocol::vector::InvalidScene) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!("display list limit: the frame is too complex to draw ({error})"),
    )
}

fn scene(error: vivid_protocol::vector::InvalidScene) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!("scene rejected: {error}"),
    )
}
