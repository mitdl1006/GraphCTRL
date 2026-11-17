//! `cargo xtask bench-quality`: the Tier B planning quality bench (plan, section 7 and appendix E), then the
//! large-scene check (`bench_scene` of `gc_session`: a frame's session queries for 50 components within 2 ms).
//!
//! The bench itself is the `bench_quality` example of `gc_motion` (it needs the domain crates, which the
//! automation crate does not depend on). This command builds it with `--release` and points its report
//! at `docs/quality/report-<date>.md`.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

/// Today's date (UTC) as `YYYY-MM-DD`.
fn today() -> Result<String> {
    let seconds = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
    let days = i64::try_from(seconds / 86_400)?;
    // Civil date from days since 1970-01-01 (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    Ok(format!("{year:04}-{month:02}-{day:02}"))
}

/// Runs the bench. `args` are passed on (`--seeds N`, `--only S2`, `--out FILE`); `--release` is implied.
pub fn bench_quality(root: &Path, args: &[String]) -> Result<()> {
    let passed: Vec<&String> = args.iter().filter(|a| a.as_str() != "--release").collect();
    let mut command = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned()));
    command
        .current_dir(root)
        .args(["run", "--release", "--locked", "-p", "gc_motion", "--example", "bench_quality", "--"])
        .args(&passed);
    let out = match passed.iter().position(|a| a.as_str() == "--out") {
        Some(i) => passed.get(i + 1).map(|p| root.join(p.as_str())),
        None => {
            let dir = root.join("docs/quality");
            std::fs::create_dir_all(&dir).with_context(|| dir.display().to_string())?;
            let file = dir.join(format!("report-{}.md", today()?));
            command.arg("--out").arg(&file);
            Some(file)
        }
    };
    let status = command.status().context("could not start cargo")?;
    if !status.success() {
        bail!("bench-quality failed ({status})");
    }
    // The large-scene check: the session side of a frame for 50 components, appended to the report.
    let mut scene = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned()));
    scene.current_dir(root).args([
        "run",
        "--release",
        "--locked",
        "-p",
        "gc_session",
        "--example",
        "bench_scene",
        "--",
    ]);
    if let Some(file) = &out {
        scene.arg("--append").arg(file);
    }
    let status = scene.status().context("could not start cargo")?;
    if !status.success() {
        bail!("bench_scene failed ({status})");
    }
    Ok(())
}
