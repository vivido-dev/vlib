//! Run it against a live Vivido:
//!
//! ```sh
//! cargo run --example gif_viewer
//! ```

#[path = "views/gif_viewer.rs"]
mod view;

use std::io;

use vivid_sdk::overlay::{OverlayWindowOptions, Rect, WindowMode};
use vlib::vui::UiApp;

use view::Gif;

fn main() -> io::Result<()> {
    let mut app = UiApp::from_env()?;
    let bounds = Rect::new(60., 60., 320.0, 260.0).map_err(io::Error::other)?;
    app.open_window(
        OverlayWindowOptions::new(bounds, WindowMode::Floating),
        Gif::default(),
    )?;
    app.run()
}
