//! What an element does when someone touches it.
//!
//! State that outlives a frame — hovered, pressed, focused — is kept by the window under the
//! element's region number and consulted while painting, so `.hover(..)` reads as a declaration
//! rather than a callback that mutates a style.

use vivid_protocol::overlay::{ScrollPhase, buttons, modifiers};
use vivid_protocol::vector::{CursorShape, HitRole};

use crate::vui::geometry::{Bounds, Point};
use crate::vui::keymap::ActionId;
use crate::vui::style::Style;

/// The modifier keys held during an event, as a set of protocol bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers(u32);

impl Modifiers {
    pub const NONE: Self = Self(0);

    pub const fn from_bits(bits: u32) -> Self {
        // Only the six defined bits survive; a host that sets a reserved bit is already
        // rejected by the protocol, and nothing here would know what to do with one.
        Self(bits & modifiers::KNOWN_MASK)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Add one modifier, for a binding written as `"cmd-s"`.
    pub const fn with_shift(self) -> Self {
        Self(self.0 | modifiers::SHIFT)
    }

    pub const fn with_control(self) -> Self {
        Self(self.0 | modifiers::CONTROL)
    }

    pub const fn with_alt(self) -> Self {
        Self(self.0 | modifiers::ALT)
    }

    pub const fn with_super(self) -> Self {
        Self(self.0 | modifiers::SUPER)
    }

    pub const fn shift(self) -> bool {
        self.has(modifiers::SHIFT)
    }

    pub const fn control(self) -> bool {
        self.has(modifiers::CONTROL)
    }

    pub const fn alt(self) -> bool {
        self.has(modifiers::ALT)
    }

    pub const fn command(self) -> bool {
        self.has(modifiers::SUPER)
    }

    /// The modifier that means "the platform's command key" on this host.
    pub const fn platform(self) -> bool {
        #[cfg(target_os = "macos")]
        {
            self.has(modifiers::SUPER)
        }
        #[cfg(not(target_os = "macos"))]
        {
            self.has(modifiers::CONTROL)
        }
    }

    const fn has(self, bit: u32) -> bool {
        self.0 & bit != 0
    }
}

/// A pointer button, in the protocol's assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MouseButton(pub u16);

impl MouseButton {
    pub const PRIMARY: Self = Self(buttons::PRIMARY);
    pub const AUXILIARY: Self = Self(buttons::AUXILIARY);
    pub const SECONDARY: Self = Self(buttons::SECONDARY);
    pub const BACK: Self = Self(buttons::BACK);
    pub const FORWARD: Self = Self(buttons::FORWARD);
}

/// What an element is told about. One variant per thing a host can report.
#[derive(Debug, Clone, PartialEq)]
pub enum UiEvent {
    Click {
        position: Point,
        clicks: u8,
        modifiers: Modifiers,
    },
    MouseDown {
        position: Point,
        button: MouseButton,
        modifiers: Modifiers,
    },
    MouseUp {
        position: Point,
        button: MouseButton,
        modifiers: Modifiers,
    },
    MouseMove {
        position: Point,
    },
    Hover {
        entered: bool,
    },
    Wheel {
        delta: Point,
        precise: bool,
        phase: ScrollPhase,
    },
    Key {
        physical: u32,
        down: bool,
        repeat: bool,
        modifiers: Modifiers,
    },
    Text(String),
    /// A composition from an input method: the text being composed, and its selection in
    /// characters.
    Ime {
        preedit: String,
        selection: Option<(u32, u32)>,
    },
    Accessibility {
        action: vivid_protocol::overlay::AccessibleAction,
    },
    Resize {
        bounds: Bounds,
        settled: bool,
    },
    /// A device that reports pressure says how hard the pointer is pressing. A host with no
    /// pressure sensor never sends one, so this is absent rather than zero.
    Pressure {
        position: Point,
        pressure: f32,
    },
    /// A bound key was pressed, and this is the action it names.
    Action(ActionId),
    Dismissed,
}

impl UiEvent {
    pub fn position(&self) -> Option<Point> {
        match self {
            Self::Click { position, .. }
            | Self::MouseDown { position, .. }
            | Self::MouseUp { position, .. }
            | Self::MouseMove { position } => Some(*position),
            _ => None,
        }
    }
}

/// A callback an element runs when something happens to it.
pub type Handler = Box<dyn FnMut(&UiEvent)>;

/// The callbacks one element responds to.
#[derive(Default)]
pub struct Handlers {
    pub click: Option<Handler>,
    /// Handlers for named actions, in the order they were declared.
    pub actions: Vec<(&'static str, Handler)>,
    pub mouse_down: Option<Handler>,
    pub mouse_up: Option<Handler>,
    pub mouse_move: Option<Handler>,
    pub hover: Option<Handler>,
    pub wheel: Option<Handler>,
    pub key: Option<Handler>,
    pub text: Option<Handler>,
    pub accessibility: Option<Handler>,
    pub pressure: Option<Handler>,
}

impl Handlers {
    /// The handler for an action, if this element has one.
    pub fn action(&mut self, id: &'static str) -> Option<&mut Handler> {
        self.actions
            .iter_mut()
            .find(|(name, _)| *name == id)
            .map(|(_, handler)| handler)
    }

    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
            && self.click.is_none()
            && self.mouse_down.is_none()
            && self.mouse_up.is_none()
            && self.mouse_move.is_none()
            && self.hover.is_none()
            && self.wheel.is_none()
            && self.key.is_none()
            && self.text.is_none()
            && self.pressure.is_none()
            && self.accessibility.is_none()
    }
}

impl std::fmt::Debug for Handlers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Handlers")
            .field("is_empty", &self.is_empty())
            .finish()
    }
}

/// The state a window keeps for one element between frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ElementState {
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
    /// Focus arrived from the keyboard rather than from a click, which is what a focus ring
    /// should follow.
    pub focus_visible: bool,
}

/// Everything an element carries for input, besides its layout and paint style.
#[derive(Default)]
pub struct Interactions {
    cursor: Option<CursorShape>,
    role: Option<HitRole>,
    hover: Option<Box<dyn Fn(Style) -> Style>>,
    pressed: Option<Box<dyn Fn(Style) -> Style>>,
    focus_visible: Option<Box<dyn Fn(Style) -> Style>>,
    pub handlers: Handlers,
}

impl Interactions {
    pub fn cursor(&self) -> Option<CursorShape> {
        self.cursor
    }

    /// The hit role this element's region takes. `Transparent` blocks whatever is painted
    /// under it without being reported itself, which is what a modal scrim is.
    pub fn role(&self) -> HitRole {
        self.role.unwrap_or(HitRole::Input)
    }

    pub fn set_role(&mut self, role: HitRole) {
        self.role = Some(role);
    }

    pub fn set_cursor(&mut self, cursor: Option<CursorShape>) {
        self.cursor = cursor;
    }

    pub fn on_hover_style(&mut self, style: impl Fn(Style) -> Style + 'static) {
        self.hover = Some(Box::new(style));
    }

    pub fn on_pressed_style(&mut self, style: impl Fn(Style) -> Style + 'static) {
        self.pressed = Some(Box::new(style));
    }

    pub fn on_focus_visible_style(&mut self, style: impl Fn(Style) -> Style + 'static) {
        self.focus_visible = Some(Box::new(style));
    }

    /// The style this element paints with, given what the window knows about it.
    ///
    /// Pressed wins over hovered, and focus-visible is applied last, so a focus ring survives a
    /// hover background and a press background.
    pub fn resolve_style(&self, base: Style, state: ElementState) -> Style {
        let mut style = base;
        if state.hovered
            && let Some(hover) = &self.hover
        {
            style = hover(style);
        }
        if state.pressed
            && let Some(pressed) = &self.pressed
        {
            style = pressed(style);
        }
        if state.focus_visible
            && let Some(focus) = &self.focus_visible
        {
            style = focus(style);
        }
        style
    }
}

impl std::fmt::Debug for Interactions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Interactions")
            .field("cursor", &self.cursor)
            .field("role", &self.role)
            .field("hover", &self.hover.is_some())
            .field("pressed", &self.pressed.is_some())
            .field("focus_visible", &self.focus_visible.is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_defined_modifier_bits_survive() {
        let masked = Modifiers::from_bits(0xffff_ffff);
        assert_eq!(masked.bits(), modifiers::KNOWN_MASK);
        assert!(masked.shift() && masked.control() && masked.alt() && masked.command());
        assert_eq!(Modifiers::NONE.bits(), 0);
    }

    #[test]
    fn pressed_beats_hovered_and_a_focus_ring_wins_over_both() {
        let mut interactions = Interactions::default();
        interactions.on_hover_style(|style| style.push_shadow(hover_marker()));
        interactions.on_pressed_style(|style| style.push_shadow(pressed_marker()));
        interactions.on_focus_visible_style(|style| style.push_shadow(focus_marker()));

        let shadow_count = |style: &Style| style.shadows.len();
        let rest = interactions.resolve_style(Style::default(), ElementState::default());
        assert_eq!(shadow_count(&rest), 0);

        let hovered = ElementState {
            hovered: true,
            ..Default::default()
        };
        assert_eq!(
            shadow_count(&interactions.resolve_style(Style::default(), hovered)),
            1
        );

        let pressed = ElementState {
            hovered: true,
            pressed: true,
            ..Default::default()
        };
        assert_eq!(
            shadow_count(&interactions.resolve_style(Style::default(), pressed)),
            2
        );

        let focused = ElementState {
            hovered: true,
            pressed: true,
            focus_visible: true,
            ..Default::default()
        };
        assert_eq!(
            shadow_count(&interactions.resolve_style(Style::default(), focused)),
            3
        );
    }

    fn hover_marker() -> crate::vui::style::BoxShadow {
        crate::vui::style::BoxShadow::new(
            Point::new(1., 0.),
            crate::vui::geometry::Pixels::ZERO,
            crate::vui::style::rgb(0x111111),
        )
    }

    fn pressed_marker() -> crate::vui::style::BoxShadow {
        crate::vui::style::BoxShadow::new(
            Point::new(2., 0.),
            crate::vui::geometry::Pixels::ZERO,
            crate::vui::style::rgb(0x222222),
        )
    }

    fn focus_marker() -> crate::vui::style::BoxShadow {
        crate::vui::style::BoxShadow::new(
            Point::new(3., 0.),
            crate::vui::geometry::Pixels::ZERO,
            crate::vui::style::rgb(0x333333),
        )
    }

    #[test]
    fn a_region_reports_its_role_and_shape() {
        let mut interactions = Interactions::default();
        interactions.set_cursor(Some(CursorShape::Pointer));
        assert_eq!(interactions.cursor(), Some(CursorShape::Pointer));
        // A region reported as transparent blocks what is under it; the default is not that.
        assert_eq!(interactions.role(), HitRole::Input);
        interactions.set_role(HitRole::Transparent);
        assert_eq!(interactions.role(), HitRole::Transparent);
    }
}
