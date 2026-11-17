//! 5.11 plumbing from the outside: the log goes to a file and to a ring, a bad settings folder does not
//! stop the start, setting up twice keeps the first. (The panic hook is not triggered here: it opens a
//! native message box.) All in one test because the logger and the hook are process-wide.

#![allow(clippy::unwrap_used, reason = "test code")]

#[test]
fn logging_reaches_the_file_and_the_ring_and_survives_a_bad_folder() {
    let dir = tempfile::tempdir().unwrap();

    // A file where the settings folder should be: no log file, but the start goes on and the ring works.
    let blocked = dir.path().join("blocked");
    std::fs::write(&blocked, "not a folder").unwrap();
    gc_app::install_diagnostics(Some(&blocked));
    log::info!("qa line before the file");
    assert!(gc_app::recent_log().iter().any(|l| l.contains("qa line before the file")));

    // Setting up again with a good folder: the first setup stays (documented), and nothing panics.
    gc_app::install_diagnostics(Some(dir.path()));
    gc_app::install_diagnostics(None);

    // The log file of the good folder gets the lines too (the file is opened on every setup that can).
    log::info!("qa line for the file");
    let file = std::fs::read_to_string(dir.path().join("logs/graphctrl.log")).unwrap();
    assert!(file.contains("qa line for the file"), "{file}");

    // Levels: info and above are kept, debug and trace are not.
    log::debug!("qa debug line");
    log::trace!("qa trace line");
    log::warn!("qa warn line");
    log::error!("qa error line with 한글 and \"quotes\"");
    let lines = gc_app::recent_log();
    assert!(!lines.iter().any(|l| l.contains("qa debug line") || l.contains("qa trace line")));
    assert!(lines.iter().any(|l| l.contains("qa warn line") && l.contains("WARN")));
    assert!(lines.iter().any(|l| l.contains("한글") && l.contains("ERROR")));

    // The ring holds the newest 500 lines, oldest first, and is never longer.
    for k in 0..700 {
        log::info!("qa flood {k}");
    }
    let lines = gc_app::recent_log();
    assert_eq!(lines.len(), 500);
    assert!(lines.last().unwrap().contains("qa flood 699"));
    assert!(!lines.iter().any(|l| l.contains("qa flood 0 ") || l.ends_with("qa flood 0")));

    // Many threads logging at once do not lose the ring or panic.
    let handles: Vec<_> = (0..8)
        .map(|t| std::thread::spawn(move || (0..200).for_each(|k| log::info!("qa thread {t} line {k}"))))
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(gc_app::recent_log().len(), 500);
}
