//! Run it against a live Vivido:
//!
//! ```sh
//! cargo run --example window_movable
//! ```

#[path = "views/window_movable.rs"]
mod view;

use std::io;

use vivid_sdk::overlay::{OverlayWindowOptions, Rect, WindowMode};
use vlib::vui::UiApp;

use view::Movable;

fn main() -> io::Result<()> {
    let mut app = UiApp::from_env()?;
    let bounds = Rect::new(60., 60., 384.0, 264.0).map_err(io::Error::other)?;
    app.open_window(
        OverlayWindowOptions::new(bounds, WindowMode::Floating),
        Movable::default(),
    )?;
    app.run()
}
