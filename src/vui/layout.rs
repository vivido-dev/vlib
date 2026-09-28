//! Layout: flex and grid, backed by taffy.
//!
//! The engine is taffy rather than a hand-rolled solver because GPUI uses taffy, so an element
//! tree ported from a GPUI example lays out the same way here. This module is the only place
//! taffy is named: elements see [`LayoutStyle`], and the result is a [`Bounds`] written onto each
//! node in window-local logical pixels.

use std::io;

use taffy::geometry::{Point as TaffyPoint, Size as TaffySize};
use taffy::style::{
    AlignContent, AlignItems, AvailableSpace, Dimension, Display as TaffyDisplay, FlexDirection,
    GridTemplateComponent, LengthPercentage, LengthPercentageAuto, Overflow, Position,
    Style as TaffyStyle, TrackSizingFunction,
};
use taffy::{NodeId as TaffyNode, TaffyTree};

use crate::vui::geometry::{Bounds, Edges, Pixels, Point, Size};
use crate::vui::node::Node;

/// Which layout algorithm arranges a node's children.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Display {
    #[default]
    Flex,
    Grid,
    /// Normal flow: children stack vertically and stretch across the line.
    Block,
    /// The node is not laid out at all.
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexDirectionSpec {
    #[default]
    Row,
    Column,
    RowReverse,
    ColumnReverse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
    Stretch,
    Baseline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// A length an element can ask for. `Auto` defers to content.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Length {
    #[default]
    Auto,
    Px(Pixels),
    /// A fraction of the parent's size on that axis.
    Percent(f32),
}

impl Length {
    pub const fn px(value: f32) -> Self {
        Self::Px(Pixels(value))
    }

    pub const fn full() -> Self {
        Self::Percent(1.)
    }

    fn dimension(self) -> Dimension {
        match self {
            Self::Auto => Dimension::auto(),
            Self::Px(Pixels(value)) => Dimension::length(value),
            Self::Percent(fraction) => Dimension::percent(fraction),
        }
    }

    /// A flex basis takes a full dimension, since `auto` means "content" there.
    fn basis(self) -> Dimension {
        self.dimension()
    }
}

/// One grid track, as `grid-template-columns` and `grid-template-rows` express it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Track {
    /// A share of the free space, as `1fr` is.
    Fraction(f32),
    Fixed(Pixels),
    Auto,
}

impl Track {
    fn sizing(self) -> TrackSizingFunction {
        use taffy::style_helpers as helpers;
        match self {
            Self::Fraction(fraction) => helpers::fr(fraction),
            Self::Fixed(Pixels(value)) => helpers::length(value),
            Self::Auto => helpers::auto(),
        }
    }

    fn component(self) -> GridTemplateComponent<String> {
        GridTemplateComponent::Single(self.sizing())
    }

    fn components(tracks: &[Track]) -> Vec<GridTemplateComponent<String>> {
        tracks.iter().copied().map(Track::component).collect()
    }
}

/// Where a node sits relative to normal flow.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum PositionSpec {
    #[default]
    Relative,
    /// Out of flow, positioned against the containing block's padding box. `None` leaves an
    /// edge to the engine, which is what centering an overlay wants.
    Absolute {
        top: Option<Pixels>,
        right: Option<Pixels>,
        bottom: Option<Pixels>,
        left: Option<Pixels>,
    },
}

/// What happens to content that does not fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverflowSpec {
    #[default]
    Visible,
    /// Clipped, and clipped for hit testing too: the host will not deliver a pointer event to a
    /// region this node's clip excludes.
    Hidden,
}

/// The layout half of an element's style.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutStyle {
    pub display: Display,
    pub direction: FlexDirectionSpec,
    pub gap: Size,
    pub padding: Edges,
    pub margin: Edges,
    pub width: Length,
    pub height: Length,
    pub min_width: Length,
    pub min_height: Length,
    pub max_width: Length,
    pub max_height: Length,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: Length,
    pub align_items: Align,
    pub justify_content: Justify,
    pub position: PositionSpec,
    pub overflow: OverflowSpec,
    pub grid_columns: Vec<Track>,
    pub grid_rows: Vec<Track>,
}

impl Default for LayoutStyle {
    fn default() -> Self {
        Self {
            display: Display::Flex,
            direction: FlexDirectionSpec::Row,
            gap: Size::ZERO,
            padding: Edges::ZERO,
            margin: Edges::ZERO,
            width: Length::Auto,
            height: Length::Auto,
            min_width: Length::Auto,
            min_height: Length::Auto,
            max_width: Length::Auto,
            max_height: Length::Auto,
            // A flex item that shrinks by default is what CSS does and what a row of elements
            // expects; growing is opt-in.
            flex_grow: 0.,
            flex_shrink: 1.,
            flex_basis: Length::Auto,
            align_items: Align::Stretch,
            justify_content: Justify::Start,
            position: PositionSpec::Relative,
            overflow: OverflowSpec::Visible,
            grid_columns: Vec::new(),
            grid_rows: Vec::new(),
        }
    }
}

impl LayoutStyle {
    fn to_taffy(&self) -> TaffyStyle {
        let position = match self.position {
            PositionSpec::Relative => Position::Relative,
            PositionSpec::Absolute { .. } => Position::Absolute,
        };
        let edge = |value: Option<Pixels>| match value {
            Some(Pixels(value)) => LengthPercentageAuto::length(value),
            None => LengthPercentageAuto::auto(),
        };
        let inset = match self.position {
            PositionSpec::Relative => taffy::geometry::Rect::zero(),
            PositionSpec::Absolute {
                top,
                right,
                bottom,
                left,
            } => taffy::geometry::Rect {
                top: edge(top),
                right: edge(right),
                bottom: edge(bottom),
                left: edge(left),
            },
        };
        TaffyStyle {
            display: match self.display {
                Display::Flex => TaffyDisplay::Flex,
                Display::Grid => TaffyDisplay::Grid,
                Display::Block => TaffyDisplay::Block,
                Display::None => TaffyDisplay::None,
            },
            position,
            inset,
            size: TaffySize {
                width: self.width.dimension(),
                height: self.height.dimension(),
            },
            min_size: TaffySize {
                width: self.min_width.dimension(),
                height: self.min_height.dimension(),
            },
            max_size: TaffySize {
                width: self.max_width.dimension(),
                height: self.max_height.dimension(),
            },
            margin: self.margin.map(LengthPercentageAuto::length),
            padding: self.padding.map(LengthPercentage::length),
            border: taffy::geometry::Rect::zero(),
            gap: TaffySize {
                width: LengthPercentage::length(self.gap.width.0),
                height: LengthPercentage::length(self.gap.height.0),
            },
            flex_direction: match self.direction {
                FlexDirectionSpec::Row => FlexDirection::Row,
                FlexDirectionSpec::Column => FlexDirection::Column,
                FlexDirectionSpec::RowReverse => FlexDirection::RowReverse,
                FlexDirectionSpec::ColumnReverse => FlexDirection::ColumnReverse,
            },
            flex_grow: self.flex_grow,
            flex_shrink: self.flex_shrink,
            flex_basis: self.flex_basis.basis(),
            // Stretch is expressed as a keyword rather than left to taffy's `None`: the two
            // mean the same thing to the engine, and saying it makes the style in a frame
            // readable.
            align_items: Some(match self.align_items {
                Align::Start => AlignItems::START,
                Align::Center => AlignItems::CENTER,
                Align::End => AlignItems::END,
                Align::Stretch => AlignItems::STRETCH,
                Align::Baseline => AlignItems::BASELINE,
            }),
            justify_content: Some(match self.justify_content {
                Justify::Start => AlignContent::START,
                Justify::Center => AlignContent::CENTER,
                Justify::End => AlignContent::END,
                Justify::SpaceBetween => AlignContent::SPACE_BETWEEN,
                Justify::SpaceAround => AlignContent::SPACE_AROUND,
                Justify::SpaceEvenly => AlignContent::SPACE_EVENLY,
            }),
            overflow: match self.overflow {
                // `Clip` differs from `Hidden` only in whether the box is scrollable; nothing
                // here scrolls by itself, so both mean the same thing to the engine.
                OverflowSpec::Visible => TaffyPoint {
                    x: Overflow::Visible,
                    y: Overflow::Visible,
                },
                OverflowSpec::Hidden => TaffyPoint {
                    x: Overflow::Hidden,
                    y: Overflow::Hidden,
                },
            },
            grid_template_columns: Track::components(&self.grid_columns),
            grid_template_rows: Track::components(&self.grid_rows),
            align_content: Some(AlignContent::START),
            ..Default::default()
        }
    }
}

/// Per-edge values in taffy's rect shape, from our own per-side vocabulary.
trait EdgesToTaffy<T> {
    fn map(self, f: impl Fn(f32) -> T) -> taffy::geometry::Rect<T>;
}

impl<T> EdgesToTaffy<T> for Edges {
    fn map(self, f: impl Fn(f32) -> T) -> taffy::geometry::Rect<T> {
        taffy::geometry::Rect {
            left: f(self.left.0),
            right: f(self.right.0),
            top: f(self.top.0),
            bottom: f(self.bottom.0),
        }
    }
}

fn invalid(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

/// Lay out a tree against a viewport, writing each node's absolute bounds into it.
///
/// `measure` answers for a text node's natural size. It is called once per text node, before the
/// engine runs, so a caller can batch every measurement it owes the host into a single round trip
/// instead of blocking inside layout.
pub(crate) fn compute(
    root: &mut Node,
    viewport: Size,
    measure: &mut dyn FnMut(&Node) -> Option<Size>,
) -> io::Result<()> {
    let mut tree: TaffyTree<()> = TaffyTree::new();

    // Depth-first so a parent's taffy node is created before its children, and every node's
    // index here is also its taffy `NodeId`.
    struct Flat {
        parent: Option<usize>,
        children: Vec<usize>,
    }
    let mut flat: Vec<Flat> = Vec::new();
    let mut taffy_nodes: Vec<TaffyNode> = Vec::new();

    fn walk(
        node: &Node,
        parent: Option<usize>,
        tree: &mut TaffyTree<()>,
        flat: &mut Vec<Flat>,
        taffy_nodes: &mut Vec<TaffyNode>,
        measure: &mut dyn FnMut(&Node) -> Option<Size>,
    ) -> io::Result<usize> {
        let index = flat.len();
        flat.push(Flat {
            parent,
            children: Vec::new(),
        });
        let mut style = node.layout.to_taffy();
        if let Some(measured) = measure(node) {
            // A leaf that knows its own size — a measured run of text, an image of known
            // dimensions — says so here rather than answering a callback. That is what keeps a
            // deep tree linear: taffy asks a callback for a leaf's size on every pass, and the
            // passes through nested auto-sized containers multiply.
            //
            // Only an axis nobody named is filled in: an element told how wide to be is that
            // wide, whatever its content would prefer.
            if node.layout.width == Length::Auto {
                style.size.width = Dimension::length(measured.width.0);
            }
            if node.layout.height == Length::Auto {
                style.size.height = Dimension::length(measured.height.0);
            }
        }
        let id = tree
            .new_leaf(style)
            .map_err(|error| invalid(format!("layout node rejected: {error}")))?;
        taffy_nodes.push(id);

        for child in &node.children {
            let child_index = walk(child, Some(index), tree, flat, taffy_nodes, measure)?;
            flat[index].children.push(child_index);
            flat[child_index].parent = Some(index);
        }
        if !flat[index].children.is_empty() {
            let children: Vec<TaffyNode> = flat[index]
                .children
                .iter()
                .map(|child| taffy_nodes[*child])
                .collect();
            tree.set_children(taffy_nodes[index], &children)
                .map_err(|error| invalid(format!("layout children rejected: {error}")))?;
        }
        Ok(index)
    }
    walk(root, None, &mut tree, &mut flat, &mut taffy_nodes, measure)?;

    let available = TaffySize {
        width: AvailableSpace::Definite(viewport.width.0),
        height: AvailableSpace::Definite(viewport.height.0),
    };
    tree.compute_layout(taffy_nodes[0], available)
        .map_err(|error| invalid(format!("layout failed: {error}")))?;

    // taffy reports a node's location relative to its parent, so accumulate down the tree.
    let mut absolute: Vec<Bounds> = vec![Bounds::ZERO; flat.len()];
    for (index, node) in flat.iter().enumerate() {
        let layout = tree
            .layout(taffy_nodes[index])
            .map_err(|error| invalid(format!("layout result missing: {error}")))?;
        let origin = match node.parent {
            None => Point::new(layout.location.x, layout.location.y),
            Some(parent) => {
                absolute[parent].origin + Point::new(layout.location.x, layout.location.y)
            }
        };
        absolute[index] = Bounds::new(origin, Size::new(layout.size.width, layout.size.height));
    }

    let mut cursor = 0;
    write_bounds(root, &mut cursor, &absolute);
    Ok(())
}

/// Apply the computed bounds in the same depth-first order the flattening used.
fn write_bounds(node: &mut Node, cursor: &mut usize, absolute: &[Bounds]) {
    let index = *cursor;
    *cursor += 1;
    node.bounds = absolute.get(index).copied().unwrap_or_default();
    for child in &mut node.children {
        write_bounds(child, cursor, absolute);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vui::node::TextSpec;

    /// A text node big enough to see, measured without a host.
    fn text(label: &str) -> Node {
        Node::text(TextSpec::plain(label, 16.))
    }

    fn measured(node: &Node) -> Option<Size> {
        // Ten logical pixels per character: arithmetic a test can predict.
        let text = node.paragraph()?;
        let run = text.runs.first().cloned().unwrap_or_default();
        Some(Size::new(run.text.len() as f32 * 10., run.size))
    }

    fn lay_out(root: &mut Node) -> Size {
        let viewport = Size::new(400., 300.);
        compute(root, viewport, &mut measured).expect("layout");
        root.bounds.size
    }

    #[test]
    fn a_deep_chain_that_says_how_wide_it_is_lays_out_all_the_way_down() {
        // A container that is auto-sized on the cross axis asks the engine for a content size at
        // every level, and that question compounds per level. A container that knows its width
        // does not, which is why a deep tree gives each level one.
        let mut root = Node::container();
        root.layout.direction = FlexDirectionSpec::Column;
        root.layout.width = Length::px(400.);
        let mut current = &mut root;
        for level in 0..64 {
            current.children.push(text(&format!("{level}")));
            current.children.push(Node::container());
            current = current.children.last_mut().unwrap();
            current.layout.direction = FlexDirectionSpec::Column;
            current.layout.padding = Edges::uniform(2.);
            current.layout.width = Length::full();
        }
        let started = std::time::Instant::now();
        lay_out(&mut root);
        let elapsed = started.elapsed();

        // Every level is four logical pixels narrower than the one above it, and the innermost
        // box is a real box, not a collapsed one.
        let mut node = &root;
        for level in 0..64 {
            node = &node.children[1];
            assert_eq!(node.bounds.size.width.get(), 400. - level as f32 * 4.);
        }
        assert_eq!(node.bounds.size.width.get(), 400. - 63. * 4.);

        // This is not a benchmark, but a chain whose cost compounds would not finish at all.
        assert!(
            elapsed < std::time::Duration::from_secs(5),
            "64 levels took {elapsed:?}"
        );
    }

    #[test]
    fn a_column_stacks_children_and_a_row_places_them_side_by_side() {
        let mut column = Node::container();
        column.layout.direction = FlexDirectionSpec::Column;
        column.children.push(text("ab"));
        column.children.push(text("cdef"));
        lay_out(&mut column);
        assert_eq!(column.children[0].bounds.origin.y.get(), 0.);
        assert_eq!(column.children[0].bounds.height().get(), 16.);
        assert_eq!(column.children[1].bounds.origin.y.get(), 16.);
        assert_eq!(column.children[1].bounds.size.width.get(), 40.);

        let mut row = Node::container();
        row.layout.direction = FlexDirectionSpec::Row;
        row.children.push(text("ab"));
        row.children.push(text("cdef"));
        lay_out(&mut row);
        assert_eq!(row.children[1].bounds.origin.x.get(), 20.);
        assert_eq!(row.children[1].bounds.origin.y.get(), 0.);
    }

    #[test]
    fn gap_and_padding_move_children_without_moving_the_box() {
        let mut node = Node::container();
        node.layout.direction = FlexDirectionSpec::Column;
        node.layout.padding = Edges::uniform(5.);
        node.layout.gap = Size::new(0., 4.);
        node.layout.width = Length::px(50.);
        node.layout.height = Length::px(60.);
        node.children.push(text("a"));
        node.children.push(text("b"));
        lay_out(&mut node);
        assert_eq!(node.bounds.size.width.get(), 50.);
        assert_eq!(node.children[0].bounds.origin.x.get(), 5.);
        assert_eq!(node.children[1].bounds.origin.y.get(), 25.);
    }

    #[test]
    fn a_full_width_child_fills_its_parent_and_a_flex_child_shares_the_remainder() {
        let mut row = Node::container();
        row.layout.width = Length::px(100.);
        row.layout.height = Length::px(20.);
        let mut fixed = Node::container();
        fixed.layout.width = Length::px(30.);
        let mut growing = Node::container();
        growing.layout.flex_grow = 1.;
        let mut other = Node::container();
        other.layout.flex_grow = 1.;
        row.children.push(fixed);
        row.children.push(growing);
        row.children.push(other);
        lay_out(&mut row);
        assert_eq!(row.children[1].bounds.origin.x.get(), 30.);
        assert_eq!(row.children[1].bounds.size.width.get(), 35.);
        assert_eq!(row.children[2].bounds.size.width.get(), 35.);
    }

    #[test]
    fn an_absolute_child_leaves_the_flow_and_lands_on_its_inset() {
        let mut parent = Node::container();
        parent.layout.width = Length::px(200.);
        parent.layout.height = Length::px(100.);
        let mut in_flow = Node::container();
        in_flow.layout.width = Length::px(10.);
        let mut floating = Node::container();
        floating.layout.width = Length::px(10.);
        floating.layout.position = PositionSpec::Absolute {
            top: Some(Pixels::new(4.)),
            right: Some(Pixels::new(6.)),
            bottom: None,
            left: None,
        };
        parent.children.push(in_flow);
        parent.children.push(floating);
        lay_out(&mut parent);
        // Right-anchored: 200 - 6 - 10.
        assert_eq!(parent.children[1].bounds.origin.x.get(), 184.);
        assert_eq!(parent.children[1].bounds.origin.y.get(), 4.);
    }

    #[test]
    fn a_display_none_child_is_not_laid_out() {
        let mut parent = Node::container();
        parent.layout.direction = FlexDirectionSpec::Column;
        let mut hidden = Node::container();
        hidden.layout.width = Length::px(50.);
        hidden.layout.display = Display::None;
        parent.children.push(hidden);
        parent.children.push(text("x"));
        lay_out(&mut parent);
        assert_eq!(parent.children[0].bounds.size.width.get(), 0.);
        assert_eq!(parent.children[1].bounds.origin.y.get(), 0.);
    }

    #[test]
    fn a_grid_places_children_on_its_tracks() {
        let mut grid = Node::container();
        grid.layout.display = Display::Grid;
        grid.layout.width = Length::px(120.);
        grid.layout.grid_columns = vec![Track::Fraction(1.), Track::Fraction(1.)];
        grid.layout.gap = Size::new(10., 0.);
        grid.children.push(Node::container());
        grid.children.push(Node::container());
        lay_out(&mut grid);
        assert_eq!(grid.children[0].bounds.size.width.get(), 55.);
        assert_eq!(grid.children[1].bounds.origin.x.get(), 65.);
    }

    #[test]
    fn a_viewport_sized_root_gets_the_viewport() {
        let mut root = Node::container();
        root.layout.width = Length::full();
        root.layout.height = Length::full();
        let size = lay_out(&mut root);
        assert_eq!((size.width.get(), size.height.get()), (400., 300.));
    }
}
