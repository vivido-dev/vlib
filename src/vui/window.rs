//! The window a UI lives in: one frame pipeline, one input router, one host window.
//!
//! The host does the hit testing, so this module never asks "what is under the pointer". It asks
//! what the host said was under the pointer, which is the region number, and turns that back into
//! an element by recomputing the same path hash painting used.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::io;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use vivid_protocol::overlay::wire::text::styled::MAX_RETAINED_LAYOUTS;
use vivid_protocol::overlay::{Event, Semantics};
use vivid_protocol::vector::{Canvas, Rect};
use vivid_sdk::overlay::{Environment, OverlaySubmission, OverlayWindow};

use crate::vui::app::AppShared;
use crate::vui::focus::{FocusHandle, FocusOrder};
use crate::vui::geometry::{Bounds, Point, Size};
use crate::vui::interactive::{ElementState, Handlers, Modifiers, MouseButton, UiEvent};
use crate::vui::keymap::{ActionId, KeyBinding};
use crate::vui::node::{ElementId, Node, TextSpec, region_id};
use crate::vui::paint::{self, Painted};

/// How long the loop sleeps when nothing is animating and nothing is pending.
pub(crate) const IDLE_TICK: Duration = Duration::from_millis(100);

/// How long a submission may stay unresolved before the loop stops waiting for it and builds the
/// next frame anyway. A host that has stalled must not freeze the application.
pub(crate) const SUBMISSION_TIMEOUT: Duration = Duration::from_secs(2);

/// The state a window keeps between frames, shared with the entities that render into it.
pub(crate) struct WindowShared {
    pub(crate) dirty: Cell<bool>,
    pub(crate) bounds: Cell<Bounds>,
    /// The window's own box, which is what the frame lays out against.
    pub(crate) viewport: Cell<Size>,
    /// The whole pane's box, for anything that has to know how much room the window sits in.
    pub(crate) pane_viewport: Cell<Size>,
    /// Host focus, which is what makes typing land here at all.
    pub(crate) host_focused: Cell<bool>,
    pub(crate) environment: RefCell<Environment>,
    pub(crate) states: RefCell<HashMap<u64, ElementState>>,
    pub(crate) focus_order: RefCell<FocusOrder>,
    /// Key bindings, most recently declared last so a later one can override an earlier one.
    pub(crate) bindings: RefCell<Vec<KeyBinding>>,
    pub(crate) focused: RefCell<Option<FocusHandle>>,
    pub(crate) focus_visible: Cell<bool>,
    pub(crate) hovered: Cell<Option<u64>>,
    /// The region a press started on, and the click count the host gave that press. A release
    /// carries no count of its own, so the press is what a click is made of.
    pub(crate) pressed: Cell<Option<(u64, u8)>>,
    pub(crate) pointer: Cell<Point>,
    /// Device pixels per logical pixel, as the host reports it. A vector is rasterized at this.
    pub(crate) scale: Cell<f32>,
    /// When this window opened, which is what an animation's elapsed time is measured from.
    pub(crate) started: Instant,
    /// When something moving wants the next frame, on the window's own clock rather than the
    /// wall's. `None` means nothing is moving.
    ///
    /// The window's clock is what everything moving is a function of, so it is what the next
    /// frame is due on too. That also means a caller who has stopped the clock — a test — has
    /// stopped the animations, rather than having to outrun them.
    pub(crate) wake_at: Cell<Option<Duration>>,
    /// How soon a view asked to be drawn again, this frame. Merged with whatever the painter
    /// found moving, so a view and an animated picture cannot each overwrite the other's answer.
    pub(crate) requested_wake: Cell<Option<Duration>>,
    /// The window clock at the last frame, which is what a spring is integrated by.
    pub(crate) last_frame: Cell<Duration>,
    /// Elapsed time, when a caller is driving it rather than the clock.
    ///
    /// A test is the caller that does: an animation is a function of time, so a test that could
    /// not choose the time could only assert by sleeping, which is how a suite becomes flaky.
    pub(crate) elapsed_override: Cell<Option<Duration>>,
    pub(crate) text: RefCell<crate::vui::text::TextSystem>,
    pub(crate) assets: RefCell<crate::vui::assets::Assets>,
    pub(crate) overlay: RefCell<Option<Arc<OverlayWindow>>>,
    /// The session the window belongs to: a few operations are the session's, not the window's,
    /// and capture is one of them.
    pub(crate) session: RefCell<Option<Rc<vivid_sdk::overlay::OverlaySession>>>,
    pub(crate) frame: RefCell<Option<Frame>>,
    pub(crate) submitted: RefCell<Option<Canvas>>,
    /// The caret the frame just painted placed, if a field had focus.
    pub(crate) editor_caret: RefCell<Option<Rect>>,
    /// Where each identified element was laid out in the frame just painted.
    pub(crate) element_bounds: RefCell<Vec<(String, Bounds)>>,
    /// The semantic tree the frame just painted describes, waiting for that frame to be
    /// presented: a description names the scene it belongs to, and until the scene is on screen
    /// there is nothing for it to name.
    pub(crate) pending_semantics: RefCell<Option<Semantics>>,
    /// The description the last frame built, kept so it can be published once that frame is
    /// presented.
    pub(crate) described: RefCell<crate::vui::a11y::Described>,
    /// Whether the host takes descriptions at all. A host that does not is asked once.
    pub(crate) describes: Cell<bool>,
    /// Semantic nodes left out of the last frame because the tree was already as large as the
    /// protocol allows.
    pub(crate) dropped_semantics: Cell<usize>,
    pub(crate) pending: RefCell<Option<OverlaySubmission>>,
    pub(crate) pending_since: Cell<Option<Instant>>,
    pub(crate) dropped_shadows: Cell<usize>,
    /// Pictures the last frame could not decode, counted like the shadows: the box they would
    /// have been drawn in is on screen and nothing else says the picture is missing.
    pub(crate) dropped_images: Cell<usize>,
}

impl WindowShared {
    pub(crate) fn new(bounds: Bounds, viewport: Size) -> Rc<Self> {
        Rc::new(Self {
            dirty: Cell::new(true),
            bounds: Cell::new(bounds),
            viewport: Cell::new(viewport),
            pane_viewport: Cell::new(viewport),
            host_focused: Cell::new(false),
            environment: RefCell::new(Environment::default()),
            states: RefCell::new(HashMap::new()),
            focus_order: RefCell::new(FocusOrder::default()),
            bindings: RefCell::new(Vec::new()),
            focused: RefCell::new(None),
            focus_visible: Cell::new(false),
            hovered: Cell::new(None),
            pressed: Cell::new(None),
            pointer: Cell::new(Point::ZERO),
            scale: Cell::new(1.),
            started: Instant::now(),
            wake_at: Cell::new(None),
            requested_wake: Cell::new(None),
            last_frame: Cell::new(Duration::ZERO),
            elapsed_override: Cell::new(None),
            text: RefCell::new(crate::vui::text::TextSystem::default()),
            assets: RefCell::new(crate::vui::assets::Assets::default()),
            overlay: RefCell::new(None),
            session: RefCell::new(None),
            frame: RefCell::new(None),
            submitted: RefCell::new(None),
            editor_caret: RefCell::new(None),
            element_bounds: RefCell::new(Vec::new()),
            pending_semantics: RefCell::new(None),
            described: RefCell::new(crate::vui::a11y::Described::default()),
            describes: Cell::new(true),
            dropped_semantics: Cell::new(0),
            pending: RefCell::new(None),
            pending_since: Cell::new(None),
            dropped_shadows: Cell::new(0),
            dropped_images: Cell::new(0),
        })
    }

    pub(crate) fn overlay(&self) -> Option<Arc<OverlayWindow>> {
        self.overlay.borrow().clone()
    }

    pub(crate) fn session(&self) -> Option<Rc<vivid_sdk::overlay::OverlaySession>> {
        self.session.borrow().clone()
    }

    pub(crate) fn mark_dirty(&self) {
        self.dirty.set(true);
    }

    /// Whether a frame is owed, and whether the previous one has settled.
    pub(crate) fn ready_to_paint(&self) -> bool {
        // Something moving is owed a frame at the moment it asked for, without anyone having to
        // call `notify` for it: an animation that had to be poked every frame would be a timer the
        // application had to run itself.
        if !self.dirty.get() && self.wake_due() {
            self.dirty.set(true);
        }
        if !self.dirty.get() {
            return false;
        }
        match self.pending_since.get() {
            None => true,
            Some(started) => started.elapsed() >= SUBMISSION_TIMEOUT,
        }
    }

    /// Ask to be drawn again in `after`, or sooner if something else already asked sooner.
    pub(crate) fn request_frame(&self, after: Duration) {
        let soonest = self
            .requested_wake
            .get()
            .map_or(after, |held| held.min(after));
        self.requested_wake.set(Some(soonest));
    }

    /// How long it has been since the last frame, which is what a spring advances by.
    pub(crate) fn delta(&self) -> Duration {
        self.clock().elapsed.saturating_sub(self.last_frame.get())
    }

    /// Whether the moment something asked to be redrawn at has arrived.
    pub(crate) fn wake_due(&self) -> bool {
        self.wake_at
            .get()
            .is_some_and(|at| self.clock().elapsed >= at)
    }

    /// How long until the next frame something moving asked for.
    pub(crate) fn wake_in(&self) -> Option<Duration> {
        self.wake_at
            .get()
            .map(|at| at.saturating_sub(self.clock().elapsed))
    }

    /// What a moving thing measures itself against.
    pub(crate) fn clock(&self) -> crate::vui::paint::Clock {
        crate::vui::paint::Clock {
            elapsed: self
                .elapsed_override
                .get()
                .unwrap_or_else(|| self.started.elapsed()),
            scale: self.scale.get(),
        }
    }

    pub(crate) fn set_state(&self, region: u64, update: impl FnOnce(&mut ElementState)) {
        let mut states = self.states.borrow_mut();
        let entry = states.entry(region).or_default();
        let before = *entry;
        update(entry);
        if *entry != before {
            drop(states);
            self.mark_dirty();
        }
    }

    /// Route one host event. Returns whether the window asked to be repainted.
    pub(crate) fn route(&self, event: &Event) -> io::Result<()> {
        match event {
            Event::Pointer {
                region,
                position,
                button,
                clicks,
                modifiers,
                pressure,
                ..
            } => {
                let position = Point::from_wire(*position);
                self.pointer.set(position);
                let modifiers = Modifiers::from_bits(*modifiers);
                match button {
                    Some((id, true)) => {
                        let button = MouseButton(*id);
                        // A press in a window is the user putting it in front, and the host
                        // delivers keys only to a window that asked. Clicking anything here asks
                        // for focus, which is what clicking a window does everywhere else.
                        if let Some(overlay) = self.overlay() {
                            let _ = overlay.request_focus();
                        }
                        // Pressing an element makes it active wherever it lives, including when
                        // the press started outside it: that is what a drag from a button is.
                        self.set_state(*region, |state| state.pressed = true);
                        self.pressed.set(Some((*region, *clicks)));
                        self.deliver(
                            *region,
                            UiEvent::MouseDown {
                                position,
                                button,
                                modifiers,
                            },
                            Slot::MouseDown,
                        );
                    }
                    Some((id, false)) => {
                        let button = MouseButton(*id);
                        self.deliver(
                            *region,
                            UiEvent::MouseUp {
                                position,
                                button,
                                modifiers,
                            },
                            Slot::MouseUp,
                        );
                        // A click belongs to the element the press started on, and only if the
                        // release landed on that same element. The count comes from the press:
                        // a release carries no count of its own.
                        if let Some((pressed, clicks)) = self.pressed.take() {
                            self.set_state(pressed, |state| state.pressed = false);
                            if pressed == *region && clicks > 0 {
                                self.deliver(
                                    *region,
                                    UiEvent::Click {
                                        position,
                                        clicks,
                                        modifiers,
                                    },
                                    Slot::Click,
                                );
                            }
                        }
                    }
                    None => {
                        self.deliver(*region, UiEvent::MouseMove { position }, Slot::MouseMove);
                        // Pressure rides with the pointer and is reported separately: a host
                        // that has no sensor sends nothing rather than a value it made up.
                        if let Some(pressure) = pressure {
                            self.deliver(
                                *region,
                                UiEvent::Pressure {
                                    position,
                                    pressure: pressure.get() as f32,
                                },
                                Slot::Pressure,
                            );
                        }
                    }
                }
            }
            Event::Hover { region, entered } => {
                if *entered {
                    self.hovered.set(Some(*region));
                } else if self.hovered.get() == Some(*region) {
                    self.hovered.set(None);
                }
                self.set_state(*region, |state| state.hovered = *entered);
                // The pointer left an element it had pressed. A release outside every region is
                // not reported at all, so leaving is the only signal that a press is over — and
                // without it a button stays pressed for good.
                if !*entered
                    && self
                        .pressed
                        .get()
                        .is_some_and(|(pressed, _)| pressed == *region)
                {
                    self.pressed.set(None);
                    self.set_state(*region, |state| state.pressed = false);
                }
                self.deliver(*region, UiEvent::Hover { entered: *entered }, Slot::Hover);
            }
            Event::Wheel {
                dx,
                dy,
                precise,
                phase,
                ..
            } => {
                let delta = Point::new(dx.get() as f32, dy.get() as f32);

                let phase = *phase;
                let target = self.hovered.get().unwrap_or(0);
                self.deliver(
                    target,
                    UiEvent::Wheel {
                        delta,
                        precise: *precise,
                        phase,
                    },
                    Slot::Wheel,
                );
            }
            Event::Key {
                physical,
                down,
                repeat,
                modifiers,
            } => {
                let modifiers = Modifiers::from_bits(*modifiers);
                self.deliver_focused(
                    UiEvent::Key {
                        physical: *physical,
                        down: *down,
                        repeat: *repeat,
                        modifiers,
                    },
                    Slot::Key,
                );
                // A binding fires on the press, once: a held key that repeats is still one
                // action, and the release that follows is not a second one.
                if *down
                    && !*repeat
                    && let Some(action) = self.binding_for(*physical, modifiers)
                {
                    self.deliver_focused(UiEvent::Action(action), Slot::Action(action.0));
                }
            }
            Event::Text(text) => {
                self.deliver_focused(UiEvent::Text(text.clone()), Slot::Text);
            }
            Event::Geometry { bounds, settled: _ } => {
                let bounds = Bounds::from_wire(*bounds);
                self.bounds.set(bounds);
                self.viewport.set(bounds.size);
                self.mark_dirty();
            }
            Event::Focus(focused) => {
                self.host_focused.set(*focused);
                if !*focused {
                    self.focus_visible.set(false);
                    // A press cannot survive the pointer leaving the window, and a window that
                    // lost focus is a window the pointer is not over.
                    if let Some((pressed, _)) = self.pressed.take() {
                        self.set_state(pressed, |state| state.pressed = false);
                    }
                }
                self.mark_dirty();
            }
            Event::Dismissed(_) => {
                // Escape and friends reach the window as a whole: only the window knows whether
                // that closes a popover or the window itself.
                self.deliver_root(UiEvent::Dismissed, Slot::Key);
            }
            Event::Cancel => {
                self.mark_dirty();
            }
            Event::Ime { preedit, selection } => {
                self.deliver_focused(
                    UiEvent::Ime {
                        preedit: preedit.clone(),
                        selection: *selection,
                    },
                    Slot::Key,
                );
            }
            Event::Accessibility { node, action } => {
                // An interactive node's identity is the one a click uses, so an action routes
                // back through exactly the path every other event takes.
                self.deliver(
                    *node,
                    UiEvent::Accessibility { action: *action },
                    Slot::Accessibility,
                );
            }
        }
        Ok(())
    }

    fn deliver_focused(&self, event: UiEvent, slot: Slot) {
        let focused = self.focused.borrow().clone();
        if let Some(handle) = focused {
            let region = self.focus_order.borrow().region_for(handle.id());
            if let Some(region) = region {
                self.deliver(region, event, slot);
                return;
            }
        }
        self.deliver_root(event, slot);
    }

    fn deliver(&self, region: u64, event: UiEvent, slot: Slot) {
        let mut frame = self.frame.borrow_mut();
        let Some(frame) = frame.as_mut() else {
            return;
        };
        let mut path = Vec::new();
        if !deliver(&mut frame.tree, &mut path, region, &event, slot)
            && let Some(handler) = handler_for(&mut frame.tree.interactions.handlers, slot)
        {
            // Nothing under the pointer wanted it, so it belongs to the window: that is what a
            // handler on the root element is for.
            handler(&event);
        }
    }

    /// Hand an event to the window as a whole, after nothing inside it wanted it.
    fn deliver_root(&self, event: UiEvent, slot: Slot) {
        let mut frame = self.frame.borrow_mut();
        let Some(frame) = frame.as_mut() else {
            return;
        };
        if let Some(handler) = handler_for(&mut frame.tree.interactions.handlers, slot) {
            handler(&event);
        }
    }

    /// The action a key press names, if the window has a binding for it.
    fn binding_for(&self, key: u32, modifiers: Modifiers) -> Option<ActionId> {
        self.bindings
            .borrow()
            .iter()
            .rev()
            .find(|binding| binding.keystroke.matches(key, modifiers))
            .map(|binding| binding.action)
    }

    /// Move focus to a handle, telling the host so the platform indicator follows.
    pub(crate) fn focus(&self, handle: FocusHandle, visible: bool) {
        *self.focused.borrow_mut() = Some(handle.clone());
        self.focus_visible.set(visible);
        let region = self.focus_order.borrow().region_for(handle.id());
        if let Some(region) = region {
            self.set_state(region, |state| {
                state.focused = true;
                state.focus_visible = visible;
            });
        }
        self.mark_dirty();
        if let Some(overlay) = self.overlay() {
            let _ = overlay.request_focus();
        }
    }

    pub(crate) fn blur(&self) {
        let previous = self.focused.borrow_mut().take();
        self.focus_visible.set(false);
        if let Some(handle) = previous
            && let Some(region) = self.focus_order.borrow().region_for(handle.id())
        {
            self.set_state(region, |state| {
                state.focused = false;
                state.focus_visible = false;
            });
        }
        self.mark_dirty();
    }

    pub(crate) fn focus_next(&self, backwards: bool, visible: bool) {
        let next = {
            let order = self.focus_order.borrow();
            let current = self.focused.borrow().clone();
            order.next(current.as_ref(), backwards)
        };
        if let Some(next) = next {
            self.focus(next, visible);
        }
    }

    /// Where an element was laid out in the frame it was last painted in.
    ///
    /// A drag needs this: the pointer is over an element the drag did not start on, and what
    /// matters is where that element is. The boxes are collected as the frame is built rather
    /// than read back out of the tree, because a handler runs while the tree is being dispatched
    /// to and cannot borrow it again.
    pub(crate) fn bounds_of(&self, id: &str) -> Option<Bounds> {
        self.element_bounds
            .borrow()
            .iter()
            .find(|(name, _)| name == id)
            .map(|(_, bounds)| *bounds)
    }

    /// What the window remembers about one element between frames.
    #[cfg(feature = "testing")]
    pub(crate) fn element_state(&self, region: u64) -> ElementState {
        self.states
            .borrow()
            .get(&region)
            .copied()
            .unwrap_or_default()
    }

    /// The region number an element with this id was last painted as.
    #[cfg_attr(not(feature = "testing"), allow(dead_code))]
    pub(crate) fn region_for(&self, id: &str) -> Option<u64> {
        let frame = self.frame.borrow();
        frame.as_ref().and_then(|frame| {
            frame
                .regions
                .iter()
                .find(|(_, element)| element.as_str() == id)
                .map(|(region, _)| *region)
        })
    }

    /// The environment the host last reported.
    pub(crate) fn environment(&self) -> Environment {
        self.environment.borrow().clone()
    }
}

/// Which callback slot an event looks for on its way up the tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Slot {
    Click,
    MouseDown,
    MouseUp,
    MouseMove,
    Hover,
    Wheel,
    Key,
    Text,
    /// An action by name, which is how a binding reaches a handler.
    Action(&'static str),
    Pressure,
    Accessibility,
}

fn handler_for(
    handlers: &mut Handlers,
    slot: Slot,
) -> Option<&mut crate::vui::interactive::Handler> {
    match slot {
        Slot::Click => handlers.click.as_mut(),
        Slot::MouseDown => handlers.mouse_down.as_mut(),
        Slot::MouseUp => handlers.mouse_up.as_mut(),
        Slot::MouseMove => handlers.mouse_move.as_mut(),
        Slot::Hover => handlers.hover.as_mut(),
        Slot::Wheel => handlers.wheel.as_mut(),
        Slot::Key => handlers.key.as_mut(),
        Slot::Text => handlers.text.as_mut(),
        Slot::Action(id) => handlers.action(id),
        Slot::Pressure => handlers.pressure.as_mut(),
        Slot::Accessibility => handlers.accessibility.as_mut(),
    }
}

/// Deliver to the element the region names, and then to its ancestors, until one of them takes
/// it.
///
/// Bubbling is what makes a row that is clickable and a close button inside it work: the button
/// takes the click, and the row only sees a click that landed on the row itself.
fn deliver(
    node: &mut Node,
    path: &mut Vec<ElementId>,
    target: u64,
    event: &UiEvent,
    slot: Slot,
) -> bool {
    let pushed = node.id.clone().map(|id| {
        path.push(id);
        true
    });
    let mut reached = pushed.is_some() && region_id(&path.iter().collect::<Vec<_>>()) == target;
    for child in &mut node.children {
        // Not an early return: an ancestor still sees an event its descendant ignored.
        if deliver(child, path, target, event, slot) {
            reached = true;
        }
    }
    if reached && let Some(handler) = handler_for(&mut node.interactions.handlers, slot) {
        handler(event);
    }
    if pushed.is_some() {
        path.pop();
    }
    reached
}

/// One painted frame, kept until the next one so input has something to route against.
pub(crate) struct Frame {
    pub(crate) tree: Node,
    pub(crate) regions: HashMap<u64, ElementId>,
}

/// Render, measure, lay out, paint. The whole of one frame.
pub(crate) fn build_frame(
    shared: &Rc<WindowShared>,
    app: &Rc<AppShared>,
    view: &mut dyn crate::vui::app::RenderObject,
) -> io::Result<Painted> {
    shared.requested_wake.set(None);
    let mut tree = view.render(shared, app);
    // The root fills the window unless it says otherwise, which is what makes the frame's
    // geometry definite from the top down. Without it, a chain of auto-sized containers asks
    // taffy for a content size at every level, and that cost doubles per level.
    if tree.layout.width == crate::vui::layout::Length::Auto {
        tree.layout.width = crate::vui::layout::Length::full();
    }
    if tree.layout.height == crate::vui::layout::Length::Auto {
        tree.layout.height = crate::vui::layout::Length::full();
    }

    let overlay = shared.overlay().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotConnected,
            "the window has no host window yet",
        )
    })?;
    let viewport = shared.viewport.get();

    // A virtualized list cannot know which rows it shows until it knows how tall it is, so a
    // frame that holds one is laid out twice: once to find the list's box, once with the rows
    // that box turned out to need. Only a list asks for the second pass, and nothing asks for a
    // third — the rows a range produced do not change the range.
    for pass in 0..2 {
        measure_and_lay_out(shared, &overlay, &mut tree, viewport)?;
        if pass == 1 || !crate::vui::list::fill_lists(&mut tree) {
            break;
        }
    }

    let painted = paint::paint(
        &tree,
        &shared.states.borrow(),
        &mut shared.text.borrow_mut(),
        &mut shared.assets.borrow_mut(),
        &overlay,
        shared.clock(),
    )?;
    // Whatever is moving asked for the next frame: a view that is animating, a picture that has
    // another frame, or both. Nothing moving means the loop can sleep until something happens.
    let wake = match (shared.requested_wake.get(), painted.wake_after) {
        (Some(view), Some(picture)) => Some(view.min(picture)),
        (asked, None) => asked,
        (None, found) => found,
    };
    let now = shared.clock().elapsed;
    shared.wake_at.set(wake.map(|after| now + after));
    shared.last_frame.set(now);
    shared.dropped_shadows.set(painted.dropped_shadows);
    shared.dropped_images.set(painted.dropped_images);
    {
        // Collected before the tree is stored: a handler that needs to know where another
        // element is reads this, and it cannot borrow the frame it is being dispatched from.
        let mut boxes = shared.element_bounds.borrow_mut();
        boxes.clear();
        tree.walk(&mut |node| {
            if let Some(id) = &node.id {
                boxes.push((id.as_str().to_owned(), node.bounds));
            }
        });
    }
    // What the frame says about itself, built now because a node describes where it is and that
    // is only known once it has been laid out.
    let described = crate::vui::a11y::describe(&tree, Bounds::new(Point::ZERO, viewport));
    shared.dropped_semantics.set(described.dropped);
    *shared.described.borrow_mut() = described;

    *shared.frame.borrow_mut() = Some(Frame {
        tree,
        regions: painted.regions.clone(),
    });
    shared
        .focus_order
        .borrow_mut()
        .reset(painted.focus_stops.clone());
    shared.dirty.set(false);
    Ok(painted)
}

/// Measure every run the tree draws, then lay the tree out against the viewport.
fn measure_and_lay_out(
    shared: &Rc<WindowShared>,
    overlay: &OverlayWindow,
    tree: &mut Node,
    viewport: Size,
) -> io::Result<()> {
    let specs: Vec<TextSpec> = {
        let mut specs = Vec::new();
        tree.walk(&mut |node| {
            // A field draws a paragraph too, so it is measured the same way: its caret is read
            // off the measurement, and a field nobody measured has no caret.
            if let Some(text) = node.paragraph() {
                specs.push(text.clone());
            }
        });
        specs
    };
    {
        let mut text = shared.text.borrow_mut();
        // Shaping first, then measuring what was not shaped. A shaped paragraph comes back with
        // its own measurement, so measuring it beforehand is a round trip for an answer already
        // on its way — which, for a table where every cell is ellipsized, is half of them.
        text.prepare_shapes(overlay, &specs, MAX_RETAINED_LAYOUTS)?;
        text.measure(overlay, &specs)?;
    }

    // Decoding comes before layout because an image sized by its content has to know what that
    // content is. A vector is the exception, and says so.
    let sources: Vec<crate::vui::assets::ImageSource> = {
        let mut sources = Vec::new();
        tree.walk(&mut |node| {
            if let Some(image) = node.image_spec() {
                sources.push(image.source.clone());
            }
        });
        sources
    };
    shared.assets.borrow_mut().prepare(&sources);

    let measured = {
        let text = shared.text.borrow();
        specs
            .iter()
            .map(|spec| (crate::vui::text::TextSystem::key_of(spec), text.size(spec)))
            .collect::<HashMap<_, _>>()
    };
    let natural = shared.assets.borrow().natural_sizes();
    crate::vui::layout::compute(tree, viewport, &mut |node: &Node| {
        if let Some(spec) = node.paragraph() {
            return Some(
                measured
                    .get(&crate::vui::text::TextSystem::key_of(spec))
                    .copied()
                    .flatten()
                    // A run nobody measured is drawn at its own line height rather than
                    // collapsed, so a failed measure is visible instead of invisible.
                    .unwrap_or(Size::new(0., spec.line_height())),
            );
        }
        // An image sized by its content is its own size, once anything has decoded it. Until
        // then it has none, and the box it is in decides — which is the frame an image whose
        // bytes are still being read gets.
        node.image_spec()
            .and_then(|image| natural.get(&image.source.key).copied())
    })
}

/// Submit a painted frame, if it differs from what is already on screen.
pub(crate) fn submit_if_changed(shared: &Rc<WindowShared>, painted: &Painted) -> io::Result<bool> {
    // The caret a field painted belongs to the scene it was painted in, and the host places its
    // input method's candidate window from it. It is only published for the frame that carried
    // it, so an editor that moved says so again on the next frame rather than leaving a stale
    // rectangle behind.
    *shared.editor_caret.borrow_mut() = painted.editor_caret;
    let changed = shared.submitted.borrow().as_ref() != Some(&painted.canvas);
    if !changed {
        return Ok(false);
    }
    let overlay = shared.overlay().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotConnected,
            "the window has no host window yet",
        )
    })?;
    let submission = overlay.submit(painted.canvas.clone())?;
    if let Some(caret) = painted.editor_caret {
        // A caret the host cannot place yet is not an error: the scene it belongs to is the one
        // being submitted, and a host that rejects the geometry will reject the next one too.
        let _ = overlay.set_editor_geometry(submission.revision(), Some(caret));
    }
    // The description belongs to this scene and names its revision. It cannot be published yet:
    // a host refuses a description of a scene it is not showing, which is the rule that stops a
    // screen reader ever announcing a control that is no longer there.
    *shared.pending_semantics.borrow_mut() =
        shared.described.borrow().semantics(submission.revision());
    *shared.submitted.borrow_mut() = Some(painted.canvas.clone());
    *shared.pending.borrow_mut() = Some(submission);
    shared.pending_since.set(Some(Instant::now()));
    Ok(true)
}

/// Tell the host what the scene now on screen is showing.
///
/// A host that does not take descriptions is asked once and then left alone: the profile is
/// optional, and a producer that kept asking would be making a request per frame that it already
/// knows the answer to.
fn publish_semantics(shared: &Rc<WindowShared>) -> io::Result<()> {
    let Some(semantics) = shared.pending_semantics.borrow_mut().take() else {
        return Ok(());
    };
    if !shared.describes.get() {
        return Ok(());
    }
    let Some(overlay) = shared.overlay() else {
        return Ok(());
    };
    if let Err(error) = overlay.set_semantics(&semantics) {
        if error.to_string().contains("overlay-a11y-v1") {
            shared.describes.set(false);
            return Ok(());
        }
        // Anything else is this frame's description being wrong, which is worth knowing about
        // rather than swallowing: the next frame would be wrong the same way.
        return Err(error);
    }
    Ok(())
}

/// Let go of the shapes the scene just replaced was drawing.
pub(crate) fn advance_text(shared: &Rc<WindowShared>) -> io::Result<()> {
    let Some(overlay) = shared.overlay() else {
        return Ok(());
    };
    let mut text = shared.text.borrow_mut();
    text.advance(&overlay)
}

/// Check the outstanding submission without waiting for it.
pub(crate) fn poll_submission(
    shared: &Rc<WindowShared>,
) -> io::Result<Option<vivid_sdk::overlay::PresentationOutcome>> {
    let resolved = {
        let mut pending = shared.pending.borrow_mut();
        match pending.as_mut() {
            Some(submission) => submission.wait(Duration::ZERO)?,
            None => None,
        }
    };
    if resolved.is_some() {
        *shared.pending.borrow_mut() = None;
        shared.pending_since.set(None);
    }
    if resolved == Some(vivid_sdk::overlay::PresentationOutcome::Presented) {
        publish_semantics(shared)?;
    } else if resolved.is_some() {
        // A scene that was superseded is not on screen, and the one that replaced it will
        // publish its own description.
        *shared.pending_semantics.borrow_mut() = None;
    }
    Ok(resolved)
}
