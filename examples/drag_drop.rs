//! Run it against a live Vivido:
//!
//! ```sh
//! cargo run --example drag_drop
//! ```

#[path = "views/drag_drop.rs"]
mod view;

use std::io;

use vivid_sdk::overlay::{OverlayWindowOptions, Rect, WindowMode};
use vlib::vui::UiApp;

use view::DragDrop;

fn main() -> io::Result<()> {
    let mut app = UiApp::from_env()?;
    let bounds = Rect::new(60., 60., 420.0, 260.0).map_err(io::Error::other)?;
    app.open_window(
        OverlayWindowOptions::new(bounds, WindowMode::Floating),
        DragDrop::default(),
    )?;
    app.run()
}
