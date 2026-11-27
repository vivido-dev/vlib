//! An interactive gallery of eight resolution-independent effects.

// Rust guideline compliant 2026-10-07
#[path = "views/effects_lab.rs"]
mod view;

use std::io;
use vivid_sdk::overlay::{OverlayWindowOptions, Rect, WindowMode};
use vlib::vui::UiApp;

fn main() -> io::Result<()> {
    let mut app = UiApp::from_env()?;
    let mut options = OverlayWindowOptions::new(
        Rect::new(24., 24., 760., 510.).map_err(io::Error::other)?,
        WindowMode::Floating,
    );
    options.title = "Effects lab".into();
    app.open_window(options, view::Lab::new()?)?;
    app.run()
}
