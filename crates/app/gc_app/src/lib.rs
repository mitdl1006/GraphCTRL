//! eframe wiring: the application window, ribbon, panels and shortcuts.

mod app;
mod diagnostics;
mod example;
mod ribbon;

pub use app::{FileDialogs, GraphCtrlApp, RunOptions, run};
pub use diagnostics::{install_diagnostics, recent_log};
