//! Run it against a live Vivido:
//!
//! ```sh
//! cargo run --example window_shadow
//! ```

#[path = "views/window_shadow.rs"]
mod view;

use std::io;

use vivid_sdk::overlay::{OverlayWindowOptions, Rect, WindowMode};
use vlib::vui::UiApp;

use view::WindowFrame;

fn main() -> io::Result<()> {
    let mut app = UiApp::from_env()?;
    let bounds = Rect::new(60., 60., 360.0, 260.0).map_err(io::Error::other)?;
    app.open_window(
        OverlayWindowOptions::new(bounds, WindowMode::Floating),
        WindowFrame,
    )?;
    app.run()
}
