//! Keyboard focus.
//!
//! Focus itself belongs to the host: a window asks to be focused and is told when that changed.
//! What lives here is which element inside the window holds that focus, and the tab order over
//! the elements that asked to be reachable.

use crate::vui::node::ElementId;

/// A place focus can be.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FocusHandle(ElementId);

impl FocusHandle {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self(id.into())
    }

    pub fn id(&self) -> &ElementId {
        &self.0
    }
}

impl From<&str> for FocusHandle {
    fn from(value: &str) -> Self {
        Self(ElementId::from(value))
    }
}

/// The tab order a frame produced, and where it currently is.
#[derive(Debug, Default, Clone)]
pub(crate) struct FocusOrder {
    /// Each stop with the region its element was painted as, so focus can be delivered without
    /// searching the tree for it.
    stops: Vec<(FocusHandle, u64)>,
}

impl FocusOrder {
    /// Rebuild from a frame's worth of focusable elements, in paint order.
    pub(crate) fn reset(&mut self, stops: Vec<(FocusHandle, u64)>) {
        self.stops = stops;
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn is_empty(&self) -> bool {
        self.stops.is_empty()
    }

    /// The region a focused element was painted as.
    pub(crate) fn region_for(&self, id: &ElementId) -> Option<u64> {
        self.stops
            .iter()
            .find(|(handle, _)| handle.id() == id)
            .map(|(_, region)| *region)
    }

    /// The next stop after `current`, wrapping. A handle no longer in the frame starts from the
    /// top rather than from a stale position.
    pub(crate) fn next(
        &self,
        current: Option<&FocusHandle>,
        backwards: bool,
    ) -> Option<FocusHandle> {
        if self.stops.is_empty() {
            return None;
        }
        let position =
            current.and_then(|handle| self.stops.iter().position(|(stop, _)| stop == handle));
        let index = match (position, backwards) {
            (None, _) => 0,
            (Some(0), true) => self.stops.len() - 1,
            (Some(0), false) => 1 % self.stops.len(),
            (Some(position), true) => position - 1,
            (Some(position), false) => (position + 1) % self.stops.len(),
        };
        self.stops.get(index).map(|(handle, _)| handle.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handle(name: &str) -> FocusHandle {
        FocusHandle::new(name)
    }

    fn stops(names: &[&str]) -> Vec<(FocusHandle, u64)> {
        names
            .iter()
            .enumerate()
            .map(|(index, name)| (handle(name), index as u64 + 1))
            .collect()
    }

    #[test]
    fn tab_walks_forward_and_wraps() {
        let mut order = FocusOrder::default();
        order.reset(stops(&["a", "b", "c"]));
        assert_eq!(order.next(None, false), Some(handle("a")));
        assert_eq!(order.next(Some(&handle("a")), false), Some(handle("b")));
        assert_eq!(order.next(Some(&handle("c")), false), Some(handle("a")));
    }

    #[test]
    fn shift_tab_walks_backward_and_wraps() {
        let mut order = FocusOrder::default();
        order.reset(stops(&["a", "b", "c"]));
        assert_eq!(order.next(Some(&handle("a")), true), Some(handle("c")));
        assert_eq!(order.next(Some(&handle("c")), true), Some(handle("b")));
    }

    #[test]
    fn a_single_stop_stays_put_in_both_directions() {
        let mut order = FocusOrder::default();
        order.reset(stops(&["only"]));
        assert_eq!(
            order.next(Some(&handle("only")), false),
            Some(handle("only"))
        );
        assert_eq!(
            order.next(Some(&handle("only")), true),
            Some(handle("only"))
        );
    }

    #[test]
    fn a_stop_knows_the_region_its_element_was_painted_as() {
        let mut order = FocusOrder::default();
        order.reset(vec![(handle("field"), 77)]);
        assert_eq!(order.region_for(&ElementId::from("field")), Some(77));
        assert_eq!(order.region_for(&ElementId::from("elsewhere")), None);
    }

    #[test]
    fn a_handle_that_left_the_frame_restarts_from_the_top() {
        let mut order = FocusOrder::default();
        order.reset(stops(&["a", "b"]));
        assert_eq!(order.next(Some(&handle("gone")), false), Some(handle("a")));
        order.reset(Vec::new());
        assert_eq!(order.next(Some(&handle("a")), false), None);
        assert!(order.is_empty());
    }
}
