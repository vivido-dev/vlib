//! Run it against a live Vivido:
//!
//! ```sh
//! cargo run --example mouse_pressure
//! ```

#[path = "views/mouse_pressure.rs"]
mod view;

use std::io;

use vivid_sdk::overlay::{OverlayWindowOptions, Rect, WindowMode};
use vlib::vui::UiApp;

use view::Pressure;

fn main() -> io::Result<()> {
    let mut app = UiApp::from_env()?;
    let bounds = Rect::new(60., 60., 420.0, 280.0).map_err(io::Error::other)?;
    app.open_window(
        OverlayWindowOptions::new(bounds, WindowMode::Floating),
        Pressure::default(),
    )?;
    app.run()
}
