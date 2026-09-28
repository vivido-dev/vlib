//! Run it against a live Vivido:
//!
//! ```sh
//! cargo run --example image_loading
//! ```

#[path = "views/image_loading.rs"]
mod view;

use std::io;

use vivid_sdk::overlay::{OverlayWindowOptions, Rect, WindowMode};
use vlib::vui::UiApp;

use view::Loading;

fn main() -> io::Result<()> {
    let mut app = UiApp::from_env()?;
    let bounds = Rect::new(60., 60., 440.0, 240.0).map_err(io::Error::other)?;
    app.open_window(
        OverlayWindowOptions::new(bounds, WindowMode::Floating),
        Loading::default(),
    )?;
    app.run()
}
