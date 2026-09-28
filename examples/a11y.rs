//! Run it against a live Vivido:
//!
//! ```sh
//! cargo run --example a11y
//! ```

#[path = "views/a11y.rs"]
mod view;

use std::io;

use vivid_sdk::overlay::{OverlayWindowOptions, Rect, WindowMode};
use vlib::vui::UiApp;

use view::Panel;

fn main() -> io::Result<()> {
    let mut app = UiApp::from_env()?;
    let bounds = Rect::new(60., 60., 360., 340.).map_err(io::Error::other)?;
    app.open_window(
        OverlayWindowOptions::new(bounds, WindowMode::Floating),
        Panel::default(),
    )?;
    app.run()
}
