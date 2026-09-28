//! A column of nested boxes, one per level, to find the depth a frame stops fitting.
//!
//! Run it against a live Vivido:
//!
//! ```sh
//! cargo run --example tree
//! ```

#[path = "views/tree.rs"]
mod view;

use std::io;

use vivid_sdk::overlay::{OverlayWindowOptions, Rect, WindowMode};
use vlib::vui::UiApp;

use view::DeepTree;

fn main() -> io::Result<()> {
    let mut app = UiApp::from_env()?;
    let bounds = Rect::new(60., 60., 500.0, 500.0).map_err(io::Error::other)?;
    app.open_window(
        OverlayWindowOptions::new(bounds, WindowMode::Floating),
        DeepTree::default(),
    )?;
    app.run()
}
