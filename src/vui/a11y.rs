//! What an element tells assistive technology about itself.
//!
//! A Canvas window is opaque: the host sees a display list, and a display list does not say which
//! rectangle is a button. So an application that wants to be usable by a screen reader says so —
//! a bounded tree of roles, labels, values and states, published for the scene it describes.
//!
//! Nothing is inferred from drawing. An element that paints a rounded box with the word "Save" in
//! it is not announced as a button unless it says it is one, because guessing wrong is worse than
//! saying nothing.

use vivid_protocol::overlay::{
    AccessibleAction, MAX_SEMANTIC_DEPTH, MAX_SEMANTIC_NODES, MAX_SEMANTIC_TEXT_BYTES,
    SemanticNode, SemanticRole, Semantics, Toggled,
};
use vivid_protocol::vector::Scalar;

use crate::vui::geometry::Bounds;
use crate::vui::node::{ElementId, Node, region_id};

/// What one element declares about itself.
///
/// An element that declares nothing is not in the tree at all — which is right for the boxes and
/// spacers a layout is mostly made of.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Accessibility {
    pub role: Option<SemanticRole>,
    pub label: Option<String>,
    /// `(current, minimum, maximum)` for something with a value: a slider, a spin button, a
    /// progress indicator.
    pub value: Option<(f32, f32, f32)>,
    /// A heading's level, `1` being the most prominent.
    pub level: Option<u8>,
    /// `(position, size)`, both one-based: "the third of eight".
    pub set: Option<(u16, u16)>,
    pub toggled: Option<Toggled>,
    pub disabled: bool,
    /// What can be asked of it. Each has a counterpart in the toolkits a host builds on.
    pub actions: Vec<AccessibleAction>,
}

impl Accessibility {
    /// Whether this element says anything at all.
    pub fn declared(&self) -> bool {
        self.role.is_some()
            || self.label.is_some()
            || self.value.is_some()
            || self.toggled.is_some()
            || !self.actions.is_empty()
    }
}

/// The builders every element shares for describing itself.
///
/// One trait rather than the same eight methods copied onto every element type: what a button
/// says about itself does not depend on whether it is a box, a run of text, or a picture.
pub trait Semantic: Sized {
    /// The element's own declaration, to be filled in.
    fn accessibility_mut(&mut self) -> &mut Accessibility;

    /// What kind of thing this is. A role is what makes an element announced at all.
    fn role(mut self, role: SemanticRole) -> Self {
        self.accessibility_mut().role = Some(role);
        self
    }

    /// What it is called. Longer than the protocol's ceiling is truncated when the tree is built,
    /// on a character boundary.
    fn label(mut self, label: impl Into<String>) -> Self {
        self.accessibility_mut().label = Some(label.into());
        self
    }

    /// What it is set to, and the range it moves in.
    fn accessible_value(mut self, current: f32, minimum: f32, maximum: f32) -> Self {
        self.accessibility_mut().value = Some((current, minimum, maximum));
        self
    }

    /// How prominent a heading is, `1` being the most.
    fn accessible_level(mut self, level: u8) -> Self {
        self.accessibility_mut().level = Some(level.max(1));
        self
    }

    /// Which of how many: "the third of eight". Both one-based.
    fn accessible_position(mut self, position: u16, size: u16) -> Self {
        self.accessibility_mut().set = Some((position.max(1), size.max(1)));
        self
    }

    /// On, off, or neither — which is what a checkbox with some of its children ticked is.
    fn toggled(mut self, toggled: Toggled) -> Self {
        self.accessibility_mut().toggled = Some(toggled);
        self
    }

    /// Present but not available.
    fn accessible_disabled(mut self, disabled: bool) -> Self {
        self.accessibility_mut().disabled = disabled;
        self
    }

    /// What can be asked of it.
    fn accessible_actions(mut self, actions: impl IntoIterator<Item = AccessibleAction>) -> Self {
        self.accessibility_mut().actions = actions.into_iter().collect();
        self
    }
}

/// A tree of semantic nodes, and how to get back to the elements that declared them.
#[derive(Debug, Default)]
pub(crate) struct Described {
    pub nodes: Vec<SemanticNode>,
    /// Node identity to the element that owns it, for routing an action back.
    pub owners: Vec<(u64, ElementId)>,
    /// Nodes left out because the tree was already as large as the protocol allows.
    pub dropped: usize,
}

impl Described {
    /// The tree as the wire carries it, for a scene that has been published.
    pub(crate) fn semantics(&self, scene_revision: u64) -> Option<Semantics> {
        (!self.nodes.is_empty()).then(|| Semantics {
            scene_revision,
            nodes: self.nodes.clone(),
        })
    }
}

/// Build the semantic tree a frame describes.
///
/// Called after layout, because a node says where it is and that is not known before then.
pub(crate) fn describe(root: &Node, window: Bounds) -> Described {
    let mut builder = Builder {
        described: Described::default(),
        path: Vec::new(),
    };
    // A placeholder root, replaced below once it is known whether the frame has a single
    // outermost node of its own. Reserving index zero keeps every child index greater than its
    // parent's, which is the rule that makes a cycle impossible.
    builder.described.nodes.push(window_root(window));
    builder.walk(root, 0, 1);

    let mut described = builder.described;
    let root_children = described.nodes[0].children.clone();
    if root_children.len() == 1 && root_children[0] == 1 {
        // Exactly one outermost node, sitting right after the placeholder: it is the real root,
        // so the placeholder is not needed and every index shifts down by one.
        described.nodes.remove(0);
        for node in &mut described.nodes {
            for child in &mut node.children {
                *child -= 1;
            }
        }
    } else if root_children.is_empty() {
        // Nothing described itself, so there is nothing to publish. A window with no semantics
        // is not a window with an empty one.
        described.nodes.clear();
        described.owners.clear();
    }
    described
}

/// The synthesized root, for a frame whose outermost described elements are siblings.
fn window_root(window: Bounds) -> SemanticNode {
    SemanticNode {
        // Even, so it can never collide with an element's own identity, which is always odd.
        id: 2,
        role: SemanticRole::Application,
        bounds: window.to_wire().unwrap_or_else(|_| {
            Bounds::from_xywh(0., 0., 1., 1.)
                .to_wire()
                .expect("a unit box")
        }),
        label: String::new(),
        numeric: None,
        level: None,
        set: None,
        toggled: None,
        disabled: false,
        actions: Vec::new(),
        children: Vec::new(),
    }
}

struct Builder<'a> {
    described: Described,
    path: Vec<&'a ElementId>,
}

impl<'a> Builder<'a> {
    /// Walk the tree, hanging each described element under the nearest described ancestor.
    ///
    /// `parent` is that ancestor's index and `depth` how deep it sits, so a subtree that would
    /// nest past the protocol's ceiling is attached higher rather than refused: a description
    /// that is slightly flatter than the layout is far better than none.
    fn walk(&mut self, node: &'a Node, parent: usize, depth: usize) {
        let pushed = node.id.as_ref().inspect(|id| self.path.push(id));
        let (parent, depth) = match self.node_for(node, parent, depth) {
            Some(index) => (index, depth + 1),
            None => (parent, depth),
        };
        for child in &node.children {
            self.walk(child, parent, depth);
        }
        if pushed.is_some() {
            self.path.pop();
        }
    }

    /// Add this element to the tree if it described itself, answering its index.
    fn node_for(&mut self, node: &'a Node, parent: usize, depth: usize) -> Option<usize> {
        let declared = &node.accessibility;
        if !declared.declared() {
            return None;
        }
        if self.described.nodes.len() >= MAX_SEMANTIC_NODES || depth > MAX_SEMANTIC_DEPTH {
            // The ceilings are the protocol's. Past them the extra nodes are left out and
            // counted, rather than the whole description being refused.
            self.described.dropped += 1;
            return None;
        }

        let id = match node.id.as_ref() {
            // The same identity a click uses, so an action comes back to the element the way
            // every other event does. Always odd.
            Some(id) => {
                let region = region_id(&self.path);
                self.described.owners.push((region, id.clone()));
                region
            }
            // Something described but not interactive — a heading, a label. Even, so it cannot
            // collide with an element's own identity. Positional, which is enough: an action is
            // only ever delivered for the tree of the scene on screen.
            None => (self.described.nodes.len() as u64 + 1) * 2,
        };

        let index = self.described.nodes.len();
        self.described.nodes.push(SemanticNode {
            id,
            role: declared.role.unwrap_or(SemanticRole::Generic),
            bounds: node.bounds.to_wire().unwrap_or_else(|_| {
                Bounds::from_xywh(0., 0., 1., 1.)
                    .to_wire()
                    .expect("a unit box")
            }),
            label: truncate(declared.label.as_deref().unwrap_or_default()),
            numeric: declared.value.and_then(numeric),
            level: declared.level,
            set: declared.set.map(|(position, size)| {
                // A position outside its own set would be refused, so it is brought inside it.
                [position.max(1).min(size.max(1)), size.max(1)]
            }),
            toggled: declared.toggled,
            disabled: declared.disabled,
            actions: declared.actions.clone(),
            children: Vec::new(),
        });
        self.described.nodes[parent].children.push(index as u32);
        Some(index)
    }
}

/// A label the wire can carry, cut on a character boundary rather than mid-character.
fn truncate(label: &str) -> String {
    if label.len() <= MAX_SEMANTIC_TEXT_BYTES {
        return label.to_owned();
    }
    let mut end = MAX_SEMANTIC_TEXT_BYTES;
    while end > 0 && !label.is_char_boundary(end) {
        end -= 1;
    }
    label[..end].to_owned()
}

/// A value and its range, in the order the wire expects, or nothing if it cannot be carried.
fn numeric(value: (f32, f32, f32)) -> Option<[Scalar; 3]> {
    let (current, minimum, maximum) = value;
    // A range the wrong way round describes nothing, so it is put the right way round.
    let (minimum, maximum) = (minimum.min(maximum), minimum.max(maximum));
    let current = current.clamp(minimum, maximum);
    Some([
        Scalar::new(current as f64).ok()?,
        Scalar::new(minimum as f64).ok()?,
        Scalar::new(maximum as f64).ok()?,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vui::element::IntoElement;
    use crate::vui::element::div;

    /// Lay a tree out roughly, so nodes have boxes to describe.
    fn placed(mut node: Node) -> Node {
        fn place(node: &mut Node, y: f32) {
            node.bounds = Bounds::from_xywh(0., y, 100., 20.);
            for (index, child) in node.children.iter_mut().enumerate() {
                place(child, y + 20. + index as f32 * 20.);
            }
        }
        place(&mut node, 0.);
        node
    }

    fn window() -> Bounds {
        Bounds::from_xywh(0., 0., 200., 200.)
    }

    #[test]
    fn a_frame_that_describes_nothing_publishes_nothing() {
        let tree = placed(div().child(div().child(div())).into_node());
        let described = describe(&tree, window());
        assert!(described.nodes.is_empty(), "no roles, no tree");
        assert_eq!(described.semantics(1), None);
    }

    #[test]
    fn one_outermost_node_is_the_root_rather_than_a_child_of_a_synthesized_one() {
        let tree = placed(
            div()
                .role(SemanticRole::Application)
                .label("Counter")
                .child(div().id("up").role(SemanticRole::Button).label("Increment"))
                .into_node(),
        );
        let described = describe(&tree, window());
        assert_eq!(described.nodes.len(), 2, "the application and its button");
        assert_eq!(described.nodes[0].role, SemanticRole::Application);
        assert_eq!(described.nodes[0].label, "Counter");
        assert_eq!(described.nodes[0].children, vec![1]);
        assert_eq!(described.nodes[1].label, "Increment");

        // And it is a tree the protocol accepts.
        described
            .semantics(7)
            .unwrap()
            .validate()
            .expect("a valid tree");
    }

    #[test]
    fn siblings_at_the_top_get_a_root_to_hang_from() {
        let tree = placed(
            div()
                .child(div().role(SemanticRole::Heading).label("One"))
                .child(div().role(SemanticRole::Heading).label("Two"))
                .into_node(),
        );
        let described = describe(&tree, window());
        assert_eq!(
            described.nodes.len(),
            3,
            "a synthesized root and two headings"
        );
        assert_eq!(described.nodes[0].role, SemanticRole::Application);
        assert_eq!(described.nodes[0].children, vec![1, 2]);
        described
            .semantics(1)
            .unwrap()
            .validate()
            .expect("a valid tree");
    }

    #[test]
    fn a_described_element_hangs_from_the_nearest_described_ancestor() {
        // Layers of plain boxes between them: the description is of the meaningful structure,
        // not of the layout that produced it.
        let tree = placed(
            div()
                .role(SemanticRole::List)
                .label("Items")
                .child(div().child(div().child(div().role(SemanticRole::ListItem).label("First"))))
                .into_node(),
        );
        let described = describe(&tree, window());
        assert_eq!(described.nodes.len(), 2);
        assert_eq!(
            described.nodes[0].children,
            vec![1],
            "the boxes between are not in the tree"
        );
        assert_eq!(described.nodes[1].role, SemanticRole::ListItem);
    }

    #[test]
    fn an_interactive_node_keeps_the_identity_a_click_would_use() {
        let tree = placed(
            div()
                .role(SemanticRole::Application)
                .child(div().id("save").role(SemanticRole::Button).label("Save"))
                .into_node(),
        );
        let described = describe(&tree, window());
        let button = &described.nodes[1];
        assert_eq!(button.id % 2, 1, "an element's own identity is odd");
        assert_eq!(
            described.owners,
            vec![(button.id, ElementId::from("save"))],
            "and it routes back to the element"
        );
        // A node nobody can act on gets an even identity, so the two can never collide.
        assert_eq!(described.nodes[0].id % 2, 0);
    }

    #[test]
    fn a_label_longer_than_the_wire_allows_is_cut_on_a_character_boundary() {
        let long = "é".repeat(MAX_SEMANTIC_TEXT_BYTES);
        let tree = placed(div().role(SemanticRole::Text).label(long).into_node());
        let described = describe(&tree, window());
        let label = &described.nodes[0].label;
        assert!(label.len() <= MAX_SEMANTIC_TEXT_BYTES);
        assert!(label.chars().all(|c| c == 'é'), "not cut mid-character");
        described
            .semantics(1)
            .unwrap()
            .validate()
            .expect("a valid tree");
    }

    #[test]
    fn a_value_is_put_in_range_rather_than_refused() {
        let tree = placed(
            div()
                .role(SemanticRole::Slider)
                .label("Volume")
                // Backwards, and out of its own range.
                .accessible_value(99., 10., 0.)
                .into_node(),
        );
        let described = describe(&tree, window());
        let numeric = described.nodes[0].numeric.expect("a slider has a value");
        assert_eq!(
            numeric[1].get(),
            0.,
            "the range was put the right way round"
        );
        assert_eq!(numeric[2].get(), 10.);
        assert_eq!(numeric[0].get(), 10., "and the value brought inside it");
        described
            .semantics(1)
            .unwrap()
            .validate()
            .expect("a valid tree");
    }

    #[test]
    fn a_position_outside_its_set_is_brought_inside_it() {
        let tree = placed(
            div()
                .role(SemanticRole::ListItem)
                .label("Ninth")
                .accessible_position(9, 3)
                .into_node(),
        );
        let described = describe(&tree, window());
        assert_eq!(described.nodes[0].set, Some([3, 3]));
        described
            .semantics(1)
            .unwrap()
            .validate()
            .expect("a valid tree");
    }

    #[test]
    fn a_tree_past_the_node_ceiling_is_trimmed_rather_than_refused() {
        let mut root = div().role(SemanticRole::List).label("Long");
        for index in 0..(MAX_SEMANTIC_NODES + 50) {
            root = root.child(
                div()
                    .role(SemanticRole::ListItem)
                    .label(format!("Item {index}")),
            );
        }
        let described = describe(&placed(root.into_node()), window());
        assert!(
            described.nodes.len() <= MAX_SEMANTIC_NODES,
            "{}",
            described.nodes.len()
        );
        assert!(
            described.dropped >= 50,
            "the overflow is counted: {}",
            described.dropped
        );
        described
            .semantics(1)
            .unwrap()
            .validate()
            .expect("a trimmed tree is still valid");
    }

    #[test]
    fn a_tree_deeper_than_the_ceiling_is_flattened_rather_than_refused() {
        // Every level describes itself, which the protocol only allows so far down.
        let mut node = div().role(SemanticRole::Group).label("deepest");
        for _ in 0..(MAX_SEMANTIC_DEPTH + 10) {
            node = div().role(SemanticRole::Group).child(node);
        }
        let described = describe(&placed(node.into_node()), window());
        assert!(
            described.dropped > 0,
            "the levels past the ceiling were left out"
        );
        let semantics = described.semantics(1).unwrap();
        semantics
            .validate()
            .expect("a flattened tree is still valid");
    }
}
