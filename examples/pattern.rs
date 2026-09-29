//! A tiled background, next to a gradient.
//!
//! Run it against a live Vivido:
//!
//! ```sh
//! cargo run --example pattern
//! ```

#[path = "views/pattern.rs"]
mod view;

use std::io;

use vivid_sdk::overlay::{OverlayWindowOptions, Rect, WindowMode};
use vlib::vui::UiApp;

use view::Patterned;

fn main() -> io::Result<()> {
    let mut app = UiApp::from_env()?;
    let bounds = Rect::new(60., 60., 400.0, 300.0).map_err(io::Error::other)?;
    app.open_window(
        OverlayWindowOptions::new(bounds, WindowMode::Floating),
        Patterned::default(),
    )?;
    app.run()
}
