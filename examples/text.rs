//! Run it against a live Vivido:
//!
//! ```sh
//! cargo run --example text
//! ```

#[path = "views/text.rs"]
mod view;

use std::io;

use vivid_sdk::overlay::{OverlayWindowOptions, Rect, WindowMode};
use vlib::vui::UiApp;

use view::Specimen;

fn main() -> io::Result<()> {
    let mut app = UiApp::from_env()?;
    let bounds = Rect::new(60., 60., 520.0, 400.0).map_err(io::Error::other)?;
    app.open_window(
        OverlayWindowOptions::new(bounds, WindowMode::Floating),
        Specimen,
    )?;
    app.run()
}
