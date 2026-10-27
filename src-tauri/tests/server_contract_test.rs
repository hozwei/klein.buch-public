//! Plan W5 — Contract-Tests des HTTP-Servers per `tower::ServiceExt::oneshot`:
//! Allowlist (Punkt 2), 404/405/401, Inventar der öffentlichen Routen (§3),
//! camelCase-Argumente, 423/413/403, Idempotenz, Header je Routenklasse.

mod server_support;

use std::collections::BTreeSet;
use std::path::PathBuf;

use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get as get_route;
use axum::Router;
use klein_buch_lib::commands::registry::{classified_kind, find, Auth, Kind, REGISTRY};
use klein_buch_lib::server::headers::{self, ResponseClass};
use server_support::*;

/// Seit Lauf F6 samt Kern entfernt (ADR 0039 Nr. 138): Vorgänger der
/// Auth-Routen, Archiv-Öffnen/Explorer, altes Einzel-Backup-Ziel, Restore-
/// Vorschau per Pfad, Lizenzpfad, MS-Graph (E9) und zwei doppelte Leser. Unter `/api/cmd` → 404.
const REMOVED_IN_F6: &[&str] = &[
    "backup_needs_onboarding",
    "backup_is_unlocked",
    "backup_setup_passphrase",
    "backup_unlock",
    "backup_set_target",
    "backup_set_sftp_target",
    "invoices_open_pdf",
    "invoices_reveal_pdf",
    "attachments_open",
    "attachments_reveal",
    "euer_reveal_path",
    "backup_open_folder",
    "backup_reveal_path",
    "backup_restore_preview",
    "third_party_licenses_path",
    "mail_oauth_status",
    "mail_oauth_connect",
    "mail_oauth_disconnect",
    // doppelt zu quotes_get (Anhänge) bzw. fiscal_year_overview (autoYearClose)
    "quotes_attachments_list",
    "fiscal_year_auto_close_get",
];

/// Re-Auth-Pflicht laut Plan W6 (Prüfung der Frische folgt dort).
const REAUTH: &[&str] = &[
    "backup_target_add_directory",
    "backup_target_add_sftp",
    "backup_target_remove",
    "backup_test_sftp",
    "backup_test_sftp_saved",
    "backup_restore_apply",
    "mail_account_create",
    "mail_account_update",
    "mail_account_delete",
    "mail_account_test_connection",
    "mail_account_test_saved",
    "mail_send_test",
    "migration_export_run",
    "dsgvo_export",
    "euer_export_stb_zip",
    "euer_export_detail_zip",
    // W14: Zahlungsdaten, unumkehrbare Schritte
    "payment_accounts_create",
    "payment_accounts_update",
    // Lauf F4a (W14-028, ASVS V11.1.4): Konto aktivieren/deaktivieren, Exporte
    "payment_accounts_set_active",
    "euer_export_datev",
    "euer_export_elster",
    "euer_export_pdf",
    "contacts_anonymize",
    "fiscal_year_close",
    // Lauf F: Freigabe nach Pause der automatischen Belegerzeugung
    "scheduler_release_hold",
    // Lauf F (D-24): Nummernkreis anheben
    "numbering_raise",
    // Lauf F (F1d): wiederhergestellten Entwurf mit vergebener Nummer freigeben
    "numbering_restored_draft_confirm",
    // Lauf F (F2a, W14-032): nummerierten Rechnungs-Entwurf verwerfen
    "invoices_void_draft",
    // Lauf F (F3a, W13-BACKUP-DOWNLOAD): verschlüsselte .kbk herunterladen
    "backup_download",
    // ADR 0042 Nr. 1: Steuerstatus nach §19 eintragen bzw. widerrufen
    "tax_status_change_add",
    "tax_status_change_revoke",
];

/// Bedingte Re-Auth: Zahlungsdaten/§19 (W14-038), Versand an einen abweichenden
/// Empfänger (W14-009, Lauf F5c).
const CONDITIONAL: &[&str] = &[
    "seller_profile_upsert",
    "mail_send_invoice",
    "mail_send_quote",
];

/// Jobs laut Plan W5.6.
const JOBS: &[&str] = &[
    "invoices_lock_and_issue",
    "quotes_issue",
    "invoices_cancel",
    "recurring_invoices_run_now",
    "recurring_invoices_run_due_check",
    "mail_send_invoice",
    "mail_send_quote",
    "package_catalog_send",
    "backup_create_now",
    "backup_restore_apply",
    "euer_export_stb_zip",
    "dsgvo_export",
    "migration_export_run",
];

fn api_ts_commands() -> BTreeSet<String> {
    let api = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/lib/api.ts");
    let text = std::fs::read_to_string(api).unwrap();
    let mut out = BTreeSet::new();
    for (idx, _) in text.match_indices("invoke") {
        let rest = &text[idx + "invoke".len()..];
        let rest = match rest.strip_prefix('<') {
            Some(r) => match r.find(">(") {
                Some(end) => &r[end + 1..],
                None => continue,
            },
            None => rest,
        };
        let Some(rest) = rest.strip_prefix("(\"") else {
            continue;
        };
        let Some(end) = rest.find('"') else { continue };
        out.insert(rest[..end].to_string());
    }
    out
}

fn registry_names() -> BTreeSet<String> {
    REGISTRY.iter().map(|c| c.name.to_string()).collect()
}

/// Die Registry ist die Allowlist und deckt sich genau mit den Commands, die
/// `api.ts` per `invoke` aufruft.
#[test]
fn allowlist_is_exactly_plan_point_2() {
    let frontend = api_ts_commands();
    assert_eq!(registry_names(), frontend, "Registry = Frontend-Commands");
    assert_eq!(REGISTRY.len(), frontend.len(), "keine doppelten Namen");
    assert_eq!(
        REGISTRY.len(),
        183,
        "183 (W4) + 3 Backup-Ziel-Commands (W7) + 2 Tests gespeicherter Ziele (W8) \
         - 5 Reset-/Drop-Folder-Commands (W11) + 2 Versionsprüfung (W12) \
         + 2 Rückstand + 2 Pause - payment_accounts_ensure_defaults (Lauf F1a) \
         + Tages-Backup-Uhrzeit + 2 Nummernkreise (Lauf F1b) \
         + 2 vorgemerkte Entwürfe + Restore-Prüfung der Abos (Lauf F1d) \
         + Entwurf verwerfen (Lauf F2a) + Backup herunterladen (Lauf F3a) \
         + Entwurf bearbeiten (Lauf F5a) - 18 gesperrte Desktop-Commands - 2 doppelte Leser (Lauf F6) \
         + Abgangs-Vorschau der Anlagen (Lauf 2026.11 U4) \
         + 5 Steuerstatus-Commands (ADR 0042)"
    );
    for gone in REMOVED_IN_F6 {
        assert!(find(gone).is_none(), "{gone} entfällt seit Lauf F6");
    }

    for spec in REGISTRY.iter() {
        let expected = if REAUTH.contains(&spec.name) {
            Auth::Reauth
        } else if CONDITIONAL.contains(&spec.name) {
            Auth::Conditional
        } else {
            Auth::Session
        };
        assert_eq!(spec.auth, expected, "{}", spec.name);
        assert_ne!(
            spec.auth,
            Auth::Public,
            "{} darf nicht öffentlich sein",
            spec.name
        );
        assert_eq!(
            spec.kind == Kind::Job,
            JOBS.contains(&spec.name),
            "{}",
            spec.name
        );
    }
}

#[tokio::test]
async fn every_allowlisted_name_without_session_is_401() {
    let srv = test_server();
    for name in registry_names() {
        let r = send(
            &srv.app,
            post(&format!("/api/cmd/{name}"), "{}", None, None),
        )
        .await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{name}: {}", r.text());
        let r = send(
            &srv.app,
            post(&format!("/api/cmd/{name}"), "{}", Some("ausgedacht"), None),
        )
        .await;
        assert_eq!(
            r.status,
            StatusCode::UNAUTHORIZED,
            "{name} mit falschem Cookie"
        );
    }
}

#[tokio::test]
async fn other_names_are_404_and_get_is_405() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let mut unknown = vec!["gibt_es_nicht", "invoices_archive_paths", "CONTACTS_LIST"];
    unknown.extend_from_slice(REMOVED_IN_F6);
    for name in unknown {
        let r = send(
            &srv.app,
            post(&format!("/api/cmd/{name}"), "{}", Some(&cookie), None),
        )
        .await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{name}");
    }
    let r = send(&srv.app, get("/api/cmd/contacts_list", Some(&cookie))).await;
    assert_eq!(r.status, StatusCode::METHOD_NOT_ALLOWED);
}

/// Öffentlich sind genau die Routen aus Plan §3; alles andere verlangt eine
/// Sitzung oder existiert nicht.
#[tokio::test]
async fn public_route_inventory() {
    let srv = test_server();
    let app = &srv.app;

    let r = send(app, get("/api/auth/status", None)).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.json()["state"], "setup_required");

    let r = send(app, get("/api/public/info", None)).await;
    assert_eq!(r.status, StatusCode::OK);
    let info = r.json();
    for key in [
        "version",
        "commit",
        "tag",
        "sourceUrl",
        "sourceLink",
        "licenseSpdx",
    ] {
        assert!(info.get(key).is_some(), "{key} fehlt: {info}");
    }
    assert_eq!(info["licenseSpdx"], "AGPL-3.0-or-later");

    // Lizenz- und Quellenseiten (W12) ohne Sitzung; ohne Dateien 404 statt SPA
    let r = send(app, get("/legal/fehlt.txt", None)).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    assert_eq!(r.json()["code"], "not_found", "JSON statt SPA");
    let legal = srv.state.ctx.paths.resource_dir.join("legal");
    std::fs::create_dir_all(&legal).unwrap();
    std::fs::write(legal.join("third-party-licenses.html"), "<h1>Lizenzen</h1>").unwrap();
    std::fs::write(legal.join("LICENSE.txt"), "GNU AFFERO").unwrap();
    for path in ["/legal/", "/legal/third-party-licenses.html"] {
        let r = send(app, get(path, None)).await;
        assert_eq!(r.status, StatusCode::OK, "{path}");
        assert!(r.text().contains("Lizenzen"), "{path}");
    }
    let r = send(app, get("/legal/LICENSE.txt", None)).await;
    assert_eq!(r.status, StatusCode::OK);
    let r = send(app, get("/legal", None)).await;
    assert_eq!(r.status, StatusCode::PERMANENT_REDIRECT);

    // ohne Loopback-Peer: nur Status, kein Körper
    let r = send(app, get("/healthz", None)).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.body.is_empty(), "{}", r.text());

    // Auth-Routen erreichbar ohne Sitzung (hier: Zustand setup)
    let r = send(
        app,
        post(
            "/api/auth/login",
            &format!(r#"{{"passphrase":"{PASS}","totpCode":"123456"}}"#),
            None,
            None,
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.text());
    let r = send(
        app,
        post("/api/auth/setup", r#"{"passphrase":"zu-kurz"}"#, None, None),
    )
    .await;
    assert_eq!(
        r.status,
        StatusCode::FORBIDDEN,
        "ohne Setup-Token: {}",
        r.text()
    );

    // Lauf F4a: Abholung einer langen Anmeldung ohne Sitzung (nur mit
    // Ticket-Cookie, sonst 410); Sitzungsübersicht und „andere abmelden“ nur
    // mit Sitzung.
    let r = send(app, post("/api/auth/login/poll", "{}", None, None)).await;
    assert_eq!(r.status, StatusCode::GONE, "{}", r.text());
    assert_eq!(r.json()["code"], "login_expired");
    let r = send(app, get("/api/auth/sessions", None)).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    let r = send(
        app,
        post("/api/auth/sessions/others/revoke", "{}", None, None),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);

    // SPA-Assets und Client-Routing
    for path in ["/", "/invoices/abc", "/settings/backup"] {
        let r = send(app, get(path, None)).await;
        assert_eq!(r.status, StatusCode::OK, "{path}");
        assert!(r.text().contains("__kb_boot"), "{path} liefert index.html");
    }
    let r = send(app, get("/_app/immutable/app.js", None)).await;
    assert_eq!(r.status, StatusCode::OK);

    // alles andere: Sitzung nötig oder 404
    let r = send(
        app,
        get(&format!("/api/jobs/{}", uuid::Uuid::new_v4()), None),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    let r = send(app, post("/api/cmd/contacts_list", "{}", None, None)).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    for path in [
        "/api/unbekannt",
        "/api",
        "/api/upload/beleg",
        "/files/unbekannt",
    ] {
        let r = send(app, get(path, None)).await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(r.json()["code"], "not_found", "{path}: JSON statt SPA");
    }
    // Datei-Routen (W7) nur mit Sitzung
    for path in ["/files/archive/1", "/files/download/1"] {
        let r = send(app, get(path, None)).await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{path}");
    }
    let r = send(app, post("/api/upload/beleg", "", None, None)).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn camel_case_args_locked_423_and_body_limit_413() {
    let srv = test_server();
    let app = &srv.app;
    let cookie = setup(&srv).await;

    let r = send(app, get("/api/auth/status", Some(&cookie))).await;
    assert_eq!(r.json()["state"], "ok");

    let r = call(
        app,
        &cookie,
        "contacts_list",
        serde_json::json!({"includeArchived": true}),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(r.json().is_array());

    // Pflichtfeld fehlt → 400 statt 500
    let r = call(app, &cookie, "contacts_get", serde_json::json!({})).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(r.json()["code"], "bad_request");
    // kaputtes JSON → 400
    let r = send(
        app,
        post("/api/cmd/contacts_list", "{", Some(&cookie), None),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);

    // 300 KiB → 413
    let big = format!("{{\"query\":\"{}\"}}", "x".repeat(300 * 1024));
    let r = send(
        app,
        post("/api/cmd/contacts_search", &big, Some(&cookie), None),
    )
    .await;
    assert_eq!(r.status, StatusCode::PAYLOAD_TOO_LARGE);

    // gesperrt → DB-Command 423
    let pool = srv.state.ctx.take_pool().unwrap();
    pool.close().await;
    let r = call(app, &cookie, "contacts_list", serde_json::json!({})).await;
    assert_eq!(r.status, StatusCode::LOCKED, "{}", r.text());
    assert_eq!(r.json()["code"], "locked");
    let r = send(app, get("/api/auth/status", Some(&cookie))).await;
    assert_eq!(r.json()["state"], "login_required");

    // erneut anmelden entsperrt; falsche Passphrase → 401 mit generischer Meldung
    let r = login(&srv, "falsche-passphrase-1234567").await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    assert_eq!(r.json()["message"], "Anmeldung fehlgeschlagen.");
    let r = login(&srv, PASS).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let fresh = session_cookie(&r);
    assert_ne!(fresh, cookie, "neue Sitzungs-ID beim Login");
    let r = call(app, &fresh, "contacts_list", serde_json::json!({})).await;
    assert_eq!(r.status, StatusCode::OK);
}

#[tokio::test]
async fn request_header_is_mandatory() {
    let srv = test_server();
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/cmd/contacts_list")
        .header("host", HOST)
        .body(axum::body::Body::from("{}"))
        .unwrap();
    let r = send(&srv.app, req).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header("host", HOST)
        .body(axum::body::Body::from(format!(
            "{{\"passphrase\":\"{PASS}\"}}"
        )))
        .unwrap();
    let r = send(&srv.app, req).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn idempotency_key_replays_write() {
    let srv = test_server();
    let app = &srv.app;
    let cookie = setup(&srv).await;
    let input = serde_json::json!({"input": {
        "contactType": "customer", "name": "Kunde GmbH", "street": "Hauptstr. 7",
        "postalCode": "12345", "city": "Musterstadt", "countryCode": "DE",
        "acceptsEinvoice": true
    }})
    .to_string();
    let a = send(
        app,
        post(
            "/api/cmd/contacts_create",
            &input,
            Some(&cookie),
            Some("k-1"),
        ),
    )
    .await;
    assert_eq!(a.status, StatusCode::OK, "{}", a.text());
    let b = send(
        app,
        post(
            "/api/cmd/contacts_create",
            &input,
            Some(&cookie),
            Some("k-1"),
        ),
    )
    .await;
    assert_eq!(b.status, StatusCode::OK);
    assert_eq!(a.json()["id"], b.json()["id"], "gleiches Ergebnis");
    let list = call(app, &cookie, "contacts_list", serde_json::json!({})).await;
    assert_eq!(
        list.json().as_array().unwrap().len(),
        1,
        "nur einmal angelegt"
    );

    let other = input.replace("Kunde GmbH", "Andere AG");
    let c = send(
        app,
        post(
            "/api/cmd/contacts_create",
            &other,
            Some(&cookie),
            Some("k-1"),
        ),
    )
    .await;
    assert_eq!(c.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(c.json()["code"], "idempotency_mismatch");
    let d = send(
        app,
        post(
            "/api/cmd/contacts_create",
            &input,
            Some(&cookie),
            Some("k 1"),
        ),
    )
    .await;
    assert_eq!(d.status, StatusCode::BAD_REQUEST);
}

fn header_snapshot(headers: &axum::http::HeaderMap) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = headers
        .iter()
        .filter(|(k, _)| {
            ![
                header::DATE,
                header::CONTENT_LENGTH,
                header::LAST_MODIFIED,
                header::ACCEPT_RANGES,
                header::ETAG,
            ]
            .contains(k)
        })
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap().to_string()))
        .collect();
    out.sort();
    out
}

const COMMON: &[(&str, &str)] = &[
    ("cross-origin-opener-policy", "same-origin"),
    ("cross-origin-resource-policy", "same-origin"),
    (
        "permissions-policy",
        "camera=(), microphone=(), geolocation=(), payment=()",
    ),
    ("referrer-policy", "no-referrer"),
    ("x-content-type-options", "nosniff"),
];

fn expected(extra: &[(&str, &str)]) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = COMMON
        .iter()
        .chain(extra.iter())
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    v.sort();
    v
}

#[tokio::test]
async fn header_snapshot_per_route_class() {
    let srv = test_server();
    let app = &srv.app;
    let spa_csp = srv.state.csp.spa.clone();
    assert!(spa_csp.contains("script-src 'self' 'sha256-"), "{spa_csp}");
    assert!(spa_csp.contains("frame-ancestors 'none'"));

    // SPA
    let r = send(app, get("/", None)).await;
    assert_eq!(
        header_snapshot(&r.headers),
        expected(&[
            ("cache-control", "no-store"),
            ("content-security-policy", &spa_csp),
            ("content-type", "text/html; charset=utf-8"),
            // vorkomprimierte Varianten (ADR 0041 Nr. 212)
            ("vary", "accept-encoding"),
        ])
    );
    // API
    let r = send(app, get("/api/auth/status", None)).await;
    assert_eq!(
        header_snapshot(&r.headers),
        expected(&[
            ("cache-control", "no-store"),
            ("content-security-policy", headers::CSP_NONE),
            ("content-type", "application/json"),
        ])
    );
    // gehashtes Asset
    let r = send(app, get("/_app/immutable/app.js", None)).await;
    assert_eq!(
        header_snapshot(&r.headers),
        expected(&[
            ("cache-control", headers::CACHE_IMMUTABLE),
            ("content-security-policy", headers::CSP_NONE),
            ("content-type", "text/javascript; charset=utf-8"),
            ("vary", "accept-encoding"),
        ])
    );

    // PDF inline und Download: Klassen, die die Routen aus W7 setzen
    let probe = Router::new()
        .route(
            "/files/pdf",
            get_route(|| async {
                let mut r =
                    ([(header::CONTENT_TYPE, "application/pdf")], "%PDF-1.7").into_response();
                r.extensions_mut().insert(ResponseClass::PdfInline);
                r
            }),
        )
        .route(
            "/files/download",
            get_route(|| async { ([(header::CONTENT_TYPE, "application/octet-stream")], "x") }),
        )
        .layer(axum::middleware::from_fn_with_state(
            srv.state.clone(),
            headers::security_headers,
        ));
    let r = send(&probe, get("/files/pdf", None)).await;
    assert_eq!(
        header_snapshot(&r.headers),
        expected(&[
            ("cache-control", "no-store"),
            ("content-security-policy", headers::CSP_PDF_INLINE),
            ("content-type", "application/pdf"),
        ])
    );
    let r = send(&probe, get("/files/download", None)).await;
    assert_eq!(
        header_snapshot(&r.headers),
        expected(&[
            ("cache-control", "no-store"),
            ("content-security-policy", headers::CSP_DOWNLOAD),
            ("content-type", "application/octet-stream"),
        ])
    );
    // kein CORS
    let r = send(app, get("/api/auth/status", None)).await;
    assert!(r.headers.get("access-control-allow-origin").is_none());
    // die Registry-Funktion für den Dispatcher findet nur Freigegebenes
    assert!(
        find("mail_oauth_connect").is_none(),
        "seit Lauf F6 entfernt"
    );
    assert!(find("factory_reset").is_none(), "seit W11 entfernt");
}

/// W14-012 (Lauf F4b): Die Art (Read/Write/Job) steht für jeden Command
/// ausdrücklich in der Registry — keine Ableitung aus der Namensendung. Ein
/// neuer Command ohne Einstufung lässt diesen Test scheitern.
#[test]
fn every_command_has_an_explicit_kind() {
    let unclassified: Vec<&str> = REGISTRY
        .iter()
        .filter(|c| classified_kind(c.name).is_none())
        .map(|c| c.name)
        .collect();
    assert!(
        unclassified.is_empty(),
        "Commands ohne ausdrückliche Art (Read/Write/Job): {unclassified:?}"
    );
    for spec in REGISTRY.iter() {
        assert_eq!(classified_kind(spec.name), Some(spec.kind), "{}", spec.name);
    }
    // Keine Namens-Heuristik mehr: ein ausgedachter `*_list` ist nicht „Read“.
    for made_up in ["ausgedacht_list", "ausgedacht_get", "ausgedacht_check"] {
        assert_eq!(classified_kind(made_up), None, "{made_up}");
    }
    // Stichproben der Einstufung
    assert_eq!(classified_kind("contacts_list"), Some(Kind::Read));
    assert_eq!(classified_kind("travel_compute"), Some(Kind::Read));
    assert_eq!(
        classified_kind("recurring_run_due_check"),
        Some(Kind::Write)
    );
    assert_eq!(classified_kind("contacts_create"), Some(Kind::Write));
    assert_eq!(classified_kind("backup_create_now"), Some(Kind::Job));
    let reads = REGISTRY.iter().filter(|c| c.kind == Kind::Read).count();
    let writes = REGISTRY.iter().filter(|c| c.kind == Kind::Write).count();
    assert_eq!(
        (reads, writes, JOBS.len()),
        (75, 95, 13),
        "Einstufung wie vor W14-012 (Lauf F4b), + invoices_update_draft (Write, Lauf F5a), \
         Lauf F6: − 4 Leser (Restore-Vorschau, OAuth-Status, zwei doppelte), − 16 gesperrte \
         Schreiber; Lauf 2026.11 U4: + assets_disposal_preview (Read); \
         Lauf 2026.11.1 V1: + tax_status_overview/_at/_change_preview (Read), \
         + tax_status_change_add/_revoke (Write)"
    );
}
