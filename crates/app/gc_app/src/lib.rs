//! eframe wiring: the application window, ribbon, panels and shortcuts.

mod agent;
mod app;
mod constrain;
mod diagnostics;
mod drive;
mod example;
mod interference;
mod measure;
mod ribbon;

pub use app::{FileDialogs, GraphCtrlApp, RunOptions, run};
pub use diagnostics::{install_diagnostics, recent_log};
