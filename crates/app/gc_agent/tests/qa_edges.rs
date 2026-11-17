//! QA edge cases of the agent API: damaged arguments, unknown names, back-to-back calls, a server that
//! restarts with another token, a busy port, an application that is closing.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use gc_agent::{Bridge, Call, execute, start};
use gc_session::{Session, library};

fn repo_library() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../robots")
}

fn cell() -> (Session, Vec<library::Package>, String) {
    let packages = library::scan(&repo_library());
    let mut session = Session::new();
    session.set_robot_library(Some(repo_library()));
    let added = execute(&mut session, &packages, &Call::AddRobot { name: "UR5e".into() }).unwrap();
    let robot = added["robot"].as_str().unwrap().to_owned();
    (session, packages, robot)
}

fn waypoint(robot: &str, reference: &str, xyz: [f64; 3]) -> Call {
    Call::AddWaypoint {
        robot: robot.into(),
        reference: reference.into(),
        xyz_mm: xyz,
        rpy_deg: [180.0, 0.0, 0.0],
    }
}

/// Every call that names something unknown, or carries unusable numbers, is an error message that leaves
/// the document exactly as it was.
#[test]
fn bad_names_and_numbers_are_errors_and_change_nothing() {
    let (mut session, packages, robot) = cell();
    let a = execute(&mut session, &packages, &waypoint(&robot, "world", [500.0, 200.0, 300.0])).unwrap();
    let a = a["waypoint"].as_str().unwrap().to_owned();
    let before = session.doc().clone();
    let revision = session.doc().revision();
    let depth = session.history().undo_depth();

    let calls = [
        Call::AddRobot { name: String::new() },
        Call::AddRobot { name: "ur5e".into() }, // names are exact
        Call::PlaceRobot { robot: "ghost".into(), xyz_mm: [0.0; 3], rpy_deg: [0.0; 3] },
        Call::PlaceRobot { robot: robot.clone(), xyz_mm: [f64::NAN, 0.0, 0.0], rpy_deg: [0.0; 3] },
        Call::PlaceRobot { robot: robot.clone(), xyz_mm: [0.0; 3], rpy_deg: [0.0, f64::INFINITY, 0.0] },
        waypoint("ghost", "world", [0.0; 3]),
        waypoint(&robot, "no-such-frame", [0.0; 3]),
        waypoint(&robot, "", [0.0; 3]),
        waypoint(&robot, "world", [f64::NAN, 0.0, 0.0]),
        waypoint(&robot, "base", [0.0, f64::NEG_INFINITY, 0.0]),
        Call::WaypointInfo { name: "ghost".into() },
        Call::WaypointInfo { name: String::new() },
        Call::CreatePath { waypoints: vec![], linear: vec![] },
        Call::CreatePath { waypoints: vec!["ghost".into()], linear: vec![] },
        Call::CreatePath { waypoints: vec![a.clone(), "ghost".into()], linear: vec![] },
        Call::PlanPath { path: "ghost".into() },
        Call::PlanPath { path: String::new() },
    ];
    for call in calls {
        let result = execute(&mut session, &packages, &call);
        assert!(result.is_err(), "{call:?} should fail, got {result:?}");
        assert!(!result.unwrap_err().is_empty());
        assert!(session.doc().same_content(&before), "{}", call.summary());
        assert_eq!(
            (session.doc().revision(), session.history().undo_depth()),
            (revision, depth),
            "{}",
            call.summary()
        );
    }
}

/// A path with more `linear` flags than waypoints, a repeated waypoint, a path through one waypoint:
/// accepted or refused, but never a panic and never a half-made path.
#[test]
fn odd_paths_are_accepted_or_refused_cleanly() {
    let (mut session, packages, robot) = cell();
    let a = execute(&mut session, &packages, &waypoint(&robot, "world", [500.0, 200.0, 300.0])).unwrap();
    let a = a["waypoint"].as_str().unwrap().to_owned();
    for (waypoints, linear) in [
        (vec![a.clone()], vec![true, true, true]),
        (vec![a.clone(), a.clone()], vec![]),
        (vec![a.clone(); 50], vec![true]),
    ] {
        let paths = session.doc().paths().len();
        let depth = session.history().undo_depth();
        match execute(&mut session, &packages, &Call::CreatePath { waypoints, linear }) {
            Ok(v) => {
                assert_eq!(session.doc().paths().len(), paths + 1, "{v}");
                // Planning an odd path ends in a status, not a panic.
                let name = v["path"].as_str().unwrap().to_owned();
                assert!(execute(&mut session, &packages, &Call::PlanPath { path: name }).is_ok());
            }
            Err(e) => {
                assert!(!e.is_empty());
                assert_eq!((session.doc().paths().len(), session.history().undo_depth()), (paths, depth));
            }
        }
    }
}

/// Undo and redo with nothing to undo or redo, many times in a row, and read-only calls repeated: the
/// answers say what happened and the document does not change behind the agent's back.
#[test]
fn back_to_back_calls_stay_consistent() {
    let mut session = Session::new();
    let packages = library::scan(&repo_library());
    for _ in 0..30 {
        assert_eq!(execute(&mut session, &packages, &Call::Undo).unwrap()["undone"], false);
        assert_eq!(execute(&mut session, &packages, &Call::Redo).unwrap()["redone"], false);
    }
    let (mut session, packages, robot) = cell();
    let revision = session.doc().revision();
    let first = execute(&mut session, &packages, &Call::Overview).unwrap();
    for _ in 0..50 {
        assert_eq!(execute(&mut session, &packages, &Call::Overview).unwrap(), first);
        execute(&mut session, &packages, &Call::LibraryRobots).unwrap();
    }
    assert_eq!(session.doc().revision(), revision, "reading changes nothing");

    // Fifty edits, fifty undos, fifty redos land on the same document.
    for k in 0..50 {
        execute(&mut session, &packages, &waypoint(&robot, "world", [400.0 + f64::from(k), 100.0, 300.0]))
            .unwrap();
    }
    let full = session.doc().clone();
    for _ in 0..50 {
        assert_eq!(execute(&mut session, &packages, &Call::Undo).unwrap()["undone"], true);
    }
    assert!(session.doc().waypoints().is_empty());
    for _ in 0..50 {
        assert_eq!(execute(&mut session, &packages, &Call::Redo).unwrap()["redone"], true);
    }
    assert!(session.doc().same_content(&full));
}

// ---- over HTTP ----

fn exchange(
    address: std::net::SocketAddr,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> (String, String) {
    let mut stream = std::net::TcpStream::connect(address).unwrap();
    stream.set_read_timeout(Some(std::time::Duration::from_secs(20))).unwrap();
    let mut request = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Type: application/json\r\n\
         Accept: application/json, text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n",
        address.port(),
        body.len()
    );
    for (k, v) in headers {
        request += &format!("{k}: {v}\r\n");
    }
    request += "\r\n";
    request += body;
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        response.extend_from_slice(&buf[..n]);
        let text = String::from_utf8_lossy(&response);
        let answered = text.contains("\"result\"") || text.contains("\"error\"");
        let complete = text.contains("\r\n\r\n") && (text.contains("\n\n") || text.ends_with('}'));
        if (answered && complete) || text.starts_with("HTTP/1.1 4") || text.starts_with("HTTP/1.1 202") {
            break;
        }
    }
    let text = String::from_utf8_lossy(&response).into_owned();
    let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    (head.to_owned(), body.to_owned())
}

fn header(head: &str, name: &str) -> Option<String> {
    head.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim().eq_ignore_ascii_case(name).then(|| v.trim().to_owned())
    })
}

const INIT: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"qa","version":"0"}}}"#;

/// Opens an MCP session; returns its id.
fn open(address: std::net::SocketAddr, token: &str) -> String {
    let auth = format!("Bearer {token}");
    let (head, body) = exchange(address, "POST", "/mcp", &[("Authorization", &auth)], INIT);
    assert!(head.starts_with("HTTP/1.1 200"), "{head}\n{body}");
    let session = header(&head, "mcp-session-id").unwrap_or_else(|| panic!("no session id in {head}"));
    exchange(
        address,
        "POST",
        "/mcp",
        &[("Authorization", &auth), ("Mcp-Session-Id", &session), ("Mcp-Protocol-Version", "2025-06-18")],
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
    );
    session
}

fn rpc(address: std::net::SocketAddr, token: &str, session: &str, body: &str) -> (String, String) {
    let auth = format!("Bearer {token}");
    exchange(
        address,
        "POST",
        "/mcp",
        &[("Authorization", &auth), ("Mcp-Session-Id", session), ("Mcp-Protocol-Version", "2025-06-18")],
        body,
    )
}

fn call_body(id: u32, tool: &str, arguments: &str) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/call","params":{{"name":"{tool}","arguments":{arguments}}}}}"#
    )
}

/// A worker that counts the calls it receives and answers each with its summary.
fn counting_worker(
    requests: std::sync::mpsc::Receiver<gc_agent::Request>,
) -> (Arc<AtomicUsize>, std::thread::JoinHandle<()>) {
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    let worker = std::thread::spawn(move || {
        while let Ok(request) = requests.recv() {
            seen.fetch_add(1, Ordering::SeqCst);
            request.reply.send(Ok(serde_json::json!({ "called": request.call.summary() }))).unwrap();
        }
    });
    (count, worker)
}

/// Arguments that do not fit the tool's schema never reach the application, and the agent is told so.
#[test]
fn damaged_arguments_are_refused_before_the_application_sees_them() {
    let (bridge, requests) = Bridge::new(Arc::new(|| {}));
    let server = start(0, "secret", bridge).unwrap();
    let address = server.address;
    let (count, worker) = counting_worker(requests);
    let session = open(address, "secret");

    let bad = [
        ("add_waypoint", r#"{}"#),
        ("add_waypoint", r#"{"robot":"R","reference":"world","xyz_mm":"abc","rpy_deg":[0,0,0]}"#),
        ("add_waypoint", r#"{"robot":"R","reference":"world","xyz_mm":[1,2],"rpy_deg":[0,0,0]}"#),
        ("add_waypoint", r#"{"robot":"R","reference":"world","xyz_mm":[1,2,3,4],"rpy_deg":[0,0,0]}"#),
        ("add_waypoint", r#"{"robot":5,"reference":"world","xyz_mm":[1,2,3],"rpy_deg":[0,0,0]}"#),
        ("add_waypoint", r#"{"robot":"R","reference":"world","xyz_mm":[null,2,3],"rpy_deg":[0,0,0]}"#),
        ("add_waypoint", r#"{"robot":"R","reference":"world","xyz_mm":["1","2","3"],"rpy_deg":[0,0,0]}"#),
        ("add_robot", r#"{"name":["UR5e"]}"#),
        ("add_robot", r#"null"#),
        ("place_robot", r#"{"robot":"R"}"#),
        ("create_path", r#"{"waypoints":"a"}"#),
        ("create_path", r#"{"waypoints":["a"],"linear":"yes"}"#),
        ("plan_path", r#"{"path":{"name":"p"}}"#),
        ("no_such_tool", r#"{}"#),
        ("", r#"{}"#),
    ];
    for (k, (tool, arguments)) in bad.iter().enumerate() {
        let (_, body) = rpc(address, "secret", &session, &call_body(10 + k as u32, tool, arguments));
        let refused = body.contains("\"error\"") || body.contains("\"isError\":true");
        assert!(refused, "{tool} {arguments} was accepted: {body}");
    }
    assert_eq!(count.load(Ordering::SeqCst), 0, "no damaged call reached the application");

    // A malformed JSON-RPC envelope is an error answer or a 4xx, not a hang or a crash.
    for garbage in [
        "{",
        "[]",
        "null",
        "{\"jsonrpc\":\"2.0\"}",
        "{\"jsonrpc\":\"1.0\",\"id\":1,\"method\":\"tools/list\"}",
        "",
    ] {
        let (head, body) = rpc(address, "secret", &session, garbage);
        assert!(
            head.starts_with("HTTP/1.1 4") || body.contains("\"error\"") || head.starts_with("HTTP/1.1 202"),
            "{garbage:?}: {head}\n{body}"
        );
    }
    assert_eq!(count.load(Ordering::SeqCst), 0);

    // The server still works afterwards.
    let (_, body) = rpc(address, "secret", &session, &call_body(99, "overview", "{}"));
    assert!(body.contains("overview"), "{body}");
    assert_eq!(count.load(Ordering::SeqCst), 1);
    drop(server);
    worker.join().unwrap();
}

/// Many calls one after the other all get their own answers, in order, on one MCP session.
#[test]
fn consecutive_calls_each_get_their_answer() {
    let (bridge, requests) = Bridge::new(Arc::new(|| {}));
    let server = start(0, "secret", bridge).unwrap();
    let address = server.address;
    let (count, worker) = counting_worker(requests);
    let session = open(address, "secret");
    for k in 0..25u32 {
        let name = format!("W{k}");
        let (_, body) = rpc(
            address,
            "secret",
            &session,
            &call_body(100 + k, "waypoint_info", &format!(r#"{{"name":"{name}"}}"#)),
        );
        assert!(body.contains(&format!("waypoint_info {name}")), "{k}: {body}");
        assert!(body.contains(&format!("\"id\":{}", 100 + k)), "{k}: {body}");
    }
    assert_eq!(count.load(Ordering::SeqCst), 25);
    drop(server);
    worker.join().unwrap();
}

/// Only the exact header is accepted, on every path, and the wrong path is not served.
#[test]
fn only_the_exact_token_gets_in() {
    let (bridge, requests) = Bridge::new(Arc::new(|| {}));
    let server = start(0, "secret", bridge).unwrap();
    let address = server.address;
    let (count, worker) = counting_worker(requests);
    for auth in [
        "bearer secret",
        "Bearer  secret",
        "Bearer secre",
        "Bearer secrets",
        "Basic secret",
        "secret",
        "Bearer ",
        "",
    ] {
        let (head, _) = exchange(address, "POST", "/mcp", &[("Authorization", auth)], INIT);
        assert!(head.starts_with("HTTP/1.1 401"), "{auth:?}: {head}");
    }
    // The token in the query string or another header is not a token.
    let (head, _) = exchange(address, "POST", "/mcp?token=secret", &[], INIT);
    assert!(head.starts_with("HTTP/1.1 401"), "{head}");
    let (head, _) = exchange(address, "POST", "/mcp", &[("X-Api-Key", "secret")], INIT);
    assert!(head.starts_with("HTTP/1.1 401"), "{head}");
    // Unauthorised GET and DELETE are refused too.
    for method in ["GET", "DELETE"] {
        let (head, _) = exchange(address, method, "/mcp", &[], "");
        assert!(head.starts_with("HTTP/1.1 401"), "{method}: {head}");
    }
    // Right token, wrong path: not found, not served.
    let (head, _) = exchange(address, "POST", "/other", &[("Authorization", "Bearer secret")], INIT);
    assert!(head.starts_with("HTTP/1.1 404") || head.starts_with("HTTP/1.1 401"), "{head}");
    assert_eq!(count.load(Ordering::SeqCst), 0);
    drop(server);
    worker.join().unwrap();
}

/// After the server restarts with another token, the old token and the old MCP session are refused;
/// the new token works. (The restart reuses the port, as the application does.)
#[test]
fn a_restart_with_a_new_token_refuses_the_old_one() {
    let (bridge, requests) = Bridge::new(Arc::new(|| {}));
    let first = start(0, "old-token", bridge).unwrap();
    let address = first.address;
    let (_, worker) = counting_worker(requests);
    let old_session = open(address, "old-token");
    drop(first);
    worker.join().unwrap();

    let (bridge, requests) = Bridge::new(Arc::new(|| {}));
    let second = start(address.port(), "new-token", bridge).unwrap();
    let (count, worker) = counting_worker(requests);
    // The old token no longer opens anything.
    let (head, _) = exchange(address, "POST", "/mcp", &[("Authorization", "Bearer old-token")], INIT);
    assert!(head.starts_with("HTTP/1.1 401"), "{head}");
    // The old session id with the new token: unknown session, not a call that runs.
    let (head, body) = rpc(address, "new-token", &old_session, &call_body(5, "overview", "{}"));
    assert!(!head.starts_with("HTTP/1.1 200") || body.contains("\"error\""), "{head}\n{body}");
    assert_eq!(count.load(Ordering::SeqCst), 0, "a stale session ran a call");
    // The old session id with the old token is refused by the token before anything else.
    let (head, _) = rpc(address, "old-token", &old_session, &call_body(6, "overview", "{}"));
    assert!(head.starts_with("HTTP/1.1 401"), "{head}");
    // A fresh session with the new token works.
    let fresh = open(address, "new-token");
    let (_, body) = rpc(address, "new-token", &fresh, &call_body(7, "overview", "{}"));
    assert!(body.contains("overview"), "{body}");
    drop(second);
    worker.join().unwrap();
}

/// A port that is taken is an error naming the address, and does not leave a server behind.
#[test]
fn a_busy_port_is_reported() {
    let taken = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = taken.local_addr().unwrap().port();
    let (bridge, _requests) = Bridge::new(Arc::new(|| {}));
    let error = start(port, "t", bridge).err().expect("the port is busy");
    assert!(error.contains(&port.to_string()), "{error}");
    // Once the port is free again the same call works.
    drop(taken);
    let (bridge, _requests) = Bridge::new(Arc::new(|| {}));
    let server = start(port, "t", bridge).unwrap();
    assert_eq!(server.address.port(), port);
}

/// The application's side going away (window closing) answers with an error instead of hanging; an
/// application that answers with an error passes it on.
#[test]
fn a_closing_or_failing_application_is_reported_to_the_agent() {
    let (bridge, requests) = Bridge::new(Arc::new(|| {}));
    let server = start(0, "secret", bridge).unwrap();
    let address = server.address;
    let session = open(address, "secret");

    // An application that refuses the call.
    let reply_to = requests;
    let worker = std::thread::spawn(move || {
        if let Ok(request) = reply_to.recv() {
            request.reply.send(Err("no robot named `ghost`".to_owned())).unwrap();
        }
        // Then the receiving end is dropped: the window closed.
    });
    let (_, body) = rpc(address, "secret", &session, &call_body(2, "overview", "{}"));
    assert!(body.contains("\"isError\":true") && body.contains("no robot named"), "{body}");
    worker.join().unwrap();
    let (_, body) = rpc(address, "secret", &session, &call_body(3, "overview", "{}"));
    assert!(body.contains("\"isError\":true") && body.contains("closing"), "{body}");
    drop(server);
}

/// An absurd (finite) base position is accepted as is. It must at least not crash anything that follows.
#[test]
fn an_absurd_placement_does_not_crash_what_follows() {
    let (mut session, packages, robot) = cell();
    let placed = Call::PlaceRobot { robot: robot.clone(), xyz_mm: [1e300, 0.0, 0.0], rpy_deg: [0.0; 3] };
    drop(execute(&mut session, &packages, &placed));
    drop(execute(&mut session, &packages, &Call::Overview));
    if let Ok(w) = execute(&mut session, &packages, &waypoint(&robot, "base", [400.0, 0.0, 300.0])) {
        let name = w["waypoint"].as_str().unwrap().to_owned();
        drop(execute(&mut session, &packages, &Call::WaypointInfo { name: name.clone() }));
        if let Ok(p) =
            execute(&mut session, &packages, &Call::CreatePath { waypoints: vec![name], linear: vec![] })
        {
            drop(execute(
                &mut session,
                &packages,
                &Call::PlanPath { path: p["path"].as_str().unwrap().to_owned() },
            ));
        }
    }
    assert_eq!(execute(&mut session, &packages, &Call::Undo).unwrap()["undone"], true);
}

/// A base position of 1e300 mm is far outside any cell. Agents cannot tell it is a mistake unless the
/// tool refuses it.
#[test]
#[ignore = "BUG: place_robot accepts absurd finite positions (1e300 mm); see docs/handoff/qa.md"]
fn an_absurd_placement_is_refused() {
    let (mut session, packages, robot) = cell();
    let before = session.doc().clone();
    let placed = Call::PlaceRobot { robot, xyz_mm: [f64::MAX, 1e300, 0.0], rpy_deg: [0.0; 3] };
    assert!(execute(&mut session, &packages, &placed).is_err());
    assert!(session.doc().same_content(&before));
}
