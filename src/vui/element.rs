//! Elements: what a view builds.
//!
//! A builder collects style into plain data and hands back a [`Node`], so an element is a
//! description rather than a widget. Nothing is retained between frames except the application
//! state the handlers reach, which is what makes a frame reproducible.

use vivid_protocol::vector::{Color, CursorShape};

use crate::vui::focus::FocusHandle;
use crate::vui::geometry::{Corners, Edges, Pixels, Point};
use crate::vui::interactive::{Handler, Modifiers, MouseButton, UiEvent};
use crate::vui::layout::{
    Align, Display, FlexDirectionSpec, Justify, Length, OverflowSpec, PositionSpec, Track,
};
use crate::vui::node::{ElementId, Node, TextRunSpec, TextSpec};
use crate::vui::style::{Background, Border, BoxShadow, GradientStopSpec, Style};
use crate::vui::text::TextAlign;

/// Anything that can become an element in a frame.
pub trait IntoElement {
    fn into_node(self) -> Node;
}

impl<E: IntoElement> IntoElement for Option<E> {
    /// A conditional child: `None` contributes nothing, not even a box.
    fn into_node(self) -> Node {
        match self {
            Some(element) => element.into_node(),
            None => {
                let mut node = Node::container();
                node.layout.display = Display::None;
                node
            }
        }
    }
}

impl IntoElement for Node {
    fn into_node(self) -> Node {
        self
    }
}

impl<E: IntoElement> IntoElement for Vec<E> {
    fn into_node(self) -> Node {
        let mut node = Node::container();
        node.layout.display = Display::None;
        node.children = self.into_iter().map(IntoElement::into_node).collect();
        node
    }
}

/// A box: the element almost everything is made of.
#[derive(Debug)]
pub struct Div {
    node: Node,
}

/// A run of text.
#[derive(Debug)]
pub struct TextEl {
    spec: TextSpec,
    node: Node,
}

/// A box with nothing in it yet.
pub fn div() -> Div {
    Div {
        node: Node::container(),
    }
}

/// A run of text, drawn in the host's default font unless one is named.
pub fn text(content: impl Into<String>) -> TextEl {
    styled_text(TextSpec::plain(content, 16.))
}

/// A paragraph of runs: one box, several styles, wrapped and aligned as one.
pub fn paragraph(runs: impl IntoIterator<Item = TextRunSpec>) -> TextEl {
    let runs = runs.into_iter().collect::<Vec<_>>();
    let spec = TextSpec {
        runs: if runs.is_empty() {
            vec![TextRunSpec::default()]
        } else {
            runs
        },
        ..TextSpec::default()
    };
    styled_text(spec)
}

fn styled_text(spec: TextSpec) -> TextEl {
    TextEl {
        node: Node::text(spec.clone()),
        spec,
    }
}

impl IntoElement for Div {
    fn into_node(self) -> Node {
        self.node
    }
}

impl IntoElement for TextEl {
    fn into_node(mut self) -> Node {
        self.node.kind = crate::vui::node::NodeKind::Text(self.spec);
        self.node
    }
}

impl Div {
    /// Give this element an identity. Anything interactive needs one: hover, press, and focus
    /// are remembered against it from one frame to the next.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.node.id = Some(id.into());
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.node.children.push(child.into_node());
        self
    }

    pub fn children(mut self, children: impl IntoIterator<Item = impl IntoElement>) -> Self {
        self.node
            .children
            .extend(children.into_iter().map(IntoElement::into_node));
        self
    }

    /// Add a child only when the condition holds. The element is still built, so a branch with
    /// side effects should use `if` instead.
    pub fn when(self, condition: bool, child: impl IntoElement) -> Self {
        if condition { self.child(child) } else { self }
    }

    // ---- layout ------------------------------------------------------------------------------

    pub fn flex(mut self) -> Self {
        self.node.layout.display = Display::Flex;
        self.node.layout.direction = FlexDirectionSpec::Row;
        self
    }

    pub fn flex_col(mut self) -> Self {
        self.node.layout.display = Display::Flex;
        self.node.layout.direction = FlexDirectionSpec::Column;
        self
    }

    pub fn flex_row_reverse(mut self) -> Self {
        self.node.layout.display = Display::Flex;
        self.node.layout.direction = FlexDirectionSpec::RowReverse;
        self
    }

    pub fn flex_col_reverse(mut self) -> Self {
        self.node.layout.display = Display::Flex;
        self.node.layout.direction = FlexDirectionSpec::ColumnReverse;
        self
    }

    pub fn grid(mut self) -> Self {
        self.node.layout.display = Display::Grid;
        self
    }

    pub fn block(mut self) -> Self {
        self.node.layout.display = Display::Block;
        self
    }

    pub fn grid_columns(mut self, tracks: impl IntoIterator<Item = Track>) -> Self {
        self.node.layout.display = Display::Grid;
        self.node.layout.grid_columns = tracks.into_iter().collect();
        self
    }

    pub fn grid_rows(mut self, tracks: impl IntoIterator<Item = Track>) -> Self {
        self.node.layout.display = Display::Grid;
        self.node.layout.grid_rows = tracks.into_iter().collect();
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.node.layout.gap = crate::vui::geometry::Size::new(gap, gap);
        self
    }

    pub fn gap_x(mut self, gap: f32) -> Self {
        self.node.layout.gap.width = Pixels(gap);
        self
    }

    pub fn gap_y(mut self, gap: f32) -> Self {
        self.node.layout.gap.height = Pixels(gap);
        self
    }

    pub fn p(mut self, value: f32) -> Self {
        self.node.layout.padding = Edges::uniform(value);
        self
    }

    pub fn px(mut self, value: f32) -> Self {
        let current = self.node.layout.padding;
        self.node.layout.padding = Edges::all(current.top.0, value, current.bottom.0, value);
        self
    }

    pub fn py(mut self, value: f32) -> Self {
        let current = self.node.layout.padding;
        self.node.layout.padding = Edges::all(value, current.right.0, value, current.left.0);
        self
    }

    pub fn pt(mut self, value: f32) -> Self {
        self.node.layout.padding.top = Pixels(value);
        self
    }

    pub fn pr(mut self, value: f32) -> Self {
        self.node.layout.padding.right = Pixels(value);
        self
    }

    pub fn pb(mut self, value: f32) -> Self {
        self.node.layout.padding.bottom = Pixels(value);
        self
    }

    pub fn pl(mut self, value: f32) -> Self {
        self.node.layout.padding.left = Pixels(value);
        self
    }

    pub fn m(mut self, value: f32) -> Self {
        self.node.layout.margin = Edges::uniform(value);
        self
    }

    pub fn mx(mut self, value: f32) -> Self {
        let current = self.node.layout.margin;
        self.node.layout.margin = Edges::all(current.top.0, value, current.bottom.0, value);
        self
    }

    pub fn my(mut self, value: f32) -> Self {
        let current = self.node.layout.margin;
        self.node.layout.margin = Edges::all(value, current.right.0, value, current.left.0);
        self
    }

    pub fn mt(mut self, value: f32) -> Self {
        self.node.layout.margin.top = Pixels(value);
        self
    }

    pub fn ml(mut self, value: f32) -> Self {
        self.node.layout.margin.left = Pixels(value);
        self
    }

    pub fn w(mut self, value: f32) -> Self {
        self.node.layout.width = Length::px(value);
        self
    }

    pub fn h(mut self, value: f32) -> Self {
        self.node.layout.height = Length::px(value);
        self
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.node.layout.width = Length::px(width);
        self.node.layout.height = Length::px(height);
        self
    }

    pub fn w_full(mut self) -> Self {
        self.node.layout.width = Length::full();
        self
    }

    pub fn h_full(mut self) -> Self {
        self.node.layout.height = Length::full();
        self
    }

    pub fn min_w(mut self, value: f32) -> Self {
        self.node.layout.min_width = Length::px(value);
        self
    }

    pub fn min_h(mut self, value: f32) -> Self {
        self.node.layout.min_height = Length::px(value);
        self
    }

    pub fn max_w(mut self, value: f32) -> Self {
        self.node.layout.max_width = Length::px(value);
        self
    }

    pub fn max_h(mut self, value: f32) -> Self {
        self.node.layout.max_height = Length::px(value);
        self
    }

    /// Take the remaining space along the parent's main axis.
    pub fn flex_1(mut self) -> Self {
        self.node.layout.flex_grow = 1.;
        self.node.layout.flex_shrink = 1.;
        self.node.layout.flex_basis = Length::px(0.);
        self
    }

    pub fn flex_grow(mut self) -> Self {
        self.node.layout.flex_grow = 1.;
        self
    }

    /// Keep the element's own size no matter how tight the parent is.
    pub fn flex_none(mut self) -> Self {
        self.node.layout.flex_grow = 0.;
        self.node.layout.flex_shrink = 0.;
        self
    }

    pub fn items_start(mut self) -> Self {
        self.node.layout.align_items = Align::Start;
        self
    }

    pub fn items_center(mut self) -> Self {
        self.node.layout.align_items = Align::Center;
        self
    }

    pub fn items_end(mut self) -> Self {
        self.node.layout.align_items = Align::End;
        self
    }

    pub fn items_stretch(mut self) -> Self {
        self.node.layout.align_items = Align::Stretch;
        self
    }

    pub fn items_baseline(mut self) -> Self {
        self.node.layout.align_items = Align::Baseline;
        self
    }

    pub fn justify_start(mut self) -> Self {
        self.node.layout.justify_content = Justify::Start;
        self
    }

    pub fn justify_center(mut self) -> Self {
        self.node.layout.justify_content = Justify::Center;
        self
    }

    pub fn justify_end(mut self) -> Self {
        self.node.layout.justify_content = Justify::End;
        self
    }

    pub fn justify_between(mut self) -> Self {
        self.node.layout.justify_content = Justify::SpaceBetween;
        self
    }

    pub fn justify_around(mut self) -> Self {
        self.node.layout.justify_content = Justify::SpaceAround;
        self
    }

    pub fn justify_evenly(mut self) -> Self {
        self.node.layout.justify_content = Justify::SpaceEvenly;
        self
    }

    /// Lay this element's floating children out against its own box. It is the default, and it
    /// is worth saying when a parent exists to be an anchor.
    pub fn relative(mut self) -> Self {
        self.node.layout.position = PositionSpec::Relative;
        self
    }

    /// Draw this element after everything else in its parent, so it covers what it overlaps.
    ///
    /// For a popover or a tooltip: it is still laid out in place, and only its paint order
    /// changes.
    pub fn deferred(mut self) -> Self {
        self.node.deferred = true;
        self
    }

    /// Float this element against an edge of its parent, out of the flow.
    ///
    /// The anchor says which corner or edge it hangs from; the offset moves it inwards from
    /// there. A popover under a button is `Anchor::BottomLeft` with a small offset.
    pub fn anchor(mut self, anchor: Anchor, offset: Point) -> Self {
        let mut insets = Insets::default();
        match anchor {
            Anchor::TopLeft => {
                insets.top = Some(offset.y);
                insets.left = Some(offset.x);
            }
            Anchor::TopRight => {
                insets.top = Some(offset.y);
                insets.right = Some(offset.x);
            }
            Anchor::BottomLeft => {
                insets.bottom = Some(offset.y);
                insets.left = Some(offset.x);
            }
            Anchor::BottomRight => {
                insets.bottom = Some(offset.y);
                insets.right = Some(offset.x);
            }
            Anchor::TopCenter => {
                insets.top = Some(offset.y);
            }
            Anchor::BottomCenter => {
                insets.bottom = Some(offset.y);
            }
        }
        self.node.layout.position = PositionSpec::Absolute {
            top: insets.top,
            right: insets.right,
            bottom: insets.bottom,
            left: insets.left,
        };
        self
    }

    pub fn absolute(mut self) -> Self {
        self.node.layout.position = PositionSpec::Absolute {
            top: None,
            right: None,
            bottom: None,
            left: None,
        };
        self
    }

    pub fn top(mut self, value: f32) -> Self {
        self.inset(|inset| inset.top = Some(Pixels(value)));
        self
    }

    pub fn right(mut self, value: f32) -> Self {
        self.inset(|inset| inset.right = Some(Pixels(value)));
        self
    }

    pub fn bottom(mut self, value: f32) -> Self {
        self.inset(|inset| inset.bottom = Some(Pixels(value)));
        self
    }

    pub fn left(mut self, value: f32) -> Self {
        self.inset(|inset| inset.left = Some(Pixels(value)));
        self
    }

    fn inset(&mut self, update: impl FnOnce(&mut Insets)) {
        let mut insets = match self.node.layout.position {
            PositionSpec::Absolute {
                top,
                right,
                bottom,
                left,
            } => Insets {
                top,
                right,
                bottom,
                left,
            },
            PositionSpec::Relative => Insets::default(),
        };
        update(&mut insets);
        self.node.layout.position = PositionSpec::Absolute {
            top: insets.top,
            right: insets.right,
            bottom: insets.bottom,
            left: insets.left,
        };
    }

    /// Clip this element's content, and its hit regions, to its own box.
    pub fn overflow_hidden(mut self) -> Self {
        self.node.layout.overflow = OverflowSpec::Hidden;
        self
    }

    pub fn hidden(mut self) -> Self {
        self.node.layout.display = Display::None;
        self
    }

    // ---- paint -------------------------------------------------------------------------------

    pub fn bg(mut self, color: Color) -> Self {
        self.node.style.background = Some(Background::Solid(color));
        self
    }

    /// A gradient at `angle` degrees: 0 runs left to right, 90 runs top to bottom.
    pub fn bg_gradient(mut self, angle: f32, stops: Vec<GradientStopSpec>) -> Self {
        self.node.style.background = Some(Background::Linear { angle, stops });
        self
    }

    /// A retained image, tiled across the box at its natural size.
    pub fn bg_image(mut self, asset: u64) -> Self {
        self.node.style.background = Some(Background::Image { asset });
        self
    }

    pub fn rounded(mut self, radius: f32) -> Self {
        self.node.style.rounded = Corners::uniform(radius);
        self
    }

    /// A radius that fills the box: clamped to half of the shorter side at paint time.
    pub fn rounded_full(mut self) -> Self {
        self.node.style.rounded = Corners::uniform(1_000_000.);
        self
    }

    pub fn rounded_top(mut self, radius: f32) -> Self {
        self.node.style.rounded.top_left = Pixels(radius);
        self.node.style.rounded.top_right = Pixels(radius);
        self
    }

    pub fn border(mut self, width: f32, color: Color) -> Self {
        self.node.style.border = Some(Border::solid(Pixels(width), color));
        self
    }

    pub fn border_1(mut self) -> Self {
        self.node.style.border = Some(Border::solid(
            Pixels(1.),
            crate::vui::style::rgba(0x000000, 0x40),
        ));
        self
    }

    pub fn border_dashed(mut self, width: f32, color: Color) -> Self {
        self.node.style.border = Some(Border::dashed(Pixels(width), color));
        self
    }

    pub fn border_b(mut self, width: f32, color: Color) -> Self {
        let mut border = self
            .node
            .style
            .border
            .unwrap_or(Border::solid(Pixels::ZERO, color));
        border.width.bottom = Pixels(width);
        border.color = color;
        self.node.style.border = Some(border);
        self
    }

    pub fn border_t(mut self, width: f32, color: Color) -> Self {
        let mut border = self
            .node
            .style
            .border
            .unwrap_or(Border::solid(Pixels::ZERO, color));
        border.width.top = Pixels(width);
        border.color = color;
        self.node.style.border = Some(border);
        self
    }

    pub fn shadow(mut self, shadow: BoxShadow) -> Self {
        self.node.style.shadows.push(shadow);
        self
    }

    pub fn shadow_lg(mut self) -> Self {
        self.node.style.shadows.push(crate::vui::style::shadows::LG);
        self
    }

    pub fn shadow_xs(mut self) -> Self {
        self.node.style.shadows.push(crate::vui::style::shadows::XS);
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.node.style.opacity = Some(opacity);
        self
    }

    // ---- interaction -------------------------------------------------------------------------

    /// The pointer shape while this element is hovered.
    pub fn cursor(mut self, cursor: CursorShape) -> Self {
        self.node.interactions.set_cursor(Some(cursor));
        self
    }

    /// Ask the host to resize the window when this region is dragged.
    ///
    /// `direction` is the protocol's assignment: 0 left, 1 right, 2 up, 3 down, 4 up-left, 5
    /// up-right, 6 down-left, 7 down-right. The resizing is the host's; this only names the edge.
    pub fn resize_region(mut self, direction: u8) -> Self {
        self.node
            .interactions
            .set_role(vivid_protocol::vector::HitRole::Resize(direction));
        self
    }

    /// Ask the host to move the window when this region is dragged, which is what a title bar is.
    ///
    /// The move itself is the host's, like the resize above: this names the handle and nothing
    /// more. It carries no handler — the host is already doing the work — so a title bar needs an
    /// id only if it wants to hear about the drag afterwards.
    pub fn drag_region(mut self) -> Self {
        self.node
            .interactions
            .set_role(vivid_protocol::vector::HitRole::Drag);
        self
    }

    /// Swallow pointer events aimed at what is painted underneath, without reporting anything
    /// itself. This is what a modal scrim is: the elements below keep their appearance and lose
    /// their input.
    pub fn block_hits(mut self) -> Self {
        self.node
            .interactions
            .set_role(vivid_protocol::vector::HitRole::Transparent);
        self
    }

    /// Style while a pointer rests on this element.
    pub fn hover(mut self, style: impl Fn(Style) -> Style + 'static) -> Self {
        self.node.interactions.on_hover_style(style);
        self
    }

    /// Style while a button is held down on this element.
    pub fn active(mut self, style: impl Fn(Style) -> Style + 'static) -> Self {
        self.node.interactions.on_pressed_style(style);
        self
    }

    /// Style while this element has keyboard focus that came from the keyboard.
    pub fn focus_ring(mut self, style: impl Fn(Style) -> Style + 'static) -> Self {
        self.node.interactions.on_focus_visible_style(style);
        self
    }

    /// Make this element a tab stop.
    pub fn track_focus(mut self, handle: &FocusHandle) -> Self {
        self.node.focus = Some(handle.clone());
        if self.node.id.is_none() {
            self.node.id = Some(handle.id().clone());
        }
        self
    }

    pub fn on_click(mut self, handler: Handler) -> Self {
        self.node.interactions.handlers.click = Some(handler);
        self
    }

    pub fn on_mouse_down(mut self, handler: Handler) -> Self {
        self.node.interactions.handlers.mouse_down = Some(handler);
        self
    }

    pub fn on_mouse_up(mut self, handler: Handler) -> Self {
        self.node.interactions.handlers.mouse_up = Some(handler);
        self
    }

    pub fn on_mouse_move(mut self, handler: Handler) -> Self {
        self.node.interactions.handlers.mouse_move = Some(handler);
        self
    }

    pub fn on_hover(mut self, handler: Handler) -> Self {
        self.node.interactions.handlers.hover = Some(handler);
        self
    }

    pub fn on_wheel(mut self, handler: Handler) -> Self {
        self.node.interactions.handlers.wheel = Some(handler);
        self
    }

    pub fn on_key(mut self, handler: Handler) -> Self {
        self.node.interactions.handlers.key = Some(handler);
        self
    }

    pub fn on_text(mut self, handler: Handler) -> Self {
        self.node.interactions.handlers.text = Some(handler);
        self
    }

    /// Pressure from a device that reports it. A host with no such sensor never sends one.
    pub fn on_pressure(mut self, handler: Handler) -> Self {
        self.node.interactions.handlers.pressure = Some(handler);
        self
    }

    /// Handle a named action, which is what a key binding, a menu item, or another element sends.
    ///
    /// The action reaches the focused element first and bubbles outwards from there.
    pub fn on_action<A: crate::vui::keymap::Action>(mut self, handler: Handler) -> Self {
        self.node
            .interactions
            .handlers
            .actions
            .push((A::ID, handler));
        self
    }

    /// Answer a request from assistive technology: a click, an increment, a toggle.
    ///
    /// Only reaches an element that has an identity, because an action is only ever delivered
    /// for a node the live tree still names — and a node without an identity cannot be named.
    pub fn on_accessibility(mut self, handler: Handler) -> Self {
        self.node.interactions.handlers.accessibility = Some(handler);
        self
    }
}

/// Which edge or corner a floating element hangs from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

#[derive(Default)]
struct Insets {
    top: Option<Pixels>,
    right: Option<Pixels>,
    bottom: Option<Pixels>,
    left: Option<Pixels>,
}

impl TextEl {
    /// The size every run is drawn at.
    pub fn size(mut self, size: f32) -> Self {
        self.spec.each_run(|run| run.size = size);
        self
    }

    /// The color every run is drawn in. A run that carries its own color keeps it.
    pub fn color(mut self, color: Color) -> Self {
        self.spec.each_run(|run| run.color = color);
        self
    }

    pub fn weight(mut self, weight: u16) -> Self {
        self.spec.each_run(|run| run.weight = weight);
        self
    }

    pub fn italic(mut self) -> Self {
        self.spec.each_run(|run| run.italic = true);
        self
    }

    pub fn underline(mut self) -> Self {
        self.spec.each_run(|run| run.underline = true);
        self
    }

    pub fn strikethrough(mut self) -> Self {
        self.spec.each_run(|run| run.strikethrough = true);
        self
    }

    /// Extra space between glyphs, in logical pixels. Negative tracking is clamped to none, since
    /// the wire carries spacing as an unsigned length.
    pub fn letter_spacing(mut self, spacing: f32) -> Self {
        self.spec.letter_spacing = spacing;
        self
    }

    /// Extra space between words, in logical pixels. Same ceiling as the letter spacing.
    pub fn word_spacing(mut self, spacing: f32) -> Self {
        self.spec.word_spacing = spacing;
        self
    }

    /// A line box this tall, rather than the one the font would choose.
    ///
    /// This is the line height a paragraph is *measured* at as well as drawn at, so a column of
    /// text set on a 24 pixel rhythm is 24 pixels apart in the layout too, not only on screen.
    pub fn line_height(mut self, height: f32) -> Self {
        self.spec.line_height = Some(height);
        self
    }

    /// Turn the font's ligatures off, which is what a monospaced code sample usually wants.
    pub fn no_ligatures(mut self) -> Self {
        self.spec.ligatures = false;
        self
    }

    /// Turn the font's kerning off.
    pub fn no_kerning(mut self) -> Self {
        self.spec.kerning = false;
        self
    }

    /// The family every run is drawn in. Empty means the host's own default.
    pub fn family(mut self, family: impl Into<String>) -> Self {
        let family = family.into();
        self.spec.each_run(|run| run.family = family.clone());
        self
    }

    /// The width the paragraph is laid out in, which is also the width it wraps at.
    pub fn max_width(mut self, width: f32) -> Self {
        self.spec.max_width = Some(width);
        self
    }

    /// Break lines at `max_width` instead of running off the edge.
    pub fn wrap(mut self) -> Self {
        self.spec.wrap = true;
        self
    }

    /// Cut the text to `lines` lines and mark the cut with an ellipsis.
    ///
    /// Truncation is a wrapping paragraph with a ceiling, so this asks for wrapping too: a run
    /// that did not wrap would be one line however many it was allowed.
    pub fn ellipsis(mut self, lines: u16) -> Self {
        self.spec.ellipsis = true;
        self.spec.wrap = true;
        self.spec.max_lines = Some(lines);
        self
    }

    /// Stop after `lines` lines without a marker.
    pub fn max_lines(mut self, lines: u16) -> Self {
        self.spec.max_lines = Some(lines);
        self
    }

    pub fn align(mut self, align: TextAlign) -> Self {
        self.spec.align = align;
        self
    }

    /// Give the run an identity, which a text element needs only if something listens to it.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.node.id = Some(id.into());
        self
    }

    pub fn on_click(mut self, handler: Handler) -> Self {
        self.node.interactions.handlers.click = Some(handler);
        self
    }

    pub fn cursor(mut self, cursor: CursorShape) -> Self {
        self.node.interactions.set_cursor(Some(cursor));
        self
    }

    /// A pointer resting on the text, for a link.
    pub fn hover(mut self, style: impl Fn(Style) -> Style + 'static) -> Self {
        self.node.interactions.on_hover_style(style);
        self
    }
}

impl crate::vui::a11y::Semantic for Div {
    fn accessibility_mut(&mut self) -> &mut crate::vui::a11y::Accessibility {
        &mut self.node.accessibility
    }
}

impl crate::vui::a11y::Semantic for TextEl {
    fn accessibility_mut(&mut self) -> &mut crate::vui::a11y::Accessibility {
        &mut self.node.accessibility
    }
}

/// Convenience for handlers that only care about a pointer position.
pub fn point_of(event: &UiEvent) -> Option<Point> {
    event.position()
}

/// Convenience for handlers that care whether a modifier was held.
pub fn modifiers_of(event: &UiEvent) -> Modifiers {
    match event {
        UiEvent::Click { modifiers, .. }
        | UiEvent::MouseDown { modifiers, .. }
        | UiEvent::MouseUp { modifiers, .. }
        | UiEvent::Key { modifiers, .. } => *modifiers,
        _ => Modifiers::NONE,
    }
}

/// Convenience for handlers that care which button was used.
pub fn button_of(event: &UiEvent) -> Option<MouseButton> {
    match event {
        UiEvent::MouseDown { button, .. } | UiEvent::MouseUp { button, .. } => Some(*button),
        _ => None,
    }
}
