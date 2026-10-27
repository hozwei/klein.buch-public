//! Plan W8 — Zugangsdaten in der verschlüsselten DB und Verbindungstests:
//! - Migration 0031: Tabelle `secrets` (STRICT), Schema-Version 31
//! - Testpasswort nicht im Klartext in DB-Datei, Audit, Migrations- und
//!   DSGVO-ZIP
//! - Hostwechsel ohne Passwort abgelehnt bzw. löscht das Passwort; Klartext-
//!   SMTP mit Zugangsdaten abgelehnt; Microsoft-Konten im Server abgelehnt (E9)
//! - nicht freigegebener Port, Test mit Benutzer ohne Passwort: Fehler ohne
//!   Verbindungsaufbau; 10 Tests je Minute; einheitliche Fehlermeldung
//! - SFTP: Test ohne Passwort ohne Verbindung, Fingerprint ≠ Probe abgelehnt,
//!   Ziel mit Fingerprint, Test per ID, Backup landet auf dem Server

mod server_support;
mod sftp_mock;

use std::io::Read;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::http::StatusCode;
use server_support::*;

const SMTP_SECRET: &str = "W8-Testpasswort-Smtp-7c1e";
const SFTP_SECRET: &str = "W8-Testpasswort-Sftp-42aa";

fn job_result(r: &Reply) -> serde_json::Value {
    let body = r.json();
    if body.get("state").is_some() {
        assert_eq!(body["state"], "done", "{}", r.text());
        body["result"].clone()
    } else {
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
        body
    }
}

fn account_input(host: &str, user: Option<&str>, tls: bool) -> serde_json::Value {
    serde_json::json!({
        "label": "W8-Konto",
        "authType": "smtp_password",
        "smtpHost": host,
        "smtpPort": 587,
        "smtpUser": user,
        "smtpUseTls": tls,
        "fromEmail": "rechnung@example.org",
        "fromName": "Beispiel IT-Service",
        "isDefault": true
    })
}

async fn create_account(srv: &TestServer, cookie: &str) -> String {
    let r = call(
        &srv.app,
        cookie,
        "mail_account_create",
        serde_json::json!({
            "input": account_input("smtp.example.org", Some("kb@example.org"), true),
            "password": SMTP_SECRET
        }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    r.json()["id"].as_str().unwrap().to_string()
}

async fn secret(srv: &TestServer, key: &str) -> Option<String> {
    let pool = srv.state.ctx.pool().unwrap();
    sqlx::query_scalar("SELECT value FROM secrets WHERE key = ?")
        .bind(key)
        .fetch_optional(&pool)
        .await
        .unwrap()
}

fn contains(hay: &[u8], needle: &str) -> bool {
    hay.windows(needle.len()).any(|w| w == needle.as_bytes())
}

/// Alle Einträge eines ZIP entpackt (Name, Inhalt).
fn zip_entries(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    (0..zip.len())
        .map(|i| {
            let mut f = zip.by_index(i).unwrap();
            let mut out = Vec::new();
            f.read_to_end(&mut out).unwrap();
            (f.name().to_string(), out)
        })
        .collect()
}

async fn download(srv: &TestServer, cookie: &str, ticket: &serde_json::Value) -> Vec<u8> {
    let id = ticket["downloadId"].as_str().expect("downloadId");
    let r = send(
        &srv.app,
        get(&format!("/files/download/{id}"), Some(cookie)),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    r.body.to_vec()
}

/// TCP-Gegenstelle auf 127.0.0.1, die angenommene Verbindungen zählt.
async fn counting_listener() -> (u16, Arc<AtomicUsize>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let count = Arc::new(AtomicUsize::new(0));
    let c = count.clone();
    tokio::spawn(async move {
        while let Ok((sock, _)) = listener.accept().await {
            c.fetch_add(1, Ordering::SeqCst);
            drop(sock);
        }
    });
    (port, count)
}

/// Freier Port auf 127.0.0.1, auf dem niemand lauscht.
fn closed_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

async fn test_connection(
    srv: &TestServer,
    cookie: &str,
    host: &str,
    port: u16,
    tls: bool,
    user: Option<&str>,
    password: Option<&str>,
) -> Reply {
    call(
        &srv.app,
        cookie,
        "mail_account_test_connection",
        serde_json::json!({ "args": {
            "smtpHost": host, "smtpPort": port, "smtpUseTls": tls,
            "smtpUser": user, "password": password
        }}),
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mail_password_is_encrypted_and_never_exported() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let _draft = draft_invoice(&srv).await;
    let cookie = reauth_cookie(&srv, &cookie).await;

    // Migration 0031: STRICT-Tabelle; Schema-Version der Binary (ab 0031)
    {
        let pool = srv.state.ctx.pool().unwrap();
        let sql: String = sqlx::query_scalar(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'secrets'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(sql.contains("STRICT"), "{sql}");
        let v: String =
            sqlx::query_scalar("SELECT value FROM app_settings WHERE key = 'schema_version'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            v,
            klein_buch_lib::db::schema_version::EXPECTED_SCHEMA_VERSION.to_string()
        );
    }

    let id = create_account(&srv, &cookie).await;
    assert_eq!(
        secret(&srv, &format!("smtp:{id}")).await.as_deref(),
        Some(SMTP_SECRET)
    );
    let list = call(
        &srv.app,
        &cookie,
        "mail_accounts_list",
        serde_json::json!({}),
    )
    .await;
    assert!(!list.text().contains(SMTP_SECRET), "{}", list.text());
    {
        let pool = srv.state.ctx.pool().unwrap();
        let audit: Vec<String> =
            sqlx::query_scalar("SELECT COALESCE(details_json, '') FROM audit_log")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(audit.iter().all(|d| !d.contains(SMTP_SECRET)));
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&pool)
            .await
            .unwrap();
    }
    // DB-Datei (SQLCipher) enthält das Passwort nicht im Klartext
    let db = srv.state.ctx.paths.db_file.clone();
    for suffix in ["", "-wal"] {
        let mut p = db.as_os_str().to_os_string();
        p.push(suffix);
        if let Ok(bytes) = std::fs::read(&p) {
            assert!(!contains(&bytes, SMTP_SECRET), "{p:?}");
        }
    }

    // Migrations-Export: lesbar (Konto drin), Passwort und `secrets` nicht
    let r = call(
        &srv.app,
        &cookie,
        "migration_export_run",
        serde_json::json!({}),
    )
    .await;
    let out = job_result(&r);
    let zip = download(&srv, &cookie, &out["download"]).await;
    let entries = zip_entries(&zip);
    assert!(
        entries.iter().any(|(_, b)| contains(b, "W8-Konto")),
        "Konto im Export"
    );
    for (name, bytes) in &entries {
        assert!(!contains(bytes, SMTP_SECRET), "Passwort in {name}");
        // nur die Schema-Datei der Migration, keine Tabellendaten
        assert!(
            !name.contains("secrets") || name.starts_with("schema/"),
            "{name}"
        );
    }

    // DSGVO-Auskunft eines Kontakts
    let contact_id: String = {
        let pool = srv.state.ctx.pool().unwrap();
        sqlx::query_scalar("SELECT id FROM contacts LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap()
    };
    let r = call(
        &srv.app,
        &cookie,
        "dsgvo_export",
        serde_json::json!({ "contactId": contact_id }),
    )
    .await;
    let out = job_result(&r);
    let zip = download(&srv, &cookie, &out["download"]).await;
    for (name, bytes) in zip_entries(&zip) {
        assert!(!contains(&bytes, SMTP_SECRET), "Passwort in {name}");
    }

    // Löschen entfernt das Passwort
    let r = call(
        &srv.app,
        &cookie,
        "mail_account_delete",
        serde_json::json!({ "id": id }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(secret(&srv, &format!("smtp:{id}")).await, None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn credentials_are_bound_to_host_port_user() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let cookie = reauth_cookie(&srv, &cookie).await;
    let id = create_account(&srv, &cookie).await;
    let key = format!("smtp:{id}");
    let update = |input: serde_json::Value, password: Option<&str>| serde_json::json!({ "id": id, "input": input, "password": password });

    // Hostwechsel ohne neues Passwort → abgelehnt, nichts geändert
    let r = call(
        &srv.app,
        &cookie,
        "mail_account_update",
        update(
            account_input("smtp.example.net", Some("kb@example.org"), true),
            None,
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text());
    assert!(r.text().contains("Passwort neu"), "{}", r.text());
    assert_eq!(secret(&srv, &key).await.as_deref(), Some(SMTP_SECRET));
    let list = call(
        &srv.app,
        &cookie,
        "mail_accounts_list",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(list.json()[0]["smtpHost"], "smtp.example.org");

    // Benutzerwechsel ebenso
    let r = call(
        &srv.app,
        &cookie,
        "mail_account_update",
        update(
            account_input("smtp.example.org", Some("anders@example.org"), true),
            None,
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text());

    // Unveränderte Verbindung ohne Passwort → Passwort bleibt
    let mut renamed = account_input("smtp.example.org", Some("kb@example.org"), true);
    renamed["label"] = serde_json::json!("W8-Konto (neu)");
    let r = call(
        &srv.app,
        &cookie,
        "mail_account_update",
        update(renamed, None),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(secret(&srv, &key).await.as_deref(), Some(SMTP_SECRET));

    // Hostwechsel mit neuem Passwort → neues gespeichert
    let r = call(
        &srv.app,
        &cookie,
        "mail_account_update",
        update(
            account_input("smtp.example.net", Some("kb@example.org"), true),
            Some("neues-passwort"),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(secret(&srv, &key).await.as_deref(), Some("neues-passwort"));

    // Portwechsel ohne Benutzer (kein Login) → erlaubt, Passwort gelöscht
    let mut no_user = account_input("smtp.example.net", None, true);
    no_user["smtpPort"] = serde_json::json!(25);
    let r = call(
        &srv.app,
        &cookie,
        "mail_account_update",
        update(no_user, None),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(secret(&srv, &key).await, None);

    // Klartext-SMTP mit Zugangsdaten → abgelehnt (anlegen und ändern)
    let r = call(
        &srv.app,
        &cookie,
        "mail_account_create",
        serde_json::json!({
            "input": account_input("smtp.example.org", Some("kb@example.org"), false),
            "password": SMTP_SECRET
        }),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text());
    assert!(r.text().contains("Unverschlüsselt"), "{}", r.text());
    let r = call(
        &srv.app,
        &cookie,
        "mail_account_update",
        update(account_input("smtp.example.net", None, false), Some("x")),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text());
    // ohne Zugangsdaten unverschlüsselt erlaubt (z. B. Relay im LAN)
    let r = call(
        &srv.app,
        &cookie,
        "mail_account_update",
        update(account_input("smtp.example.net", None, false), None),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());

    // Microsoft-Konten (Graph) gibt es im Server nicht (E9)
    let mut oauth = account_input("smtp.example.org", None, true);
    oauth["authType"] = serde_json::json!("oauth_microsoft");
    let r = call(
        &srv.app,
        &cookie,
        "mail_account_create",
        serde_json::json!({ "input": oauth, "password": null }),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text());
    for name in [
        "mail_oauth_status",
        "mail_oauth_connect",
        "mail_oauth_disconnect",
    ] {
        let r = call(
            &srv.app,
            &cookie,
            name,
            serde_json::json!({ "accountId": id }),
        )
        .await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{name}: {}", r.text());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn connection_tests_are_guarded() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let cookie = reauth_cookie(&srv, &cookie).await;
    let (port, accepted) = counting_listener().await;

    // nicht freigegebener Port → abgelehnt ohne Verbindungsaufbau
    let r = test_connection(&srv, &cookie, "127.0.0.1", port, true, None, None).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text());
    assert!(r.text().contains("nicht freigegeben"), "{}", r.text());
    let r = call(
        &srv.app,
        &cookie,
        "backup_test_sftp",
        serde_json::json!({ "host": "127.0.0.1", "port": port, "user": "kb",
                            "remoteDir": "kb", "password": "x" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text());

    let dead = closed_port();
    srv.state
        .ctx
        .net_tests
        .set_allowed_ports(Some(vec![port, dead]));
    // Benutzer ohne Passwort (das gespeicherte geht nie an einen frei
    // gewählten Host) und Klartext mit Zugangsdaten → ohne Verbindung
    let r = test_connection(
        &srv,
        &cookie,
        "127.0.0.1",
        port,
        true,
        Some("kb@example.org"),
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text());
    let r = test_connection(
        &srv,
        &cookie,
        "127.0.0.1",
        port,
        false,
        Some("kb@example.org"),
        Some("x"),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text());
    // SFTP ohne Passwort ebenso
    let r = call(
        &srv.app,
        &cookie,
        "backup_test_sftp",
        serde_json::json!({ "host": "127.0.0.1", "port": port, "user": "kb",
                            "remoteDir": "kb", "password": "" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.text());
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert_eq!(accepted.load(Ordering::SeqCst), 0, "kein Verbindungsaufbau");

    // Einheitliche Meldung: geschlossener Port und unbekannter Host
    let r = test_connection(&srv, &cookie, "127.0.0.1", dead, true, None, None).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text());
    let closed_msg = r.json()["message"].as_str().unwrap().to_string();
    assert_eq!(
        closed_msg,
        format!(
            "Verbindung zu 127.0.0.1:{dead} fehlgeschlagen (Host, Port, Zugangsdaten und \
             Firewall prüfen)."
        )
    );
    let r = test_connection(&srv, &cookie, "unbekannt.invalid", dead, true, None, None).await;
    assert_eq!(
        r.json()["message"].as_str().unwrap(),
        closed_msg.replace("127.0.0.1", "unbekannt.invalid")
    );

    // Kontingent: 10 Tests je Minute (2 verbraucht)
    for _ in 0..8 {
        let r = test_connection(&srv, &cookie, "127.0.0.1", dead, true, None, None).await;
        assert!(r.text().contains("fehlgeschlagen"), "{}", r.text());
    }
    let r = test_connection(&srv, &cookie, "127.0.0.1", dead, true, None, None).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text());
    assert!(r.text().contains("Zu viele"), "{}", r.text());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sftp_target_with_probed_fingerprint() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let mock = sftp_mock::start(srv.dir.path().join("sftp-server"), "kb", SFTP_SECRET).await;
    srv.state
        .ctx
        .net_tests
        .set_allowed_ports(Some(vec![mock.port]));
    let cookie = reauth_cookie(&srv, &cookie).await;
    let probe_args = |password: &str| {
        serde_json::json!({ "host": "127.0.0.1", "port": mock.port, "user": "kb",
                            "remoteDir": "kb/backups", "password": password })
    };

    // ohne Passwort → ohne Verbindung abgelehnt
    let r = call(&srv.app, &cookie, "backup_test_sftp", probe_args("")).await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.text());
    assert_eq!(mock.connection_count(), 0);
    // falsches Passwort → einheitliche Meldung
    let r = call(&srv.app, &cookie, "backup_test_sftp", probe_args("falsch")).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text());
    assert!(r.text().contains("fehlgeschlagen"), "{}", r.text());
    // richtig → Fingerprint des Servers, Schreibtest ok
    let r = call(
        &srv.app,
        &cookie,
        "backup_test_sftp",
        probe_args(SFTP_SECRET),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let probe = r.json();
    assert_eq!(probe["fingerprint"], mock.fingerprint.as_str());
    assert_eq!(probe["writeOk"], true, "{probe}");

    let add = |host: &str, fingerprint: &str| {
        serde_json::json!({ "label": "Server", "host": host, "port": mock.port,
                            "user": "kb", "remoteDir": "kb/backups",
                            "hostFingerprint": fingerprint, "password": SFTP_SECRET })
    };
    // Fingerprint ≠ Probe → abgelehnt; Probe galt nur für 127.0.0.1
    for (host, fp) in [
        ("127.0.0.1", "SHA256:frei-erfunden"),
        ("localhost", mock.fingerprint.as_str()),
    ] {
        let r = call(&srv.app, &cookie, "backup_target_add_sftp", add(host, fp)).await;
        assert_eq!(r.status, StatusCode::CONFLICT, "{host}: {}", r.text());
        assert!(r.text().contains("Verbindungstest"), "{}", r.text());
    }
    let r = call(
        &srv.app,
        &cookie,
        "backup_target_add_sftp",
        add("127.0.0.1", &mock.fingerprint),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let id = r.json()["id"].as_str().unwrap().to_string();
    assert_eq!(
        secret(&srv, &format!("sftp:{id}")).await.as_deref(),
        Some(SFTP_SECRET)
    );
    {
        let pool = srv.state.ctx.pool().unwrap();
        let targets: String =
            sqlx::query_scalar("SELECT value FROM app_settings WHERE key = 'backup_targets'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(!targets.contains(SFTP_SECRET), "{targets}");
    }

    // gespeichertes Ziel per ID testen
    let r = call(
        &srv.app,
        &cookie,
        "backup_test_sftp_saved",
        serde_json::json!({ "id": id }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["writeOk"], true);

    // Backup wird auf den Server gespiegelt
    let r = call(
        &srv.app,
        &cookie,
        "backup_create_now",
        serde_json::json!({}),
    )
    .await;
    let out = job_result(&r);
    let sftp = out["offsite"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["targetId"] == id.as_str())
        .cloned()
        .unwrap();
    assert!(sftp["error"].is_null(), "{out}");
    let file = mock
        .root
        .join("kb/backups")
        .join(out["fileName"].as_str().unwrap());
    let local = srv
        .state
        .ctx
        .paths
        .backups_dir
        .join(out["fileName"].as_str().unwrap());
    assert_eq!(
        std::fs::read(&file).unwrap(),
        std::fs::read(&local).unwrap()
    );
    let leftovers: Vec<_> = std::fs::read_dir(mock.root.join("kb/backups"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with('.'))
        .collect();
    assert!(
        leftovers.is_empty(),
        "keine .part-/Probe-Reste: {leftovers:?}"
    );

    // Entfernen löscht das Passwort
    let r = call(
        &srv.app,
        &cookie,
        "backup_target_remove",
        serde_json::json!({ "id": id }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(secret(&srv, &format!("sftp:{id}")).await, None);
}

/// ASVS V5.2.6/V12.6.1 (Lauf F5c): Gespeicherte SMTP-Ports unterliegen derselben
/// Allowlist wie die Verbindungstests; private und Loopback-Adressen bleiben
/// erlaubt (Relay im LAN bzw. auf dem Docker-Host). Ein unveränderter
/// Bestandsport bleibt nutzbar, ein neuer muss freigegeben sein.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn saved_smtp_port_follows_allowlist() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let cookie = reauth_cookie(&srv, &cookie).await;
    let with_port = |host: &str, port: u16| {
        let mut input = account_input(host, None, true);
        input["smtpPort"] = serde_json::json!(port);
        input
    };

    // Nicht freigegebener Port (z. B. ein interner Dienst) → abgelehnt.
    let r = call(
        &srv.app,
        &cookie,
        "mail_account_create",
        serde_json::json!({ "input": with_port("smtp.example.org", 6379), "password": null }),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text());
    assert!(r.text().contains("nicht freigegeben"), "{}", r.text());

    // Loopback und Standardport → erlaubt (kein Verbindungsaufbau beim Speichern).
    let r = call(
        &srv.app,
        &cookie,
        "mail_account_create",
        serde_json::json!({ "input": with_port("127.0.0.1", 25), "password": null }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let id = r.json()["id"].as_str().unwrap().to_string();

    // Wechsel auf einen nicht freigegebenen Port → abgelehnt.
    let r = call(
        &srv.app,
        &cookie,
        "mail_account_update",
        serde_json::json!({ "id": id, "input": with_port("127.0.0.1", 6379), "password": null }),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text());

    // Betreiber gibt den Port frei (`KLEIN_BUCH_ALLOWED_TEST_PORTS`) → erlaubt.
    srv.state.ctx.net_tests.set_allowed_ports(Some(vec![1025]));
    let r = call(
        &srv.app,
        &cookie,
        "mail_account_update",
        serde_json::json!({ "id": id, "input": with_port("127.0.0.1", 1025), "password": null }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());

    // Bestandsport nach Änderung der Freigabe: nur Umbenennen bleibt möglich.
    srv.state.ctx.net_tests.set_allowed_ports(None);
    let mut renamed = with_port("127.0.0.1", 1025);
    renamed["label"] = serde_json::json!("Relay im LAN");
    let r = call(
        &srv.app,
        &cookie,
        "mail_account_update",
        serde_json::json!({ "id": id, "input": renamed, "password": null }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
}

/// Lauf F6 (ADR 0039 Nr. 138): Ein aus der Desktop-Fassung übernommenes
/// Microsoft-Konto (Graph, E9) versendet im Server nicht — ohne Verbindungsaufbau
/// und mit klarer Meldung — und verliert beim Umstellen auf SMTP den
/// gespeicherten Token samt OAuth-Metadaten.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_microsoft_account_is_refused_and_cleared_on_switch_to_smtp() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let cookie = reauth_cookie(&srv, &cookie).await;
    let pool = srv.state.ctx.pool().unwrap();
    let id = uuid::Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO mail_accounts (
            id, label, auth_type, from_email, from_name, is_default,
            oauth_tenant_id, oauth_client_id, oauth_account_email, oauth_scopes,
            oauth_token_expires_at
         ) VALUES (?, 'Desktop-Konto', 'oauth_microsoft', 'rechnung@example.org',
                   'Beispiel IT-Service', 1, 'common',
                   '11111111-2222-3333-4444-555555555555', 'rechnung@example.org',
                   'offline_access Mail.Send User.Read', '2026-06-01T00:00:00+00:00')",
    )
    .bind(&id)
    .execute(&pool)
    .await
    .unwrap();
    let token_key = format!("oauth:{id}");
    klein_buch_lib::secrets::set(&pool, &token_key, "F6-Test-Refresh-Token")
        .await
        .unwrap();

    // Testversand: abgelehnt, bevor ein Kanal gewählt wird; kein Protokolleintrag.
    let r = call(
        &srv.app,
        &cookie,
        "mail_send_test",
        serde_json::json!({ "accountId": id, "to": "kunde@example.org" }),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text());
    assert!(r.text().contains("Microsoft-Anmeldung"), "{}", r.text());
    let logged: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM email_log")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(logged, 0);

    // Umstellen auf SMTP: Token und alle OAuth-Spalten weg.
    let r = call(
        &srv.app,
        &cookie,
        "mail_account_update",
        serde_json::json!({
            "id": id,
            "input": account_input("smtp.example.org", None, true),
            "password": null
        }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let row = r.json();
    assert_eq!(row["authType"], "smtp_password");
    for field in [
        "oauthTenantId",
        "oauthClientId",
        "oauthAccountEmail",
        "oauthScopes",
        "oauthTokenExpiresAt",
    ] {
        assert!(row[field].is_null(), "{field}: {row}");
    }
    assert_eq!(secret(&srv, &token_key).await, None);
}
