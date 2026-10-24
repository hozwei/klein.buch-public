//! Plan W7 — mehrere Off-Site-Ziele (W7d):
//! - Verzeichnis-Ziele nur unter der Wurzel `/backup-targets` (hier im
//!   Testverzeichnis), Symlink nach draußen abgelehnt
//! - zwei Ziele (Verzeichnis + nicht erreichbares SFTP): das Verzeichnis wird
//!   geschrieben, das SFTP-Ziel protokolliert seinen Fehler, Inbox-Warnung,
//!   Status `failing`; ohne Ziel `missing`
//! - Hinzufügen/Entfernen nur mit Re-Auth

mod server_support;

use axum::http::StatusCode;
use server_support::*;

async fn settings(srv: &TestServer, cookie: &str) -> serde_json::Value {
    let r = call(
        &srv.app,
        cookie,
        "backup_get_settings",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    r.json()
}

async fn backup_now(srv: &TestServer, cookie: &str) -> serde_json::Value {
    let r = call(&srv.app, cookie, "backup_create_now", serde_json::json!({})).await;
    let body = r.json();
    if body.get("state").is_some() {
        assert_eq!(body["state"], "done", "{}", r.text());
        body["result"].clone()
    } else {
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
        body
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_targets_one_fails_other_is_written() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let root = srv.state.ctx.paths.backup_targets_dir.clone();
    std::fs::create_dir_all(root.join("nas")).unwrap();

    let s = settings(&srv, &cookie).await;
    assert_eq!(s["offsiteStatus"], "missing");
    assert_eq!(s["availableDirs"], serde_json::json!(["nas"]));

    // ohne Re-Auth → 403
    let add = serde_json::json!({ "label": "NAS", "directory": "nas/klein-buch" });
    let r = call(
        &srv.app,
        &cookie,
        "backup_target_add_directory",
        add.clone(),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.text());
    let cookie = reauth_cookie(&srv, &cookie).await;

    // außerhalb der Wurzel / Symlink nach draußen → abgelehnt
    let outside = srv.dir.path().join("draussen");
    std::fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();
    for bad in [
        "../draussen",
        "/etc",
        "link",
        "link/kb",
        "nas/../../draussen",
    ] {
        let r = call(
            &srv.app,
            &cookie,
            "backup_target_add_directory",
            serde_json::json!({ "label": "x", "directory": bad }),
        )
        .await;
        assert_eq!(
            r.status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{bad}: {}",
            r.text()
        );
    }
    assert!(
        std::fs::read_dir(&outside).unwrap().next().is_none(),
        "nichts draußen"
    );

    let r = call(&srv.app, &cookie, "backup_target_add_directory", add).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    // SFTP-Ziel, das nicht erreichbar ist (Port 1 auf Loopback; ohne Passwort)
    {
        let pool = srv.state.ctx.pool().unwrap();
        let mut targets: Vec<serde_json::Value> = serde_json::from_str(
            &sqlx::query_scalar::<_, String>(
                "SELECT value FROM app_settings WHERE key = 'backup_targets'",
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
        )
        .unwrap();
        targets.push(serde_json::json!({
            "id": "sftp-tot", "label": "Server (aus)", "kind": "sftp",
            "host": "127.0.0.1", "port": 1, "user": "kb", "remote_dir": "kb",
            "host_fingerprint": "SHA256:test"
        }));
        sqlx::query("UPDATE app_settings SET value = ? WHERE key = 'backup_targets'")
            .bind(serde_json::to_string(&targets).unwrap())
            .execute(&pool)
            .await
            .unwrap();
    }

    let out = backup_now(&srv, &cookie).await;
    let offsite = out["offsite"].as_array().unwrap();
    assert_eq!(offsite.len(), 2, "{out}");
    let nas = offsite.iter().find(|o| o["label"] == "NAS").unwrap();
    let dead = offsite
        .iter()
        .find(|o| o["targetId"] == "sftp-tot")
        .unwrap();
    assert!(nas["error"].is_null() && dead["error"].is_string(), "{out}");
    let written: Vec<_> = std::fs::read_dir(root.join("nas/klein-buch"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    // (der automatische Tages-Lauf kann zusätzlich geschrieben haben)
    let manual = out["fileName"].as_str().unwrap();
    assert!(written.iter().any(|n| n == manual), "{written:?}");
    assert!(written
        .iter()
        .all(|n| n.ends_with(".kbk") && !n.starts_with('.')));

    let s = settings(&srv, &cookie).await;
    assert_eq!(s["offsiteStatus"], "failing");
    let pool = srv.state.ctx.pool().unwrap();
    let failed: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM backup_log WHERE status = 'failed' AND target_kind = 'sftp'
         AND target_label = 'Server (aus)'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(failed >= 1, "Fehler je Ziel im Protokoll");
    let warned: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications WHERE title = 'Externe Backup-Spiegelung fehlgeschlagen'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(warned, 1, "Inbox-Warnung");
    let history: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM backup_history WHERE target_path LIKE '%/nas/klein-buch/%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(history >= 1);

    // Loopback-Healthcheck meldet den Status
    let r = send(&srv.app, from_peer(get("/healthz", None), "127.0.0.1:5000")).await;
    assert_eq!(r.json()["offsite"], "failing", "{}", r.text());

    // defektes Ziel entfernen → ok
    let r = call(
        &srv.app,
        &cookie,
        "backup_target_remove",
        serde_json::json!({ "id": "sftp-tot" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let s = settings(&srv, &cookie).await;
    assert_eq!(s["offsiteStatus"], "ok");
    assert_eq!(s["targets"].as_array().unwrap().len(), 1);
    assert_eq!(s["targets"][0]["directory"], "nas/klein-buch");
    // doppelt anlegen → abgelehnt
    let r = call(
        &srv.app,
        &cookie,
        "backup_target_add_directory",
        serde_json::json!({ "label": "NAS 2", "directory": "nas/klein-buch" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_target_warns_but_does_not_block() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let out = backup_now(&srv, &cookie).await;
    assert!(out["offsite"].as_array().unwrap().is_empty());
    let pool = srv.state.ctx.pool().unwrap();
    let warned: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications WHERE title = 'Kein externes Backup-Ziel'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(warned, 1);
    // zweites Backup: kein zweiter Hinweis (Dedup)
    backup_now(&srv, &cookie).await;
    let warned: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications WHERE title = 'Kein externes Backup-Ziel'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(warned, 1);
}
