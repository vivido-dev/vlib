//! A button that must not stay pressed when the pointer leaves it.
//!
//! Run it against a live Vivido:
//!
//! ```sh
//! cargo run --example active_state_bug
//! ```

#[path = "views/active_state_bug.rs"]
mod view;

use std::io;

use vivid_sdk::overlay::{OverlayWindowOptions, Rect, WindowMode};
use vlib::vui::UiApp;

use view::Toggle;

fn main() -> io::Result<()> {
    let mut app = UiApp::from_env()?;
    let bounds = Rect::new(60., 60., 320.0, 160.0).map_err(io::Error::other)?;
    app.open_window(
        OverlayWindowOptions::new(bounds, WindowMode::Floating),
        Toggle::default(),
    )?;
    app.run()
}
