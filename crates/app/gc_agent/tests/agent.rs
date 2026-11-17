//! The agent API end to end: calls carried out on a session, and the MCP server over HTTP.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gc_agent::{Bridge, Call, execute, start};
use gc_session::Session;
use gc_session::library;

fn repo_library() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../robots")
}

#[test]
fn an_agent_builds_a_cell_and_undoes_it() {
    let packages = library::scan(&repo_library());
    let mut session = Session::new();
    session.set_robot_library(Some(repo_library()));

    let robots = execute(&mut session, &packages, &Call::LibraryRobots).unwrap();
    assert!(robots.as_array().unwrap().iter().any(|r| r["name"] == "UR5e"), "{robots}");

    let added = execute(&mut session, &packages, &Call::AddRobot { name: "UR5e".into() }).unwrap();
    let robot = added["robot"].as_str().unwrap().to_owned();
    execute(
        &mut session,
        &packages,
        &Call::PlaceRobot { robot: robot.clone(), xyz_mm: [100.0, 0.0, 0.0], rpy_deg: [0.0; 3] },
    )
    .unwrap();

    let a = execute(
        &mut session,
        &packages,
        &Call::AddWaypoint {
            robot: robot.clone(),
            reference: "world".into(),
            xyz_mm: [500.0, 200.0, 300.0],
            rpy_deg: [180.0, 0.0, 0.0],
        },
    )
    .unwrap();
    assert_eq!(a["reachable"], true, "{a}");
    let b = execute(
        &mut session,
        &packages,
        &Call::AddWaypoint {
            robot: robot.clone(),
            reference: "base".into(),
            xyz_mm: [-400.0, 200.0, 300.0],
            rpy_deg: [180.0, 0.0, 0.0],
        },
    )
    .unwrap();

    // The waypoint taught in world reads back the same numbers in world.
    let name_a = a["waypoint"].as_str().unwrap().to_owned();
    let info = execute(&mut session, &packages, &Call::WaypointInfo { name: name_a.clone() }).unwrap();
    let xyz = info["seen_from"]["world"]["xyz_mm"].as_array().unwrap();
    assert!((xyz[0].as_f64().unwrap() - 500.0).abs() < 1e-6, "{info}");
    assert!((xyz[2].as_f64().unwrap() - 300.0).abs() < 1e-6, "{info}");

    let path = execute(
        &mut session,
        &packages,
        &Call::CreatePath {
            waypoints: vec![name_a, b["waypoint"].as_str().unwrap().to_owned()],
            linear: vec![],
        },
    )
    .unwrap();
    let path = path["path"].as_str().unwrap().to_owned();
    let overview = execute(&mut session, &packages, &Call::Overview).unwrap();
    assert_eq!(overview["robots"].as_array().unwrap().len(), 1);
    assert_eq!(overview["waypoints"].as_array().unwrap().len(), 2);
    assert_eq!(overview["paths"][0]["name"], path.as_str());

    // Every edit undoes like the user's own.
    for _ in 0..5 {
        assert_eq!(execute(&mut session, &packages, &Call::Undo).unwrap()["undone"], true);
    }
    assert!(session.doc().robots().is_empty());

    // Mistakes come back as messages an agent can act on.
    let error = execute(&mut session, &packages, &Call::PlanPath { path: "nope".into() }).unwrap_err();
    assert!(error.contains("nope"), "{error}");
    let error = execute(&mut session, &packages, &Call::AddRobot { name: "R2D2".into() }).unwrap_err();
    assert!(error.contains("library_robots"), "{error}");
}

/// One HTTP exchange; returns the head (status line and headers) and the body.
fn post(
    address: std::net::SocketAddr,
    token: Option<&str>,
    session: Option<&str>,
    body: &str,
) -> (String, String) {
    let mut stream = std::net::TcpStream::connect(address).unwrap();
    stream.set_read_timeout(Some(std::time::Duration::from_secs(20))).unwrap();
    let mut request = format!(
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Type: application/json\r\n\
         Accept: application/json, text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n",
        address.port(),
        body.len()
    );
    if let Some(token) = token {
        request += &format!("Authorization: Bearer {token}\r\n");
    }
    if let Some(session) = session {
        request += &format!("Mcp-Session-Id: {session}\r\nMcp-Protocol-Version: 2025-06-18\r\n");
    }
    request += "\r\n";
    request += body;
    stream.write_all(request.as_bytes()).unwrap();
    // A streamed (SSE) reply may stay open: read until the answer is in.
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

#[test]
fn the_mcp_server_lists_and_runs_tools_for_the_right_token() {
    let (bridge, requests) = Bridge::new(Arc::new(|| {}));
    let server = start(0, "secret", bridge).unwrap();
    let address = server.address;

    // The application's side: answers every call with what it was.
    let worker = std::thread::spawn(move || {
        while let Ok(request) = requests.recv() {
            request.reply.send(Ok(serde_json::json!({ "called": request.call.summary() }))).unwrap();
        }
    });

    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#;
    let (head, _) = post(address, None, None, init);
    assert!(head.starts_with("HTTP/1.1 401"), "{head}");
    let (head, _) = post(address, Some("wrong"), None, init);
    assert!(head.starts_with("HTTP/1.1 401"), "{head}");

    let (head, body) = post(address, Some("secret"), None, init);
    assert!(head.starts_with("HTTP/1.1 200"), "{head}\n{body}");
    assert!(body.contains("GraphCTRL"), "{body}");
    let session = header(&head, "mcp-session-id").expect("a session id");

    post(
        address,
        Some("secret"),
        Some(&session),
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
    );
    let (_, body) =
        post(address, Some("secret"), Some(&session), r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#);
    for tool in [
        "overview",
        "add_robot",
        "place_robot",
        "add_waypoint",
        "waypoint_info",
        "create_path",
        "plan_path",
        "undo",
    ] {
        assert!(body.contains(&format!("\"name\":\"{tool}\"")), "{tool} missing from {body}");
    }

    let call = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"add_waypoint","arguments":{"robot":"UR5e_1","reference":"base","xyz_mm":[1,2,3],"rpy_deg":[180,0,0]}}}"#;
    let (_, body) = post(address, Some("secret"), Some(&session), call);
    assert!(body.contains("add_waypoint UR5e_1 in base"), "{body}");

    drop(server);
    worker.join().unwrap();
}
