//! A declarative UI toolkit over Vivid 1.5 pane overlays.
//!
//! An application is state that renders itself: [`Render`] returns a tree of elements, the tree is
//! laid out, painted into one display list, and submitted to the host. Nothing is retained between
//! frames except the state your handlers reach, and nothing repaints until something asks with
//! [`Context::notify`].
//!
//! ```
//! use vlib::vui::prelude::*;
//! use vivid_protocol::vector::CursorShape;
//!
//! struct Counter {
//!     count: i32,
//! }
//!
//! impl Render for Counter {
//!     fn render(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
//!         div()
//!             .flex_col()
//!             .gap(8.)
//!             .p(16.)
//!             .bg(rgb(0x1e1e28))
//!             .child(text(format!("{}", self.count)).size(24.).color(Colors::WHITE))
//!             .child(
//!                 div()
//!                     .id("increment")
//!                     .px(12.)
//!                     .py(6.)
//!                     .rounded(6.)
//!                     .bg(rgb(0x4a5fd0))
//!                     .cursor(CursorShape::Pointer)
//!                     .on_click(cx.listener(|state: &mut Counter, _event, cx| {
//!                         state.count += 1;
//!                         cx.notify();
//!                     }))
//!                     .child(text("+1").color(Colors::WHITE)),
//!             )
//!     }
//! }
//! ```
//!
//! The host does hit testing, so an element that wants input needs an identity: `.id(..)` is what
//! hover, press, and focus are remembered against between frames. Everything else is optional.

mod a11y;
pub mod animation;
mod app;
mod assets;
pub mod canvas;
mod element;
mod focus;
mod geometry;
mod image;
mod interactive;
pub mod keymap;
mod layout;
mod list;
mod node;
mod paint;
mod style;
mod text;
mod text_input;
mod window;

#[cfg(feature = "testing")]
pub mod testing;

pub use a11y::{Accessibility, Semantic};
pub use animation::{Animation, Easing, Repeat, Running, Spring, SpringState};

/// `#[macro_export]` places the macro at the crate root; this is its home in the namespace.
pub use crate::actions;
pub use app::{Context, Entity, Render, UiApp, WindowId};
pub use assets::{Decoded, ImageSource, ObjectFit, SourceData, decode, rasterize_svg};
pub use canvas::{CanvasEl, Drawing, IntoPath, canvas};
pub use element::{
    Anchor, Div, IntoElement, TextEl, button_of, div, modifiers_of, paragraph, point_of, text,
};
pub use focus::FocusHandle;
pub use geometry::{Bounds, Corners, Edges, Pixels, Point, Size};
pub use image::{ImgEl, img, svg};
pub use interactive::{ElementState, Handler, Modifiers, MouseButton, UiEvent};
pub use keymap::{Action, ActionId, KeyBinding, Keystroke};
pub use layout::{
    Align, Display, FlexDirectionSpec, Justify, Length, OverflowSpec, PositionSpec, Track,
};
pub use list::{ListEl, ListState, uniform_list};
pub use node::{ElementId, Node, TextRunSpec, TextSpec};
pub use style::{
    Background, Border, BoxShadow, Colors, GradientStopSpec, Style, rgb, rgba, shadows,
};
pub use text::TextAlign;
pub use text_input::{Preedit, TextInput, TextInputEl, text_input};
/// The vocabulary a semantic tree is written in, straight from the protocol: a closed set of
/// roles and a closed set of actions, so nothing is advertised that a host would have to ignore.
pub use vivid_protocol::overlay::{AccessibleAction, SemanticRole, Toggled};

/// Wire types an element names directly: a pointer shape, a color, where a scroll gesture is.
pub use vivid_protocol::overlay::ScrollPhase;
pub use vivid_protocol::vector::{Color, CursorShape};

/// The toolkit's error type: the SDK's, since almost everything that fails here is I/O against the
/// host, and a display list the protocol refused.
pub type Error = std::io::Error;

pub type Result<T> = std::io::Result<T>;

/// Everything a view usually needs, in one import.
pub mod prelude {
    pub use crate::vui::a11y::Semantic;
    pub use crate::vui::actions;
    pub use crate::vui::animation::{Animation, Easing, Spring, SpringState};
    pub use crate::vui::assets::{ImageSource, ObjectFit};
    pub use crate::vui::element::{
        IntoElement, button_of, div, modifiers_of, paragraph, point_of, text,
    };
    pub use crate::vui::focus::FocusHandle;
    pub use crate::vui::geometry::{Bounds, Pixels, Point, Size};
    pub use crate::vui::image::{img, svg};
    pub use crate::vui::interactive::{Modifiers, MouseButton, UiEvent};
    pub use crate::vui::keymap::{Action, ActionId, KeyBinding, Keystroke};
    pub use crate::vui::layout::{Align, Justify, Length, Track};
    pub use crate::vui::list::{ListState, uniform_list};
    pub use crate::vui::node::{TextRunSpec, TextSpec};
    pub use crate::vui::style::{BoxShadow, Colors, Style, rgb, rgba, shadows};
    pub use crate::vui::text::TextAlign;
    pub use crate::vui::{Color, Context, CursorShape, Entity, Render, UiApp};
    pub use vivid_protocol::overlay::{AccessibleAction, SemanticRole, Toggled};
}
