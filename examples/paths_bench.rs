//! Run it against a live Vivido:
//!
//! ```sh
//! cargo run --example paths_bench
//! ```

#[path = "views/paths_bench.rs"]
mod view;

use std::io;

use vivid_sdk::overlay::{OverlayWindowOptions, Rect, WindowMode};
use vlib::vui::UiApp;

use view::PathsBench;

fn main() -> io::Result<()> {
    let mut app = UiApp::from_env()?;
    let bounds = Rect::new(60., 60., 640.0, 420.0).map_err(io::Error::other)?;
    app.open_window(
        OverlayWindowOptions::new(bounds, WindowMode::Floating),
        PathsBench::default(),
    )?;
    app.run()
}
