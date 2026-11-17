//! Logs and panics: every log line goes to stderr, to a file in the `logs` folder and to a ring buffer
//! the log window shows; a panic is written to the log, the last automatic save is kept as a crash
//! recovery file, and the user is told in a native message.

use std::collections::VecDeque;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use eframe::egui;

/// How many lines the log window can show.
const RING: usize = 500;

static RING_BUFFER: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());
static LOG_FILE: OnceLock<Mutex<std::fs::File>> = OnceLock::new();

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Info
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        write_line(&format!("{} {:5} {}: {}", stamp(), record.level(), record.target(), record.args()));
    }

    fn flush(&self) {}
}

/// Seconds since the epoch: a stamp that needs no calendar.
fn stamp() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    format!("[{secs}]")
}

#[allow(clippy::print_stderr, reason = "the logger is the one place that writes to stderr")]
fn write_line(line: &str) {
    eprintln!("{line}");
    if let Some(file) = LOG_FILE.get()
        && let Ok(mut file) = file.lock()
    {
        // A log that cannot be written is not worth stopping for.
        drop(writeln!(file, "{line}"));
    }
    if let Ok(mut ring) = RING_BUFFER.lock() {
        if ring.len() >= RING {
            ring.pop_front();
        }
        ring.push_back(line.to_owned());
    }
}

/// The last log lines, oldest first.
pub fn recent_log() -> Vec<String> {
    RING_BUFFER.lock().map(|ring| ring.iter().cloned().collect()).unwrap_or_default()
}

/// Sets up logging and the panic hook. The application calls it once at start. `settings_dir` is where `logs` and `recovery` live; without it the
/// log goes to stderr and the ring buffer only. Calling it twice keeps the first setup.
pub fn install_diagnostics(settings_dir: Option<&Path>) {
    if let Some(dir) = settings_dir {
        let logs = dir.join("logs");
        if std::fs::create_dir_all(&logs).is_ok()
            && let Ok(file) =
                std::fs::OpenOptions::new().create(true).append(true).open(logs.join("graphctrl.log"))
        {
            drop(LOG_FILE.set(Mutex::new(file)));
        }
    }
    if log::set_logger(&Logger).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
    let dir = settings_dir.map(Path::to_path_buf);
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        write_line(&format!("{} PANIC {info}\n{backtrace}", stamp()));
        let saved = dir.as_deref().and_then(keep_last_autosave);
        let mut text = format!("GraphCTRL stopped unexpectedly.\n\n{info}");
        if let Some(saved) = saved {
            text.push_str(&format!("\n\nYour work was kept as an automatic save:\n{}", saved.display()));
        }
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title("GraphCTRL")
            .set_description(text)
            .show();
        previous(info);
    }));
}

/// Copies the last automatic save to a file of its own, so that the next automatic save cannot replace
/// it. The next start also offers the automatic save itself.
fn keep_last_autosave(dir: &Path) -> Option<PathBuf> {
    let (autosave, _) = gc_persist::places::autosave_files(dir);
    if !autosave.is_file() {
        return None;
    }
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let target = autosave.with_file_name(format!("crash-{secs}.gctrl"));
    std::fs::copy(&autosave, &target).ok().map(|_| target)
}

/// The window icon, from the built-in PNG.
pub(crate) fn window_icon() -> Option<egui::IconData> {
    let image =
        image::load_from_memory_with_format(include_bytes!("../assets/icon.png"), image::ImageFormat::Png)
            .ok()?
            .into_rgba8();
    let (width, height) = image.dimensions();
    Some(egui::IconData { rgba: image.into_raw(), width, height })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_icon_decodes() {
        let icon = window_icon().unwrap();
        assert_eq!((icon.width, icon.height), (64, 64));
        assert_eq!(icon.rgba.len(), 64 * 64 * 4);
    }

    #[test]
    fn the_last_autosave_is_kept_under_its_own_name() {
        let dir = std::env::temp_dir().join(format!("gc_diag_{}", std::process::id()));
        let (autosave, _) = gc_persist::places::autosave_files(&dir);
        std::fs::create_dir_all(autosave.parent().unwrap()).unwrap();
        std::fs::write(&autosave, b"bytes").unwrap();
        let kept = keep_last_autosave(&dir).unwrap();
        assert_eq!(std::fs::read(&kept).unwrap(), b"bytes");
        assert!(autosave.is_file());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn log_lines_reach_the_ring() {
        write_line("test line for the ring");
        assert!(recent_log().iter().any(|l| l == "test line for the ring"));
    }
}
