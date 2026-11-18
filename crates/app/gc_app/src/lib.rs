//! eframe wiring: the application window, ribbon, panels and shortcuts.

mod agent;
mod app;
mod assemble;
mod assembly_import;
mod conflict;
mod constrain;
mod cut_edges;
mod diagnostics;
mod doc_settings;
mod drive;
mod example;
mod interference;
mod iproperties;
mod joint_limits;
mod measure;
mod parameters;
mod pattern;
mod replicate;
mod ribbon;
mod sketch_dialogs;
mod work_feature;

pub use app::{FileDialogs, GraphCtrlApp, RunOptions, run};
pub use diagnostics::{install_diagnostics, recent_log};
