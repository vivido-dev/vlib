//! A page laid out on grid tracks.
//!
//! Run it against a live Vivido:
//!
//! ```sh
//! cargo run --example grid_layout
//! ```

#[path = "views/grid_layout.rs"]
mod view;

use std::io;

use vivid_sdk::overlay::{OverlayWindowOptions, Rect, WindowMode};
use vlib::vui::UiApp;

use view::Page;

fn main() -> io::Result<()> {
    let mut app = UiApp::from_env()?;
    let bounds = Rect::new(60., 60., 640.0, 360.0).map_err(io::Error::other)?;
    app.open_window(
        OverlayWindowOptions::new(bounds, WindowMode::Floating),
        Page::default(),
    )?;
    app.run()
}
