//! `gate` and `test-gpu`: the commands that run the other checks in order.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::{layers, licences, meta};

const WINDOWS_TARGET: &str = "x86_64-pc-windows-msvc";
/// Upper bound for the `target` directory; the gate fails above it so the disk never fills silently.
const TARGET_DIR_LIMIT_BYTES: u64 = 20 * 1024 * 1024 * 1024;

/// Runs one command in the workspace root and fails when it does.
fn step(root: &Path, label: &str, command: &mut Command) -> Result<()> {
    println!("\n== {label}");
    let status = command.current_dir(root).status().with_context(|| format!("could not start: {label}"))?;
    if !status.success() {
        bail!("{label} failed ({status})");
    }
    Ok(())
}

fn cargo_step<I, S>(root: &Path, label: &str, args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    step(root, label, meta::cargo().args(args))
}

/// True when rustup reports the Windows target as installed (so the cross-check can run).
fn windows_target_installed() -> bool {
    Command::new("rustup").args(["target", "list", "--installed"]).output().is_ok_and(|out| {
        String::from_utf8_lossy(&out.stdout).lines().any(|line| line.trim() == WINDOWS_TARGET)
    })
}

/// Everything a commit must pass, in the order the project rules prescribe.
pub fn gate(root: &Path) -> Result<()> {
    cargo_step(root, "fmt", ["fmt", "--all", "--check"])?;
    cargo_step(root, "check", ["check", "--workspace", "--all-targets", "--locked"])?;
    let clippy = ["clippy", "--workspace", "--all-targets", "--locked"];
    cargo_step(root, "clippy", clippy.iter().chain(&["--", "-D", "warnings"]))?;
    if cfg!(windows) {
        println!("\n== clippy ({WINDOWS_TARGET}): this is a Windows host, already covered");
    } else if windows_target_installed() {
        let cross = clippy.iter().chain(&["--target", WINDOWS_TARGET, "--", "-D", "warnings"]);
        cargo_step(root, &format!("clippy ({WINDOWS_TARGET})"), cross)?;
    } else {
        bail!("the {WINDOWS_TARGET} target is not installed; run `rustup target add {WINDOWS_TARGET}`");
    }
    cargo_step(root, "test", ["test", "--workspace", "--locked"])?;
    test_gpu(root)?;
    println!("\n== check-layers");
    layers::check(root)?;
    println!("\n== check-licences");
    licences::check(root)?;
    println!("\n== disk");
    let size = dir_size(&root.join("target"));
    println!("target directory: {:.1} GB", size as f64 / 1e9);
    if size > TARGET_DIR_LIMIT_BYTES {
        bail!("target directory exceeds {} GB; run `cargo clean`", TARGET_DIR_LIMIT_BYTES >> 30);
    }
    println!("\ngate: all checks passed");
    Ok(())
}

/// Runs the tests with `GC_REQUIRE_GPU=1`, so a GPU test that finds no adapter fails instead of skipping.
///
/// Where the SwiftShader software renderer is installed (the sandbox and CI) it is selected for the child
/// process only. On a developer PC nothing is overridden and the real GPU is used.
pub fn test_gpu(root: &Path) -> Result<()> {
    let mut command = meta::cargo();
    command.args(["test", "--workspace", "--locked", "--", "--test-threads=1"]).env("GC_REQUIRE_GPU", "1");
    let mut label = "test-gpu (system adapter)".to_owned();
    if std::env::var_os("VK_ICD_FILENAMES").is_none()
        && let Some(icd) = swiftshader_icd()
    {
        label = format!("test-gpu (SwiftShader: {})", icd.display());
        command.env("VK_ICD_FILENAMES", icd).env("WGPU_BACKEND", "vulkan");
    }
    step(root, &label, &mut command)
}

/// Finds `/opt/pw-browsers/chromium-*/chrome-linux*/vk_swiftshader_icd.json`.
fn swiftshader_icd() -> Option<PathBuf> {
    let children = |dir: &Path, prefix: &str| -> Vec<PathBuf> {
        let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(prefix))
            .map(|entry| entry.path())
            .collect();
        found.sort();
        found
    };
    children(Path::new("/opt/pw-browsers"), "chromium-")
        .iter()
        .flat_map(|chromium| children(chromium, "chrome-linux"))
        .map(|chrome| chrome.join("vk_swiftshader_icd.json"))
        .find(|icd| icd.is_file())
}

/// Total size of the files below `dir`; unreadable entries count as zero.
fn dir_size(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => dir_size(&entry.path()),
            Ok(kind) if kind.is_file() => entry.metadata().map_or(0, |m| m.len()),
            _ => 0,
        })
        .sum()
}
