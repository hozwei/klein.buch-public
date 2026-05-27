//! Integration-Tests für den Roh-XML-Viewer (PV1-A5).
//!
//! Testet [`klein_buch_lib::commands::expenses::load_receipt_xml`] direkt
//! über die zugrundeliegende Pipeline (Expenses-Repo + Archive). Die
//! Tauri-Command-Hülle (`expenses_receipt_xml_text`) hängt nur den realen
//! Mustang-Sidecar ein und ist ohne `AppHandle` nicht aufrufbar; deshalb
//! injizieren die Tests die PDF→XML-Extraktion als Closure und umgehen so
//! den Java-Sidecar (kein heavy JRE in CI).
//!
//! PRD R7.3.2 Done-Checks:
//! - `reads_archived_xml_for_received_einvoice_expense`
//! - `extracts_xml_from_zugferd_pdf_expense`
//! - `returns_none_for_expense_without_archive`
//! - `returns_none_for_non_einvoice_source_format`

use chrono::NaiveDate;
use klein_buch_lib::archive::{store_bytes, ArchiveKind};
use klein_buch_lib::commands::expenses::{load_receipt_xml, XmlViewerPayload};
use klein_buch_lib::db::repo::expenses;
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::expense::ExpenseInput;
use klein_buch_lib::error::Error;
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::str::FromStr;

// =============================================================================
// Setup
// =============================================================================

async fn setup_pool() -> (SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("test.sqlite");
    let url = format!("sqlite://{}", db_path.to_string_lossy());
    let opts = SqliteConnectOptions::from_str(&url)
        .unwrap()
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(opts)
        .await
        .unwrap();
    MIGRATOR.run(&pool).await.unwrap();
    (pool, dir)
}

fn archive_root(dir: &tempfile::TempDir) -> PathBuf {
    // Komponenten-weise statt String-Join — Windows-Pfad-Separator-Safe
    // (Memory `feedback_windows_path_separator_in_tests`).
    let mut p = dir.path().to_path_buf();
    p.push("archive");
    p
}

fn expense_input() -> ExpenseInput {
    ExpenseInput {
        expense_date: NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
        paid_date: None,
        paid_from_account_id: None,
        vendor_contact_id: None,
        vendor_name: "Lieferant GmbH".into(),
        vendor_invoice_number: Some("LF-2026-0042".into()),
        category: "other".into(),
        description: "Empfangene E-Rechnung".into(),
        net_amount_cents: 10_000,
        tax_amount_cents: 1_900,
        gross_amount_cents: 11_900,
        currency_code: "EUR".into(),
        reverse_charge_13b: false,
        notes: None,
    }
}

/// Minimal-CII-XRechnung als realistische Fixture. Roh, ohne KoSIT-Konformität —
/// für den Viewer-Pfad reicht ein erkennbares Root-Element.
const SAMPLE_CII_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rsm:CrossIndustryInvoice xmlns:rsm="urn:un:unece:uncefact:data:standard:CrossIndustryInvoice:100">
  <rsm:ExchangedDocument>
    <ram:ID xmlns:ram="urn:un:unece:uncefact:data:standard:ReusableAggregateBusinessInformationEntity:100">LF-2026-0042</ram:ID>
  </rsm:ExchangedDocument>
</rsm:CrossIndustryInvoice>
"#;

/// Minimal-UBL-XRechnung — Root-Element `Invoice` ohne CII-Namespace; deckt den
/// `xrechnung-ubl`-Branch von [`load_receipt_xml`] ab.
const SAMPLE_UBL_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Invoice xmlns="urn:oasis:names:specification:ubl:schema:xsd:Invoice-2">
  <cbc:ID xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2">LF-2026-0042</cbc:ID>
</Invoice>
"#;

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let digest = h.finalize();
    let mut s = String::with_capacity(digest.len() * 2);
    for b in digest {
        use std::fmt::Write;
        write!(&mut s, "{b:02x}").unwrap();
    }
    s
}

// PDF→XML-Extractor-Closures werden direkt an [`load_receipt_xml`]-Aufrufe
// inline-übergeben (statt durch eine Helfer-Funktion zurückgeliefert, die wegen
// `impl FnOnce -> Future` einen hässlichen Pin<Box<…>>-Typ verlangen würde).
// `unreachable_pdf` panics, wenn der XML-Pfad ihn dennoch ruft — Regression-Trap.
// Param-less, damit kein `clippy::needless_pass_by_value` auf einem nie
// konsumierten `Vec<u8>` triggert; die Closure droppt den eigenen Param.
fn unreachable_pdf() -> klein_buch_lib::error::Result<String> {
    panic!("PDF-Extractor wurde gerufen, obwohl Beleg eine XML-Rechnung ist");
}

// =============================================================================
// Tests
// =============================================================================

#[tokio::test]
async fn reads_archived_xml_for_received_einvoice_expense() {
    let (pool, dir) = setup_pool().await;
    let bytes = SAMPLE_CII_XML.as_bytes();
    let expected_hash = sha256_hex(bytes);

    let stored = store_bytes(
        &pool,
        &archive_root(&dir),
        2026,
        ArchiveKind::ReceivedEinvoice,
        "KO-2026-0001-LF-2026-0042.xml",
        "application/xml",
        bytes,
    )
    .await
    .unwrap();

    let row = expenses::create(
        &pool,
        &expense_input(),
        "KO-2026-0001",
        2026,
        Some(&stored.archive_id),
    )
    .await
    .unwrap();

    let payload: Option<XmlViewerPayload> =
        load_receipt_xml(&pool, &row.id, |_bytes| async move { unreachable_pdf() })
            .await
            .unwrap();

    let payload = payload.expect("Empfangene E-Rechnung muss XML liefern");
    assert_eq!(payload.source_format, "xrechnung-cii");
    assert_eq!(
        payload.xml.trim_start(),
        SAMPLE_CII_XML.trim_start(),
        "Roh-XML wird unverändert ausgeliefert"
    );
    assert_eq!(
        payload.sha256_hex, expected_hash,
        "SHA-256 stimmt mit der archivierten Datei überein"
    );
    assert_eq!(
        payload.byte_size as usize,
        bytes.len(),
        "byte_size = Größe des archivierten Originals"
    );
}

#[tokio::test]
async fn detects_ubl_syntax_for_oasis_xml() {
    // Komplement zu den drei Spec-Tests: stellt sicher, dass die feine
    // CII/UBL-Unterscheidung im Viewer-Command sauber greift (Spec-Reihe
    // `xrechnung-ubl`).
    let (pool, dir) = setup_pool().await;
    let stored = store_bytes(
        &pool,
        &archive_root(&dir),
        2026,
        ArchiveKind::ReceivedEinvoice,
        "KO-2026-0002-UBL.xml",
        "application/xml",
        SAMPLE_UBL_XML.as_bytes(),
    )
    .await
    .unwrap();
    let row = expenses::create(
        &pool,
        &expense_input(),
        "KO-2026-0002",
        2026,
        Some(&stored.archive_id),
    )
    .await
    .unwrap();

    let payload = load_receipt_xml(&pool, &row.id, |_bytes| async move { unreachable_pdf() })
        .await
        .unwrap()
        .expect("UBL-Rechnung muss Payload liefern");
    assert_eq!(payload.source_format, "xrechnung-ubl");
}

#[tokio::test]
async fn extracts_xml_from_zugferd_pdf_expense() {
    let (pool, dir) = setup_pool().await;

    // Wir tun so, als läge ein ZUGFeRD-PDF im Archiv. Inhalt ist ein gültiger
    // PDF-Header — der Mustang-Bridge-Aufruf wird im Test durch die Closure
    // ersetzt und nimmt die Bytes nicht ernst.
    let fake_pdf: &[u8] = b"%PDF-1.7\n%fake-zugferd-bytes\n";
    let expected_hash = sha256_hex(fake_pdf);

    let stored = store_bytes(
        &pool,
        &archive_root(&dir),
        2026,
        ArchiveKind::ReceivedEinvoice,
        "KO-2026-0001-LF-2026-0042.pdf",
        "application/pdf",
        fake_pdf,
    )
    .await
    .unwrap();
    let row = expenses::create(
        &pool,
        &expense_input(),
        "KO-2026-0001",
        2026,
        Some(&stored.archive_id),
    )
    .await
    .unwrap();

    let payload = load_receipt_xml(&pool, &row.id, |_bytes| async {
        Ok(SAMPLE_CII_XML.to_string())
    })
    .await
    .unwrap()
    .expect("ZUGFeRD-Beleg muss extrahiertes XML liefern");

    assert_eq!(payload.source_format, "zugferd");
    assert_eq!(
        payload.xml.trim(),
        SAMPLE_CII_XML.trim(),
        "Fixture-XML der Mustang-Closure landet 1:1 im Payload"
    );
    assert_eq!(
        payload.sha256_hex, expected_hash,
        "Hash bezieht sich aufs archivierte PDF, nicht aufs extrahierte XML"
    );
    assert_eq!(payload.byte_size as usize, fake_pdf.len());
}

#[tokio::test]
async fn returns_none_for_expense_without_archive() {
    let (pool, _dir) = setup_pool().await;
    let row = expenses::create(&pool, &expense_input(), "KO-2026-0001", 2026, None)
        .await
        .unwrap();

    let payload = load_receipt_xml(&pool, &row.id, |_bytes| async move { unreachable_pdf() })
        .await
        .unwrap();
    assert!(
        payload.is_none(),
        "Kosten ohne receipt_archive_id → Ok(None), kein Sidecar-Call"
    );
}

#[tokio::test]
async fn returns_none_for_non_einvoice_source_format() {
    // Beleg-Original existiert (z. B. Foto vom Tankbeleg), aber Archive-Kind
    // ist `ExpenseOriginal`, nicht `ReceivedEinvoice` — der Viewer muss
    // `Ok(None)` liefern, kein Inhalt, kein Sidecar-Call.
    let (pool, dir) = setup_pool().await;
    let stored = store_bytes(
        &pool,
        &archive_root(&dir),
        2026,
        ArchiveKind::ExpenseOriginal,
        "KO-2026-0001-tankbeleg.pdf",
        "application/pdf",
        b"%PDF-1.7\n%scan\n",
    )
    .await
    .unwrap();
    let row = expenses::create(
        &pool,
        &expense_input(),
        "KO-2026-0001",
        2026,
        Some(&stored.archive_id),
    )
    .await
    .unwrap();

    let payload = load_receipt_xml(&pool, &row.id, |_bytes| async move { unreachable_pdf() })
        .await
        .unwrap();
    assert!(
        payload.is_none(),
        "Archive-Eintrag mit source != received_einvoice → Ok(None)"
    );
}

#[tokio::test]
async fn detects_tamper_on_archive_modification() {
    // PV1-A5 leitet die Tamper-Detection an `archive::read_and_verify_silent`
    // weiter. Hier prüfen wir, dass ein manipuliertes Archive-File als
    // Domain-Error durchschlägt — das Frontend zeigt darauf einen Toast.
    let (pool, dir) = setup_pool().await;
    let stored = store_bytes(
        &pool,
        &archive_root(&dir),
        2026,
        ArchiveKind::ReceivedEinvoice,
        "KO-2026-0001-cii.xml",
        "application/xml",
        SAMPLE_CII_XML.as_bytes(),
    )
    .await
    .unwrap();
    let row = expenses::create(
        &pool,
        &expense_input(),
        "KO-2026-0001",
        2026,
        Some(&stored.archive_id),
    )
    .await
    .unwrap();

    // Read-only-Flag entfernen + Inhalt verändern.
    let mut perms = std::fs::metadata(&stored.file_path).unwrap().permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(0o600);
    }
    #[cfg(windows)]
    {
        // Tamper-Test will das Archive bewusst beschreibbar machen — clippy-Lint
        // (world-writable auf Unix) ist hier durch cfg(windows) eingerahmt.
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
    }
    std::fs::set_permissions(&stored.file_path, perms).unwrap();
    let tampered = b"<rsm:CrossIndustryInvoice>tampered</rsm:CrossIndustryInvoice>";
    std::fs::write(&stored.file_path, tampered).unwrap();

    let err = load_receipt_xml(&pool, &row.id, |_bytes| async move { unreachable_pdf() })
        .await
        .expect_err("manipuliertes Archive muss Fehler werfen");
    match err {
        Error::Domain(msg) => assert!(
            msg.contains("tamper"),
            "Domain-Error mit 'tamper'-Marker erwartet, got: {msg}"
        ),
        other => panic!("Unerwarteter Fehler-Typ: {other:?}"),
    }
}
