//! Run it against a live Vivido:
//!
//! ```sh
//! cargo run --example list_example
//! ```

#[path = "views/list_example.rs"]
mod view;

use std::io;

use vivid_sdk::overlay::{OverlayWindowOptions, Rect, WindowMode};
use vlib::vui::UiApp;

use view::Log;

fn main() -> io::Result<()> {
    let mut app = UiApp::from_env()?;
    let bounds = Rect::new(60., 60., 420.0, 260.0).map_err(io::Error::other)?;
    app.open_window(
        OverlayWindowOptions::new(bounds, WindowMode::Floating),
        Log::default(),
    )?;
    app.run()
}
