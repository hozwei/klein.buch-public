//! Plan W6 — Done-Checks Auth: Setup-Token, TOTP vor der KDF, Replay,
//! Einmal-Codes, `totp-reset`, identische Antworten, Rate-Limit je IP bzw.
//! Gerät, vertraute Proxys, Host/Origin, unsicheres HTTP, Cookie-Attribute,
//! Re-Auth über die Registry, „Datenbank sperren“, Passphrase-Wechsel,
//! Klartext-DB.

mod server_support;

use axum::http::{header, StatusCode};
use klein_buch_lib::commands::registry::{Auth, Cost, REGISTRY};
use klein_buch_lib::server::policy::parse_trusted_proxies;
use server_support::*;

fn login_body(pass: &str, code: &str) -> String {
    serde_json::json!({ "passphrase": pass, "totpCode": code }).to_string()
}

#[tokio::test]
async fn setup_requires_valid_token_and_token_dies_after_success() {
    let srv = test_server();
    let token = srv
        .state
        .setup_token_for_tests()
        .expect("Token beim Serverstart");
    for body in [
        serde_json::json!({ "step": "totp" }),
        serde_json::json!({ "setupToken": "FALSCH", "step": "totp" }),
        serde_json::json!({ "setupToken": token.clone() + "X", "passphrase": PASS, "totpCode": "000000" }),
    ] {
        let r = send(
            &srv.app,
            post("/api/auth/setup", &body.to_string(), None, None),
        )
        .await;
        assert_eq!(r.status, StatusCode::FORBIDDEN, "{body}: {}", r.text());
        assert_eq!(r.json()["code"], "setup_token_invalid");
    }
    // ohne angefordertes Geheimnis → 409
    let r = send(
        &srv.app,
        post(
            "/api/auth/setup",
            &serde_json::json!({ "setupToken": token, "passphrase": PASS, "totpCode": "000000" })
                .to_string(),
            None,
            None,
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.text());
    assert!(
        !srv.state.ctx.paths.db_file.exists(),
        "keine DB vor TOTP-Bestätigung"
    );

    let reply = setup_reply(&srv).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text());
    let codes = reply.json()["recoveryCodes"].as_array().unwrap().len();
    assert_eq!(codes, 10);
    // Cookie-Attribute exakt; zusätzlich Gerätecookie
    let cookies: Vec<String> = reply
        .headers
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap().to_string())
        .collect();
    let session = cookies
        .iter()
        .find(|c| c.starts_with("__Host-kb_session="))
        .unwrap();
    assert!(
        session.ends_with("; Path=/; HttpOnly; Secure; SameSite=Strict"),
        "{session}"
    );
    let device = cookies
        .iter()
        .find(|c| c.starts_with("__Host-kb_device="))
        .unwrap();
    assert!(
        device.ends_with("; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=31536000"),
        "{device}"
    );

    assert!(
        srv.state.setup_token_for_tests().is_none(),
        "Token nach Erfolg ungültig"
    );
    let r = send(
        &srv.app,
        post(
            "/api/auth/setup",
            &serde_json::json!({ "setupToken": token, "step": "totp" }).to_string(),
            None,
            None,
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn wrong_totp_never_reaches_the_kdf_and_answers_match() {
    let srv = test_server();
    setup(&srv).await;
    let kdf_before = srv.state.admission.admitted(Cost::Kdf);
    let wrong_totp = send(
        &srv.app,
        post("/api/auth/login", &login_body(PASS, "000000"), None, None),
    )
    .await;
    assert_eq!(wrong_totp.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        srv.state.admission.admitted(Cost::Kdf),
        kdf_before,
        "keine KDF ohne gültiges TOTP"
    );
    let code = next_totp(&srv);
    let wrong_pass = send(
        &srv.app,
        post(
            "/api/auth/login",
            &login_body("falsche-passphrase-1234567", &code),
            None,
            None,
        ),
    )
    .await;
    assert_eq!(wrong_pass.status, StatusCode::UNAUTHORIZED);
    assert_eq!(srv.state.admission.admitted(Cost::Kdf), kdf_before + 1);
    assert_eq!(wrong_totp.body, wrong_pass.body, "identische Antwort");
    // der Code ist trotz falscher Passphrase verbraucht
    let again = send(
        &srv.app,
        post("/api/auth/login", &login_body(PASS, &code), None, None),
    )
    .await;
    assert_eq!(again.status, StatusCode::UNAUTHORIZED, "Replay");
}

#[tokio::test]
async fn replay_is_rejected_also_after_restart() {
    let srv = test_server();
    setup(&srv).await;
    let code = next_totp(&srv);
    let ok = send(
        &srv.app,
        post("/api/auth/login", &login_body(PASS, &code), None, None),
    )
    .await;
    assert_eq!(ok.status, StatusCode::OK, "{}", ok.text());
    let replay = send(
        &srv.app,
        post("/api/auth/login", &login_body(PASS, &code), None, None),
    )
    .await;
    assert_eq!(replay.status, StatusCode::UNAUTHORIZED);

    // „Neustart“: neuer Zustand auf demselben Datenverzeichnis, gleiche Uhr.
    let paths = srv.state.ctx.paths.clone();
    let offset = srv.state.totp_now() as i64 - klein_buch_lib::server::totp::unix_now() as i64;
    let dir = srv.dir;
    drop(srv.app);
    drop(srv.state);
    let state = klein_buch_lib::server::prepare(ctx_for(paths), &local_config()).unwrap();
    state.shift_totp_clock_for_tests(offset);
    let app = klein_buch_lib::server::router(state.clone());
    let replay = send(
        &app,
        post("/api/auth/login", &login_body(PASS, &code), None, None),
    )
    .await;
    assert_eq!(
        replay.status,
        StatusCode::UNAUTHORIZED,
        "Replay nach Neustart"
    );
    drop(dir);
}

#[tokio::test]
async fn missing_totp_file_blocks_login_and_reset_ends_sessions() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let r = call(&srv.app, &cookie, "contacts_list", serde_json::json!({})).await;
    assert_eq!(r.status, StatusCode::OK);

    // totp-reset (wie der Subcommand)
    assert!(srv.state.totp.reset().unwrap());
    let r = call(&srv.app, &cookie, "contacts_list", serde_json::json!({})).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED, "Sitzungen ungültig");
    let r = send(&srv.app, get("/api/auth/status", None)).await;
    assert_eq!(r.json()["state"], "totp_setup_required");
    let r = send(
        &srv.app,
        post("/api/auth/login", &login_body(PASS, "123456"), None, None),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert_eq!(r.json()["code"], "totp_setup_required");
    assert!(
        srv.state.setup_token_for_tests().is_some(),
        "neuer Setup-Token"
    );

    // Neueinrichtung nur mit Token + bestehender Passphrase
    let token = srv.state.setup_token_for_tests().unwrap();
    let offer = send(
        &srv.app,
        post(
            "/api/auth/setup",
            &serde_json::json!({ "setupToken": token, "step": "totp" }).to_string(),
            None,
            None,
        ),
    )
    .await;
    let secret =
        klein_buch_lib::server::totp::decode_secret(offer.json()["secret"].as_str().unwrap())
            .unwrap();
    let code = |srv: &TestServer| {
        klein_buch_lib::server::totp::code_for_step(
            &secret,
            klein_buch_lib::server::totp::step_at(srv.state.totp_now()),
        )
    };
    let wrong = send(
        &srv.app,
        post(
            "/api/auth/setup",
            &serde_json::json!({ "setupToken": token, "passphrase": "falsche-passphrase-1234567", "totpCode": code(&srv) })
                .to_string(),
            None,
            None,
        ),
    )
    .await;
    assert_eq!(wrong.status, StatusCode::UNAUTHORIZED, "{}", wrong.text());
    let ok = send(
        &srv.app,
        post(
            "/api/auth/setup",
            &serde_json::json!({ "setupToken": token, "passphrase": PASS, "totpCode": code(&srv) })
                .to_string(),
            None,
            None,
        ),
    )
    .await;
    assert_eq!(ok.status, StatusCode::OK, "{}", ok.text());
    assert!(srv.state.totp.exists());
}

#[tokio::test]
async fn recovery_code_works_once() {
    let srv = test_server();
    let reply = setup_reply(&srv).await;
    let code = reply.json()["recoveryCodes"][0]
        .as_str()
        .unwrap()
        .to_string();
    let body = serde_json::json!({ "passphrase": PASS, "recoveryCode": code }).to_string();
    let r = send(&srv.app, post("/api/auth/login", &body, None, None)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["usedRecoveryCode"], true);
    assert_eq!(r.json()["recoveryLeft"], 9);
    let r = send(&srv.app, post("/api/auth/login", &body, None, None)).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn backoff_per_ip_and_device_cookie_exemption() {
    let srv = test_server();
    let reply = setup_reply(&srv).await;
    let device = reply
        .headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|v| v.starts_with("__Host-kb_device="))
        .map(|v| v.split(';').next().unwrap().to_string())
        .unwrap();
    let peer = "198.51.100.20:4000";
    let wrong = || {
        from_peer(
            post("/api/auth/login", &login_body(PASS, "000000"), None, None),
            peer,
        )
    };
    for _ in 0..5 {
        let r = send(&srv.app, wrong()).await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    }
    let r = send(&srv.app, wrong()).await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(r.headers.get(header::RETRY_AFTER).is_some());
    assert_eq!(r.json()["code"], "rate_limited");
    // andere IP nicht betroffen
    let other = from_peer(
        post("/api/auth/login", &login_body(PASS, "000000"), None, None),
        "198.51.100.21:4000",
    );
    assert_eq!(send(&srv.app, other).await.status, StatusCode::UNAUTHORIZED);
    // gleiche IP mit gültigem Gerätecookie: nicht blockiert
    let code = next_totp(&srv);
    let mut req = from_peer(
        post("/api/auth/login", &login_body(PASS, &code), None, None),
        peer,
    );
    req.headers_mut()
        .insert(header::COOKIE, device.parse().unwrap());
    let r = send(&srv.app, req).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        r.json()["failedAttempts"],
        6,
        "Fehlversuche seit der letzten Anmeldung"
    );
}

#[tokio::test]
async fn global_budget_never_locks_out_recovery_codes_and_device_cookie_slides() {
    // W14: Ein Angreifer mit vielen Adressen schöpft das globale Budget aus.
    // Ein neues Gerät mit TOTP muss warten, ein Einmal-Code kommt trotzdem
    // hinein, und das Gerätecookie wird mit gleichem Wert erneuert.
    let srv = test_server();
    let reply = setup_reply(&srv).await;
    let codes: Vec<String> = serde_json::from_value(reply.json()["recoveryCodes"].clone()).unwrap();
    let device_of = |r: &Reply| {
        r.headers
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .find(|v| v.starts_with("__Host-kb_device="))
            .map(|v| v.split(';').next().unwrap().to_string())
    };
    let device = device_of(&reply).unwrap();
    // Alle folgenden Anmeldungen laufen durch die KDF; nicht parallel zu
    // anderen Tests (ein KDF-Platz, sonst 30-s-Zeitlimit dort).
    let _serial = kdf_serial().await;
    for i in 0..60 {
        let peer = format!("203.0.113.{}:4000", i + 1);
        let req = from_peer(
            post("/api/auth/login", &login_body(PASS, "000000"), None, None),
            &peer,
        );
        send(&srv.app, req).await;
    }
    let code = next_totp(&srv);
    let fresh_device = from_peer(
        post("/api/auth/login", &login_body(PASS, &code), None, None),
        "198.51.100.77:4000",
    );
    let r = send(&srv.app, fresh_device).await;
    assert_eq!(
        r.status,
        StatusCode::TOO_MANY_REQUESTS,
        "globales Budget greift"
    );
    let body = serde_json::json!({ "passphrase": PASS, "recoveryCode": codes[0] }).to_string();
    let r = send(
        &srv.app,
        from_peer(
            post("/api/auth/login", &body, None, None),
            "198.51.100.78:4000",
        ),
    )
    .await;
    assert_eq!(
        r.status,
        StatusCode::OK,
        "Einmal-Code trotz Budget: {}",
        r.text()
    );
    let code = next_totp(&srv);
    let mut req = from_peer(
        post("/api/auth/login", &login_body(PASS, &code), None, None),
        "198.51.100.79:4000",
    );
    req.headers_mut()
        .insert(header::COOKIE, device.parse().unwrap());
    let r = send(&srv.app, req).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        device_of(&r).as_deref(),
        Some(device.as_str()),
        "gleitend erneuert"
    );
}

#[tokio::test]
async fn reauth_rotates_session_and_old_cookie_loses_reauth() {
    let srv = test_server();
    let old = setup(&srv).await;
    let new = reauth_cookie(&srv, &old).await;
    assert_ne!(old, new);
    let probe = |cookie: &str| {
        post(
            "/api/cmd/backup_target_remove",
            r#"{"id":"gibt-es-nicht"}"#,
            Some(cookie),
            None,
        )
    };
    let r = send(&srv.app, probe(&new)).await;
    assert_ne!(
        r.status,
        StatusCode::FORBIDDEN,
        "neues Cookie trägt die Re-Auth"
    );
    let r = send(&srv.app, probe(&old)).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert_eq!(
        r.json()["code"],
        "reauth_required",
        "altes Cookie ohne Re-Auth"
    );
    let r = call(&srv.app, &old, "contacts_list", serde_json::json!({})).await;
    assert_eq!(r.status, StatusCode::OK, "altes Cookie gilt noch kurz");
    let r = reauth(&srv, &old).await;
    assert_eq!(
        r.status,
        StatusCode::UNAUTHORIZED,
        "ersetzte Sitzung nicht erneut"
    );
}

#[tokio::test]
async fn forwarded_for_only_from_trusted_proxies() {
    let srv = test_server_with(|cfg| {
        cfg.policy.trusted_proxies = parse_trusted_proxies("192.0.2.0/24").unwrap();
    });
    setup(&srv).await;
    // nicht vertraute Gegenstelle: XFF wechselt, gesperrt wird trotzdem die Gegenstelle
    for i in 0..6 {
        let mut req = from_peer(
            post("/api/auth/login", &login_body(PASS, "000000"), None, None),
            "198.51.100.30:4000",
        );
        req.headers_mut()
            .insert("x-forwarded-for", format!("203.0.113.{i}").parse().unwrap());
        let r = send(&srv.app, req).await;
        let expected = if i < 5 {
            StatusCode::UNAUTHORIZED
        } else {
            StatusCode::TOO_MANY_REQUESTS
        };
        assert_eq!(r.status, expected, "Versuch {i}");
    }
    // über den vertrauten Proxy zählt die Client-IP aus der Kette
    let via_proxy = |client: &str| {
        let mut req = from_peer(
            post("/api/auth/login", &login_body(PASS, "000000"), None, None),
            "192.0.2.10:4000",
        );
        req.headers_mut()
            .insert("x-forwarded-for", client.parse().unwrap());
        req.headers_mut()
            .insert("x-forwarded-proto", "https".parse().unwrap());
        req
    };
    for _ in 0..5 {
        send(&srv.app, via_proxy("203.0.113.50")).await;
    }
    assert_eq!(
        send(&srv.app, via_proxy("203.0.113.50")).await.status,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        send(&srv.app, via_proxy("198.51.100.200, 203.0.113.51"))
            .await
            .status,
        StatusCode::UNAUTHORIZED,
        "andere Client-IP hinter demselben Proxy nicht gesperrt"
    );
}

#[tokio::test]
async fn host_origin_and_insecure_http_rules() {
    let srv = test_server_with(|cfg| cfg.policy.insecure_http = false);
    // falscher Host → 421
    let mut req = get("/api/auth/status", None);
    req.headers_mut()
        .insert(header::HOST, "evil.example.org".parse().unwrap());
    assert_eq!(
        send(&srv.app, req).await.status,
        StatusCode::MISDIRECTED_REQUEST
    );
    // fremder Origin → 403, eigener Origin ok
    let mut req = post("/api/cmd/contacts_list", "{}", None, None);
    req.headers_mut()
        .insert(header::ORIGIN, "https://evil.example.org".parse().unwrap());
    let r = send(&srv.app, req).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert_eq!(r.json()["code"], "origin_mismatch");
    let mut req = post("/api/cmd/contacts_list", "{}", None, None);
    req.headers_mut()
        .insert(header::ORIGIN, format!("http://{HOST}").parse().unwrap());
    assert_eq!(send(&srv.app, req).await.status, StatusCode::UNAUTHORIZED);
    // ohne Pflicht-Header → 403
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/auth/logout")
        .header(header::HOST, HOST)
        .body(axum::body::Body::empty())
        .unwrap();
    assert_eq!(send(&srv.app, req).await.status, StatusCode::FORBIDDEN);

    // HTTP ohne Proxy und ohne INSECURE → Setup/Login 403; über Loopback erlaubt
    let token = srv.state.setup_token_for_tests().unwrap();
    let body = serde_json::json!({ "setupToken": token, "step": "totp" }).to_string();
    let r = send(
        &srv.app,
        from_peer(
            post("/api/auth/setup", &body, None, None),
            "203.0.113.9:5000",
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert_eq!(r.json()["code"], "insecure_transport");
    let r = send(
        &srv.app,
        from_peer(
            post("/api/auth/login", &login_body(PASS, "123456"), None, None),
            "203.0.113.9:5000",
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "Login ohne HTTPS");
    assert_eq!(r.json()["code"], "insecure_transport");
    let r = send(
        &srv.app,
        from_peer(post("/api/auth/setup", &body, None, None), "127.0.0.1:5000"),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    // /healthz über Loopback auch mit fremdem Host
    let mut req = from_peer(get("/healthz", None), "127.0.0.1:5000");
    req.headers_mut()
        .insert(header::HOST, "127.0.0.1:8080".parse().unwrap());
    assert_eq!(send(&srv.app, req).await.status, StatusCode::OK);
}

#[tokio::test]
async fn hsts_only_for_https_via_trusted_proxy() {
    let srv = test_server_with(|cfg| {
        cfg.policy.trusted_proxies = parse_trusted_proxies("192.0.2.10").unwrap();
    });
    let plain = send(&srv.app, get("/api/auth/status", None)).await;
    assert!(plain
        .headers
        .get(header::STRICT_TRANSPORT_SECURITY)
        .is_none());
    let mut req = from_peer(get("/api/auth/status", None), "192.0.2.10:4000");
    req.headers_mut()
        .insert("x-forwarded-proto", "https".parse().unwrap());
    let r = send(&srv.app, req).await;
    assert!(r.headers.get(header::STRICT_TRANSPORT_SECURITY).is_some());
}

/// Jeder Re-Auth-Command ohne Re-Auth → 403, mit → 2xx bzw. Fachfehler.
#[tokio::test]
async fn reauth_commands_require_fresh_reauth() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let reauth_cmds: Vec<&str> = REGISTRY
        .iter()
        .filter(|c| c.auth == Auth::Reauth)
        .map(|c| c.name)
        .collect();
    assert_eq!(
        reauth_cmds.len(),
        31,
        "22 + numbering_raise (Lauf F1b) + numbering_restored_draft_confirm (F1d) \
         + invoices_void_draft (F2a) + backup_download (F3a) + euer_export_datev/_elster/_pdf \
         + payment_accounts_set_active (F4a) - seller_profile_upsert (F4a: bedingt) \
         + tax_status_change_add/_revoke (ADR 0042)"
    );
    // W14-038 (Lauf F4a) und W14-009 (Lauf F5c): bedingte Re-Auth; die Matrizen
    // prüfen `seller_profile_reauth_matrix` und `mail_send_reauth_matrix`.
    let mut conditional: Vec<&str> = REGISTRY
        .iter()
        .filter(|c| c.auth == Auth::Conditional)
        .map(|c| c.name)
        .collect();
    conditional.sort_unstable();
    assert_eq!(
        conditional,
        vec![
            "mail_send_invoice",
            "mail_send_quote",
            "seller_profile_upsert"
        ]
    );
    for name in [
        "euer_export_datev",
        "euer_export_elster",
        "euer_export_pdf",
        "payment_accounts_set_active",
    ] {
        assert!(reauth_cmds.contains(&name), "{name} (W14-028)");
    }
    for name in &reauth_cmds {
        let r = send(
            &srv.app,
            post(&format!("/api/cmd/{name}"), "{}", Some(&cookie), None),
        )
        .await;
        assert_eq!(r.status, StatusCode::FORBIDDEN, "{name}");
        assert_eq!(r.json()["code"], "reauth_required", "{name}");
    }
    let cookie = reauth_cookie(&srv, &cookie).await;
    for name in &reauth_cmds {
        let r = send(
            &srv.app,
            post(&format!("/api/cmd/{name}"), "{}", Some(&cookie), None),
        )
        .await;
        assert_ne!(r.status, StatusCode::FORBIDDEN, "{name}: {}", r.text());
        assert_ne!(r.status, StatusCode::UNAUTHORIZED, "{name}");
    }
}

/// R1-012 (Lauf F2a): Ein verletzter Fremdschlüssel blockiert das Entsperren
/// mit 409 `db_integrity_failed`; die Instanz bleibt gesperrt, nichts wird
/// angefasst.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn foreign_key_violation_refuses_unlock() {
    let srv = test_server();
    let _cookie = setup(&srv).await;
    srv.state.ctx.take_pool().unwrap().close().await;
    srv.state.ctx.session.clear();
    {
        // SQLCipher-KDF beim Öffnen: nicht parallel zu anderen Anmeldungen
        let _serial = kdf_serial().await;
        let pool = klein_buch_lib::db::open_pool(&srv.state.ctx.paths.db_file, Some(PASS))
            .await
            .unwrap();
        let mut conn = pool.acquire().await.unwrap();
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *conn)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO notifications (id, rule_id, title, body, severity)
             VALUES ('n-fk', 'rule_gibt_es_nicht', 'T', 'B', 'info')",
        )
        .execute(&mut *conn)
        .await
        .unwrap();
        drop(conn);
        pool.close().await;
    }
    let r = login(&srv, PASS).await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.text());
    assert_eq!(r.json()["code"], "db_integrity_failed", "{}", r.text());
    assert!(!srv.state.ctx.is_unlocked(), "bleibt gesperrt");
}

#[tokio::test]
async fn lock_closes_pool_and_ends_sessions() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let r = send(&srv.app, post("/api/auth/lock", "{}", Some(&cookie), None)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(!srv.state.ctx.is_unlocked(), "Pool zu");
    assert!(
        srv.state.ctx.session.get().is_none(),
        "Passphrase aus dem RAM"
    );
    assert!(
        srv.state
            .ctx
            .scheduler_paused
            .load(std::sync::atomic::Ordering::SeqCst),
        "Scheduler angehalten"
    );
    let r = call(&srv.app, &cookie, "contacts_list", serde_json::json!({})).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED, "Sitzungen ungültig");
    // eine (theoretisch) gültige Sitzung bei gesperrter DB → 423
    let fresh = srv.state.sessions.create();
    let r = call(&srv.app, &fresh, "contacts_list", serde_json::json!({})).await;
    assert_eq!(r.status, StatusCode::LOCKED);
    // Anmelden entsperrt wieder, Scheduler läuft weiter
    let r = login(&srv, PASS).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(srv.state.ctx.is_unlocked());
    assert!(!srv
        .state
        .ctx
        .scheduler_paused
        .load(std::sync::atomic::Ordering::SeqCst));
}

#[tokio::test]
async fn logout_ends_only_this_session() {
    let srv = test_server();
    let first = setup(&srv).await;
    let second = session_cookie(&login(&srv, PASS).await);
    let r = send(&srv.app, post("/api/auth/logout", "{}", Some(&first), None)).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r
        .headers
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .contains("Max-Age=0"));
    assert_eq!(
        call(&srv.app, &first, "contacts_list", serde_json::json!({}))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&srv.app, &second, "contacts_list", serde_json::json!({}))
            .await
            .status,
        StatusCode::OK
    );
}

#[tokio::test]
async fn passphrase_change_rekeys_db_and_keeps_old_backups_readable() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let other = session_cookie(&login(&srv, PASS).await);
    let new_pass = "neue-lange-passphrase-zum-testen";
    // Daten vor dem Wechsel
    let created = call(
        &srv.app,
        &cookie,
        "contacts_create",
        serde_json::json!({"input": {
            "contactType": "customer", "name": "Bleibt GmbH", "street": "Weg 1",
            "postalCode": "12345", "city": "Musterstadt", "countryCode": "DE",
            "acceptsEinvoice": true
        }}),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.text());
    let body = serde_json::json!({ "newPassphrase": new_pass }).to_string();
    // ohne Re-Auth → 403
    let r = send(
        &srv.app,
        post("/api/auth/passphrase", &body, Some(&cookie), None),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    let cookie = reauth_cookie(&srv, &cookie).await;
    let short = serde_json::json!({ "newPassphrase": "zu-kurz" }).to_string();
    let r = send(
        &srv.app,
        post("/api/auth/passphrase", &short, Some(&cookie), None),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    let r = {
        let _serial = kdf_serial().await;
        send(
            &srv.app,
            post("/api/auth/passphrase", &body, Some(&cookie), None),
        )
        .await
    };
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["backupStarted"], true, "Backup nach dem Wechsel");
    // Verifier: neu gilt, alt nicht
    let pool = srv.state.ctx.pool().unwrap();
    assert!(klein_buch_lib::backup::verify_passphrase(&pool, new_pass)
        .await
        .unwrap());
    assert!(!klein_buch_lib::backup::verify_passphrase(&pool, PASS)
        .await
        .unwrap());
    // Daten erhalten
    let list = call(&srv.app, &cookie, "contacts_list", serde_json::json!({})).await;
    assert!(list.text().contains("Bleibt GmbH"), "{}", list.text());

    // aktuelle Sitzung bleibt, andere enden
    assert_eq!(
        call(&srv.app, &cookie, "contacts_list", serde_json::json!({}))
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(
        call(&srv.app, &other, "contacts_list", serde_json::json!({}))
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );

    // nur die neue Passphrase öffnet DB und Login
    let db = srv.state.ctx.paths.db_file.clone();
    assert!(
        klein_buch_lib::commands::backup::open_encrypted(&srv.state.ctx.paths, PASS)
            .await
            .unwrap()
            .is_none()
    );
    send(&srv.app, post("/api/auth/lock", "{}", Some(&cookie), None)).await;
    assert_eq!(login(&srv, PASS).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(login(&srv, new_pass).await.status, StatusCode::OK);
    assert!(db.exists());

    // das Backup der Einrichtung bleibt mit der alten Passphrase lesbar; das
    // neue entsteht im Hintergrund
    let list_backups = || -> Vec<std::path::PathBuf> {
        std::fs::read_dir(&srv.state.ctx.paths.backups_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "kbk"))
            .collect()
    };
    let mut backups = list_backups();
    for _ in 0..600 {
        let readable_new = backups.iter().any(|p| {
            klein_buch_lib::backup::restore::decrypt_backup(&std::fs::read(p).unwrap(), new_pass)
                .is_ok()
        });
        if readable_new {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        backups = list_backups();
    }
    assert!(backups.len() >= 2);
    let readable_old = backups.iter().any(|p| {
        klein_buch_lib::backup::restore::decrypt_backup(&std::fs::read(p).unwrap(), PASS).is_ok()
    });
    let readable_new = backups.iter().any(|p| {
        klein_buch_lib::backup::restore::decrypt_backup(&std::fs::read(p).unwrap(), new_pass)
            .is_ok()
    });
    assert!(readable_old, "altes Backup mit alter Passphrase");
    assert!(readable_new, "neues Backup mit neuer Passphrase");
}

#[tokio::test]
async fn totp_and_recovery_renewal_need_reauth() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let r = send(
        &srv.app,
        post("/api/auth/recovery/renew", "{}", Some(&cookie), None),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    // zweite Sitzung (anderes Gerät), die die Erneuerung beenden muss (W14)
    let other = session_cookie(&login(&srv, PASS).await);
    let cookie = reauth_cookie(&srv, &cookie).await;
    let r = send(
        &srv.app,
        post("/api/auth/recovery/renew", "{}", Some(&cookie), None),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.json()["recoveryCodes"].as_array().unwrap().len(), 10);
    let r = call(&srv.app, &other, "contacts_list", serde_json::json!({})).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED, "andere Sitzung beendet");

    let offer = send(
        &srv.app,
        post(
            "/api/auth/totp/renew",
            r#"{"step":"totp"}"#,
            Some(&cookie),
            None,
        ),
    )
    .await;
    assert_eq!(offer.status, StatusCode::OK);
    let secret =
        klein_buch_lib::server::totp::decode_secret(offer.json()["secret"].as_str().unwrap())
            .unwrap();
    srv.state.shift_totp_clock_for_tests(300);
    let code = klein_buch_lib::server::totp::code_for_step(
        &secret,
        klein_buch_lib::server::totp::step_at(srv.state.totp_now()),
    );
    let r = send(
        &srv.app,
        post(
            "/api/auth/totp/renew",
            &serde_json::json!({ "totpCode": code }).to_string(),
            Some(&cookie),
            None,
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(totp_secret(&srv), secret, "neues Geheimnis gespeichert");
}

#[tokio::test]
async fn control_characters_in_secrets_are_rejected() {
    let srv = test_server();
    setup(&srv).await;
    let body = serde_json::json!({ "passphrase": "mit\u{0}nul-zeichen-xxxxxxxx", "totpCode": "1" })
        .to_string();
    let r = send(&srv.app, post("/api/auth/login", &body, None, None)).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
}

/// W14-027: „Datenbank sperren“ wartet nicht unbegrenzt auf einen laufenden
/// Scheduler-Tick (hier vom Test gehalten): Sitzungen sind sofort ungültig,
/// gesperrt wird nach höchstens `SCHEDULER_STOP_WAIT`, der Tick bekommt die
/// Abbruch-Anforderung.
#[tokio::test]
async fn lock_does_not_wait_forever_for_running_tick() {
    use klein_buch_lib::server::state::SCHEDULER_STOP_WAIT;
    use std::sync::atomic::Ordering;
    let srv = test_server();
    let cookie = setup(&srv).await;
    let tick = srv.state.ctx.scheduler_gate.lock().await; // simulierter langer Tick
    let started = std::time::Instant::now();
    let lock = {
        let state = srv.state.clone();
        tokio::spawn(async move { klein_buch_lib::server::auth::lock_database(&state).await })
    };
    for _ in 0..200 {
        if srv.state.sessions.is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let r = call(&srv.app, &cookie, "contacts_list", serde_json::json!({})).await;
    assert_eq!(
        r.status,
        StatusCode::UNAUTHORIZED,
        "Sitzungen sofort ungültig"
    );
    assert!(lock.await.unwrap().is_ok(), "gesperrt trotz laufendem Tick");
    // Obergrenze: Frist für den Tick + Schließen des Pools (unter Volllast mit
    // fsync auf ZFS mehrere Sekunden) — der Tick selbst hängt unbegrenzt.
    assert!(
        started.elapsed() < SCHEDULER_STOP_WAIT + std::time::Duration::from_secs(20),
        "nach {:?}",
        started.elapsed()
    );
    assert!(!srv.state.ctx.is_unlocked(), "Pool zu");
    assert!(
        srv.state.ctx.session.get().is_none(),
        "Passphrase aus dem RAM"
    );
    assert!(
        srv.state.ctx.scheduler_stop.load(Ordering::SeqCst),
        "Tick hört nach dem laufenden Termin auf"
    );
    drop(tick);
    // Anmelden setzt den Scheduler fort
    let r = login(&srv, PASS).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(!srv.state.ctx.scheduler_stop.load(Ordering::SeqCst));
    assert!(!srv
        .state
        .ctx
        .scheduler_paused
        .load(std::sync::atomic::Ordering::SeqCst));
}

/// F1d: Läuft der beim Sperren noch aktive Tick weiter (hängender Sidecar,
/// SMTP-Timeout), hängt das Entsperren nicht an ihm — `tick::resume` wartet
/// nicht auf die Tick-Sperre.
#[tokio::test]
async fn unlock_does_not_wait_for_tick_still_running_from_lock() {
    use std::sync::atomic::Ordering;
    let srv = test_server();
    let cookie = setup(&srv).await;
    use klein_buch_lib::server::state::{DB_GATE_WAIT, SCHEDULER_STOP_WAIT};
    // Lauf F8b: Ohne ausstehendes Lock-Backup wartet das Nachholen
    // (`flush_pending_backup`) nicht auf die vom Test gehaltene Tick-Sperre —
    // die Dauer des Sperrens ist dann SCHEDULER_STOP_WAIT (fest, der Tick
    // „hängt“) plus Audit und Schließen des Pools (WAL-Checkpoint, fsync).
    let pool = srv.state.ctx.pool().unwrap();
    assert!(
        !klein_buch_lib::backup::is_backup_pending(&pool)
            .await
            .unwrap(),
        "kein Lock-Backup ausstehend"
    );
    drop(pool);
    let tick = srv.state.ctx.scheduler_gate.lock().await; // alter Tick läuft weiter
    let lock_started = std::time::Instant::now();
    let r = send(&srv.app, post("/api/auth/lock", "{}", Some(&cookie), None)).await;
    // Die Verlängerung ist echt, kein Fehler: 10 s der 30-s-Antwortfrist
    // (REQUEST_TIMEOUT) verbraucht das Abwarten des hängenden Ticks; unter
    // Volllast des Gates (parallele Testprogramme, synchrone Schreibzugriffe)
    // können Audit und Schließen des Pools den Rest überschreiten. Dann endet
    // die Antwort mit 503 `timeout`, und das Sperren läuft im Hintergrund zu
    // Ende (Lauf F8, ADR 0040 Nr. 63/64). Das prüft der Test ausdrücklich:
    // `lock_database` hält `transition` bis zum Schluss.
    let timed_out = r.status == StatusCode::SERVICE_UNAVAILABLE && r.json()["code"] == "timeout";
    if !timed_out {
        assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    }
    let bound = SCHEDULER_STOP_WAIT + DB_GATE_WAIT + std::time::Duration::from_secs(10);
    let done = tokio::time::timeout(
        bound.saturating_sub(lock_started.elapsed()),
        srv.state.transition.lock(),
    )
    .await
    .expect("Sperren im Hintergrund nicht abgeschlossen");
    drop(done);
    let took = lock_started.elapsed();
    if timed_out {
        eprintln!("Datenbank sperren endete nach {took:?} (Antwort: 503 timeout)");
    }
    assert!(
        took >= SCHEDULER_STOP_WAIT,
        "hat auf den Tick gewartet: {took:?}"
    );
    assert!(!srv.state.ctx.is_unlocked(), "Pool zu");
    assert!(
        srv.state.ctx.session.get().is_none(),
        "Passphrase aus dem RAM"
    );
    let started = std::time::Instant::now();
    let r = tokio::time::timeout(std::time::Duration::from_secs(60), login(&srv, PASS))
        .await
        .expect("Entsperren hängt am laufenden Tick");
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(
        started.elapsed() < std::time::Duration::from_secs(60),
        "nach {:?}",
        started.elapsed()
    );
    assert!(srv.state.ctx.is_unlocked());
    assert!(!srv.state.ctx.scheduler_stop.load(Ordering::SeqCst));
    assert!(!srv.state.ctx.scheduler_paused.load(Ordering::SeqCst));
    drop(tick);
}

/// W14-027: „Passphrase ändern“ braucht die Datei exklusiv — läuft der Tick nach
/// `SCHEDULER_STOP_WAIT` noch, antwortet der Server mit 503 statt unbegrenzt zu
/// warten; nichts ist geändert, der Scheduler läuft weiter.
#[tokio::test]
async fn passphrase_change_does_not_wait_forever_for_running_tick() {
    use klein_buch_lib::server::state::SCHEDULER_STOP_WAIT;
    use std::sync::atomic::Ordering;
    let srv = test_server();
    let cookie = setup(&srv).await;
    let cookie = reauth_cookie(&srv, &cookie).await;
    let tick = srv.state.ctx.scheduler_gate.lock().await;
    let started = std::time::Instant::now();
    let body =
        serde_json::json!({ "newPassphrase": "ganz neue lange Passphrase 2026" }).to_string();
    let r = send(
        &srv.app,
        post("/api/auth/passphrase", &body, Some(&cookie), None),
    )
    .await;
    assert_eq!(r.status, StatusCode::SERVICE_UNAVAILABLE, "{}", r.text());
    assert_eq!(r.json()["code"], "busy");
    assert!(started.elapsed() < SCHEDULER_STOP_WAIT + std::time::Duration::from_secs(5));
    drop(tick);
    assert!(srv.state.ctx.is_unlocked());
    assert!(
        !srv.state.ctx.scheduler_stop.load(Ordering::SeqCst),
        "Anforderung zurückgenommen"
    );
    let pool = srv.state.ctx.pool().unwrap();
    assert!(klein_buch_lib::backup::verify_passphrase(&pool, PASS)
        .await
        .unwrap());
}

/// W5-STOP-GRACE / W14-025b: Das Herunterfahren fordert den Abbruch des
/// laufenden Ticks an, wartet höchstens `SCHEDULER_SHUTDOWN_WAIT` (hier hält
/// der Test den Tick fest) und schließt danach Pool und Sitzung — innerhalb
/// der `stop_grace_period` des Compose-Beispiels.
#[tokio::test]
async fn shutdown_does_not_wait_forever_for_running_tick() {
    use klein_buch_lib::server::state::{
        JOB_DRAIN_TIMEOUT, POOL_CLOSE_WAIT, SCHEDULER_SHUTDOWN_WAIT,
    };
    use std::sync::atomic::Ordering;
    let srv = test_server();
    let _cookie = setup(&srv).await;
    let tick = srv.state.ctx.scheduler_gate.lock().await; // simulierter langer Tick
    let started = std::time::Instant::now();
    srv.state.finish_shutdown().await;
    let took = started.elapsed();
    // Vertrag: höchstens max(Tick, Tasks) + Pool schließen (+ Messreserve).
    let bound = JOB_DRAIN_TIMEOUT.max(SCHEDULER_SHUTDOWN_WAIT)
        + POOL_CLOSE_WAIT
        + std::time::Duration::from_secs(3);
    assert!(took < bound, "nach {took:?}");
    assert!(
        took >= SCHEDULER_SHUTDOWN_WAIT - std::time::Duration::from_secs(1),
        "hat auf den Tick gewartet: {took:?}"
    );
    assert!(
        srv.state.ctx.scheduler_stop.load(Ordering::SeqCst),
        "Abbruch angefordert"
    );
    assert!(!srv.state.ctx.is_unlocked(), "Pool geschlossen");
    assert!(
        srv.state.ctx.session.get().is_none(),
        "Passphrase aus dem RAM"
    );
    drop(tick);
}

// ---- Lauf F4a: Anmeldung, Sitzung, Geheimnisse im Speicher ---------------------

/// Gerätecookie (`name=wert`) aus einer Antwort.
fn device_cookie_of(r: &Reply) -> Option<String> {
    r.headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|v| v.starts_with("__Host-kb_device="))
        .map(|v| v.split(';').next().unwrap().to_string())
}

fn profile_args(
    iban: Option<&str>,
    bic: Option<&str>,
    klein: bool,
    street: &str,
) -> serde_json::Value {
    serde_json::json!({ "input": {
        "name": "Beispiel IT-Service", "legalForm": null, "street": street,
        "postalCode": "12345", "city": "Musterstadt", "countryCode": "DE",
        "taxNumber": "123/456/78901", "vatId": null, "email": "kontakt@example.org",
        "phone": null, "iban": iban, "bic": bic, "logoFilename": null,
        "isKleinunternehmer": klein, "defaultPdfTemplate": "default",
        "defaultCurrency": "EUR",
        "confirmWaiveParagraph19": if klein { None } else { Some(true) }
    }})
}

async fn stored_iban(srv: &TestServer, cookie: &str) -> Option<String> {
    let r = call(
        &srv.app,
        cookie,
        "seller_profile_get",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    r.json()["iban"].as_str().map(str::to_string)
}

/// W14-038 (Lauf F4a): Re-Auth-Matrix des Verkäuferprofils. Re-Auth nur bei
/// IBAN oder BIC (der §19-Status hat seit ADR 0042 eigene Commands) — und nicht in den ersten 5 min nach Einrichtung
/// bzw. Anmeldung. Eine 403 wird nicht als Idempotenz-Ergebnis gespeichert.
#[tokio::test]
async fn seller_profile_reauth_matrix() {
    use klein_buch_lib::server::session::REAUTH_WINDOW;
    const IBAN_A: &str = "DE02 1203 0000 0000 2020 51";
    const IBAN_B: &str = "DE89370400440532013000";
    const BIC: &str = "BYLADEM1001";
    let later = REAUTH_WINDOW + std::time::Duration::from_secs(1);
    let srv = test_server();
    let cookie = setup(&srv).await;
    let upsert = "seller_profile_upsert";

    // 1. Direkt nach der Einrichtung: Zahlungsdaten ohne Re-Auth.
    let r = call(
        &srv.app,
        &cookie,
        upsert,
        profile_args(Some(IBAN_A), Some(BIC), true, "Weg 1"),
    )
    .await;
    assert_eq!(
        r.status,
        StatusCode::OK,
        "frisch eingerichtet: {}",
        r.text()
    );

    // 2. Fünf Minuten später: Adresse und Schreibweise frei, Zahlungsdaten
    // und §19-Status nicht.
    srv.state.sessions.age_for_tests(later);
    let r = call(
        &srv.app,
        &cookie,
        upsert,
        profile_args(Some(IBAN_A), Some(BIC), true, "Weg 2"),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "Adresse: {}", r.text());
    let r = call(
        &srv.app,
        &cookie,
        upsert,
        profile_args(
            Some("de02120300000000202051"),
            Some(" BYLADEM1001 "),
            true,
            "Weg 2",
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "nur Schreibweise: {}", r.text());
    for (what, args) in [
        ("IBAN", profile_args(Some(IBAN_B), Some(BIC), true, "Weg 2")),
        ("IBAN leeren", profile_args(None, Some(BIC), true, "Weg 2")),
        (
            "BIC",
            profile_args(Some(IBAN_A), Some("COBADEFFXXX"), true, "Weg 2"),
        ),
    ] {
        let r = call(&srv.app, &cookie, upsert, args).await;
        assert_eq!(r.status, StatusCode::FORBIDDEN, "{what}: {}", r.text());
        assert_eq!(r.json()["code"], "reauth_required", "{what}");
    }
    // ADR 0042 Nr. 1: Den §19-Status ändert das Profil nicht mehr; ein
    // abweichender Wert ist ein Fachfehler (eigene Commands mit Re-Auth).
    let r = call(
        &srv.app,
        &cookie,
        upsert,
        profile_args(
            Some("de02120300000000202051"),
            Some(" BYLADEM1001 "),
            false,
            "Weg 2",
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text());
    assert!(r.text().contains("Statuswechsel eintragen"), "{}", r.text());
    assert_eq!(
        stored_iban(&srv, &cookie).await.as_deref(),
        Some("de02120300000000202051"),
        "unverändert (zuletzt gespeicherte Schreibweise)"
    );

    // 3. 403 mit Idempotenz-Schlüssel, Re-Auth (rotiert die ID), dieselbe
    // Anfrage mit demselben Schlüssel: wird jetzt ausgeführt.
    let body = profile_args(Some(IBAN_B), Some(BIC), true, "Weg 2").to_string();
    let r = send(
        &srv.app,
        post(
            "/api/cmd/seller_profile_upsert",
            &body,
            Some(&cookie),
            Some("profil-1"),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.text());
    let fresh = reauth_cookie(&srv, &cookie).await;
    let r = send(
        &srv.app,
        post(
            "/api/cmd/seller_profile_upsert",
            &body,
            Some(&fresh),
            Some("profil-1"),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "nach Re-Auth: {}", r.text());
    assert_eq!(r.json()["iban"], IBAN_B);

    // 4. Re-Auth abgelaufen: wieder Pflicht.
    srv.state.sessions.age_for_tests(later);
    let r = call(
        &srv.app,
        &fresh,
        upsert,
        profile_args(Some(IBAN_A), Some(BIC), true, "Weg 2"),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.text());

    // 5. Eine frische Anmeldung (Passphrase + TOTP) zählt 5 min.
    let login_cookie = session_cookie(&login(&srv, PASS).await);
    let r = call(
        &srv.app,
        &login_cookie,
        upsert,
        profile_args(Some(IBAN_A), Some(BIC), true, "Weg 2"),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "frisch angemeldet: {}", r.text());
    // … aber nicht für Commands mit fester Re-Auth-Pflicht (W14: Anmeldung
    // ist keine Re-Auth).
    let r = send(
        &srv.app,
        post(
            "/api/cmd/payment_accounts_set_active",
            "{}",
            Some(&login_cookie),
            None,
        ),
    )
    .await;
    assert_eq!(r.json()["code"], "reauth_required", "{}", r.text());
}

/// W14-005 (Lauf F4a): Sitzungsübersicht und „andere abmelden“ (Re-Auth,
/// Audit mit Anzahl).
#[tokio::test]
async fn sessions_overview_and_revoke_others() {
    let srv = test_server();
    let a = setup(&srv).await;
    let b = session_cookie(&login(&srv, PASS).await);
    let r = send(&srv.app, get("/api/auth/sessions", Some(&a))).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let list = r.json();
    assert_eq!(list["count"], 2);
    let sessions = list["sessions"].as_array().unwrap();
    assert_eq!(sessions.iter().filter(|s| s["current"] == true).count(), 1);
    for s in sessions {
        assert_eq!(s["id"].as_str().unwrap().len(), 8, "{s}");
        let login_at = s["loginAt"].as_str().unwrap();
        assert!(
            klein_buch_lib::clock::db_utc(login_at).is_some(),
            "UTC wie DB-Zeitstempel: {login_at}"
        );
        assert!(!s.to_string().contains(&a) && !s.to_string().contains(&b));
    }
    assert_eq!(
        send(&srv.app, get("/api/auth/sessions", None)).await.status,
        StatusCode::UNAUTHORIZED
    );

    let revoke = |cookie: &str| post("/api/auth/sessions/others/revoke", "{}", Some(cookie), None);
    let r = send(&srv.app, revoke(&a)).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert_eq!(r.json()["code"], "reauth_required");
    let a2 = reauth_cookie(&srv, &a).await;
    let r = send(&srv.app, revoke(&a2)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["revoked"], 1, "nur die fremde Anmeldung zählt");
    let r = send(&srv.app, get("/api/auth/status", Some(&b))).await;
    assert_eq!(
        r.json()["state"],
        "login_required",
        "andere Sitzung beendet"
    );
    let r = send(&srv.app, get("/api/auth/sessions", Some(&a2))).await;
    assert_eq!(r.json()["count"], 1);
    let details: String = {
        let pool = srv.state.ctx.pool().unwrap();
        sqlx::query_scalar(
            "SELECT details_json FROM audit_log WHERE action = 'auth.sessions_revoked'",
        )
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    assert!(details.contains("\"count\":1"), "{details}");
}

/// W14-033 (Lauf F4a): Idempotenz-Schlüssel gelten nur innerhalb einer
/// Anmeldung — auch über die Rotation durch Re-Auth hinweg.
#[tokio::test]
async fn idempotency_key_is_bound_to_the_login() {
    let srv = test_server();
    let a = setup(&srv).await;
    let b = session_cookie(&login(&srv, PASS).await);
    let body = serde_json::json!({"input": {
        "contactType": "customer", "name": "Schlüssel GmbH", "street": "Weg 1",
        "postalCode": "12345", "city": "Musterstadt", "countryCode": "DE",
        "acceptsEinvoice": true
    }})
    .to_string();
    let create = |cookie: &str| {
        post(
            "/api/cmd/contacts_create",
            &body,
            Some(cookie),
            Some("gleicher-schluessel"),
        )
    };
    let r1 = send(&srv.app, create(&a)).await;
    assert_eq!(r1.status, StatusCode::OK, "{}", r1.text());
    let r2 = send(&srv.app, create(&b)).await;
    assert_eq!(r2.status, StatusCode::OK, "{}", r2.text());
    assert_ne!(
        r1.json()["id"],
        r2.json()["id"],
        "andere Anmeldung: eigene Ausführung"
    );
    let r3 = send(&srv.app, create(&a)).await;
    assert_eq!(
        r3.json()["id"],
        r1.json()["id"],
        "gleiche Anmeldung: Wiederholung"
    );
    let a2 = reauth_cookie(&srv, &a).await;
    let r4 = send(&srv.app, create(&a2)).await;
    assert_eq!(
        r4.json()["id"],
        r1.json()["id"],
        "nach Rotation dieselbe Anmeldung"
    );
}

/// W14-015/W14-033 (Lauf F4a): „Datenbank sperren“ leert gespeicherte
/// Antworten, beendete Job-Ergebnisse und offene Anmelde-Ergebnisse.
#[tokio::test]
async fn lock_clears_stored_results_and_pending_logins() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let body = serde_json::json!({"input": {
        "contactType": "customer", "name": "Gespeichert GmbH", "street": "Weg 1",
        "postalCode": "12345", "city": "Musterstadt", "countryCode": "DE",
        "acceptsEinvoice": true
    }})
    .to_string();
    let r = send(
        &srv.app,
        post(
            "/api/cmd/contacts_create",
            &body,
            Some(&cookie),
            Some("k-1"),
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(!srv.state.idem.is_empty());
    let job = srv.state.jobs.reserve("backup_create_now").unwrap();
    srv.state.jobs.finish(
        job,
        Ok(serde_json::json!({"download": {"downloadId": "x"}})),
    );
    assert!(srv.state.jobs.view(job).is_some());
    let _parked = srv.state.pending_auth.park().unwrap();
    let r = send(&srv.app, post("/api/auth/lock", "{}", Some(&cookie), None)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(srv.state.idem.is_empty(), "Idempotenz-Speicher leer");
    assert!(srv.state.jobs.view(job).is_none(), "Job-Ergebnis verworfen");
    assert!(
        srv.state.pending_auth.is_empty(),
        "offene Anmeldungen verworfen"
    );
    assert!(srv.state.ctx.session.get().is_none());
}

/// W14-004 (Lauf F4a, NIST SP 800-63B §5.2.2): Nach 100 Fehlversuchen in Folge
/// ohne Gerätecookie ist die TOTP-Anmeldung ohne Gerätecookie gesperrt —
/// Einmal-Code und bekanntes Gerät bleiben, eine solche Anmeldung hebt die
/// Sperre auf; Hinweise in der Inbox; der Zähler übersteht einen Neustart.
#[tokio::test]
async fn totp_streak_lock_spares_recovery_codes_and_known_devices() {
    use klein_buch_lib::server::ratelimit::TOTP_LOCK_AFTER;
    let srv = test_server();
    let reply = setup_reply(&srv).await;
    let cookie = session_cookie(&reply);
    let device = device_cookie_of(&reply).unwrap();
    let codes: Vec<String> = serde_json::from_value(reply.json()["recoveryCodes"].clone()).unwrap();
    let notices = |title: &'static str| {
        let pool = srv.state.ctx.pool().unwrap();
        async move {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM notifications WHERE title = ?")
                .bind(title)
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    srv.state
        .ratelimit
        .set_streak_for_tests(TOTP_LOCK_AFTER - 1);
    // Der 100. Fehlversuch sperrt (Hinweis, da entsperrt).
    let r = send(
        &srv.app,
        from_peer(
            post("/api/auth/login", &login_body(PASS, "000000"), None, None),
            "198.51.100.30:4000",
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    assert!(srv.state.ratelimit.totp_locked());
    assert_eq!(notices("Anmeldung mit App-Code gesperrt").await, 1);
    // Lauf F4d: Der Hinweis unterstellt keine Kenntnis der Passphrase (die
    // Sperre ist schon mit falschen App-Codes auslösbar).
    {
        let pool = srv.state.ctx.pool().unwrap();
        let body: String = sqlx::query_scalar(
            "SELECT body FROM notifications WHERE title = 'Anmeldung mit App-Code gesperrt'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(body.contains("nicht verraten"), "{body}");
        assert!(!body.contains("kennt"), "{body}");
        assert!(body.contains("Einmal-Code gilt nur einmal"), "{body}");
    }
    // Gültiger TOTP ohne Gerätecookie: gesperrt, ohne KDF.
    let kdf_before = srv.state.admission.admitted(Cost::Kdf);
    let code = next_totp(&srv);
    let r = send(
        &srv.app,
        from_peer(
            post("/api/auth/login", &login_body(PASS, &code), None, None),
            "198.51.100.31:4000",
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS, "{}", r.text());
    assert_eq!(r.json()["code"], "totp_locked");
    assert_eq!(
        srv.state.admission.admitted(Cost::Kdf),
        kdf_before,
        "keine KDF"
    );
    // Re-Auth mit TOTP ohne Gerätecookie ebenso.
    let code = next_totp(&srv);
    let r = send(
        &srv.app,
        post(
            "/api/auth/reauth",
            &login_body(PASS, &code),
            Some(&cookie),
            None,
        ),
    )
    .await;
    assert_eq!(r.json()["code"], "totp_locked", "{}", r.text());
    // Neustart hebt die Sperre nicht auf.
    let reloaded = klein_buch_lib::server::ratelimit::RateLimiter::load_or_create(
        &srv.state.ctx.paths.data_dir,
    )
    .unwrap();
    assert!(reloaded.totp_locked(), "Zähler in /data/auth");
    // Bekanntes Gerät: TOTP-Anmeldung geht und hebt die Sperre auf.
    let code = next_totp(&srv);
    let mut req = from_peer(
        post("/api/auth/login", &login_body(PASS, &code), None, None),
        "198.51.100.32:4000",
    );
    req.headers_mut()
        .insert(header::COOKIE, device.parse().unwrap());
    let r = {
        let _serial = kdf_serial().await;
        send(&srv.app, req).await
    };
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(!srv.state.ratelimit.totp_locked());
    assert_eq!(notices("Sperre der App-Code-Anmeldung aufgehoben").await, 1);
    // Erneut gesperrt: ein Einmal-Code (neues Gerät) hebt sie ebenso auf.
    srv.state.ratelimit.set_streak_for_tests(TOTP_LOCK_AFTER);
    let body = serde_json::json!({ "passphrase": PASS, "recoveryCode": codes[0] }).to_string();
    let r = {
        let _serial = kdf_serial().await;
        send(
            &srv.app,
            from_peer(
                post("/api/auth/login", &body, None, None),
                "198.51.100.33:4000",
            ),
        )
        .await
    };
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(!srv.state.ratelimit.totp_locked());
    let r = login(&srv, PASS).await;
    assert_eq!(
        r.status,
        StatusCode::OK,
        "TOTP ohne Gerätecookie wieder frei: {}",
        r.text()
    );
}

/// Lauf F4d (Befund F4 S3 zu W14-004): Aussagekräftig ist eine falsche
/// Passphrase nach **gültigem** App-Code. Ab drei in Folge entsteht genau ein
/// dringender Hinweis „Gültiger App-Code mit falschem Daten-Passwort“
/// (Neueinrichtung des App-Codes empfohlen); einzelne Tippfehler und falsche
/// App-Codes lösen ihn nicht aus. Bei gesperrter Instanz (nach Neustart) folgt
/// der Hinweis mit der nächsten erfolgreichen Anmeldung.
#[tokio::test]
async fn wrong_passphrase_after_valid_totp_warns_once_per_streak() {
    const TITLE: &str = "Gültiger App-Code mit falschem Daten-Passwort";
    const WRONG: &str = "falsch-aber-lang-genug-1234";
    let srv = test_server();
    let _ = setup(&srv).await;
    async fn notices(srv: &TestServer) -> i64 {
        let pool = srv.state.ctx.pool().unwrap();
        sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE title = ?")
            .bind(TITLE)
            .fetch_one(&pool)
            .await
            .unwrap()
    }
    // Zwei Tippfehler, dann Erfolg: kein Hinweis, Folge beendet.
    for _ in 0..2 {
        assert_eq!(login(&srv, WRONG).await.status, StatusCode::UNAUTHORIZED);
    }
    assert_eq!(login(&srv, PASS).await.status, StatusCode::OK);
    assert_eq!(notices(&srv).await, 0, "Tippfehler lösen nichts aus");
    // Falsche App-Codes zählen nicht für diesen Hinweis.
    for _ in 0..3 {
        let r = send(
            &srv.app,
            post("/api/auth/login", &login_body(PASS, "000000"), None, None),
        )
        .await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    }
    assert_eq!(notices(&srv).await, 0, "falsche App-Codes zählen nicht");
    // (Erfolg setzt das Rate-Limit des Schlüssels zurück.)
    assert_eq!(login(&srv, PASS).await.status, StatusCode::OK);
    // Vier in Folge: genau ein Hinweis (beim dritten); der Erfolg danach
    // schreibt ihn nicht erneut.
    for _ in 0..4 {
        assert_eq!(login(&srv, WRONG).await.status, StatusCode::UNAUTHORIZED);
    }
    assert_eq!(notices(&srv).await, 1);
    {
        let pool = srv.state.ctx.pool().unwrap();
        let (body, severity): (String, String) =
            sqlx::query_as("SELECT body, severity FROM notifications WHERE title = ?")
                .bind(TITLE)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(severity, "urgent");
        assert!(body.contains("neu ein"), "{body}");
    }
    assert_eq!(login(&srv, PASS).await.status, StatusCode::OK);
    assert_eq!(notices(&srv).await, 1, "ein Hinweis je Folge");

    // Gesperrte Instanz: keine offene DB beim Erreichen — der Hinweis folgt
    // mit der nächsten erfolgreichen Anmeldung.
    let srv = restart(srv).await;
    for _ in 0..3 {
        assert_eq!(login(&srv, WRONG).await.status, StatusCode::UNAUTHORIZED);
    }
    assert_eq!(login(&srv, PASS).await.status, StatusCode::OK);
    assert_eq!(notices(&srv).await, 2, "nachgeholt nach dem Entsperren");
}

/// ASVS V2.1.7/V2.1.2 (Lauf F4a): neue Passphrasen gegen Sperrliste, Muster
/// und Profil (Firmenname/E-Mail); Höchstlänge 1024. Bestehende gelten weiter.
#[tokio::test]
async fn new_passphrases_are_checked_against_blocklist_patterns_and_profile() {
    let srv = test_server();
    // Vorprüfung schon beim Anfordern des QR-Codes (Ersteinrichtung).
    let token = srv.state.setup_token_for_tests().unwrap();
    let r = send(
        &srv.app,
        post(
            "/api/auth/setup",
            &serde_json::json!({
                "setupToken": token, "step": "totp", "passphrase": "aaaaaaaaaaaaaaaaaaaa"
            })
            .to_string(),
            None,
            None,
        ),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text());
    assert_eq!(r.json()["code"], "passphrase_common");
    // Verbindlich im letzten Schritt (auch ohne Vorprüfung).
    let r = setup_reply_with(&srv, "korrekt-pferd-batterie-klammer").await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text());
    assert_eq!(r.json()["code"], "passphrase_common");
    assert!(!srv.state.ctx.paths.db_file.exists(), "nichts angelegt");
    let cookie = setup(&srv).await;
    let r = call(
        &srv.app,
        &cookie,
        "seller_profile_upsert",
        profile_args(None, None, true, "Weg 1"),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let cookie = reauth_cookie(&srv, &cookie).await;
    let change = |p: &str| {
        post(
            "/api/auth/passphrase",
            &serde_json::json!({ "newPassphrase": p }).to_string(),
            Some(&cookie),
            None,
        )
    };
    for (p, code) in [
        ("Beispiel IT-Service 2026!", "passphrase_common"),
        ("kontakt@example.org-2026", "passphrase_common"),
        ("qwertzuiopasdfghjkl", "passphrase_common"),
        ("Klein.Buch-Passwort-2026", "passphrase_common"),
    ] {
        let r = send(&srv.app, change(p)).await;
        assert_eq!(
            r.status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{p}: {}",
            r.text()
        );
        assert_eq!(r.json()["code"], code, "{p}");
    }
    let long = "Gletscher Moped Ahorn Tinte ".repeat(40);
    let r = send(&srv.app, change(&long)).await;
    assert_eq!(r.json()["code"], "passphrase_too_long", "{}", r.text());
    // Bestehende (in diesem Test beliebige) Passphrase: Anmeldung geht weiter.
    assert_eq!(login(&srv, PASS).await.status, StatusCode::OK);
}

/// W14-037 (Lauf F4a): Dauert die Anmeldung länger als die Wartezeit, kommt
/// 202 mit Ticket-Cookie; die Abholung liefert genau einmal das Ergebnis.
#[tokio::test]
async fn slow_login_answers_202_and_poll_delivers_session_once() {
    let srv = test_server();
    let _ = setup(&srv).await;
    srv.state
        .set_login_sync_wait_for_tests(std::time::Duration::ZERO);
    let poll = |ticket: Option<&str>| {
        let mut req = post("/api/auth/login/poll", "{}", None, None);
        if let Some(t) = ticket {
            req.headers_mut().insert(
                header::COOKIE,
                format!("__Host-kb_login={t}").parse().unwrap(),
            );
        }
        req
    };
    let ticket_of = |r: &Reply| {
        r.headers
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .find_map(|raw| {
                raw.split(';')
                    .next()
                    .and_then(|c| c.strip_prefix("__Host-kb_login="))
                    .map(str::to_string)
            })
    };
    for (pass, expected) in [
        (PASS, StatusCode::OK),
        ("falsche-lange-passphrase-1", StatusCode::UNAUTHORIZED),
    ] {
        // KDF im Hintergrund-Task: bis zur Abholung serialisiert.
        let _serial = kdf_serial().await;
        let code = next_totp(&srv);
        let r = send(
            &srv.app,
            post("/api/auth/login", &login_body(pass, &code), None, None),
        )
        .await;
        assert_eq!(r.status, StatusCode::ACCEPTED, "{}", r.text());
        assert_eq!(r.json()["state"], "pending");
        let ticket = ticket_of(&r).expect("Ticket-Cookie");
        let raw = r
            .headers
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .find(|v| v.starts_with("__Host-kb_login="))
            .unwrap()
            .to_string();
        for attr in ["HttpOnly", "Secure", "SameSite=Strict", "Path=/"] {
            assert!(raw.contains(attr), "{raw}");
        }
        let mut done = None;
        for _ in 0..600 {
            let r = send(&srv.app, poll(Some(&ticket))).await;
            if r.status != StatusCode::ACCEPTED {
                done = Some(r);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        let done = done.expect("Ergebnis nach höchstens 30 s");
        assert_eq!(done.status, expected, "{pass}: {}", done.text());
        assert_eq!(ticket_of(&done).as_deref(), Some(""), "Ticket gelöscht");
        if expected == StatusCode::OK {
            let cookie = session_cookie(&done);
            let s = send(&srv.app, get("/api/auth/status", Some(&cookie))).await;
            assert_eq!(s.json()["state"], "ok");
        }
        let again = send(&srv.app, poll(Some(&ticket))).await;
        assert_eq!(again.status, StatusCode::GONE, "nur einmal abholbar");
        assert_eq!(again.json()["code"], "login_expired");
    }
    let r = send(&srv.app, poll(None)).await;
    assert_eq!(r.status, StatusCode::GONE);
}

/// W14-009 (Lauf F5c): Re-Auth-Matrix für den Versand mit frei gewähltem
/// Empfänger. Außerhalb der 5-Minuten-Frische gilt: hinterlegte Adresse (auch in
/// anderer Schreibweise) und geänderter Text ohne Re-Auth; ein abweichender
/// Empfänger ergibt 403 `reauth_required` vor der Annahme des Jobs — ohne
/// Versandversuch. Nach der Re-Auth geht derselbe Aufruf durch.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mail_send_reauth_matrix() {
    use klein_buch_lib::db::repo::mail_accounts;
    use klein_buch_lib::server::session::REAUTH_WINDOW;
    let later = REAUTH_WINDOW + std::time::Duration::from_secs(1);
    let srv = test_server();
    let cookie = setup(&srv).await;
    let invoice_id = draft_invoice(&srv).await;
    let r = call(
        &srv.app,
        &cookie,
        "invoices_lock_and_issue",
        serde_json::json!({ "id": invoice_id }),
    )
    .await;
    assert_eq!(r.json()["state"], "done", "{}", r.text());
    settle_lock_backups(&srv).await;
    let pool = srv.state.ctx.pool().unwrap();
    let mail_templates = srv.state.ctx.paths.inputs_dir.join("mail-templates");
    std::fs::create_dir_all(&mail_templates).unwrap();
    std::fs::write(
        mail_templates.join("invoice-de.txt"),
        "Subject: Rechnung {{ invoice.number }}\n\nAnbei die Rechnung {{ invoice.number }}.\n",
    )
    .unwrap();

    // Postfach auf einem Port ohne Gegenstelle: ein zugelassener Versand
    // scheitert erst beim Verbinden (Protokoll-Eintrag „failed“).
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let account = mail_accounts::create(
        &pool,
        &mail_accounts::MailAccountInput {
            label: "W14-009".into(),
            auth_type: "smtp_password".into(),
            smtp_host: Some("127.0.0.1".into()),
            smtp_port: Some(i64::from(port)),
            smtp_user: None,
            smtp_use_tls: false,
            from_email: "rechnung@example.org".into(),
            from_name: "Beispiel IT-Service".into(),
            is_default: true,
        },
    )
    .await
    .unwrap()
    .id;
    // Angebots-Entwurf an denselben Kontakt (für die Vorprüfung genügt er).
    let contact_id: String = sqlx::query_scalar("SELECT contact_id FROM invoices WHERE id = ?")
        .bind(&invoice_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let today = klein_buch_lib::clock::today_berlin();
    let quote_id = klein_buch_lib::commands::quotes::create_draft_core(
        &pool,
        klein_buch_lib::commands::quotes::CreateDraftArgs {
            contact_id,
            input: klein_buch_lib::domain::quote::QuoteInput {
                quote_date: today,
                valid_until: today + chrono::Days::new(30),
                currency_code: "EUR".into(),
                items: vec![klein_buch_lib::domain::quote::QuoteItemInput {
                    position: 1,
                    description: "Beratung".into(),
                    quantity: 1.0,
                    unit_code: "HUR".into(),
                    unit_price_cents: 10_000,
                    tax_rate_percent: 0.0,
                    tax_category_code: "E".into(),
                    description_title: None,
                    description_markup: None,
                    source_package_id: None,
                    source_package_revision: None,
                }],
                notes: None,
                pdf_template: "default".into(),
            },
        },
    )
    .await
    .unwrap()
    .quote
    .id;

    let log_rows = || async {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM email_log")
            .fetch_one(&pool)
            .await
            .unwrap()
    };
    let invoice_args = |to: Option<&str>, body: Option<&str>| {
        serde_json::json!({ "args": {
            "accountId": account, "invoiceId": invoice_id,
            "to": to, "subject": null, "body": body
        }})
    };
    let quote_args = |to: Option<&str>| {
        serde_json::json!({ "args": {
            "accountId": account, "quoteId": quote_id,
            "to": to, "subject": null, "body": null
        }})
    };
    // Zugelassen: Job läuft und scheitert am Verbinden bzw. am Angebotsstatus.
    let admitted = |r: &Reply, what: &str| {
        assert_ne!(r.status, StatusCode::FORBIDDEN, "{what}: {}", r.text());
        assert_eq!(r.json()["state"], "failed", "{what}: {}", r.text());
        assert_ne!(r.json()["error"]["code"], "reauth_required", "{what}");
    };

    // 1. Weder Re-Auth noch Anmeldung frisch.
    srv.state.sessions.age_for_tests(later);
    for (to, body, what) in [
        (None, None, "hinterlegter Empfänger"),
        (Some("KUNDE@Example.org"), None, "andere Schreibweise"),
        (
            Some("kunde@example.org"),
            Some("Geänderter Text"),
            "geänderter Text",
        ),
    ] {
        let r = call(
            &srv.app,
            &cookie,
            "mail_send_invoice",
            invoice_args(to, body),
        )
        .await;
        admitted(&r, what);
    }
    assert_eq!(log_rows().await, 3, "drei Versandversuche protokolliert");
    let r = call(&srv.app, &cookie, "mail_send_quote", quote_args(None)).await;
    admitted(&r, "Angebot an den Kontakt");

    for (name, args) in [
        (
            "mail_send_invoice",
            invoice_args(Some("fremd@example.net"), None),
        ),
        ("mail_send_quote", quote_args(Some("fremd@example.net"))),
    ] {
        let r = call(&srv.app, &cookie, name, args).await;
        assert_eq!(r.status, StatusCode::FORBIDDEN, "{name}: {}", r.text());
        assert_eq!(r.json()["code"], "reauth_required", "{name}");
    }
    assert_eq!(log_rows().await, 3, "kein Versandversuch ohne Re-Auth");
    let bound: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM quote_legal_documents")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(bound, 0, "abgelehnter Versand bindet keine Rechtsdokumente");

    // 2. Nach der Re-Auth: derselbe Empfänger geht durch.
    let cookie = reauth_cookie(&srv, &cookie).await;
    let r = call(
        &srv.app,
        &cookie,
        "mail_send_invoice",
        invoice_args(Some("fremd@example.net"), None),
    )
    .await;
    admitted(&r, "nach Re-Auth");
    let to: String = sqlx::query_scalar("SELECT to_email FROM email_log ORDER BY id DESC LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(to, "fremd@example.net");
    let r = call(
        &srv.app,
        &cookie,
        "mail_send_quote",
        quote_args(Some("fremd@example.net")),
    )
    .await;
    admitted(&r, "Angebot nach Re-Auth");
}
