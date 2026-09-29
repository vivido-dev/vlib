//! Content taller than its box, scrolled by the wheel.
//!
//! Run it against a live Vivido:
//!
//! ```sh
//! cargo run --example scrollable
//! ```

#[path = "views/scrollable.rs"]
mod view;

use std::io;

use vivid_sdk::overlay::{OverlayWindowOptions, Rect, WindowMode};
use vlib::vui::UiApp;

use view::Scrollable;

fn main() -> io::Result<()> {
    let mut app = UiApp::from_env()?;
    let bounds = Rect::new(60., 60., 300.0, 168.0).map_err(io::Error::other)?;
    app.open_window(
        OverlayWindowOptions::new(bounds, WindowMode::Floating),
        Scrollable::default(),
    )?;
    app.run()
}
