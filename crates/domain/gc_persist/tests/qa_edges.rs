//! QA edge cases for saving, loading, recent files, settings and crash recovery.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_doc::Document;
use gc_persist::places::{
    RECENT, Settings, clear_autosave, load_recent, load_settings, pending_autosave, remember_recent,
    save_settings, settings_file, write_autosave,
};

fn dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("gc-qa-{name}-{}", std::process::id()));
    drop(std::fs::remove_dir_all(&d));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn damaged_project_text_never_panics() {
    let text = gc_persist::to_text(&Document::new()).unwrap();
    for cut in (0..text.trim_end().len()).filter(|i| text.is_char_boundary(*i)) {
        let result = gc_persist::from_text(&text[..cut]);
        assert!(result.is_err(), "a file cut at {cut} of {} must be refused", text.len());
    }
    for junk in
        ["", "null", "[]", "{}", "\u{feff}", "{\"format\":1}", "\0\0\0", "{\"version\":99999999999999999999}"]
    {
        assert!(gc_persist::from_text(junk).is_err(), "{junk:?}");
    }
}

#[test]
fn a_missing_or_directory_project_path_is_an_error() {
    let d = dir("missing");
    assert!(gc_persist::load(&d.join("nope.gctrl")).is_err());
    assert!(gc_persist::load(&d).is_err());
}

#[test]
fn saving_into_a_missing_folder_or_over_a_directory_fails_cleanly() {
    let d = dir("saveerr");
    let doc = Document::new();
    // A directory in the way of the file: the save fails and leaves the directory alone.
    let target = d.join("x.gctrl");
    std::fs::create_dir(&target).unwrap();
    assert!(gc_persist::save(&doc, &target).is_err());
    assert!(target.is_dir());
}

#[test]
fn recent_list_is_deduplicated_capped_and_skips_deleted_files() {
    let d = dir("recent");
    let mut files = Vec::new();
    for k in 0..RECENT + 3 {
        let f = d.join(format!("p{k}.gctrl"));
        std::fs::write(&f, "x").unwrap();
        remember_recent(&d, &f).unwrap();
        files.push(f);
    }
    let list = load_recent(&d);
    assert_eq!(list.len(), RECENT);
    assert_eq!(list[0], files.last().unwrap().canonicalize().unwrap());
    // Remembering an old one again moves it to the front without duplicating it.
    let list = remember_recent(&d, &files[RECENT]).unwrap();
    assert_eq!(list.iter().filter(|p| p.ends_with("p8.gctrl")).count(), 1);
    assert!(list[0].ends_with("p8.gctrl"));
    // A deleted file disappears from the list.
    std::fs::remove_file(&files[RECENT]).unwrap();
    assert!(!load_recent(&d).iter().any(|p| p.ends_with("p8.gctrl")));
}

#[test]
fn settings_survive_damage_and_older_files() {
    let d = dir("settings");
    assert_eq!(load_settings(&d), Settings::default());
    std::fs::write(settings_file(&d), "{ not json").unwrap();
    assert_eq!(load_settings(&d), Settings::default());
    std::fs::write(settings_file(&d), "{\"language\":\"en\",\"future_field\":1}").unwrap();
    assert_eq!(load_settings(&d).language, "en");
    save_settings(&d, &Settings { language: "ko".into(), ..Settings::default() }).unwrap();
    assert_eq!(load_settings(&d).language, "ko");
}

#[test]
fn recovery_file_roundtrips_and_clears() {
    let d = dir("recovery");
    assert!(pending_autosave(&d).is_none());
    let doc = Document::new();
    write_autosave(&d, &doc, None).unwrap();
    let (file, project) = pending_autosave(&d).unwrap();
    assert!(project.is_none(), "an unsaved document has no project");
    assert!(gc_persist::load(&file).unwrap().doc.same_content(&doc));
    // Naming a project (with a non-ASCII name) is remembered.
    let named = d.join("프로젝트.gctrl");
    write_autosave(&d, &doc, Some(&named)).unwrap();
    assert_eq!(pending_autosave(&d).unwrap().1, Some(named));
    clear_autosave(&d);
    assert!(pending_autosave(&d).is_none());
    clear_autosave(&d); // clearing twice is harmless
}
