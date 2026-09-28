//! Run it against a live Vivido:
//!
//! ```sh
//! cargo run --example focus_visible
//! ```

#[path = "views/focus_visible.rs"]
mod view;

use std::io;

use vivid_sdk::overlay::{OverlayWindowOptions, Rect, WindowMode};
use vlib::vui::UiApp;

use view::FocusRings;

fn main() -> io::Result<()> {
    let mut app = UiApp::from_env()?;
    let bounds = Rect::new(60., 60., 360.0, 220.0).map_err(io::Error::other)?;
    app.open_window(
        OverlayWindowOptions::new(bounds, WindowMode::Floating),
        FocusRings::default(),
    )?;
    app.run()
}
