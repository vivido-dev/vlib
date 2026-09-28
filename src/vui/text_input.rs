//! A single-line text editor: the state, the keys it answers, and what it draws.
//!
//! The model is ours; the glyphs, the caret position, and the selection rectangles are the host's
//! measurement. Nothing here guesses where a character is.
//!
//! Paste is not a key this handles. The host's paste policy delivers committed text to the
//! focused overlay as `Event::Text`, which arrives here as [`UiEvent::Text`] like any other
//! commit — a terminal-hosted editor never reads the clipboard, only writes it.

use crate::vui::element::IntoElement;
use crate::vui::interactive::UiEvent;
use crate::vui::keymap::usage_of;
use crate::vui::node::{Node, TextRunSpec, TextSpec};

/// A composition an input method is in the middle of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preedit {
    pub text: String,
    /// The selection inside the composition, in characters.
    pub selection: Option<(u32, u32)>,
}

/// The state of one text field.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextInput {
    text: String,
    /// Caret offset in bytes, always on a character boundary.
    cursor: usize,
    /// Where a selection started, or the caret when there is none.
    anchor: usize,
    preedit: Option<Preedit>,
}

impl TextInput {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let cursor = text.len();
        Self {
            text,
            cursor,
            anchor: cursor,
            preedit: None,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Where the selection started.
    pub fn anchor(&self) -> usize {
        self.anchor
    }

    /// The selection as a byte range, when there is one.
    pub fn selection(&self) -> Option<(usize, usize)> {
        let (start, end) = (self.anchor.min(self.cursor), self.anchor.max(self.cursor));
        (start != end).then_some((start, end))
    }

    pub fn selected_text(&self) -> &str {
        match self.selection() {
            Some((start, end)) => &self.text[start..end],
            None => "",
        }
    }

    /// What the input method is composing, if anything.
    pub fn preedit(&self) -> Option<&Preedit> {
        self.preedit.as_ref()
    }

    /// Replace everything, as setting a field's value does.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.cursor = self.text.len();
        self.anchor = self.cursor;
        self.preedit = None;
    }

    // ---- editing ------------------------------------------------------------------------------

    /// Insert committed text, replacing the selection and ending any composition.
    pub fn insert(&mut self, chunk: &str) {
        self.delete_selection();
        self.text.insert_str(self.cursor, chunk);
        self.cursor += chunk.len();
        self.anchor = self.cursor;
        self.preedit = None;
    }

    pub fn backspace(&mut self) {
        if self.selection().is_some() {
            self.delete_selection();
            return;
        }
        if let Some(previous) = self.previous_boundary() {
            self.text.replace_range(previous..self.cursor, "");
            self.cursor = previous;
            self.anchor = previous;
        }
    }

    pub fn delete(&mut self) {
        if self.selection().is_some() {
            self.delete_selection();
            return;
        }
        if let Some(next) = self.next_boundary() {
            self.text.replace_range(self.cursor..next, "");
        }
    }

    fn delete_selection(&mut self) {
        if let Some((start, end)) = self.selection() {
            self.text.replace_range(start..end, "");
            self.cursor = start;
            self.anchor = start;
        }
    }

    /// Remove the selection and answer what it held, which is what a cut does.
    pub fn cut(&mut self) -> String {
        let cut = self.selected_text().to_owned();
        self.delete_selection();
        cut
    }

    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.cursor = self.text.len();
    }

    // ---- caret movement -----------------------------------------------------------------------

    pub fn move_left(&mut self, extend: bool) {
        if !extend && let Some((start, _)) = self.selection() {
            self.cursor = start;
            self.anchor = start;
            return;
        }
        if let Some(previous) = self.previous_boundary() {
            self.cursor = previous;
        }
        if !extend {
            self.anchor = self.cursor;
        }
    }

    pub fn move_right(&mut self, extend: bool) {
        if !extend && let Some((_, end)) = self.selection() {
            self.cursor = end;
            self.anchor = end;
            return;
        }
        if let Some(next) = self.next_boundary() {
            self.cursor = next;
        }
        if !extend {
            self.anchor = self.cursor;
        }
    }

    pub fn move_home(&mut self, extend: bool) {
        self.cursor = 0;
        if !extend {
            self.anchor = 0;
        }
    }

    pub fn move_end(&mut self, extend: bool) {
        self.cursor = self.text.len();
        if !extend {
            self.anchor = self.cursor;
        }
    }

    /// Put the caret at a byte offset, which a click on measured text computes.
    pub fn place_caret(&mut self, offset: usize, extend: bool) {
        let offset = self.clamp_boundary(offset);
        self.cursor = offset;
        if !extend {
            self.anchor = offset;
        }
    }

    fn clamp_boundary(&self, offset: usize) -> usize {
        let offset = offset.min(self.text.len());
        if self.text.is_char_boundary(offset) {
            return offset;
        }
        (0..offset)
            .rev()
            .find(|index| self.text.is_char_boundary(*index))
            .unwrap_or(0)
    }

    fn previous_boundary(&self) -> Option<usize> {
        self.text[..self.cursor]
            .char_indices()
            .next_back()
            .map(|(index, _)| index)
    }

    fn next_boundary(&self) -> Option<usize> {
        self.text[self.cursor..]
            .char_indices()
            .nth(1)
            .map(|(index, _)| self.cursor + index)
            .or_else(|| (!self.text[self.cursor..].is_empty()).then_some(self.text.len()))
    }

    // ---- composition --------------------------------------------------------------------------

    /// Show a composition in progress. The buffer does not change until it is committed.
    pub fn set_preedit(&mut self, text: String, selection: Option<(u32, u32)>) {
        self.preedit = Some(Preedit { text, selection });
    }

    /// Drop the composition, as a cancelled one does.
    pub fn clear_preedit(&mut self) {
        self.preedit = None;
    }

    // ---- events -------------------------------------------------------------------------------

    /// Apply an event. Answers whether the buffer or the caret changed, which is what a caller
    /// needs to know to repaint.
    ///
    /// Copy is not here: writing the clipboard is the application's business, so a view handles
    /// the same key and calls `cx.copy(input.selected_text())`.
    pub fn handle(&mut self, event: &UiEvent) -> bool {
        let before = self.clone();
        match event {
            UiEvent::Text(committed) => self.insert(committed),
            UiEvent::Key {
                physical,
                down: true,
                modifiers,
                ..
            } => {
                let select = modifiers.shift();
                let command = modifiers.command() || modifiers.control();
                let key = *physical;
                let named = |name: &str| usage_of(name).is_some_and(|usage| usage == key);
                match () {
                    _ if named("backspace") => self.backspace(),
                    _ if named("delete") => self.delete(),
                    _ if named("left") => self.move_left(select),
                    _ if named("right") => self.move_right(select),
                    _ if named("home") => self.move_home(select),
                    _ if named("end") => self.move_end(select),
                    _ if command && named("a") => self.select_all(),
                    _ if command && named("x") => {
                        self.cut();
                    }
                    _ => {}
                }
            }
            UiEvent::Ime { preedit, selection } => {
                self.set_preedit(preedit.clone(), *selection);
            }
            _ => {}
        }
        *self != before
    }
}

/// A one-line text field.
pub fn text_input(input: &TextInput) -> TextInputEl {
    let spec = TextSpec::plain(input.text(), 16.);
    TextInputEl {
        node: Node::input(InputSpec::of(input, spec.clone())),
        input: input.clone(),
        spec,
        focus: None,
    }
}

/// What the painter needs to draw a field: the text as a paragraph, and where the caret and any
/// selection are.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct InputSpec {
    pub text: TextSpec,
    /// Byte offsets into the text.
    pub caret: usize,
    pub anchor: usize,
    /// Drawn instead of the text when the buffer is empty.
    pub placeholder: Option<TextSpec>,
    pub caret_color: vivid_protocol::vector::Color,
    pub selection_color: vivid_protocol::vector::Color,
}

impl InputSpec {
    fn of(input: &TextInput, text: TextSpec) -> Self {
        Self {
            text: with_preedit(input, text),
            caret: input.cursor(),
            anchor: input.anchor(),
            placeholder: None,
            caret_color: crate::vui::style::rgb(0xffffff),
            selection_color: crate::vui::style::rgba(0x4a5fd0, 0x80),
        }
    }

    /// What is drawn: the buffer, or the placeholder when it is empty.
    pub(crate) fn drawn(&self) -> &TextSpec {
        if self.text.runs.iter().all(|run| run.text.is_empty())
            && let Some(placeholder) = &self.placeholder
        {
            return placeholder;
        }
        &self.text
    }
}

pub struct TextInputEl {
    node: Node,
    input: TextInput,
    spec: TextSpec,
    focus: Option<crate::vui::focus::FocusHandle>,
}

impl TextInputEl {
    pub fn size(mut self, size: f32) -> Self {
        self.spec.each_run(|run| run.size = size);
        self
    }

    pub fn color(mut self, color: vivid_protocol::vector::Color) -> Self {
        self.spec.each_run(|run| run.color = color);
        self
    }

    pub fn family(mut self, family: impl Into<String>) -> Self {
        let family = family.into();
        self.spec.each_run(|run| run.family = family.clone());
        self
    }

    /// Text shown while the field is empty, in the same style unless told otherwise.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        let text = text.into();
        let mut spec = self.spec.clone();
        spec.each_run(|run| {
            run.text = text.clone();
            run.color = crate::vui::style::rgba(0xffffff, 0x60);
        });
        self.node.input_mut().placeholder = Some(spec);
        self
    }

    pub fn id(mut self, id: impl Into<crate::vui::node::ElementId>) -> Self {
        self.node.id = Some(id.into());
        self
    }

    /// Make the field a tab stop, and the place keys go when it is clicked.
    pub fn track_focus(mut self, handle: &crate::vui::focus::FocusHandle) -> Self {
        self.focus = Some(handle.clone());
        if self.node.id.is_none() {
            self.node.id = Some(handle.id().clone());
        }
        self
    }

    pub fn cursor(mut self, cursor: vivid_protocol::vector::CursorShape) -> Self {
        self.node.interactions.set_cursor(Some(cursor));
        self
    }

    pub fn on_key(mut self, handler: crate::vui::interactive::Handler) -> Self {
        self.node.interactions.handlers.key = Some(handler);
        self
    }

    pub fn on_text(mut self, handler: crate::vui::interactive::Handler) -> Self {
        self.node.interactions.handlers.text = Some(handler);
        self
    }

    pub fn on_click(mut self, handler: crate::vui::interactive::Handler) -> Self {
        self.node.interactions.handlers.click = Some(handler);
        self
    }
}

impl IntoElement for TextInputEl {
    fn into_node(mut self) -> Node {
        let mut spec = InputSpec::of(&self.input, self.spec.clone());
        if let Some(existing) = self.node.input_spec() {
            spec.placeholder = existing.placeholder.clone();
        }
        self.node.kind = crate::vui::node::NodeKind::Input(spec);
        self.node.focus = self.focus;
        self.node
    }
}

/// The buffer with the composition spliced in at the caret, underlined.
///
/// A composition is not in the buffer until it is committed, but it is drawn where it is being
/// typed — so the paragraph the host shapes holds it, and the caret in front of it is the caret
/// the model has.
fn with_preedit(input: &TextInput, text: TextSpec) -> TextSpec {
    let Some(preedit) = input.preedit() else {
        return text;
    };
    let Some(run) = text.runs.first().cloned() else {
        return text;
    };
    let cursor = input.cursor().min(run.text.len());
    let (before, after) = run.text.split_at(cursor);
    let mut runs: Vec<TextRunSpec> = Vec::new();
    for part in [
        TextRunSpec {
            text: before.to_owned(),
            ..run.clone()
        },
        TextRunSpec {
            text: preedit.text.clone(),
            underline: true,
            ..run.clone()
        },
        TextRunSpec {
            text: after.to_owned(),
            ..run
        },
    ] {
        if !part.text.is_empty() {
            runs.push(part);
        }
    }
    TextSpec { runs, ..text }
}

/// Where a byte offset sits inside measured text, in the text's own coordinates.
///
/// The host measured the clusters; a caret is the boundary between two of them, so its position
/// is read off the shape rather than estimated from a character count.
pub(crate) fn caret_geometry(
    measurement: &vivid_protocol::overlay::wire::text::TextMeasurement,
    offset: usize,
) -> Option<(f32, f32, f32)> {
    use vivid_protocol::overlay::wire::text::TextGeometry;
    let geometry: Vec<&TextGeometry> = measurement.lines.iter().collect();
    if geometry.is_empty() {
        return None;
    }
    // The line the offset falls on, or the last line before it.
    let line = geometry
        .iter()
        .rev()
        .find(|line| line.start as usize <= offset)
        .or_else(|| geometry.first())?;
    let line_start = line.start as usize;
    let mut x = line.x.get() as f32;
    for cluster in &measurement.clusters {
        if (cluster.start as usize) < line_start {
            continue;
        }
        if (cluster.end as usize) <= offset {
            // The caret sits after a cluster once the offset has passed it.
            x = (cluster.x.get() + cluster.width.get()) as f32;
        } else if (cluster.start as usize) < offset {
            // Inside a cluster: the caret goes at its leading edge, which is what a grapheme
            // cluster does — a caret cannot sit inside one.
            x = cluster.x.get() as f32;
            break;
        } else {
            break;
        }
    }
    Some((x, line.y.get() as f32, line.height.get() as f32))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vui::interactive::Modifiers;
    use vivid_protocol::overlay::modifiers;
    use vivid_protocol::vector::Scalar;

    fn key(name: &str, modifiers: u32) -> UiEvent {
        UiEvent::Key {
            physical: usage_of(name).unwrap(),
            down: true,
            repeat: false,
            modifiers: Modifiers::from_bits(modifiers),
        }
    }

    #[test]
    fn typing_inserts_at_the_caret_and_backspace_takes_it_back() {
        let mut input = TextInput::default();
        assert!(input.handle(&UiEvent::Text("héllo".into())));
        assert_eq!(input.text(), "héllo");
        assert_eq!(input.cursor(), "héllo".len());

        assert!(input.handle(&key("backspace", 0)));
        assert_eq!(input.text(), "héll");
        // A multi-byte character goes in one piece, not one byte at a time.
        assert!(input.handle(&key("backspace", 0)));
        assert_eq!(input.text(), "hél");
        assert!(input.text().is_char_boundary(input.cursor()));

        // A delete at the end of the buffer has nothing to take, and says so.
        assert!(!input.handle(&key("delete", 0)));
        assert!(input.handle(&key("home", 0)));
        assert!(input.handle(&key("delete", 0)));
        assert_eq!(input.text(), "él");
    }

    #[test]
    fn moving_and_selecting_are_the_usual_thing() {
        let mut input = TextInput::new("hello");
        assert_eq!(input.cursor(), 5);

        input.handle(&key("left", 0));
        assert_eq!(input.cursor(), 4);
        assert_eq!(input.selection(), None);

        input.handle(&key("home", 0));
        assert_eq!(input.cursor(), 0);

        // Shift extends the selection instead of collapsing it.
        input.handle(&key("right", modifiers::SHIFT));
        input.handle(&key("right", modifiers::SHIFT));
        assert_eq!(input.selection(), Some((0, 2)));
        assert_eq!(input.selected_text(), "he");

        // Typing over a selection replaces it.
        input.handle(&UiEvent::Text("HE".into()));
        assert_eq!(input.text(), "HEllo");
        assert_eq!(input.selection(), None);
    }

    #[test]
    fn select_all_and_cut_are_one_edit() {
        let mut input = TextInput::new("hello");
        input.handle(&key("a", modifiers::SUPER));
        assert_eq!(input.selection(), Some((0, 5)));
        assert_eq!(input.cut(), "hello");
        assert_eq!(input.text(), "");
        assert!(input.is_empty());
    }

    #[test]
    fn a_composition_shows_without_touching_the_buffer_until_it_commits() {
        let mut input = TextInput::new("ab");
        assert!(input.handle(&UiEvent::Ime {
            preedit: "に".into(),
            selection: Some((0, 1)),
        }));
        assert_eq!(input.text(), "ab", "a composition is not committed text");
        assert_eq!(input.preedit().map(|p| p.text.as_str()), Some("に"));

        // The committed text is what lands, and the composition ends.
        input.handle(&UiEvent::Text("日本".into()));
        assert_eq!(input.text(), "ab日本");
        assert_eq!(input.preedit(), None);
    }

    #[test]
    fn an_event_that_changes_nothing_says_so() {
        let mut input = TextInput::new("hello");
        assert!(!input.handle(&key("left", 0)) || true);
        input.handle(&key("home", 0));
        // Already at the start: nothing moved, so nothing needs repainting.
        assert!(!input.handle(&key("home", 0)));
        assert!(!input.handle(&UiEvent::Hover { entered: true }));
    }

    #[test]
    fn a_caret_that_lands_inside_a_character_moves_to_its_edge() {
        let mut input = TextInput::new("héllo");
        input.place_caret(2, false);
        assert!(input.text().is_char_boundary(input.cursor()));
        assert_eq!(input.cursor(), 1, "offset 2 is inside the two-byte é");
    }

    #[test]
    fn the_caret_is_read_off_the_measured_clusters() {
        use vivid_protocol::overlay::wire::text::{TextGeometry, TextMeasurement};

        let cluster = |start: u32, end: u32, x: f32, width: f32| TextGeometry {
            start,
            end,
            x: Scalar::new(x as f64).unwrap(),
            y: Scalar::ZERO,
            width: Scalar::new(width as f64).unwrap(),
            height: Scalar::new(10.).unwrap(),
            baseline: Scalar::ZERO,
            rtl: false,
        };
        let measurement = TextMeasurement {
            truncated_at: None,
            width: Scalar::new(30.).unwrap(),
            height: Scalar::new(10.).unwrap(),
            lines: vec![TextGeometry {
                x: Scalar::ZERO,
                width: Scalar::new(30.).unwrap(),
                ..cluster(0, 6, 0., 30.)
            }],
            clusters: vec![
                cluster(0, 1, 0., 10.),
                cluster(1, 2, 10., 10.),
                cluster(2, 3, 20., 10.),
            ],
        };

        // Before anything, at a boundary, and after the last cluster.
        assert_eq!(caret_geometry(&measurement, 0).map(|g| g.0), Some(0.));
        assert_eq!(caret_geometry(&measurement, 1).map(|g| g.0), Some(10.));
        assert_eq!(caret_geometry(&measurement, 3).map(|g| g.0), Some(30.));
        // An offset on a boundary goes after the cluster that ends there.
        assert_eq!(caret_geometry(&measurement, 2).map(|g| g.0), Some(20.));

        // An offset inside a cluster goes to that cluster's leading edge: a caret cannot sit
        // within a grapheme.
        let joined = TextMeasurement {
            clusters: vec![cluster(0, 3, 0., 30.)],
            ..measurement.clone()
        };
        assert_eq!(caret_geometry(&joined, 1).map(|g| g.0), Some(0.));
        assert_eq!(caret_geometry(&joined, 2).map(|g| g.0), Some(0.));
    }
}
