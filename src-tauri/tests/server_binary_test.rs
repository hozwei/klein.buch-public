//! Plan W5 Done-Check: Server läuft als Binary mit `KLEIN_BUCH_*` aus der
//! Umgebung; der Subcommand `healthcheck` liefert `setup`, nach der
//! Einrichtung `unlocked`, nach einem Neustart `locked` und nach dem Login
//! wieder `unlocked`. SIGTERM beendet den Server geordnet mit Exit 0.

mod server_support;

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use klein_buch_lib::config::Paths;
use server_support::*;

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn server_cmd(paths: &Paths, port: u16, sub: Option<&str>) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_klein-buch-server"));
    if let Some(s) = sub {
        cmd.arg(s);
    }
    cmd.env("KLEIN_BUCH_DATA_DIR", &paths.data_dir)
        .env("KLEIN_BUCH_INPUTS_DIR", &paths.inputs_dir)
        .env("KLEIN_BUCH_RESOURCE_DIR", &paths.resource_dir)
        .env("KLEIN_BUCH_WEB_DIR", &paths.web_dir)
        .env("KLEIN_BUCH_LISTEN", format!("127.0.0.1:{port}"))
        .env("KLEIN_BUCH_ALLOW_UNMOUNTED", "1")
        .env_remove("KLEIN_BUCH_SIDECAR_MOCK")
        .env("RUST_LOG", "info");
    cmd
}

fn start(paths: &Paths, port: u16, log: &Path) -> Child {
    let file = std::fs::File::create(log).unwrap();
    server_cmd(paths, port, None)
        .stdout(Stdio::from(file.try_clone().unwrap()))
        .stderr(Stdio::from(file))
        .spawn()
        .unwrap()
}

/// `klein-buch-server healthcheck` → (Exit-Erfolg, stdout).
fn healthcheck(paths: &Paths, port: u16) -> (bool, String) {
    let out = server_cmd(paths, port, Some("healthcheck"))
        .output()
        .unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).trim().to_string(),
    )
}

fn wait_healthy(paths: &Paths, port: u16, log: &Path) -> String {
    for _ in 0..200 {
        let (ok, body) = healthcheck(paths, port);
        if ok {
            return body;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!(
        "Server nicht gesund: {}",
        std::fs::read_to_string(log).unwrap_or_default()
    );
}

fn terminate(child: &mut Child) -> std::process::ExitStatus {
    let status = Command::new("kill")
        .arg("-TERM")
        .arg(child.id().to_string())
        .status()
        .unwrap();
    assert!(status.success());
    for _ in 0..300 {
        if let Some(st) = child.try_wait().unwrap() {
            return st;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    child.kill().unwrap();
    panic!("Server reagiert nicht auf SIGTERM");
}

async fn post_auth(port: u16, path: &str) -> (u16, Option<String>) {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{port}{path}"))
        .header("X-KleinBuch-Request", "1")
        .json(&serde_json::json!({ "passphrase": PASS }))
        .send()
        .await
        .unwrap();
    let cookie = resp
        .headers()
        .get("set-cookie")
        .map(|v| v.to_str().unwrap().to_string());
    (resp.status().as_u16(), cookie)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn healthcheck_reports_setup_unlocked_locked() {
    let dir = tempfile::tempdir().unwrap();
    let paths = prepare_dirs(dir.path());
    let log = dir.path().join("server.log");
    let port = free_port();

    let mut child = start(&paths, port, &log);
    assert_eq!(wait_healthy(&paths, port, &log), r#"{"state":"setup"}"#);

    let (status, cookie) = post_auth(port, "/api/auth/setup").await;
    assert_eq!(status, 200);
    let cookie = cookie.unwrap();
    assert!(cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict"));
    assert_eq!(healthcheck(&paths, port).1, r#"{"state":"unlocked"}"#);
    assert!(paths.data_dir.join(".klein-buch-instance").exists());

    let exit = terminate(&mut child);
    assert!(exit.success(), "geordnetes Herunterfahren: {exit:?}");

    // Neustart: gesperrt bis zum Login (E3), gilt als gesund.
    let mut child = start(&paths, port, &log);
    assert_eq!(wait_healthy(&paths, port, &log), r#"{"state":"locked"}"#);
    let (status, _) = post_auth(port, "/api/auth/login").await;
    assert_eq!(status, 200);
    assert_eq!(healthcheck(&paths, port).1, r#"{"state":"unlocked"}"#);
    let exit = terminate(&mut child);
    assert!(exit.success());

    let text = std::fs::read_to_string(&log).unwrap();
    assert!(!text.contains(PASS), "Passphrase im Server-Log");
}
