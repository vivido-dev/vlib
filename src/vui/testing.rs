//! A UI driven headlessly, so a frame can be asserted without a GPU or a terminal.
//!
//! The presenter is the SDK's own in-process one: the real handshake, the real record framing, and
//! the real display list at the other end. What a test sees here is what a host would see.

use std::io;
use std::time::{Duration, Instant};

use vivid_protocol::vector::{Canvas, CursorShape, Rect};
use vivid_sdk::ProducerConfig;
use vivid_sdk::overlay::{OverlayWindowOptions, WindowMode, buttons};
use vivid_sdk::testing::{ROOT_SECRET_HEX, TestPresenter};

use crate::vui::app::{Entity, Render, UiApp, WindowId};
use crate::vui::geometry::Bounds;
use crate::vui::interactive::ElementState;
use crate::vui::window::WindowShared;

/// How long a test waits for the host to catch up before calling it a failure.
const STEP_TIMEOUT: Duration = Duration::from_secs(5);

/// A window, a host, and the loop that connects them.
pub struct TestUi<V: Render + 'static> {
    app: UiApp,
    presenter: TestPresenter,
    window: WindowId,
    entity: Entity<V>,
}

impl<V: Render + 'static> TestUi<V> {
    /// Start a presenter, connect to it, and open one floating window of `size`.
    pub fn start(view: V, width: f64, height: f64) -> io::Result<Self> {
        let presenter = TestPresenter::start(80, 24)?;
        let config = ProducerConfig {
            endpoint_control: Some(presenter.endpoint().to_owned()),
            authentication: vivid_sdk::ProducerAuthentication::root_hex(ROOT_SECRET_HEX)
                .map_err(io::Error::other)?,
            producer_name: "vlib-vui-test".into(),
            ..ProducerConfig::default()
        };
        let mut app = UiApp::connect(config)?;
        let bounds = Rect::new(0., 0., width, height).map_err(io::Error::other)?;
        let window = app.open_window(
            OverlayWindowOptions::new(bounds, WindowMode::Floating),
            view,
        )?;
        let entity = app
            .entity::<V>(window)
            .ok_or_else(|| io::Error::other("the window did not keep its view"))?;
        let mut ui = Self {
            app,
            presenter,
            window,
            entity,
        };
        // A test drives the clock. Everything that moves is a function of it, so a test that let
        // it run would be asserting against whatever the machine happened to do between two
        // lines — and would have to outrun its own animations to settle.
        ui.shared().elapsed_override.set(Some(Duration::ZERO));
        ui.paint()?;
        Ok(ui)
    }

    pub fn app(&self) -> &UiApp {
        &self.app
    }

    pub fn presenter(&self) -> &TestPresenter {
        &self.presenter
    }

    /// The view's state, for asserting what a handler did to it.
    pub fn read<R>(&self, f: impl FnOnce(&V) -> R) -> R {
        self.entity.read(f)
    }

    pub fn entity(&self) -> &Entity<V> {
        &self.entity
    }

    /// Change the view's state directly, for a test that wants a different starting point.
    pub fn update<R>(&self, f: impl FnOnce(&mut V) -> R) -> R {
        self.entity.update(f)
    }

    pub fn app_mut(&mut self) -> &mut UiApp {
        &mut self.app
    }

    /// Whether the loop is on its way out, as escape or `q` asks it to be.
    pub fn quitting(&self) -> bool {
        self.app.quitting()
    }

    /// Whether an element is currently pressed.
    pub fn pressed(&self, region: u64) -> bool {
        self.shared().element_state(region).pressed
    }

    /// How many shaped paragraphs the host is holding for this window. Retained layouts are the
    /// host's memory, so a test asserts on them the way it asserts on any other budget.
    pub fn retained_layouts(&self) -> usize {
        self.shared().text.borrow().retained()
    }

    /// Move the window's clock forward, and repaint at the new time.
    ///
    /// Anything that moves is a function of elapsed time, so this is how a test watches an
    /// animation without waiting for one.
    pub fn advance(&mut self, by: Duration) -> io::Result<()> {
        let shared = self.shared();
        let now = shared
            .elapsed_override
            .get()
            .unwrap_or_else(|| shared.started.elapsed());
        shared.elapsed_override.set(Some(now + by));
        shared.mark_dirty();
        self.paint()
    }

    /// The window's clock, as anything moving sees it.
    pub fn elapsed(&self) -> Duration {
        let shared = self.shared();
        shared
            .elapsed_override
            .get()
            .unwrap_or_else(|| shared.started.elapsed())
    }

    /// How soon something in the last frame asked to be drawn again, if anything did.
    pub fn wake_in(&self) -> Option<Duration> {
        self.shared().wake_in()
    }

    /// The description the host is holding for what is on screen.
    ///
    /// `None` when the window has described nothing, which is not the same as describing nothing
    /// interesting: a frame with no roles publishes no tree at all.
    pub fn semantics(&self) -> Option<vivid_protocol::overlay::Semantics> {
        self.presenter.overlay_semantics()
    }

    /// One node of that description, by the label it carries.
    pub fn semantic_node(&self, label: &str) -> Option<vivid_protocol::overlay::SemanticNode> {
        self.semantics()?
            .nodes
            .into_iter()
            .find(|node| node.label == label)
    }

    /// Ask for something on the application's behalf, as assistive technology would.
    ///
    /// Answers whether the host had a node to ask about: a node the live description does not
    /// name is refused rather than delivered, so this says which happened.
    pub fn accessibility_action(
        &mut self,
        label: &str,
        action: vivid_protocol::overlay::AccessibleAction,
    ) -> io::Result<bool> {
        let Some(node) = self.semantic_node(label) else {
            return Ok(false);
        };
        let delivered = self
            .presenter
            .overlay_accessibility_action(node.id, action)?;
        self.settle()?;
        Ok(delivered)
    }

    /// How many semantic nodes the last frame had to leave out.
    /// Pictures the last frame could not decode. A view that showed one still has its box on
    /// screen, so this is the only way to know the picture is not there.
    pub fn dropped_images(&self) -> usize {
        self.shared().dropped_images.get()
    }

    pub fn dropped_semantics(&self) -> usize {
        self.shared().dropped_semantics.get()
    }

    /// How many image uploads the host is holding for this window. Uploads are the host's
    /// memory and they are bounded, so a test asserts on them the way it does on any budget.
    pub fn retained_images(&self) -> usize {
        self.shared().assets.borrow().retained()
    }

    /// Everything the window knows about the element a region belongs to.
    pub fn state_at(&self, region: u64) -> ElementState {
        self.shared().element_state(region)
    }

    /// Everything the window knows about one element's interaction state.
    pub fn state_of(&self, id: &str) -> ElementState {
        match self.shared().region_for(id) {
            Some(region) => self.shared().element_state(region),
            None => ElementState::default(),
        }
    }

    pub(crate) fn shared(&self) -> &std::rc::Rc<WindowShared> {
        self.app.window(self.window.index())
    }

    /// Build and submit a frame, then wait for the host to accept it.
    pub fn paint(&mut self) -> io::Result<()> {
        self.app.paint_once(self.window.index())?;
        self.settle()
    }

    /// The display list the host is holding for this window.
    pub fn canvas(&self) -> Canvas {
        self.presenter
            .overlay_scenes()
            .into_iter()
            .next()
            .map(|scene| scene.canvas)
            .unwrap_or_default()
    }

    /// The window-local box an element with this id was laid out into.
    pub fn bounds(&self, id: &str) -> Option<Bounds> {
        let shared = self.shared();
        let frame = shared.frame.borrow();
        let frame = frame.as_ref()?;
        let mut found = None;
        let mut path: Vec<crate::vui::node::ElementId> = Vec::new();
        walk(&frame.tree, &mut path, &mut |node, path| {
            if node
                .id
                .as_ref()
                .is_some_and(|element| element.as_str() == id)
            {
                found = Some(node.bounds);
                let _ = path;
            }
        });
        found
    }

    /// Every run of text the frame is showing, in paint order.
    ///
    /// Read from the frame rather than the display list: a shaped paragraph reaches the host as
    /// a layout identity, so the wire does not carry its text and a canvas-only assertion would
    /// see wrapped or ellipsized text as absent rather than as drawn.
    pub fn texts(&self) -> Vec<String> {
        let shared = self.shared();
        let frame = shared.frame.borrow();
        let mut found = Vec::new();
        if let Some(frame) = frame.as_ref() {
            frame.tree.walk(&mut |node| {
                if let Some(text) = node.paragraph() {
                    let run: String = text.runs.iter().map(|run| run.text.as_str()).collect();
                    found.push(run);
                }
            });
        }
        found
    }

    /// The region number an element with this id was painted as.
    pub fn region(&self, id: &str) -> Option<u64> {
        self.shared().region_for(id)
    }

    /// Press and release the primary button over an element.
    pub fn click(&mut self, id: &str) -> io::Result<()> {
        let center = self.center_of(id)?;
        self.presenter
            .overlay_pointer(center.0, center.1, Some((buttons::PRIMARY, true)), 1)?;
        self.presenter
            .overlay_pointer(center.0, center.1, Some((buttons::PRIMARY, false)), 0)?;
        self.settle()
    }

    /// Move the pointer over an element without pressing.
    pub fn hover(&mut self, id: &str) -> io::Result<()> {
        let center = self.center_of(id)?;
        self.presenter
            .overlay_pointer(center.0, center.1, None, 0)?;
        self.settle()
    }

    /// Move the pointer somewhere in the window, whether or not an element is there.
    pub fn move_pointer(&mut self, x: f64, y: f64) -> io::Result<()> {
        self.presenter.overlay_pointer(x, y, None, 0)?;
        self.settle()
    }

    pub fn wheel(&mut self, x: f64, y: f64, dx: f64, dy: f64) -> io::Result<()> {
        self.presenter.overlay_wheel(x, y, dx, dy, 0)?;
        self.settle()
    }

    pub fn key(&mut self, usage: u32, down: bool, modifiers: u32) -> io::Result<()> {
        self.presenter.overlay_key(usage, down, modifiers)?;
        self.settle()
    }

    pub fn text(&mut self, committed: &str) -> io::Result<()> {
        self.presenter.overlay_text(committed)?;
        self.settle()
    }

    /// The cursor the element under the pointer is asking for.
    pub fn cursor(&self) -> Option<CursorShape> {
        self.presenter.overlay_cursor()
    }

    /// Give the window host focus, as a click in the pane would.
    pub fn focus_window(&mut self) -> io::Result<()> {
        self.presenter.set_overlay_pane_focus(true)?;
        self.settle()
    }

    fn center_of(&self, id: &str) -> io::Result<(f64, f64)> {
        let bounds = self
            .bounds(id)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("no element {id:?}")))?;
        Ok((
            bounds.center().x.get() as f64,
            bounds.center().y.get() as f64,
        ))
    }

    /// Deliver whatever the host has queued, and repaint if anything asked for it.
    ///
    /// A test drives the same loop the application does, one turn at a time, so nothing here can
    /// pass by waiting in a way the real loop does not.
    pub fn settle(&mut self) -> io::Result<()> {
        let deadline = Instant::now() + STEP_TIMEOUT;
        loop {
            // A short wait, not a long one: the in-process presenter flushes what it owes before
            // it returns, so anything outstanding is already on the socket. Waiting longer only
            // adds real seconds to a suite that is otherwise driven by a clock it controls.
            let progressed = self.app.step(Duration::from_millis(2))?;
            // Settled means the host has caught up: nothing it sent is unanswered, and the frame
            // just submitted has been resolved. Waiting only for "no new events" would let a
            // test read a scene the host has not processed yet.
            let outstanding = self.shared().pending.borrow().is_some();
            if !progressed && !outstanding {
                break;
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "the host kept sending events without settling",
                ));
            }
        }
        Ok(())
    }
}

fn walk(
    node: &crate::vui::node::Node,
    path: &mut Vec<crate::vui::node::ElementId>,
    visit: &mut dyn FnMut(&crate::vui::node::Node, &[crate::vui::node::ElementId]),
) {
    visit(node, path);
    for child in &node.children {
        walk(child, path, visit);
    }
}
