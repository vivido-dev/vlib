//! Virtualized lists.
//!
//! A list is not an optimization here, it is the only way a long one can exist at all: a display
//! list has a fixed command ceiling, and a row costs commands whether or not anyone can see it.
//! So a list builds only the rows its own box can show, and the rest cost nothing.
//!
//! Which rows those are is not known until the list has been laid out, so a frame containing one
//! lays out twice: once to find the list's box, once with the rows it turned out to need. The
//! second pass happens only when a list is present, and never more than once.

use std::fmt;
use std::ops::Range;
use std::rc::Rc;

use crate::vui::geometry::{Bounds, Pixels};
use crate::vui::node::{ElementId, Node, NodeKind};

/// How far a list may be scrolled past its last row, so the final row is not glued to the edge.
const OVERSCROLL: f32 = 0.;

/// Where a list is scrolled to, and what it is scrolling through.
///
/// The view owns this: scrolling is a change to the application's state like any other, and a
/// list that scrolled without the view knowing would be a list the view could not restore.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ListState {
    offset: f32,
    row_height: f32,
    count: usize,
}

impl ListState {
    pub fn new(count: usize, row_height: f32) -> Self {
        Self {
            offset: 0.,
            row_height: row_height.max(1.),
            count,
        }
    }

    pub fn offset(&self) -> f32 {
        self.offset
    }

    pub fn row_height(&self) -> f32 {
        self.row_height
    }

    pub fn count(&self) -> usize {
        self.count
    }

    /// Change how many rows there are, keeping the scroll position where it still can be.
    pub fn set_count(&mut self, count: usize, viewport_height: f32) {
        self.count = count;
        self.offset = self.offset.clamp(0., self.maximum_offset(viewport_height));
    }

    /// The whole content's height, which is what a scrollbar is a fraction of.
    pub fn content_height(&self) -> f32 {
        self.count as f32 * self.row_height
    }

    /// The largest offset that still leaves content on screen.
    pub fn maximum_offset(&self, viewport_height: f32) -> f32 {
        (self.content_height() + OVERSCROLL - viewport_height).max(0.)
    }

    /// Scroll by a wheel delta. A positive `dy` is a scroll down, so the content moves up.
    pub fn scroll_by(&mut self, dy: f32, viewport_height: f32) {
        self.scroll_to(self.offset + dy, viewport_height);
    }

    pub fn scroll_to(&mut self, offset: f32, viewport_height: f32) {
        self.offset = offset.clamp(0., self.maximum_offset(viewport_height));
    }

    /// Scroll until a row is on screen, moving as little as possible.
    pub fn scroll_into_view(&mut self, row: usize, viewport_height: f32) {
        let top = row as f32 * self.row_height;
        let bottom = top + self.row_height;
        if top < self.offset {
            self.scroll_to(top, viewport_height);
        } else if bottom > self.offset + viewport_height {
            self.scroll_to(bottom - viewport_height, viewport_height);
        }
    }

    /// The rows a box of this height can show, given where the list is scrolled to.
    ///
    /// One row past the bottom is included: a row half off the edge is still drawn, clipped, so
    /// scrolling reveals it sliding in rather than appearing whole.
    pub fn visible_range(&self, viewport_height: f32) -> Range<usize> {
        if self.count == 0 || viewport_height <= 0. {
            return 0..0;
        }
        let first = (self.offset / self.row_height).floor().max(0.) as usize;
        let visible = (viewport_height / self.row_height).ceil() as usize + 1;
        let end = first.saturating_add(visible).min(self.count);
        first.min(self.count)..end
    }

    /// Where the scrollbar's thumb sits inside a track of this height: its top and its length.
    ///
    /// `None` when everything fits, because a list that does not scroll has nothing to indicate.
    pub fn thumb(&self, viewport_height: f32) -> Option<(f32, f32)> {
        let content = self.content_height();
        if content <= viewport_height || viewport_height <= 0. {
            return None;
        }
        // A thumb below a few pixels is not a thumb anyone can see or grab.
        let length = (viewport_height * viewport_height / content).max(16.);
        let travel = viewport_height - length;
        let maximum = self.maximum_offset(viewport_height);
        let top = if maximum > 0. {
            travel * (self.offset / maximum)
        } else {
            0.
        };
        Some((top, length))
    }
}

/// Builds the rows in a range, once the list's own box is known.
#[derive(Clone)]
pub(crate) struct RowBuilder(Rc<dyn Fn(Range<usize>) -> Vec<Node>>);

impl RowBuilder {
    pub(crate) fn build(&self, rows: Range<usize>) -> Vec<Node> {
        (self.0)(rows)
    }
}

impl PartialEq for RowBuilder {
    /// Two builders are the same builder, not two closures that behave alike: nothing can compare
    /// closures, and identity is all a frame needs.
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for RowBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RowBuilder")
    }
}

/// What the frame needs to fill a list once it has been laid out.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ListSpec {
    pub state: ListState,
    pub build: RowBuilder,
    /// The rows the second pass materialized, so a third pass is never needed.
    pub filled: Option<Range<usize>>,
}

impl ListSpec {
    /// Build the rows a box of these bounds shows, as absolutely positioned children.
    ///
    /// Each row is placed at its own offset in the content rather than stacked in flow, because
    /// the rows before it do not exist: the first visible row is row 400, and it belongs 400 rows
    /// down however few nodes precede it.
    pub(crate) fn fill(&self, bounds: Bounds) -> (Range<usize>, Vec<Node>) {
        let rows = self.state.visible_range(bounds.height().get());
        let mut children = self.build.build(rows.clone());
        for (offset, child) in rows.clone().zip(children.iter_mut()) {
            let top = offset as f32 * self.state.row_height - self.state.offset;
            child.layout.position = crate::vui::layout::PositionSpec::Absolute {
                top: Some(Pixels::new(top)),
                right: None,
                bottom: None,
                left: Some(Pixels::ZERO),
            };
            child.layout.width = crate::vui::layout::Length::full();
            child.layout.height = crate::vui::layout::Length::px(self.state.row_height);
        }
        (rows, children)
    }
}

/// A list of fixed-height rows, of which only the visible ones are built.
///
/// `build` is handed the range the list can show and returns exactly that many elements. It runs
/// once per frame, after the list's box is known.
pub fn uniform_list(
    state: &ListState,
    build: impl Fn(Range<usize>) -> Vec<Node> + 'static,
) -> ListEl {
    ListEl {
        node: Node::list(ListSpec {
            state: *state,
            build: RowBuilder(Rc::new(build)),
            filled: None,
        }),
    }
}

/// A list element: a clipped box whose rows are materialized after layout.
#[derive(Debug)]
pub struct ListEl {
    node: Node,
}

impl ListEl {
    /// Give the list an identity, which it needs: a scroll handler asks how tall it turned out
    /// to be, and that answer is looked up by id.
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

    pub fn w_full(mut self) -> Self {
        self.node.layout.width = crate::vui::layout::Length::full();
        self
    }

    pub fn h_full(mut self) -> Self {
        self.node.layout.height = crate::vui::layout::Length::full();
        self
    }

    /// Take the remaining space on the parent's main axis, which is what a list in a column does.
    pub fn flex_1(mut self) -> Self {
        self.node.layout.flex_grow = 1.;
        self.node.layout.flex_shrink = 1.;
        self.node.layout.flex_basis = crate::vui::layout::Length::px(0.);
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
        self.node.style.border = Some(crate::vui::style::Border::solid(Pixels::new(width), color));
        self
    }

    pub fn on_wheel(mut self, handler: crate::vui::interactive::Handler) -> Self {
        self.node.interactions.handlers.wheel = Some(handler);
        self
    }

    pub fn on_click(mut self, handler: crate::vui::interactive::Handler) -> Self {
        self.node.interactions.handlers.click = Some(handler);
        self
    }
}

impl crate::vui::a11y::Semantic for ListEl {
    fn accessibility_mut(&mut self) -> &mut crate::vui::a11y::Accessibility {
        &mut self.node.accessibility
    }
}

impl crate::vui::element::IntoElement for ListEl {
    fn into_node(mut self) -> Node {
        // The rows are clipped to the list: a row scrolled past the edge is neither drawn nor
        // reachable, which is the same rule every other clipped box follows.
        self.node.layout.overflow = crate::vui::layout::OverflowSpec::Hidden;
        self.node
    }
}

/// Fill every list in a tree whose rows have not been built yet.
///
/// Answers whether anything was filled, which is what tells the frame a second layout pass is
/// owed.
pub(crate) fn fill_lists(node: &mut Node) -> bool {
    let mut filled = false;
    if let NodeKind::List(spec) = &node.kind {
        let bounds = node.bounds;
        let (rows, children) = spec.fill(bounds);
        if spec.filled.as_ref() != Some(&rows) {
            node.children = children;
            if let NodeKind::List(spec) = &mut node.kind {
                spec.filled = Some(rows);
            }
            filled = true;
        }
    }
    for child in &mut node.children {
        if fill_lists(child) {
            filled = true;
        }
    }
    filled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_shows_the_rows_its_box_can_hold_and_one_more() {
        let state = ListState::new(1000, 20.);
        // A 100 pixel box holds five whole rows; the sixth is the one sliding in.
        assert_eq!(state.visible_range(100.), 0..6);

        let mut scrolled = state;
        scrolled.scroll_to(200., 100.);
        assert_eq!(scrolled.visible_range(100.), 10..16);

        // Part way through a row: the row that is half off the top is still the first one.
        scrolled.scroll_to(210., 100.);
        assert_eq!(scrolled.visible_range(100.), 10..16);
    }

    #[test]
    fn a_list_never_builds_more_rows_than_it_has() {
        let state = ListState::new(3, 20.);
        assert_eq!(state.visible_range(500.), 0..3);
        assert_eq!(ListState::new(0, 20.).visible_range(100.), 0..0);
        // A box with no height shows nothing rather than one row.
        assert_eq!(state.visible_range(0.), 0..0);
    }

    #[test]
    fn scrolling_clamps_at_both_ends() {
        let mut state = ListState::new(100, 20.);
        assert_eq!(state.maximum_offset(100.), 1900.);

        state.scroll_by(-500., 100.);
        assert_eq!(state.offset(), 0.);

        state.scroll_by(100_000., 100.);
        assert_eq!(state.offset(), 1900.);

        // A list that fits does not scroll at all.
        let mut short = ListState::new(2, 20.);
        short.scroll_by(500., 100.);
        assert_eq!(short.offset(), 0.);
        assert_eq!(short.maximum_offset(100.), 0.);
    }

    #[test]
    fn scrolling_a_row_into_view_moves_as_little_as_it_can() {
        let mut state = ListState::new(100, 20.);
        // Already visible: nothing moves.
        state.scroll_into_view(2, 100.);
        assert_eq!(state.offset(), 0.);

        // Below the fold: the row comes to the bottom edge, not the top.
        state.scroll_into_view(10, 100.);
        assert_eq!(state.offset(), 120.);

        // Above it: the row comes to the top edge.
        state.scroll_into_view(1, 100.);
        assert_eq!(state.offset(), 20.);
    }

    #[test]
    fn a_thumb_is_a_fraction_of_the_track_and_absent_when_everything_fits() {
        let mut state = ListState::new(100, 20.);
        let (top, length) = state
            .thumb(100.)
            .expect("2000 pixels of content in 100 scrolls");
        assert_eq!(top, 0.);
        assert_eq!(length, 16., "a tiny thumb is floored so it stays grabbable");

        // At the end of the travel the thumb is at the end of the track.
        state.scroll_to(state.maximum_offset(100.), 100.);
        let (top, length) = state.thumb(100.).unwrap();
        assert!(
            (top + length - 100.).abs() < 0.01,
            "the thumb ends at the track's end"
        );

        // A list that fits has no thumb.
        assert_eq!(ListState::new(2, 20.).thumb(100.), None);
    }

    #[test]
    fn removing_rows_pulls_the_scroll_back_into_range() {
        let mut state = ListState::new(100, 20.);
        state.scroll_to(1900., 100.);
        assert_eq!(state.offset(), 1900.);

        state.set_count(10, 100.);
        assert_eq!(state.offset(), 100., "the offset came back to the new end");
        assert_eq!(state.visible_range(100.), 5..10);
    }
}
