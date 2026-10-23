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
    let token = srv.state.setup_token_for_tests().expect("Token beim Start");
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
        .filter(|c| c.exposed() && c.auth == Auth::Reauth)
        .map(|c| c.name)
        .collect();
    assert_eq!(reauth_cmds.len(), 13);
    for name in &reauth_cmds {
        let r = send(
            &srv.app,
            post(&format!("/api/cmd/{name}"), "{}", Some(&cookie), None),
        )
        .await;
        assert_eq!(r.status, StatusCode::FORBIDDEN, "{name}");
        assert_eq!(r.json()["code"], "reauth_required", "{name}");
    }
    let r = reauth(&srv, &cookie).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
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
        *srv.state.ctx.scheduler_gate.lock().await,
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
    assert!(!*srv.state.ctx.scheduler_gate.lock().await);
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
    assert_eq!(reauth(&srv, &cookie).await.status, StatusCode::OK);
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
    assert_eq!(reauth(&srv, &cookie).await.status, StatusCode::OK);
    let r = send(
        &srv.app,
        post("/api/auth/recovery/renew", "{}", Some(&cookie), None),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.json()["recoveryCodes"].as_array().unwrap().len(), 10);

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
