//! Workspace automation entry point: `cargo xtask <command>`.
#![allow(clippy::print_stdout, clippy::print_stderr, reason = "command-line tool: printing is its output")]

mod gate;
mod layers;
mod licences;
mod meta;

use std::process::ExitCode;

use anyhow::{Result, bail};

const USAGE: &str = "\
usage: cargo xtask <command>

  gate            run every check a commit must pass (fmt, check, clippy, tests, GPU tests, layers, licences)
  check-layers    verify crate dependencies against layers.toml
  check-licences  verify dependency licences and sources against licences.toml
  test-gpu        run the test suite with a GPU required (uses SwiftShader when it is installed)";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("xtask: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = meta::workspace_root()?;
    match args.first().map(String::as_str) {
        Some("gate") => gate::gate(&root),
        Some("check-layers") => layers::check(&root),
        Some("check-licences") => licences::check(&root),
        Some("test-gpu") => gate::test_gpu(&root),
        Some("help" | "--help" | "-h") | None => {
            println!("{USAGE}");
            Ok(())
        }
        Some(other) => bail!("unknown command `{other}`\n{USAGE}"),
    }
}
