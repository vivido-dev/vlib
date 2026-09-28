//! The application: entities, the context they render from, and the loop that turns host events
//! into frames.
//!
//! One loop, one thread. An event arrives, the entities it concerns are updated, and if anything
//! asked to be repainted the frame is rebuilt and submitted — but only when the previous frame
//! has been presented, so a fast producer cannot bury the host in superseded scenes.

use std::cell::{Cell, RefCell};
use std::io;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use vivid_sdk::overlay::{Environment, OverlaySession, OverlayWindow, OverlayWindowOptions};
use vivid_sdk::{OverlayLaneEvent, ProducerConfig};

use crate::vui::element::IntoElement;
use crate::vui::focus::FocusHandle;
use crate::vui::geometry::{Bounds, Size};
use crate::vui::interactive::{Handler, UiEvent};
use crate::vui::node::Node;
use crate::vui::window::{self, IDLE_TICK, WindowShared};

/// Which window, in the order the application opened them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct WindowId(usize);

impl WindowId {
    #[cfg(feature = "testing")]
    pub(crate) fn index(self) -> usize {
        self.0
    }
}

/// A shared handle to one piece of application state.
///
/// The handle is what makes a callback able to reach the state it belongs to: an element holds a
/// clone of it, so a click can update the view that drew the element without the caller passing
/// anything around.
pub struct Entity<T> {
    id: u64,
    state: Rc<RefCell<T>>,
}

impl<T> Clone for Entity<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            state: Rc::clone(&self.state),
        }
    }
}

impl<T: 'static> Entity<T> {
    /// Borrow the state for as long as the closure runs.
    ///
    /// A closure that reaches back into the same entity while it is borrowed is a programming
    /// error, and panics where the cycle is, not somewhere later.
    pub fn update<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        f(&mut self.state.borrow_mut())
    }

    pub fn read<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        f(&self.state.borrow())
    }

    pub fn id(&self) -> u64 {
        self.id
    }
}

impl<T> std::fmt::Debug for Entity<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Entity").field("id", &self.id).finish()
    }
}

/// State the whole application shares.
pub(crate) struct AppShared {
    pub(crate) next_entity: Cell<u64>,
    pub(crate) quit: Cell<bool>,
}

impl AppShared {
    fn new() -> Self {
        Self {
            next_entity: Cell::new(1),
            quit: Cell::new(false),
        }
    }

    fn entity_id(&self) -> u64 {
        let id = self.next_entity.get();
        self.next_entity.set(id + 1);
        id
    }
}

/// What a view can do to the window it is being drawn into.
pub struct Context<T> {
    pub(crate) entity: Entity<T>,
    pub(crate) window: Rc<WindowShared>,
    pub(crate) app: Rc<AppShared>,
}

impl<T: 'static> Context<T> {
    /// The state this context renders.
    pub fn entity(&self) -> &Entity<T> {
        &self.entity
    }

    /// Ask for another frame. Nothing repaints until this is called.
    pub fn notify(&self) {
        self.window.mark_dirty();
    }

    /// The window's own rectangle, in the window's logical pixels.
    pub fn bounds(&self) -> Bounds {
        self.window.bounds.get()
    }

    pub fn size(&self) -> Size {
        self.window.viewport.get()
    }

    /// What the host said about its fonts, appearance, motion preference, and refresh rate.
    pub fn environment(&self) -> Environment {
        self.window.environment()
    }

    /// Whether the host considers this window focused. Typing arrives only when it is.
    pub fn is_focused(&self) -> bool {
        self.window.host_focused.get()
    }

    /// Bind keys to actions, window-wide.
    ///
    /// Binding the same keystroke twice replaces the earlier binding, so calling this from a
    /// frame is harmless: the later declaration wins.
    pub fn bind_keys(&self, bindings: impl IntoIterator<Item = crate::vui::keymap::KeyBinding>) {
        let mut held = self.window.bindings.borrow_mut();
        for binding in bindings {
            held.retain(|existing| existing.keystroke != binding.keystroke);
            held.push(binding);
        }
    }

    /// Give an element keyboard focus, as a click on it would.
    pub fn focus(&self, handle: FocusHandle) {
        self.window.focus(handle.clone(), true);
    }

    /// Give an element focus without marking it keyboard-reached, so a focus ring does not
    /// appear because someone used a mouse.
    pub fn focus_from_pointer(&self, handle: FocusHandle) {
        self.window.focus(handle, false);
    }

    pub fn blur(&self) {
        self.window.blur();
    }

    /// Move focus to the next element that asked to be reachable.
    pub fn focus_next(&self) {
        self.window.focus_next(false, true);
    }

    pub fn focus_previous(&self) {
        self.window.focus_next(true, true);
    }

    /// Upload pixels and keep them alive as long as the handle is held.
    ///
    /// An image is referenced by an id, so the handle is what makes the id mean something: drop
    /// it and the next frame's reference is refused.
    pub fn upload_rgba(
        &self,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> io::Result<vivid_sdk::overlay::RetainedImage> {
        match self.window.overlay() {
            Some(overlay) => overlay.upload_rgba(width, height, rgba),
            None => Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "no host window",
            )),
        }
    }

    /// Keep receiving pointer events after the pointer leaves the element that was pressed.
    ///
    /// A drag turns this on when it starts and off when it ends; without it the moves stop at
    /// the element's edge, and a drag that lets go outside would never hear about the release.
    pub fn capture_pointer(&self, capture: bool) -> io::Result<()> {
        match (self.window.session(), self.window.overlay()) {
            (Some(session), Some(overlay)) => session.capture_pointer(&overlay, capture),
            _ => Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "no host window",
            )),
        }
    }

    /// How long this window has been up. Everything that moves is a function of this.
    pub fn elapsed(&self) -> Duration {
        self.window.clock().elapsed
    }

    /// How long it has been since the last frame.
    pub fn delta(&self) -> Duration {
        self.window.delta()
    }

    /// Whether the user has asked for less movement.
    ///
    /// A host that cannot tell reports nothing rather than guessing, and nothing is not a
    /// preference for stillness — so animating is what an unanswered question means.
    pub fn reduced_motion(&self) -> bool {
        self.window
            .environment
            .borrow()
            .reduced_motion
            .unwrap_or(false)
    }

    /// Ask to be drawn again as soon as the display can, which is what something moving does.
    ///
    /// Paced to the host's refresh interval when it reports one: asking faster than the display
    /// draws only produces scenes that supersede each other.
    pub fn request_frame(&self) {
        self.window.request_frame(self.frame_interval());
    }

    /// Ask to be drawn again after a wait of this caller's choosing.
    pub fn request_frame_after(&self, after: Duration) {
        self.window.request_frame(after);
    }

    /// How long one frame is on this host.
    fn frame_interval(&self) -> Duration {
        self.window
            .environment
            .borrow()
            .refresh_interval_us
            .map(Duration::from_micros)
            .unwrap_or(crate::vui::animation::DEFAULT_FRAME)
            .max(crate::vui::animation::FASTEST_FRAME)
    }

    /// Where an animation has got to, asking for the next frame while it is still moving.
    ///
    /// One call does both, so an animation cannot be drawn without being scheduled — which is
    /// the bug where something animates once and then stops halfway.
    ///
    /// When the user has asked for less movement, this is the finished value straight away: the
    /// destination is what the movement was for.
    pub fn animate(&self, running: &crate::vui::animation::Running) -> f32 {
        if self.reduced_motion() {
            return 1.;
        }
        let now = self.elapsed();
        if !running.finished(now) {
            self.request_frame();
        }
        running.value(now)
    }

    /// An animation's value scaled between two numbers, scheduling as [`Context::animate`] does.
    pub fn animate_between(
        &self,
        running: &crate::vui::animation::Running,
        from: f32,
        to: f32,
    ) -> f32 {
        from + (to - from) * self.animate(running)
    }

    /// Advance a spring to now and answer where it is, asking for the next frame until it
    /// settles.
    ///
    /// When the user has asked for less movement the spring is put on its target rather than
    /// sent travelling to it.
    pub fn animate_spring(
        &self,
        state: &mut crate::vui::animation::SpringState,
        spring: crate::vui::animation::Spring,
    ) -> f32 {
        if self.reduced_motion() {
            state.value = state.target;
            state.velocity = 0.;
            return state.value;
        }
        state.advance(spring, self.delta());
        if !state.at_rest() {
            self.request_frame();
        }
        state.value
    }

    /// Where an element was laid out in the frame just painted.
    ///
    /// Handlers run between frames, so this is the frame the user is looking at.
    pub fn bounds_of(&self, id: &str) -> Option<Bounds> {
        self.window.bounds_of(id)
    }

    /// Copy text out, on the user's behalf. The host refuses this unless a recent key or pointer
    /// press in this window caused it, so it belongs in an event handler and nowhere else.
    pub fn copy(&self, text: &str) -> io::Result<()> {
        match self.window.overlay() {
            Some(overlay) => overlay.set_clipboard(text),
            None => Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "no host window",
            )),
        }
    }

    /// Close the whole application after the current event.
    pub fn quit(&self) {
        self.app.quit.set(true);
    }

    /// Turn a closure over this view's state into an element callback.
    pub fn listener(&self, f: impl Fn(&mut T, &UiEvent, &mut Context<T>) + 'static) -> Handler {
        let entity = self.entity.clone();
        let window = Rc::clone(&self.window);
        let app = Rc::clone(&self.app);
        Box::new(move |event: &UiEvent| {
            let mut cx = Context {
                entity: entity.clone(),
                window: window.clone(),
                app: app.clone(),
            };
            entity.update(|state| f(state, event, &mut cx));
        })
    }
}

/// A view: state that renders itself into an element tree.
pub trait Render: Sized + 'static {
    fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement;
}

/// The object-safe form the loop can hold. One per window.
pub(crate) trait RenderObject {
    fn render(&mut self, window: &Rc<WindowShared>, app: &Rc<AppShared>) -> Node;

    /// The view's state handle, so the application can hand it back to whoever opened the window.
    fn entity_any(&self) -> &dyn std::any::Any;
}

struct ViewSlot<T> {
    entity: Entity<T>,
}

impl<T: Render> RenderObject for ViewSlot<T> {
    fn render(&mut self, window: &Rc<WindowShared>, app: &Rc<AppShared>) -> Node {
        let mut cx = Context {
            entity: self.entity.clone(),
            window: Rc::clone(window),
            app: Rc::clone(app),
        };
        self.entity
            .update(|state| state.render(&mut cx).into_node())
    }

    fn entity_any(&self) -> &dyn std::any::Any {
        &self.entity
    }
}

struct WindowEntry {
    shared: Rc<WindowShared>,
    view: Box<dyn RenderObject>,
    overlay: Arc<OverlayWindow>,
}

/// A running UI: one session, one or more windows, one loop.
pub struct UiApp {
    session: Rc<OverlaySession>,
    shared: Rc<AppShared>,
    windows: Vec<WindowEntry>,
}

impl UiApp {
    /// Connect using the SDK's environment settings.
    pub fn from_env() -> io::Result<Self> {
        Self::connect(ProducerConfig::default())
    }

    pub fn connect(config: ProducerConfig) -> io::Result<Self> {
        Ok(Self {
            session: Rc::new(OverlaySession::connect(config)?),
            shared: Rc::new(AppShared::new()),
            windows: Vec::new(),
        })
    }

    pub fn session(&self) -> &OverlaySession {
        &self.session
    }

    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// Open a host window and render `view` into it.
    pub fn open_window<V: Render + 'static>(
        &mut self,
        options: OverlayWindowOptions,
        view: V,
    ) -> io::Result<WindowId> {
        let bounds = Bounds::from_wire(options.bounds);
        let overlay = Arc::new(self.session.create_window(options)?);
        let shared = WindowShared::new(bounds, Size::new(bounds.width().0, bounds.height().0));
        *shared.overlay.borrow_mut() = Some(Arc::clone(&overlay));
        *shared.session.borrow_mut() = Some(Rc::clone(&self.session));
        let entity = Entity {
            id: self.shared.entity_id(),
            state: Rc::new(RefCell::new(view)),
        };
        self.windows.push(WindowEntry {
            shared,
            view: Box::new(ViewSlot { entity }),
            overlay,
        });
        Ok(WindowId(self.windows.len() - 1))
    }

    /// The state handle for a window's view, for whoever opened it.
    pub fn entity<V: Render>(&self, window: WindowId) -> Option<Entity<V>> {
        self.windows
            .get(window.0)?
            .view
            .entity_any()
            .downcast_ref::<Entity<V>>()
            .cloned()
    }

    /// Run until every window is closed or an entity asks to quit.
    pub fn run(&mut self) -> io::Result<()> {
        self.paint()?;
        while !self.shared.quit.get() {
            self.step(IDLE_TICK)?;
        }
        self.close()
    }

    /// One turn of the loop: resolve what is outstanding, repaint if a frame is owed, and take
    /// one host event. Returns whether anything happened.
    ///
    /// A test drives the same turns the application does, so a test cannot pass by waiting in a
    /// way the real loop does not.
    pub fn step(&mut self, timeout: Duration) -> io::Result<bool> {
        let mut progressed = false;
        for entry in &self.windows {
            // A resolved submission is what paces the next frame; an unresolved one is waited
            // on rather than painted over. The moment a scene is replaced is also the moment
            // the shapes only it drew can be released.
            if window::poll_submission(&entry.shared)?.is_some() {
                window::advance_text(&entry.shared)?;
                progressed = true;
            }
        }
        if self
            .windows
            .iter()
            .any(|entry| entry.shared.ready_to_paint())
        {
            self.paint()?;
            return Ok(true);
        }
        // Nothing sleeps past the moment something moving asked to be drawn at.
        let timeout = self
            .windows
            .iter()
            .filter_map(|entry| entry.shared.wake_in())
            .min()
            .map_or(timeout, |wake| wake.min(timeout));
        if let Some(event) = self.session.wait_event(timeout)? {
            self.dispatch(event)?;
            return Ok(true);
        }
        Ok(progressed)
    }

    /// Build and submit a frame for every window that owes one.
    pub fn paint(&mut self) -> io::Result<()> {
        for entry in &mut self.windows {
            if !entry.shared.ready_to_paint() {
                continue;
            }
            let painted = window::build_frame(&entry.shared, &self.shared, &mut *entry.view)?;
            window::submit_if_changed(&entry.shared, &painted)?;
        }
        Ok(())
    }

    /// Run one frame for one window, without waiting for anything. Used by tests.
    #[cfg_attr(not(feature = "testing"), allow(dead_code))]
    pub fn paint_once(&mut self, index: usize) -> io::Result<()> {
        let entry = &mut self.windows[index];
        let painted = window::build_frame(&entry.shared, &self.shared, &mut *entry.view)?;
        window::submit_if_changed(&entry.shared, &painted)?;
        Ok(())
    }

    /// Mark a window dirty, as an entity's `notify` does.
    #[cfg_attr(not(feature = "testing"), allow(dead_code))]
    pub fn notify(&mut self, index: usize) {
        self.windows[index].shared.mark_dirty();
    }

    #[cfg(feature = "testing")]
    pub(crate) fn window(&self, index: usize) -> &Rc<WindowShared> {
        &self.windows[index].shared
    }

    fn dispatch(&mut self, event: OverlayLaneEvent) -> io::Result<()> {
        match event {
            OverlayLaneEvent::Input(input) => {
                for entry in &self.windows {
                    if self.session.event_targets(&input, &entry.overlay)? {
                        entry.shared.route(&input.event)?;
                        break;
                    }
                }
            }
            OverlayLaneEvent::Viewport(update) => {
                for entry in &self.windows {
                    entry.shared.pane_viewport.set(Size::new(
                        update.viewport.width.get() as f32,
                        update.viewport.height.get() as f32,
                    ));
                    // How many device pixels a logical one is. A vector is rasterized at this,
                    // so a scaled display gets a sharp picture rather than a stretched one.
                    let scale = if update.viewport.scale_denominator > 0 {
                        update.viewport.scale_numerator as f32
                            / update.viewport.scale_denominator as f32
                    } else {
                        1.
                    };
                    if (entry.shared.scale.get() - scale).abs() > f32::EPSILON {
                        entry.shared.scale.set(scale.clamp(0.25, 8.));
                        // Everything rasterized for the old scale is the wrong size now.
                        entry.shared.mark_dirty();
                    }
                }
            }
            OverlayLaneEvent::Environment(update) => {
                for entry in &self.windows {
                    let changed = *entry.shared.environment.borrow() != update.environment;
                    *entry.shared.environment.borrow_mut() = update.environment.clone();
                    if changed {
                        // Fonts and appearance come from here, so every measurement and every
                        // shape this window is holding is out of date at once.
                        if let Some(overlay) = entry.shared.overlay() {
                            entry.shared.text.borrow_mut().invalidate(&overlay);
                        }
                        entry.shared.mark_dirty();
                    }
                }
            }
            OverlayLaneEvent::Outcome(_) => {
                // Submission outcomes pace the loop; poll_submission is where they are read.
            }
            OverlayLaneEvent::Accessibility { node, action, .. } => {
                // The window that described the node is the one the action belongs to. That is a
                // better question than which window the address names: a node exists because a
                // window described it, and only the window still describing it can answer.
                for entry in &self.windows {
                    let describes = entry
                        .shared
                        .described
                        .borrow()
                        .owners
                        .iter()
                        .any(|(id, _)| *id == node);
                    if describes {
                        entry
                            .shared
                            .route(&vivid_protocol::overlay::Event::Accessibility {
                                node,
                                action,
                            })?;
                        break;
                    }
                }
            }
            OverlayLaneEvent::ConnectionLost { diagnostic } => {
                return Err(io::Error::new(io::ErrorKind::ConnectionAborted, diagnostic));
            }
        }
        Ok(())
    }

    /// Close every window, then the session.
    pub fn close(&mut self) -> io::Result<()> {
        let mut first_error = None;
        for entry in &self.windows {
            // Uploads and shapes are the host's memory, held on this window's behalf. Closing
            // the window releases them here rather than leaving them to the session teardown.
            entry.shared.assets.borrow_mut().clear(&entry.overlay);
            entry.shared.text.borrow_mut().invalidate(&entry.overlay);
            if let Err(error) = entry.overlay.close()
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        }
        self.windows.clear();
        let closed = self.session.close();
        match (first_error, closed) {
            (Some(error), _) => Err(error),
            (None, Err(error)) => Err(error),
            (None, Ok(())) => Ok(()),
        }
    }
}

impl Drop for UiApp {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
