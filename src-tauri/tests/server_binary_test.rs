//! Plan W5/W6 Done-Check: Server läuft als Binary mit `KLEIN_BUCH_*` aus der
//! Umgebung; der Subcommand `healthcheck` liefert `setup`, nach der
//! Einrichtung (Setup-Token aus dem Log + Passphrase + TOTP) `unlocked`, nach
//! einem Neustart `locked` und nach dem Login (Einmal-Code) wieder
//! `unlocked`. `totp-reset` beendet alle Sitzungen. SIGTERM beendet den
//! Server geordnet mit Exit 0.

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
        .env("KLEIN_BUCH_PUBLIC_URL", format!("http://127.0.0.1:{port}"))
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

/// `POST` an eine Auth-Route über Loopback; liefert Status, Körper, Cookie.
async fn post_auth(
    port: u16,
    path: &str,
    body: serde_json::Value,
    cookie: Option<&str>,
) -> (u16, serde_json::Value, Option<String>) {
    let client = reqwest::Client::new();
    let mut req = client
        .post(format!("http://127.0.0.1:{port}{path}"))
        .header("X-KleinBuch-Request", "1")
        .json(&body);
    if let Some(c) = cookie {
        req = req.header("Cookie", format!("__Host-kb_session={c}"));
    }
    let resp = req.send().await.unwrap();
    let status = resp.status().as_u16();
    let cookie = resp
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|v| v.starts_with("__Host-kb_session="))
        .map(str::to_string);
    let json = resp.json().await.unwrap_or(serde_json::Value::Null);
    (status, json, cookie)
}

async fn get_status(port: u16, cookie: &str) -> String {
    reqwest::Client::new()
        .get(format!("http://127.0.0.1:{port}/api/auth/status"))
        .header("Cookie", format!("__Host-kb_session={cookie}"))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap()["state"]
        .as_str()
        .unwrap()
        .to_string()
}

/// Setup-Token aus dem Log (eigene Zeile auf stderr).
fn setup_token(log: &Path) -> String {
    for _ in 0..100 {
        let text = std::fs::read_to_string(log).unwrap_or_default();
        if let Some(line) = text.lines().find(|l| l.starts_with("Setup-Token: ")) {
            return line["Setup-Token: ".len()..].trim().to_string();
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("kein Setup-Token im Log");
}

fn cookie_value(set_cookie: &str) -> String {
    set_cookie
        .split(';')
        .next()
        .unwrap()
        .trim_start_matches("__Host-kb_session=")
        .to_string()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn healthcheck_reports_setup_unlocked_locked() {
    use klein_buch_lib::server::totp;
    let dir = tempfile::tempdir().unwrap();
    let paths = prepare_dirs(dir.path());
    let log = dir.path().join("server.log");
    let port = free_port();

    let mut child = start(&paths, port, &log);
    assert_eq!(wait_healthy(&paths, port, &log), r#"{"state":"setup"}"#);

    // Einrichtung: Token aus dem Log → QR/Geheimnis → Passphrase + Code.
    let token = setup_token(&log);
    let (status, offer, _) = post_auth(
        port,
        "/api/auth/setup",
        serde_json::json!({ "setupToken": token, "step": "totp" }),
        None,
    )
    .await;
    assert_eq!(status, 200, "{offer}");
    let secret = totp::decode_secret(offer["secret"].as_str().unwrap()).unwrap();
    let code = totp::code_for_step(&secret, totp::step_at(totp::unix_now()));
    let (status, done, cookie) = post_auth(
        port,
        "/api/auth/setup",
        serde_json::json!({ "setupToken": token, "passphrase": PASS, "totpCode": code }),
        None,
    )
    .await;
    assert_eq!(status, 200, "{done}");
    let cookie = cookie.unwrap();
    assert!(cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict"));
    let recovery: Vec<String> = serde_json::from_value(done["recoveryCodes"].clone()).unwrap();
    assert_eq!(recovery.len(), 10);
    assert_eq!(healthcheck(&paths, port).1, r#"{"state":"unlocked"}"#);
    assert!(paths.data_dir.join(".klein-buch-instance").exists());
    // Token nach Erfolg ungültig
    let (status, _, _) = post_auth(
        port,
        "/api/auth/setup",
        serde_json::json!({ "setupToken": token, "step": "totp" }),
        None,
    )
    .await;
    assert_eq!(status, 403);

    let exit = terminate(&mut child);
    assert!(exit.success(), "geordnetes Herunterfahren: {exit:?}");

    // Neustart: gesperrt bis zum Login (E3), gilt als gesund.
    let mut child = start(&paths, port, &log);
    assert_eq!(wait_healthy(&paths, port, &log), r#"{"state":"locked"}"#);
    let (status, body, cookie) = post_auth(
        port,
        "/api/auth/login",
        serde_json::json!({ "passphrase": PASS, "recoveryCode": recovery[0] }),
        None,
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(healthcheck(&paths, port).1, r#"{"state":"unlocked"}"#);
    let session = cookie_value(&cookie.unwrap());
    assert_eq!(get_status(port, &session).await, "ok");

    // totp-reset per Subcommand: alle Sitzungen ungültig, Neueinrichtung nötig.
    let out = server_cmd(&paths, port, Some("totp-reset"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(get_status(port, &session).await, "totp_setup_required");
    let exit = terminate(&mut child);
    assert!(exit.success());

    let text = std::fs::read_to_string(&log).unwrap();
    assert!(!text.contains(PASS), "Passphrase im Server-Log");
    assert!(!text.contains(&recovery[0]), "Einmal-Code im Server-Log");
}
