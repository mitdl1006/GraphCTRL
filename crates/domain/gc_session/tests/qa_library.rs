//! QA edge cases of the robot library (`robots/`, `library://`): no library, damaged packages, projects
//! opened on another PC.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use gc_session::{Session, library};
use std::path::{Path, PathBuf};

fn six_axis() -> String {
    let path = format!("{}/../../../fixtures/urdf/six_axis.urdf", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn repo_library() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../robots")
}

fn package(lib: &Path, id: &str, json: &str, urdf: Option<&str>) {
    std::fs::create_dir_all(lib.join(id)).unwrap();
    std::fs::write(lib.join(id).join("robot.json"), json).unwrap();
    if let Some(urdf) = urdf {
        std::fs::write(lib.join(id).join("arm.urdf"), urdf).unwrap();
    }
}

const GOOD: &str = r#"{ "name": "Arm", "description": "arm.urdf" }"#;

#[test]
fn the_shipped_library_lists_and_loads_every_package() {
    let packages = library::scan(&repo_library());
    let ids: Vec<_> = packages.iter().map(|p| p.id.as_str()).collect();
    for want in ["ur3e", "ur5e", "ur10e", "ur16e", "sample-6r"] {
        assert!(ids.contains(&want), "{want} missing from {ids:?}");
    }
    let mut session = Session::new();
    session.set_robot_library(Some(repo_library()));
    for package in &packages {
        let robot = session.add_library_robot(package).unwrap_or_else(|e| panic!("{}: {e}", package.id));
        assert!(session.robot_model(robot).is_some(), "{}", package.id);
    }
}

#[test]
fn no_library_means_an_empty_list_not_a_crash() {
    let d = tempfile::tempdir().unwrap();
    assert!(library::scan(&d.path().join("missing")).is_empty());
    assert!(library::scan(d.path()).is_empty(), "an empty folder");
    let file = d.path().join("robots");
    std::fs::write(&file, "not a folder").unwrap();
    assert!(library::scan(&file).is_empty(), "a file where the folder should be");
    assert_eq!(library::find(&[d.path().to_path_buf()]), None);
    assert_eq!(library::find(&[]), None);
}

#[test]
fn damaged_packages_are_left_out_and_the_good_ones_stay() {
    let d = tempfile::tempdir().unwrap();
    let lib = d.path().join("robots");
    package(&lib, "good", GOOD, Some(&six_axis()));
    package(&lib, "bad-json", "{ not json", Some(&six_axis()));
    package(&lib, "empty-json", "", Some(&six_axis()));
    package(&lib, "no-name", r#"{ "description": "arm.urdf" }"#, Some(&six_axis()));
    package(&lib, "no-urdf", GOOD, None);
    package(&lib, "wrong-types", r#"{ "name": 5, "description": ["x"] }"#, Some(&six_axis()));
    package(&lib, "binary", "\u{0}\u{0}\u{1}", Some(&six_axis()));
    let ids: Vec<_> = library::scan(&lib).into_iter().map(|p| p.id).collect();
    assert_eq!(ids, ["good"]);
}

#[test]
fn a_package_whose_description_is_broken_is_refused_and_changes_nothing() {
    let d = tempfile::tempdir().unwrap();
    let lib = d.path().join("robots");
    for (id, urdf) in [
        ("garbage", "this is not xml"),
        ("empty", ""),
        ("no-links", "<robot name=\"x\"/>"),
        ("truncated", &six_axis()[..200]),
    ] {
        package(&lib, id, GOOD, Some(urdf));
    }
    let mut session = Session::new();
    session.set_robot_library(Some(lib.clone()));
    let before = session.doc().clone();
    for package in library::scan(&lib) {
        assert!(session.add_library_robot(&package).is_err(), "{}", package.id);
        assert!(session.doc().same_content(&before), "{}", package.id);
    }
}

/// The description is fine but the meshes it names are gone (a package copied without `meshes/`).
#[test]
fn a_package_missing_its_meshes_does_not_crash() {
    let d = tempfile::tempdir().unwrap();
    let lib = d.path().join("robots");
    std::fs::create_dir_all(lib.join("ur5e")).unwrap();
    for file in ["robot.json", "ur5e.urdf"] {
        std::fs::copy(repo_library().join("ur5e").join(file), lib.join("ur5e").join(file)).unwrap();
    }
    let mut session = Session::new();
    session.set_robot_library(Some(lib.clone()));
    let package = &library::scan(&lib)[0];
    // Either refused with the reason, or added with the missing meshes reported; never a panic.
    match session.add_library_robot(package) {
        Ok(robot) => assert!(session.robot_model(robot).is_some()),
        Err(reason) => assert!(!reason.is_empty()),
    }
}

/// A project that names a library robot, opened where there is no library, or where the library lacks
/// that package, or where the recorded folder is an absolute path of another PC.
#[test]
fn projects_open_without_their_robot_library() {
    let d = tempfile::tempdir().unwrap();
    let lib = d.path().join("robots");
    package(&lib, "arm", GOOD, Some(&six_axis()));
    let mut session = Session::new();
    session.set_robot_library(Some(lib.clone()));
    let robot = session.add_library_robot(&library::scan(&lib)[0]).unwrap();
    let file = d.path().join("cell.gctrl");
    session.save_as(&file).unwrap();

    let other_pcs: [Option<PathBuf>; 3] =
        [None, Some(d.path().join("elsewhere")), Some(d.path().join("empty-robots"))];
    std::fs::create_dir_all(d.path().join("empty-robots")).unwrap();
    for library in other_pcs {
        let mut reopened = Session::new();
        reopened.set_robot_library(library.clone());
        reopened.open(&file).unwrap_or_else(|e| panic!("{library:?}: {e}"));
        // The robot is in the document; it carries its own description, so it still works.
        assert!(reopened.doc().get(robot).is_some(), "{library:?}");
        assert_eq!(reopened.doc().get(robot).unwrap().resource_dir, "library://arm");
        // Saving again does not rewrite the name into a path of this machine.
        let again = d.path().join("again.gctrl");
        reopened.save_as(&again).unwrap();
        assert!(std::fs::read_to_string(&again).unwrap().contains("library://arm"), "{library:?}");
    }

    // An absolute folder of another PC (Windows style) in the file: opens, robot still in the document.
    let text = std::fs::read_to_string(&file)
        .unwrap()
        .replace("library://arm", "C:\\\\Users\\\\someone\\\\robots\\\\arm");
    let foreign = d.path().join("foreign.gctrl");
    std::fs::write(&foreign, text).unwrap();
    let mut reopened = Session::new();
    reopened.open(&foreign).unwrap();
    assert!(reopened.doc().get(robot).is_some());
}

#[test]
fn package_folder_names_with_spaces_and_hangul_work() {
    let d = tempfile::tempdir().unwrap();
    let lib = d.path().join("로봇 라이브러리");
    package(&lib, "내 로봇 1", GOOD, Some(&six_axis()));
    let packages = library::scan(&lib);
    assert_eq!(packages.len(), 1);
    let mut session = Session::new();
    session.set_robot_library(Some(lib.clone()));
    let robot = session.add_library_robot(&packages[0]).unwrap();
    assert_eq!(session.doc().get(robot).unwrap().resource_dir, "library://내 로봇 1");
    let file = d.path().join("셀.gctrl");
    session.save_as(&file).unwrap();
    let mut reopened = Session::new();
    reopened.set_robot_library(Some(lib));
    reopened.open(&file).unwrap();
    assert!(reopened.robot_model(robot).is_some());
}

/// A description file name that climbs out of the package folder must not read outside it. Low severity
/// (the library is a local add-on folder), but a downloaded package should not read arbitrary files.
#[test]
fn a_description_path_cannot_leave_the_package() {
    let d = tempfile::tempdir().unwrap();
    let lib = d.path().join("robots");
    std::fs::write(d.path().join("secret.urdf"), six_axis()).unwrap();
    package(&lib, "evil", r#"{ "name": "Evil", "description": "../../secret.urdf" }"#, None);
    assert!(library::scan(&lib).is_empty(), "a description outside its package is not a package");
    let absolute = d.path().join("secret.urdf").to_string_lossy().replace('\\', "/");
    package(&lib, "evil2", &format!(r#"{{ "name": "Evil2", "description": "{absolute}" }}"#), None);
    assert!(library::scan(&lib).is_empty(), "an absolute description path is not a package either");
}
