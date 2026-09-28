//! Run it against a live Vivido:
//!
//! ```sh
//! cargo run --example data_table
//! ```

#[path = "views/data_table.rs"]
mod view;

use std::io;

use vivid_sdk::overlay::{OverlayWindowOptions, Rect, WindowMode};
use vlib::vui::UiApp;

use view::Table;

fn main() -> io::Result<()> {
    let mut app = UiApp::from_env()?;
    let bounds = Rect::new(60., 60., 460.0, 300.0).map_err(io::Error::other)?;
    app.open_window(
        OverlayWindowOptions::new(bounds, WindowMode::Floating),
        Table::default(),
    )?;
    app.run()
}
