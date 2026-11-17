//! eframe wiring: the application window, ribbon, panels and shortcuts.

mod app;
mod ribbon;

pub use app::{FileDialogs, GraphCtrlApp, RunOptions, run};
