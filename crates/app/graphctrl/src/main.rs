//! GraphCTRL desktop application entry point.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let mut options = gc_app::RunOptions::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--smoke-frames" => options.smoke_frames = args.next().and_then(|v| v.parse().ok()),
            "--screenshot" => options.screenshot = args.next().map(PathBuf::from),
            other => anyhow::bail!("unknown argument: {other}"),
        }
    }
    gc_app::run(options)
}
