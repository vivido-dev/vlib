//! The shadow token scale, and the offsets, spread and inset behind it.
//!
//! Run it against a live Vivido:
//!
//! ```sh
//! cargo run --example shadow
//! ```

#[path = "views/shadow.rs"]
mod view;

use std::io;

use vivid_sdk::overlay::{OverlayWindowOptions, Rect, WindowMode};
use vlib::vui::UiApp;

use view::Shadows;

fn main() -> io::Result<()> {
    let mut app = UiApp::from_env()?;
    let bounds = Rect::new(60., 60., 440.0, 420.0).map_err(io::Error::other)?;
    app.open_window(
        OverlayWindowOptions::new(bounds, WindowMode::Floating),
        Shadows::default(),
    )?;
    app.run()
}
