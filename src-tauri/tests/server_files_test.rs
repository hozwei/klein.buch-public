//! Plan W7 — Downloads und Archiv-Auslieferung (W7a):
//! - Archiv: Hash-Prüfung (manipuliert → kein Byte), Audit `archive.read`,
//!   Pfad außerhalb `archive_dir` → Fehler, inline nur für eigene PDFs,
//!   Fremdes als attachment + CSP sandbox.
//! - Exporte nur als Download, einmalig, danach nichts in `<data>/tmp`.
//! - Vorschauen als Bytes (inline, mehrfach abrufbar).
//! - Kein Args-Struct eines freigegebenen Commands hat ein Pfadfeld.

mod server_support;

use std::path::PathBuf;

use axum::http::{header, StatusCode};
use klein_buch_lib::commands::registry::REGISTRY;
use server_support::*;

async fn audit_count(srv: &TestServer, action: &str) -> i64 {
    let pool = srv.state.ctx.pool().unwrap();
    sqlx::query_scalar("SELECT COUNT(*) FROM audit_log WHERE action = ?")
        .bind(action)
        .fetch_one(&pool)
        .await
        .unwrap()
}

fn header_str(reply: &Reply, name: header::HeaderName) -> String {
    reply
        .headers
        .get(name)
        .map(|v| v.to_str().unwrap().to_string())
        .unwrap_or_default()
}

/// Festgeschriebene Rechnung; liefert (PDF-Archiv-ID, XML-Archiv-ID).
async fn locked_invoice(srv: &TestServer, cookie: &str) -> (String, String) {
    let id = draft_invoice(srv).await;
    let r = call(
        &srv.app,
        cookie,
        "invoices_lock_and_issue",
        serde_json::json!({ "id": id }),
    )
    .await;
    assert_eq!(r.json()["state"], "done", "{}", r.text());
    let res = &r.json()["result"];
    (
        res["pdfArchiveId"].as_str().unwrap().to_string(),
        res["xmlArchiveId"].as_str().unwrap().to_string(),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn archive_download_verifies_and_sets_headers() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let (pdf_id, xml_id) = locked_invoice(&srv, &cookie).await;

    let before = audit_count(&srv, "archive.read").await;
    let r = send(
        &srv.app,
        get(&format!("/files/archive/{pdf_id}"), Some(&cookie)),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(r.body.starts_with(b"%PDF"));
    assert_eq!(header_str(&r, header::CONTENT_TYPE), "application/pdf");
    assert!(header_str(&r, header::CONTENT_DISPOSITION).starts_with("inline; filename="));
    assert_eq!(
        header_str(&r, header::CONTENT_SECURITY_POLICY),
        klein_buch_lib::server::headers::CSP_PDF_INLINE
    );
    assert_eq!(header_str(&r, header::X_CONTENT_TYPE_OPTIONS), "nosniff");
    assert_eq!(audit_count(&srv, "archive.read").await, before + 1, "Audit");

    // XML: Fremdformat → attachment + sandbox
    let r = send(
        &srv.app,
        get(&format!("/files/archive/{xml_id}"), Some(&cookie)),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(header_str(&r, header::CONTENT_DISPOSITION).starts_with("attachment;"));
    assert!(header_str(&r, header::CONTENT_SECURITY_POLICY).starts_with("sandbox"));

    // ohne Sitzung → 401, unbekannt → 404
    let r = send(&srv.app, get(&format!("/files/archive/{pdf_id}"), None)).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    let r = send(&srv.app, get("/files/archive/gibt-es-nicht", Some(&cookie))).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);

    // manipuliert → Fehler, kein Byte der Datei
    let pool = srv.state.ctx.pool().unwrap();
    let path: String = sqlx::query_scalar("SELECT file_path FROM archive_entries WHERE id = ?")
        .bind(&pdf_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let mut bytes = std::fs::read(&path).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xff;
    std::fs::write(&path, &bytes).unwrap();
    let r = send(
        &srv.app,
        get(&format!("/files/archive/{pdf_id}"), Some(&cookie)),
    )
    .await;
    assert_ne!(r.status, StatusCode::OK);
    assert!(
        !r.body.starts_with(b"%PDF"),
        "kein Byte der manipulierten Datei"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn foreign_attachment_and_outside_path() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let pool = srv.state.ctx.pool().unwrap();
    // HTML-Anlage (Fremdinhalt)
    let stored = klein_buch_lib::archive::store_bytes(
        &pool,
        &srv.state.ctx.paths.archive_dir,
        2026,
        klein_buch_lib::archive::ArchiveKind::Attachment,
        "boese.html",
        "text/html",
        b"<script>alert(1)</script>",
    )
    .await
    .unwrap();
    let r = send(
        &srv.app,
        get(
            &format!("/files/archive/{}", stored.archive_id),
            Some(&cookie),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(
        header_str(&r, header::CONTENT_TYPE),
        "application/octet-stream"
    );
    assert!(header_str(&r, header::CONTENT_DISPOSITION).starts_with("attachment;"));
    assert!(header_str(&r, header::CONTENT_SECURITY_POLICY).starts_with("sandbox"));

    // Eintrag mit Pfad außerhalb des Archivs → Fehler ohne Pfad in der Antwort
    let outside = srv.dir.path().join("draussen.pdf");
    std::fs::write(&outside, b"%PDF-1.7 fremd").unwrap();
    sqlx::query(
        "INSERT INTO archive_entries
         (id, file_path, file_name, file_hash_sha256, file_size_bytes, mime_type, source)
         VALUES ('outside1', ?, 'draussen.pdf', 'x', 14, 'application/pdf', 'issued_invoice_pdf')",
    )
    .bind(outside.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .unwrap();
    let r = send(&srv.app, get("/files/archive/outside1", Some(&cookie))).await;
    assert_eq!(r.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(!r.text().contains("draussen"), "{}", r.text());
}

fn downloads_dir(srv: &TestServer) -> PathBuf {
    srv.state.ctx.paths.data_dir.join("tmp").join("downloads")
}

fn remaining_downloads(srv: &TestServer) -> usize {
    std::fs::read_dir(downloads_dir(srv))
        .map(|it| it.count())
        .unwrap_or(0)
}

/// Holt einen Download; prüft Header und Einmaligkeit.
async fn fetch_download(srv: &TestServer, cookie: &str, ticket: &serde_json::Value) -> Reply {
    let id = ticket["downloadId"].as_str().expect("downloadId");
    let r = send(
        &srv.app,
        get(&format!("/files/download/{id}"), Some(cookie)),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(header_str(&r, header::CONTENT_DISPOSITION).starts_with("attachment;"));
    assert_eq!(r.body.len() as u64, ticket["sizeBytes"].as_u64().unwrap());
    let again = send(
        &srv.app,
        get(&format!("/files/download/{id}"), Some(cookie)),
    )
    .await;
    assert_eq!(again.status, StatusCode::NOT_FOUND, "einmalig");
    r
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn all_exports_are_downloads_and_leave_nothing_behind() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let (_pdf, _xml) = locked_invoice(&srv, &cookie).await;
    let year = klein_buch_lib::clock::today_berlin()
        .format("%Y")
        .to_string();
    let y: i64 = year.parse().unwrap();
    let cookie = reauth_cookie(&srv, &cookie).await;

    let contact_id: String = {
        let pool = srv.state.ctx.pool().unwrap();
        sqlx::query_scalar("SELECT id FROM contacts LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap()
    };
    let exports: Vec<(&str, serde_json::Value)> = vec![
        (
            "euer_export_detail_zip",
            serde_json::json!({ "fiscalYear": y }),
        ),
        ("euer_export_elster", serde_json::json!({ "fiscalYear": y })),
        ("euer_export_pdf", serde_json::json!({ "fiscalYear": y })),
        (
            "euer_export_datev",
            serde_json::json!({ "fiscalYear": y, "skr": "SKR03" }),
        ),
        (
            "euer_export_stb_zip",
            serde_json::json!({ "fiscalYear": y, "skr": "SKR03" }),
        ),
        ("migration_export_run", serde_json::json!({})),
        (
            "dsgvo_export",
            serde_json::json!({ "contactId": contact_id }),
        ),
    ];
    for (name, args) in exports {
        let r = call(&srv.app, &cookie, name, args).await;
        let body = r.json();
        // Jobs liefern {state, result}, direkte Commands das Ergebnis
        let result = if body.get("state").is_some() {
            assert_eq!(body["state"], "done", "{name}: {}", r.text());
            body["result"].clone()
        } else {
            assert_eq!(r.status, StatusCode::OK, "{name}: {}", r.text());
            body
        };
        let ticket = &result["download"];
        assert!(ticket.is_object(), "{name}: {result}");
        let text = result.to_string();
        assert!(
            !text.contains("/tmp/") && !text.contains("Path"),
            "{name}: kein Serverpfad: {text}"
        );
        let got = fetch_download(&srv, &cookie, ticket).await;
        assert!(!got.body.is_empty(), "{name}");
    }
    assert_eq!(remaining_downloads(&srv), 0, "nichts mehr in tmp/downloads");
    let dsgvo_dir = srv.state.ctx.paths.data_dir.join("tmp").join("dsgvo");
    assert_eq!(
        std::fs::read_dir(&dsgvo_dir)
            .map(|it| it.count())
            .unwrap_or(0),
        0,
        "DSGVO-ZIP nicht liegen geblieben"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn previews_are_inline_and_reusable() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let r = call(
        &srv.app,
        &cookie,
        "pdf_template_preview",
        serde_json::json!({"name": "default"}),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let id = r.json()["downloadId"].as_str().unwrap().to_string();
    for _ in 0..2 {
        let got = send(
            &srv.app,
            get(&format!("/files/download/{id}"), Some(&cookie)),
        )
        .await;
        assert_eq!(got.status, StatusCode::OK);
        assert!(got.body.starts_with(b"%PDF"));
        assert!(header_str(&got, header::CONTENT_DISPOSITION).starts_with("inline;"));
        assert_eq!(
            header_str(&got, header::CONTENT_SECURITY_POLICY),
            klein_buch_lib::server::headers::CSP_PDF_INLINE
        );
    }
    let r = send(&srv.app, get(&format!("/files/download/{id}"), None)).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
}

const PATH_ARGS_UNTIL_W7CD: &[&str] = &[];

/// Kein Args-Struct eines freigegebenen Commands hat ein Pfadfeld (Plan W7).
#[test]
fn no_exposed_command_takes_a_path() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/commands");
    let mut sources = String::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "rs") {
            sources.push_str(&std::fs::read_to_string(path).unwrap());
        }
    }
    let mut offenders = Vec::new();
    for spec in REGISTRY.iter().filter(|c| c.exposed()) {
        let struct_name: String = spec
            .name
            .split('_')
            .map(|p| {
                let mut c = p.chars();
                c.next()
                    .map(|f| f.to_ascii_uppercase().to_string() + c.as_str())
                    .unwrap_or_default()
            })
            .collect::<String>()
            + "Args";
        let marker = format!("pub struct {struct_name} {{");
        let start = sources
            .find(&marker)
            .unwrap_or_else(|| panic!("{struct_name} nicht gefunden"));
        let body = &sources[start..start + sources[start..].find('}').unwrap()];
        for line in body.lines().skip(1) {
            let field = line.trim().trim_start_matches("pub ");
            let name = field.split(':').next().unwrap_or("").trim();
            if name.contains("path") {
                offenders.push(format!("{}.{name}", spec.name));
            }
        }
    }
    offenders.sort();
    // Bis W7c/W7d (Restore aus Upload/Historie, Backup-Ziele relativ zur
    // Mount-Wurzel) noch vorhanden; die Liste muss danach leer sein.
    let mut pending: Vec<String> = PATH_ARGS_UNTIL_W7CD.iter().map(|s| s.to_string()).collect();
    pending.sort();
    assert_eq!(offenders, pending, "Pfadfelder freigegebener Commands");
}
