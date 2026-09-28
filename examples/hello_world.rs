//! A window with text and a button, in the shape of GPUI's `hello_world`.
//!
//! Run it against a live Vivido:
//!
//! ```sh
//! cargo run --example hello_world
//! ```

#[path = "views/hello_world.rs"]
mod hello_world;

use std::io;

use vivid_sdk::overlay::{OverlayWindowOptions, Rect, WindowMode};
use vlib::vui::UiApp;

use hello_world::Counter;

fn main() -> io::Result<()> {
    let mut app = UiApp::from_env()?;
    let bounds = Rect::new(60., 60., 420., 240.).map_err(io::Error::other)?;
    app.open_window(
        OverlayWindowOptions::new(bounds, WindowMode::Floating),
        Counter::default(),
    )?;
    app.run()
}
