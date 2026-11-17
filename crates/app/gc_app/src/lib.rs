//! eframe wiring: the application window, ribbon, panels and shortcuts.

mod agent;
mod app;
mod diagnostics;
mod example;
mod measure;
mod ribbon;

pub use app::{FileDialogs, GraphCtrlApp, RunOptions, run};
pub use diagnostics::{install_diagnostics, recent_log};
