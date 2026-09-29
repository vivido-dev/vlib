//! The element tree a frame is built from.
//!
//! An element is a builder that materializes into a [`Node`]. Building a tree of plain data
//! rather than keeping boxed trait objects means layout, painting, and input dispatch all walk
//! the same structure with no borrow choreography, and a frame can be inspected in a test
//! without a window behind it.

use vivid_protocol::vector::Color;

use crate::vui::focus::FocusHandle;
use crate::vui::geometry::Bounds;
use crate::vui::interactive::Interactions;
use crate::vui::layout::LayoutStyle;
use crate::vui::style::Style;

/// The identity of one element, stable across frames.
///
/// GPUI requires an explicit id on anything stateful for the same reason: hover, press, and focus
/// belong to an element the frame after they happened, and an index into a rebuilt tree is not an
/// identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ElementId(String);

impl ElementId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ElementId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for ElementId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// The region number a path of element ids maps to, as the host echoes it back.
///
/// FNV-1a, so the same path always produces the same number in every process, and two different
/// paths almost never collide. A collision is caught when the regions are collected, not
/// silently merged.
pub fn region_id(path: &[&ElementId]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for (index, id) in path.iter().enumerate() {
        if index > 0 {
            // A separator, so ("ab", "c") and ("a", "bc") are not the same region.
            hash ^= 0x1f;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        for byte in id.as_str().as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    // Zero means "the whole window" to the protocol, so it is never a region.
    hash | 1
}

/// One styling run of a paragraph.
///
/// A run carries everything the host needs to draw it: size, weight, slant, family, color, and
/// the two decorations. Cutting text into runs is what makes a line of mixed emphasis one
/// paragraph instead of several boxes.
#[derive(Debug, Clone, PartialEq)]
pub struct TextRunSpec {
    pub text: String,
    pub size: f32,
    pub weight: u16,
    pub family: String,
    pub italic: bool,
    pub color: Color,
    pub underline: bool,
    pub strikethrough: bool,
}

impl Default for TextRunSpec {
    fn default() -> Self {
        Self {
            text: String::new(),
            size: 16.,
            weight: 400,
            family: String::new(),
            italic: false,
            color: crate::vui::style::Colors::WHITE,
            underline: false,
            strikethrough: false,
        }
    }
}

/// A paragraph: one or more runs, and how the paragraph as a whole is laid out.
#[derive(Debug, Clone, PartialEq)]
pub struct TextSpec {
    pub runs: Vec<TextRunSpec>,
    /// Break lines at the available width. Needs `max_width`, since a paragraph cannot wrap to a
    /// width nobody named.
    pub wrap: bool,
    pub max_width: Option<f32>,
    /// Draw an ellipsis where the text was cut. Needs `max_width`.
    pub ellipsis: bool,
    /// Stop after this many lines, clipping the rest.
    pub max_lines: Option<u16>,
    pub align: crate::vui::text::TextAlign,
    /// Extra space between glyphs, in logical pixels. `0.` is the font's own spacing.
    ///
    /// Something to be aware of: `overlay-typography-v1` carries it as an unsigned length, so a
    /// negative value — tighter tracking — is clamped to none rather than sent.
    pub letter_spacing: f32,
    /// Extra space between words, in logical pixels. Same ceiling as the letter spacing.
    pub word_spacing: f32,
    /// A line box this tall, in logical pixels, rather than the font's own. `None` is the font's.
    pub line_height: Option<f32>,
    /// Whether the font may use its ligatures. On by default, as a browser does.
    pub ligatures: bool,
    /// Whether the font may adjust spacing between glyph pairs. On by default.
    pub kerning: bool,
}

impl Default for TextSpec {
    fn default() -> Self {
        Self {
            runs: vec![TextRunSpec::default()],
            wrap: false,
            max_width: None,
            ellipsis: false,
            max_lines: None,
            align: crate::vui::text::TextAlign::Start,
            letter_spacing: 0.,
            word_spacing: 0.,
            line_height: None,
            ligatures: true,
            kerning: true,
        }
    }
}

impl TextSpec {
    /// One run with default styling, at `size`.
    pub fn plain(text: impl Into<String>, size: f32) -> Self {
        Self {
            runs: vec![TextRunSpec {
                text: text.into(),
                size,
                ..TextRunSpec::default()
            }],
            ..Self::default()
        }
    }

    /// The height of one line, which is what a run that nobody measured falls back to.
    pub(crate) fn line_height(&self) -> f32 {
        match self.line_height {
            Some(height) => height,
            None => self.runs.first().map_or(16., |run| run.size * 1.2),
        }
    }

    /// Whether anything here is typography the plain text command cannot carry.
    ///
    /// `Command::Text` is a run with no spacing, no line height, and no font features, so an
    /// element that asks for any of those has to be drawn through a retained layout or the
    /// request is silently dropped. This is the question `needs_shape` asks.
    pub(crate) fn carries_typography(&self) -> bool {
        self.letter_spacing != 0.
            || self.word_spacing != 0.
            || self.line_height.is_some()
            || !self.ligatures
            || !self.kerning
    }

    /// Every run, for the element-level setters.
    pub(crate) fn each_run(&mut self, update: impl Fn(&mut TextRunSpec)) {
        for run in &mut self.runs {
            update(run);
        }
    }
}

/// What a node draws, once it has been laid out.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum NodeKind {
    /// A box: its background, border, and shadows, with children painted inside it.
    Container,
    /// A run of text, positioned at the node's origin.
    Text(TextSpec),
    /// A text field: the paragraph, and the caret and selection drawn over it.
    Input(crate::vui::text_input::InputSpec),
    /// A virtualized list, whose rows are built after it has been laid out.
    List(crate::vui::list::ListSpec),
    /// Pixels: decoded here, uploaded once, drawn into the box.
    Image(crate::vui::image::ImageSpec),
    /// A drawing of the view's own: paths painted into the box once it has been laid out.
    Canvas(crate::vui::canvas::CanvasSpec),
}

/// One element in one frame.
#[derive(Debug)]
pub struct Node {
    pub(crate) kind: NodeKind,
    pub(crate) layout: LayoutStyle,
    pub(crate) style: Style,
    /// Filled by layout, in window-local logical pixels.
    pub(crate) bounds: Bounds,
    pub(crate) id: Option<ElementId>,
    pub(crate) interactions: Interactions,
    pub(crate) focus: Option<FocusHandle>,
    /// What this element tells assistive technology about itself. Empty for most of them.
    pub(crate) accessibility: crate::vui::a11y::Accessibility,
    /// Draw this node after its siblings, which is what a popover needs: it is laid out where it
    /// belongs and painted over everything it overlaps.
    pub(crate) deferred: bool,
    pub(crate) children: Vec<Node>,
}

impl Node {
    pub(crate) fn container() -> Self {
        Self {
            kind: NodeKind::Container,
            layout: LayoutStyle::default(),
            style: Style::default(),
            bounds: Bounds::ZERO,
            id: None,
            interactions: Interactions::default(),
            focus: None,
            accessibility: crate::vui::a11y::Accessibility::default(),
            deferred: false,
            children: Vec::new(),
        }
    }

    pub(crate) fn text(spec: TextSpec) -> Self {
        Self {
            kind: NodeKind::Text(spec),
            ..Self::container()
        }
    }

    pub(crate) fn image(spec: crate::vui::image::ImageSpec) -> Self {
        Self {
            kind: NodeKind::Image(spec),
            ..Self::container()
        }
    }

    /// What this node draws pixels from, if it draws any.
    pub(crate) fn image_spec(&self) -> Option<&crate::vui::image::ImageSpec> {
        match &self.kind {
            NodeKind::Image(spec) => Some(spec),
            _ => None,
        }
    }

    pub(crate) fn image_mut(&mut self) -> &mut crate::vui::image::ImageSpec {
        match &mut self.kind {
            NodeKind::Image(spec) => spec,
            _ => unreachable!("only an image carries image state"),
        }
    }

    pub(crate) fn canvas(spec: crate::vui::canvas::CanvasSpec) -> Self {
        Self {
            kind: NodeKind::Canvas(spec),
            ..Self::container()
        }
    }

    pub(crate) fn list(spec: crate::vui::list::ListSpec) -> Self {
        Self {
            kind: NodeKind::List(spec),
            ..Self::container()
        }
    }

    pub(crate) fn input(spec: crate::vui::text_input::InputSpec) -> Self {
        Self {
            kind: NodeKind::Input(spec),
            ..Self::container()
        }
    }

    /// The field state of this node, if it is a field.
    pub(crate) fn input_spec(&self) -> Option<&crate::vui::text_input::InputSpec> {
        match &self.kind {
            NodeKind::Input(spec) => Some(spec),
            _ => None,
        }
    }

    /// The field state of this node. Only a text field carries one.
    pub(crate) fn input_mut(&mut self) -> &mut crate::vui::text_input::InputSpec {
        match &mut self.kind {
            NodeKind::Input(spec) => spec,
            _ => unreachable!("only a text field carries field state"),
        }
    }

    /// The paragraph this node draws, if it draws one.
    pub(crate) fn paragraph(&self) -> Option<&TextSpec> {
        match &self.kind {
            NodeKind::Text(spec) => Some(spec),
            NodeKind::Input(spec) => Some(spec.drawn()),
            // A canvas draws paths, not glyphs, so there is nothing here to measure: it is sized
            // by its box and not by its content.
            NodeKind::Container | NodeKind::List(_) | NodeKind::Image(_) | NodeKind::Canvas(_) => {
                None
            }
        }
    }

    /// Visit every node in paint order, depth first.
    pub(crate) fn walk(&self, visit: &mut dyn FnMut(&Node)) {
        visit(self);
        for child in &self.children {
            child.walk(visit);
        }
    }
}
