//! Plan W7 — Uploads (W7b): `POST /api/upload/{art}`.
//! - 413 je Art (vorab über `Content-Length` und beim Streamen), Teil gelöscht
//! - falscher Typ (Magic Bytes) → 415, SVG nur für Logo/Unterschrift
//! - Dateinamen bereinigt, MIME aus der Inhaltsprüfung
//! - Rechnungseingang per Multipart mit XRechnung (CII)
//! - Sitzung, Pflicht-Header, unbekannte Art, freier Platz (507)
//! - Commands nehmen die `uploadId`; nach Erfolg verbraucht

mod server_support;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use chrono::NaiveDate;
use klein_buch_lib::domain::invoice::{
    BuyerView, InvoiceDirection, InvoiceInput, InvoiceItemInput, SellerView,
};
use klein_buch_lib::einvoice::generator::to_xrechnung;
use klein_buch_lib::uploads::UploadLimits;
use server_support::*;

const MIB: usize = 1024 * 1024;

fn pending_uploads(srv: &TestServer) -> usize {
    std::fs::read_dir(srv.state.ctx.uploads.root())
        .map(|it| it.count())
        .unwrap_or(0)
}

fn pdf_of_len(len: usize) -> Vec<u8> {
    let mut v = b"%PDF-1.7\n".to_vec();
    v.resize(len, b'0');
    v
}

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
const SVG: &[u8] = b"<?xml version=\"1.0\"?><svg xmlns=\"http://www.w3.org/2000/svg\"/>";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn too_large_per_kind_is_413_and_leaves_nothing() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    for (art, max) in [
        ("beleg", 20 * MIB),
        ("anhang", 20 * MIB),
        ("vertrag", 20 * MIB),
        ("rechtsdokument", 20 * MIB),
        ("erechnung", 10 * MIB),
        ("logo", 2 * MIB),
        ("unterschrift", 2 * MIB),
    ] {
        // beim Streamen gezählt
        let r = upload(&srv, &cookie, art, "gross.pdf", &pdf_of_len(max + 1)).await;
        assert_eq!(
            r.status,
            StatusCode::PAYLOAD_TOO_LARGE,
            "{art}: {}",
            r.text()
        );
        // genau am Limit geht (Typ passt bei Logo/Unterschrift nicht → 415)
        let r = upload(&srv, &cookie, art, "genau.pdf", &pdf_of_len(max)).await;
        let expected = if matches!(art, "logo" | "unterschrift") {
            StatusCode::UNSUPPORTED_MEDIA_TYPE
        } else {
            StatusCode::OK
        };
        assert_eq!(r.status, expected, "{art}: {}", r.text());
    }
    // vorab über Content-Length
    let mut req = upload_request("logo", Some(&cookie), "x.png", PNG);
    req.headers_mut().insert(
        header::CONTENT_LENGTH,
        (3 * MIB).to_string().parse().unwrap(),
    );
    let r = send(&srv.app, req).await;
    assert_eq!(r.status, StatusCode::PAYLOAD_TOO_LARGE);
    // Restore-Limit aus der Umgebung
    srv.state.ctx.uploads.set_limits(UploadLimits {
        min_free_bytes: 0,
        max_restore_bytes: 1024,
    });
    let mut kbk = klein_buch_lib::backup::manifest::MAGIC.as_bytes().to_vec();
    kbk.resize(2048, b'x');
    let r = upload(&srv, &cookie, "restore", "alt.kbk", &kbk).await;
    assert_eq!(r.status, StatusCode::PAYLOAD_TOO_LARGE);
    // nur die angenommenen Uploads liegen noch da
    assert_eq!(pending_uploads(&srv), 5);
    assert_eq!(srv.state.ctx.uploads.len(), 5);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn type_is_checked_by_content() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let cases: &[(&str, &[u8], StatusCode)] = &[
        (
            "beleg",
            b"<html><script>alert(1)</script></html>",
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ),
        ("beleg", SVG, StatusCode::UNSUPPORTED_MEDIA_TYPE),
        ("beleg", PNG, StatusCode::OK),
        ("logo", SVG, StatusCode::OK),
        ("unterschrift", PNG, StatusCode::OK),
        ("logo", b"%PDF-1.7", StatusCode::UNSUPPORTED_MEDIA_TYPE),
        ("rechtsdokument", PNG, StatusCode::UNSUPPORTED_MEDIA_TYPE),
        ("rechtsdokument", b"%PDF-1.7", StatusCode::OK),
        ("erechnung", PNG, StatusCode::UNSUPPORTED_MEDIA_TYPE),
        ("restore", b"%PDF-1.7", StatusCode::UNSUPPORTED_MEDIA_TYPE),
    ];
    for (art, bytes, expected) in cases {
        let r = upload(&srv, &cookie, art, "datei.pdf", bytes).await;
        assert_eq!(r.status, *expected, "{art}: {}", r.text());
    }
    // XML in falscher Kodierung: klare Meldung (R3-006)
    let mut latin1 = b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><Invoice><Name>M".to_vec();
    latin1.push(0xFC);
    latin1.extend_from_slice(b"ller</Name></Invoice>");
    let r = upload(&srv, &cookie, "erechnung", "eingang.xml", &latin1).await;
    assert_eq!(r.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert!(r.text().contains("nicht UTF-8-codiert"), "{}", r.text());
    // MIME aus dem Inhalt, nicht aus der Endung
    let r = upload(&srv, &cookie, "beleg", "foto.pdf", PNG).await;
    assert_eq!(r.json()["mimeType"], "image/png");
    // leer
    let r = upload(&srv, &cookie, "beleg", "leer.pdf", b"").await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn access_rules() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let r = send(
        &srv.app,
        upload_request("beleg", None, "a.pdf", b"%PDF-1.7"),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    let mut req = upload_request("beleg", Some(&cookie), "a.pdf", b"%PDF-1.7");
    req.headers_mut().remove("x-kleinbuch-request");
    let r = send(&srv.app, req).await;
    assert_eq!(r.json()["code"], "request_header_missing", "{}", r.text());
    let r = upload(&srv, &cookie, "unbekannt", "a.pdf", b"%PDF-1.7").await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
    // kein multipart
    let r = send(
        &srv.app,
        post("/api/upload/beleg", "%PDF-1.7", Some(&cookie), None),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    // fremder Origin (Plan W6)
    let mut req = upload_request("beleg", Some(&cookie), "a.pdf", b"%PDF-1.7");
    req.headers_mut()
        .insert(header::ORIGIN, "https://evil.example.org".parse().unwrap());
    let r = send(&srv.app, req).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    // zu wenig Platz → 507
    srv.state.ctx.uploads.set_limits(UploadLimits {
        min_free_bytes: u64::MAX / 2,
        ..Default::default()
    });
    let r = upload(&srv, &cookie, "beleg", "a.pdf", b"%PDF-1.7").await;
    assert_eq!(r.status, StatusCode::INSUFFICIENT_STORAGE);
    assert_eq!(pending_uploads(&srv), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn attachment_upload_is_sanitized_and_consumed() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let invoice = draft_invoice(&srv).await;
    let r = upload(&srv, &cookie, "anhang", "../../etc/Böse Datei.pdf", PNG).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(r.json()["fileName"], "Böse Datei.pdf");
    let id = r.json()["uploadId"].as_str().unwrap().to_string();

    // falsche Art im Command
    let r = call(
        &srv.app,
        &cookie,
        "seller_logo_set",
        serde_json::json!({ "uploadId": id }),
    )
    .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text());

    let add = serde_json::json!({ "args": {
        "parentType": "invoice", "parentId": invoice, "uploadId": id, "label": null
    }});
    let r = call(&srv.app, &cookie, "attachments_add", add.clone()).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let archive_id = r.json()[0]["archiveEntryId"].as_str().unwrap().to_string();
    let pool = srv.state.ctx.pool().unwrap();
    let (name, mime): (String, String) =
        sqlx::query_as("SELECT file_name, mime_type FROM archive_entries WHERE id = ?")
            .bind(&archive_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        name.ends_with("Böse Datei.pdf") && !name.contains('/'),
        "{name}"
    );
    assert_eq!(mime, "image/png", "MIME aus der Inhaltsprüfung");
    // verbraucht
    let r = call(&srv.app, &cookie, "attachments_add", add).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text());
    assert_eq!(pending_uploads(&srv), 0);
    // alter Weg (Bytes im JSON) gibt es nicht mehr
    let legacy = serde_json::json!({ "args": {
        "parentType": "invoice", "parentId": invoice,
        "fileBytes": vec![0u8; 300 * 1024], "fileName": "x.pdf", "label": null
    }});
    let r = call(&srv.app, &cookie, "attachments_add", legacy).await;
    assert_eq!(r.status, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn receipt_logo_and_legal_document_via_upload() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let _ = draft_invoice(&srv).await; // Verkäuferprofil
    let year = klein_buch_lib::clock::today_berlin()
        .format("%Y-%m-%d")
        .to_string();

    let receipt = upload_id(&srv, &cookie, "beleg", "quittung.jpg", b"%PDF-1.7 beleg").await;
    let r = call(
        &srv.app,
        &cookie,
        "expenses_create",
        serde_json::json!({ "args": {
            "input": {
                "expenseDate": year, "paidDate": null, "paidFromAccountId": null,
                "vendorContactId": null, "vendorName": "Bürobedarf Beispiel",
                "vendorInvoiceNumber": null, "category": "office",
                "description": "Druckerpapier", "netAmountCents": 1000,
                "taxAmountCents": 0, "grossAmountCents": 1000, "currencyCode": "EUR",
                "reverseCharge13b": false, "notes": null
            },
            "fiscalYear": null,
            "receiptUploadId": receipt
        }}),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let archive_id = r.json()["expense"]["receiptArchiveId"]
        .as_str()
        .unwrap()
        .to_string();
    let got = send(
        &srv.app,
        get(&format!("/files/archive/{archive_id}"), Some(&cookie)),
    )
    .await;
    assert_eq!(got.status, StatusCode::OK);
    assert_eq!(got.headers[header::CONTENT_TYPE], "application/pdf");
    assert!(got.headers[header::CONTENT_DISPOSITION]
        .to_str()
        .unwrap()
        .starts_with("attachment;"));

    let logo = upload_id(&srv, &cookie, "logo", "firma.png", SVG).await;
    let r = call(
        &srv.app,
        &cookie,
        "seller_logo_set",
        serde_json::json!({ "uploadId": logo }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        r.json()["logoFilename"],
        "logo.svg",
        "Endung aus dem Inhalt"
    );
    // Gegenprüfung W7: der Logo-Dateiname kommt nie vom Client (sonst Lesen/
    // Löschen beliebiger Dateien über seller_logo_data/seller_logo_clear).
    // Seit W14 verlangt seller_profile_upsert eine Re-Auth (Zahlungsdaten;
    // seit Lauf F4a nur bei IBAN/BIC/§19-Änderung, W14-038).
    let cookie = reauth_cookie(&srv, &cookie).await;
    let secret = srv.state.ctx.paths.data_dir.join("auth").join("totp.json");
    let r = call(
        &srv.app,
        &cookie,
        "seller_profile_upsert",
        serde_json::json!({ "input": {
            "name": "Beispiel IT-Service", "legalForm": null, "street": "Beispielweg 1",
            "postalCode": "12345", "city": "Musterstadt", "countryCode": "DE",
            "taxNumber": "123/456/78901", "vatId": null, "email": "kontakt@example.org",
            "phone": "+49 000 000000", "iban": null, "bic": null,
            "logoFilename": secret.to_string_lossy(),
            "isKleinunternehmer": true, "defaultPdfTemplate": "default",
            "defaultCurrency": "EUR", "confirmWaiveParagraph19": null
        }}),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(
        r.json()["logoFilename"],
        "logo.svg",
        "Client-Wert ignoriert"
    );
    let r = call(&srv.app, &cookie, "seller_logo_data", serde_json::json!({})).await;
    assert!(!r.text().contains("secret_b32"), "{}", r.text());
    let r = call(
        &srv.app,
        &cookie,
        "seller_logo_clear",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert!(secret.exists(), "nichts außerhalb gelöscht");

    let legal = upload_id(&srv, &cookie, "rechtsdokument", "agb.pdf", b"%PDF-1.7 agb").await;
    let r = call(
        &srv.app,
        &cookie,
        "legal_documents_upload",
        serde_json::json!({ "docType": "agb", "title": "AGB", "uploadId": legal }),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    assert_eq!(pending_uploads(&srv), 0);
}

fn sample_xrechnung() -> String {
    let seller = SellerView {
        name: "Lieferant Beispiel GmbH",
        street: "Industriestr. 5",
        postal_code: "12345",
        city: "Musterstadt",
        country_code: "DE",
        tax_number: Some("11/222/33333"),
        vat_id: Some("DE123456789"),
        email: "rechnung@example.org",
        phone: Some("+49 000 000000"),
        contact_name: None,
        iban: None,
        bic: None,
        is_kleinunternehmer: false,
    };
    let buyer = BuyerView {
        name: "Beispiel IT-Service",
        street: Some("Beispielweg 1"),
        postal_code: Some("12345"),
        city: Some("Musterstadt"),
        country_code: "DE",
        vat_id: None,
        email: Some("kontakt@example.org"),
    };
    let input = InvoiceInput {
        service_period_start: None,
        service_period_end: None,
        buyer_reference: None,
        direction: InvoiceDirection::Issued,
        invoice_date: NaiveDate::from_ymd_opt(2026, 4, 10).unwrap(),
        delivery_date: None,
        due_date: Some(NaiveDate::from_ymd_opt(2026, 5, 10).unwrap()),
        currency_code: "EUR".into(),
        items: vec![InvoiceItemInput {
            position: 1,
            description: "Netzwerk-Switch 24-Port".into(),
            quantity: 1.0,
            unit_code: "C62".into(),
            unit_price_cents: 10_000,
            tax_rate_percent: 19.0,
            tax_category_code: "S".into(),
            description_title: None,
            description_markup: None,
            source_package_id: None,
            source_package_revision: None,
        }],
        notes: None,
        payment_note: None,
        pdf_template: "default".into(),
        is_storno_for: None,
        cancel_reason: None,
    };
    to_xrechnung("LF-2026-0042", &input, &seller, &buyer, "N/A", &[]).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn einvoice_receive_via_multipart() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let xml = sample_xrechnung();
    let id = upload_id(
        &srv,
        &cookie,
        "erechnung",
        "LF-2026-0042.xml",
        xml.as_bytes(),
    )
    .await;
    let parsed = call(
        &srv.app,
        &cookie,
        "expenses_parse_einvoice",
        serde_json::json!({ "uploadId": id }),
    )
    .await;
    assert_eq!(parsed.status, StatusCode::OK, "{}", parsed.text());
    let p = parsed.json();
    assert_eq!(p["sourceFormat"], "xrechnung-cii");
    assert_eq!(p["input"]["grossAmountCents"], 11_900);
    // Einlesen verbraucht den Upload nicht (Schritt 2 braucht das Original)
    assert_eq!(srv.state.ctx.uploads.len(), 1);
    let r = call(
        &srv.app,
        &cookie,
        "expenses_create_from_einvoice",
        serde_json::json!({ "args": {
            "input": p["input"], "fiscalYear": null, "uploadId": id,
            "sourceFormat": p["sourceFormat"], "validation": p["validation"]
        }}),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text());
    let archive_id = r.json()["expense"]["receiptArchiveId"]
        .as_str()
        .unwrap()
        .to_string();
    let pool = srv.state.ctx.pool().unwrap();
    let (mime, source): (String, String) =
        sqlx::query_as("SELECT mime_type, source FROM archive_entries WHERE id = ?")
            .bind(&archive_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(mime, "application/xml");
    assert_eq!(source, "received_einvoice");
    let stored = klein_buch_lib::archive::read_and_verify(&pool, &archive_id)
        .await
        .unwrap();
    assert_eq!(stored, xml.as_bytes(), "Original unverändert archiviert");
    assert_eq!(pending_uploads(&srv), 0);
}

/// Kaputte Multipart-Daten und ein Abbruch mitten im Upload hinterlassen nichts.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn broken_multipart_leaves_nothing() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    let body = "--b\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.pdf\"\r\n\r\n%PDF-1.7 ohne Ende";
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/upload/beleg")
        .header(header::HOST, HOST)
        .header(header::CONTENT_TYPE, "multipart/form-data; boundary=b")
        .header(header::COOKIE, format!("__Host-kb_session={cookie}"))
        .header("x-kleinbuch-request", "1")
        .body(Body::from(body))
        .unwrap();
    let r = send(&srv.app, req).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(pending_uploads(&srv), 0);
    assert!(srv.state.ctx.uploads.is_empty());
}

/// Gegenprüfung W7: der ganze Körper ist begrenzt (auch Präambel/Part-Header,
/// ohne Content-Length), und genau ein Feld „file“ ist erlaubt.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn whole_body_is_limited_and_single_field() {
    let srv = test_server();
    let cookie = setup(&srv).await;
    // 3 MiB Präambel ohne Boundary an „logo“ (Limit 2 MiB + Rahmen), ohne Content-Length
    let body = vec![b'x'; 3 * MIB];
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/upload/logo")
        .header(header::HOST, HOST)
        .header(header::CONTENT_TYPE, "multipart/form-data; boundary=b")
        .header(header::COOKIE, format!("__Host-kb_session={cookie}"))
        .header("x-kleinbuch-request", "1")
        .body(Body::from_stream(futures_util::stream::iter(
            body.chunks(64 * 1024)
                .map(|c| Ok::<_, std::io::Error>(axum::body::Bytes::copy_from_slice(c)))
                .collect::<Vec<_>>(),
        )))
        .unwrap();
    let r = send(&srv.app, req).await;
    assert_eq!(r.status, StatusCode::PAYLOAD_TOO_LARGE, "{}", r.text());
    // zusätzliches Feld → 400
    let b = "kb-test-boundary-7f3a";
    let body = format!(
        "--{b}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.pdf\"\r\n\r\n%PDF-1.7\r\n\
         --{b}\r\nContent-Disposition: form-data; name=\"extra\"\r\n\r\nx\r\n--{b}--\r\n"
    );
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/upload/beleg")
        .header(header::HOST, HOST)
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={b}"),
        )
        .header(header::COOKIE, format!("__Host-kb_session={cookie}"))
        .header("x-kleinbuch-request", "1")
        .body(Body::from(body))
        .unwrap();
    let r = send(&srv.app, req).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text());
    assert_eq!(pending_uploads(&srv), 0);
    // HEAD auf Datei-Routen verbraucht nichts
    let req = Request::builder()
        .method(Method::HEAD)
        .uri("/files/download/00000000-0000-0000-0000-000000000000")
        .header(header::HOST, HOST)
        .header(header::COOKIE, format!("__Host-kb_session={cookie}"))
        .body(Body::empty())
        .unwrap();
    let r = send(&srv.app, req).await;
    assert_eq!(r.status, StatusCode::METHOD_NOT_ALLOWED);
}
