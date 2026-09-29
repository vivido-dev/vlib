//! Run it against a live Vivido:
//!
//! ```sh
//! cargo run --example text_layout
//! ```

#[path = "views/text_layout.rs"]
mod view;

use std::io;

use vivid_sdk::overlay::{OverlayWindowOptions, Rect, WindowMode};
use vlib::vui::UiApp;

use view::TextLayout;

fn main() -> io::Result<()> {
    let mut app = UiApp::from_env()?;
    let bounds = Rect::new(60., 60., 420.0, 460.0).map_err(io::Error::other)?;
    app.open_window(
        OverlayWindowOptions::new(bounds, WindowMode::Floating),
        TextLayout,
    )?;
    app.run()
}
