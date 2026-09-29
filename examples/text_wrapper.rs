//! Run it against a live Vivido:
//!
//! ```sh
//! cargo run --example text_wrapper
//! ```

#[path = "views/text_wrapper.rs"]
mod view;

use std::io;

use vivid_sdk::overlay::{OverlayWindowOptions, Rect, WindowMode};
use vlib::vui::UiApp;

use view::TextWrapper;

fn main() -> io::Result<()> {
    let mut app = UiApp::from_env()?;
    let bounds = Rect::new(60., 60., 560.0, 320.0).map_err(io::Error::other)?;
    app.open_window(
        OverlayWindowOptions::new(bounds, WindowMode::Floating),
        TextWrapper,
    )?;
    app.run()
}
