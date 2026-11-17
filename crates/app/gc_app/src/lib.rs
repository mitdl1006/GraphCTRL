//! eframe wiring: the application window, ribbon, panels and shortcuts.

mod agent;
mod app;
mod example;
mod ribbon;

pub use app::{FileDialogs, GraphCtrlApp, RunOptions, run};
