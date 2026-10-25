//! Integrationstest für die DSGVO-Auskunft nach Art. 15 (Block 18).
//!
//! Prüft den read-only Export-Kern [`export_core`]:
//! - sammelt ausschließlich Daten **dieses** Kontakts (Fremd-Kontakt fehlt),
//! - schreibt ein ZIP mit `auskunft.pdf` + `auskunft.json` + `dokumente/`,
//! - legt die archivierte Original-Datei bei (Hash-re-verifiziert),
//! - lässt interne Notizen weg,
//! - protokolliert die Erstellung genau einmal im append-only `audit_log`.

use std::io::Read;
use std::path::PathBuf;
use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

use klein_buch_lib::archive::{store_bytes, ArchiveKind};
use klein_buch_lib::commands::dsgvo::export_core;
use klein_buch_lib::config::Paths;
use klein_buch_lib::db::repo::{contacts, seller_profile};
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::domain::contact::{ContactInput, ContactType};

struct Env {
    pool: SqlitePool,
    paths: Paths,
    _tmp: tempfile::TempDir,
}

async fn setup_env() -> Env {
    let tmp = tempfile::tempdir().expect("tempdir");
    let data_dir = tmp.path().join("data");
    let archive_dir = data_dir.join("archive");
    let backups_dir = data_dir.join("backups");
    let inputs_dir = tmp.path().join("inputs");
    let db_file = data_dir.join("test.sqlite");
    std::fs::create_dir_all(&archive_dir).unwrap();
    std::fs::create_dir_all(&backups_dir).unwrap();
    std::fs::create_dir_all(&inputs_dir).unwrap();

    let url = format!("sqlite://{}", db_file.to_string_lossy());
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

    seller_profile::upsert(
        &pool,
        &seller_profile::SellerProfileInput {
            name: "Beispiel IT-Service".into(),
            legal_form: None,
            street: "Beispielweg 1".into(),
            postal_code: "84028".into(),
            city: "Landshut".into(),
            country_code: "DE".into(),
            tax_number: Some("123/456/78901".into()),
            vat_id: None,
            email: "inhaber@example.org".into(),
            phone: Some("+49 000 000000".into()),
            iban: None,
            bic: None,
            logo_filename: None,
            is_kleinunternehmer: true,
            default_pdf_template: Some("default".into()),
            default_currency: Some("EUR".into()),
            confirm_waive_paragraph_19: Some(true),
        },
    )
    .await
    .unwrap();

    let paths = Paths {
        data_dir,
        db_file,
        archive_dir,
        backups_dir,
        inputs_dir,
        sidecar_dir: PathBuf::from("/non/existent/sidecar"),
        resource_dir: std::path::PathBuf::from("/non/existent/resources"),
        web_dir: std::path::PathBuf::from("/non/existent/web"),
        backup_targets_dir: std::path::PathBuf::from("/backup-targets"),
    };

    Env {
        pool,
        paths,
        _tmp: tmp,
    }
}

async fn create_contact(
    pool: &SqlitePool,
    name: &str,
    email: Option<&str>,
    notes: Option<&str>,
) -> String {
    contacts::create(
        pool,
        &ContactInput {
            contact_type: ContactType::Customer,
            name: name.into(),
            legal_form: None,
            vat_id: Some("DE123456789".into()),
            tax_number: None,
            street: "Hauptstr. 7".into(),
            postal_code: "80331".into(),
            city: "München".into(),
            country_code: "DE".into(),
            email: email.map(str::to_string),
            phone: None,
            iban: None,
            bic: None,
            accepts_einvoice: true,
            notes: notes.map(str::to_string),
        },
    )
    .await
    .unwrap()
    .id
}

#[allow(clippy::too_many_arguments)]
async fn insert_invoice(
    pool: &SqlitePool,
    id: &str,
    number: &str,
    contact_id: &str,
    pdf_archive_id: Option<&str>,
) {
    sqlx::query(
        "INSERT INTO invoices
            (id, invoice_number, fiscal_year, direction, invoice_date, contact_id,
             seller_name, seller_street, seller_postal_code, seller_city,
             net_amount_cents, gross_amount_cents, status, pdf_archive_id)
         VALUES (?, ?, 2026, 'issued', '2026-03-01', ?,
             'Beispiel IT', 'Beispielweg 1', '84028', 'Landshut',
             10000, 10000, 'paid', ?)",
    )
    .bind(id)
    .bind(number)
    .bind(contact_id)
    .bind(pdf_archive_id)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO invoice_items
            (id, invoice_id, position, description, quantity, unit_code,
             unit_price_cents, net_amount_cents, tax_rate_percent, tax_category_code)
         VALUES (?, ?, 1, 'Vor-Ort-Service', 2.0, 'HUR', 5000, 10000, 0.0, 'E')",
    )
    .bind(format!("{id}-it1"))
    .bind(id)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn export_creates_zip_with_pdf_json_and_bundles_originals_only_for_this_contact() {
    let env = setup_env().await;

    let target = create_contact(
        &env.pool,
        "Ziel Kunde",
        Some("ziel@example.org"),
        Some("INTERN: zahlt spät"),
    )
    .await;
    let other = create_contact(
        &env.pool,
        "Anderer Kunde",
        Some("anderer@example.org"),
        None,
    )
    .await;

    // Archivierte Rechnung-PDF (write-once) für den Ziel-Kontakt.
    let stored = store_bytes(
        &env.pool,
        &env.paths.archive_dir,
        2026,
        ArchiveKind::InvoicePdf,
        "RE-2026-0001.pdf",
        "application/pdf",
        b"%PDF-1.7 fake invoice",
    )
    .await
    .unwrap();

    insert_invoice(
        &env.pool,
        "i-target",
        "RE-2026-0001",
        &target,
        Some(stored.archive_id.as_str()),
    )
    .await;
    insert_invoice(&env.pool, "i-other", "RE-2026-9999", &other, None).await;

    // Versandprotokoll-Eintrag mit Bezug zur Ziel-Rechnung.
    sqlx::query(
        "INSERT INTO email_log
            (id, channel, related_kind, related_id, related_number, from_email, to_email, subject, status, attachment_count)
         VALUES ('m-1', 'graph', 'invoice', 'i-target', 'RE-2026-0001',
             'info@beispiel-it.example.org', 'ziel@example.org', 'Ihre Rechnung RE-2026-0001', 'success', 1)",
    )
    .execute(&env.pool)
    .await
    .unwrap();

    // Audit-Bezug auf die Ziel-Rechnung.
    sqlx::query(
        "INSERT INTO audit_log (action, entity_type, entity_id) VALUES ('invoice.lock', 'invoice', 'i-target')",
    )
    .execute(&env.pool)
    .await
    .unwrap();

    let res = export_core(
        &env.pool,
        &env.paths,
        &target,
        "2026-05-24 12:00",
        "2026-05-24",
    )
    .await
    .unwrap();

    assert_eq!(res.invoice_count, 1, "nur die Ziel-Rechnung");
    assert_eq!(res.document_count, 1);
    assert_eq!(res.bundled_document_count, 1, "Original-PDF muss beiliegen");
    assert_eq!(res.email_count, 1);
    assert_eq!(res.file_name, "auskunft-ziel-kunde-2026-05-24.zip");

    // ZIP öffnen + Inhalte prüfen.
    let file = std::fs::File::open(&res.zip_path).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    let names: Vec<String> = (0..zip.len())
        .map(|i| zip.by_index(i).unwrap().name().to_string())
        .collect();
    assert!(
        names.iter().any(|n| n == "auskunft.pdf"),
        "PDF fehlt: {names:?}"
    );
    assert!(
        names.iter().any(|n| n == "auskunft.json"),
        "JSON fehlt: {names:?}"
    );
    assert!(names.iter().any(|n| n == "LIESMICH.txt"));
    assert!(
        names.iter().any(|n| n.starts_with("dokumente/")),
        "Original-Datei fehlt im ZIP: {names:?}"
    );

    // PDF nicht leer.
    {
        let pdf = zip.by_name("auskunft.pdf").unwrap();
        assert!(pdf.size() > 0, "PDF ist leer");
    }

    // JSON inhaltlich prüfen.
    let mut json = String::new();
    zip.by_name("auskunft.json")
        .unwrap()
        .read_to_string(&mut json)
        .unwrap();
    assert!(json.contains("RE-2026-0001"), "Ziel-Rechnung fehlt");
    assert!(json.contains("Ziel Kunde"), "Subjekt-Name fehlt");
    assert!(
        !json.contains("RE-2026-9999"),
        "Fremd-Rechnung darf NICHT in der Auskunft erscheinen"
    );
    assert!(
        !json.contains("Anderer Kunde"),
        "Fremd-Kontakt darf NICHT erscheinen"
    );
    assert!(
        !json.contains("INTERN"),
        "interne Notizen dürfen nicht in der Auskunft stehen"
    );

    // Genau ein 'dsgvo.export'-Audit-Eintrag.
    let cnt: i64 = sqlx::query("SELECT COUNT(*) AS c FROM audit_log WHERE action = 'dsgvo.export'")
        .fetch_one(&env.pool)
        .await
        .unwrap()
        .get("c");
    assert_eq!(cnt, 1, "Erstellung muss genau einmal protokolliert sein");

    // R2-029: `details_json` des Audit-Eintrags darf KEIN PII enthalten —
    // weder den slugifizierten Kontakt-Namen ("ziel-kunde") noch das
    // Dateinamen-Prefix ("auskunft-"). Der Eintrag identifiziert den Export
    // ausschließlich über `entity_id` (Kontakt-UUID) + `timestamp_utc`.
    let details: Option<String> =
        sqlx::query("SELECT details_json FROM audit_log WHERE action = 'dsgvo.export'")
            .fetch_one(&env.pool)
            .await
            .unwrap()
            .get("details_json");
    let details = details.unwrap_or_default();
    assert!(
        !details.contains("auskunft-"),
        "Audit-Detail darf den auskunft-<slug>-Dateinamen nicht enthalten (PII-Leak): {details}"
    );
    assert!(
        !details.to_lowercase().contains("ziel-kunde"),
        "Audit-Detail darf den slugifizierten Kontakt-Namen nicht enthalten: {details}"
    );
}

#[tokio::test]
async fn export_unknown_contact_errors() {
    let env = setup_env().await;
    let res = export_core(
        &env.pool,
        &env.paths,
        "no-such-id",
        "2026-05-24 12:00",
        "2026-05-24",
    )
    .await;
    assert!(res.is_err(), "unbekannter Kontakt muss Fehler liefern");
}

/// Lauf F (KB-0049, R2-S2-DSGVO, R1-S2-NPLUS1): Versandprotokoll über die
/// aktuelle Adresse, frühere Adressen aus den Empfänger-Snapshots der Belege
/// und den Kontakt selbst (Katalogversand) — unabhängig von Groß-/
/// Kleinschreibung; Audit-Bezüge nur mit passendem Entitätstyp; Positionen
/// mehrerer Belege richtig zugeordnet.
#[tokio::test]
async fn gather_matches_old_addresses_case_insensitively_and_filters_audit_by_type() {
    use klein_buch_lib::db::repo::dsgvo;
    let env = setup_env().await;
    let target = create_contact(&env.pool, "Ziel Kunde", Some("neu@example.org"), None).await;
    let other = create_contact(&env.pool, "Anderer", Some("fremd@example.org"), None).await;
    insert_invoice(&env.pool, "i-1", "RE-2026-0001", &target, None).await;
    insert_invoice(&env.pool, "i-2", "RE-2026-0002", &target, None).await;
    insert_invoice(&env.pool, "i-x", "RE-2026-0099", &other, None).await;
    // Früherer Empfänger-Stand auf der ersten Rechnung (andere Schreibweise).
    sqlx::query("UPDATE invoices SET buyer_email = 'Alt@Example.ORG' WHERE id = 'i-1'")
        .execute(&env.pool)
        .await
        .unwrap();
    let mail = |id: &str, kind: &str, related: Option<&str>, to: &str| {
        sqlx::query(
            "INSERT INTO email_log
                (id, channel, related_kind, related_id, from_email, to_email, subject, status)
             VALUES (?, 'smtp', ?, ?, 'info@example.org', ?, 'Betreff', 'success')",
        )
        .bind(id.to_string())
        .bind(kind.to_string())
        .bind(related.map(str::to_string))
        .bind(to.to_string())
    };
    for q in [
        mail("m-old", "test", None, " ALT@example.org "),
        mail("m-cur", "test", None, "NEU@EXAMPLE.ORG"),
        mail(
            "m-cat",
            "package_catalog",
            Some(&target),
            "einkauf@example.net",
        ),
        mail("m-inv", "invoice", Some("i-2"), "buchhaltung@example.net"),
        mail("m-foreign", "test", None, "fremd@example.org"),
        mail("m-foreign-inv", "invoice", Some("i-x"), "fremd@example.org"),
    ] {
        q.execute(&env.pool).await.unwrap();
    }
    for (action, etype, eid) in [
        ("invoice.lock", "invoice", "i-1"),
        ("contact.anonymize", "contact", target.as_str()),
        // gleiche ID, anderer Typ → kein Bezug
        ("backup.x", "backup", "i-1"),
        (
            "recurring_invoice.run",
            "recurring_invoice",
            target.as_str(),
        ),
        ("invoice.lock", "invoice", "i-x"),
    ] {
        sqlx::query("INSERT INTO audit_log (action, entity_type, entity_id) VALUES (?, ?, ?)")
            .bind(action)
            .bind(etype)
            .bind(eid)
            .execute(&env.pool)
            .await
            .unwrap();
    }

    let g = dsgvo::gather(&env.pool, &target).await.unwrap().unwrap();
    let mut mails: Vec<&str> = g.emails.iter().map(|e| e.id.as_str()).collect();
    mails.sort_unstable();
    assert_eq!(mails, vec!["m-cat", "m-cur", "m-inv", "m-old"]);
    let audit: Vec<(String, Option<String>)> = g
        .audit
        .iter()
        .map(|a| (a.action.clone(), a.entity_type.clone()))
        .collect();
    assert_eq!(
        audit,
        vec![
            ("invoice.lock".to_string(), Some("invoice".to_string())),
            ("contact.anonymize".to_string(), Some("contact".to_string())),
        ]
    );
    assert_eq!(g.invoices.len(), 2);
    for (inv, items) in &g.invoices {
        assert_eq!(items.len(), 1, "{}", inv.id);
        assert_eq!(items[0].invoice_id, inv.id);
    }
}

/// Lauf F2c: Die Auskunft enthält Abo-Rechnungs-Vorlagen (mit Positionen),
/// Kosten-Abos und Anlagen des Kontakts samt deren Audit-Bezügen; ein
/// verworfener Entwurf steht als `voided` mit Zeitpunkt und Grund, nicht als
/// offener Entwurf. Fremde Vorlagen und interne Notizen fehlen; das PDF
/// rendert mit den neuen Abschnitten.
#[tokio::test]
async fn export_includes_templates_subscriptions_assets_and_voided_drafts() {
    let env = setup_env().await;
    let pool = &env.pool;
    let target = create_contact(pool, "Ziel Kunde", Some("ziel@example.org"), None).await;
    let other = create_contact(pool, "Anderer Kunde", Some("anderer@example.org"), None).await;

    for (id, label, contact, notes) in [
        (
            "rt-1",
            "Wartung Ziel",
            target.as_str(),
            Some("INTERN: Rabatt"),
        ),
        ("rt-2", "Fremdvorlage", other.as_str(), None),
    ] {
        sqlx::query(
            "INSERT INTO recurring_invoices
                (id, label, contact_id, frequency, day_of_period, next_due_date, notes)
             VALUES (?, ?, ?, 'monthly', 1, '2026-06-01', ?)",
        )
        .bind(id)
        .bind(label)
        .bind(contact)
        .bind(notes)
        .execute(pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO recurring_invoice_items
            (id, recurring_invoice_id, position, description, quantity, unit_price_cents,
             net_amount_cents)
         VALUES ('rti-1', 'rt-1', 1, 'Server-Wartung', 1.0, 4900, 4900)",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO recurring_subscriptions
            (id, label, vendor_contact_id, frequency, day_of_period, next_due_date,
             expected_amount_cents, category, description_template)
         VALUES ('rs-1', 'Hosting beim Ziel', ?, 'monthly', 5, '2026-06-05', 1500,
                 'software', 'Hosting')",
    )
    .bind(&target)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO assets
            (id, asset_number, label, acquisition_date, acquisition_cost_cents,
             acquisition_fiscal_year, vendor_contact_id, depreciation_method,
             useful_life_years, book_value_cents, notes)
         VALUES ('a-1', 'AV-2026-0001', 'Laptop vom Ziel', '2026-02-01', 120000, 2026, ?,
                 'linear', 3.0, 120000, 'INTERN: gebraucht')",
    )
    .bind(&target)
    .execute(pool)
    .await
    .unwrap();
    // verworfener Entwurf
    sqlx::query(
        "INSERT INTO invoices
            (id, invoice_number, fiscal_year, direction, invoice_date, contact_id,
             seller_name, seller_street, seller_postal_code, seller_city,
             net_amount_cents, gross_amount_cents, status, buyer_name)
         VALUES ('i-void', 'RE-2026-0002', 2026, 'issued', '2026-03-05', ?,
             'Beispiel IT', 'Beispielweg 1', '84028', 'Landshut', 5000, 5000, 'draft', 'Ziel Kunde')",
    )
    .bind(&target)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE invoices SET voided_at = '2026-03-06 08:00:00',
                void_reason = 'Doppelt angelegt, neu erstellt' WHERE id = 'i-void'",
    )
    .execute(pool)
    .await
    .unwrap();
    for (action, et, eid) in [
        ("recurring_invoice.update", "recurring_invoice", "rt-1"),
        ("recurring.update", "recurring", "rs-1"),
        ("asset.update", "asset", "a-1"),
        ("recurring_invoice.update", "recurring_invoice", "rt-2"),
        ("asset.update", "invoice", "a-1"),
    ] {
        sqlx::query("INSERT INTO audit_log (action, entity_type, entity_id) VALUES (?, ?, ?)")
            .bind(action)
            .bind(et)
            .bind(eid)
            .execute(pool)
            .await
            .unwrap();
    }

    let res = export_core(pool, &env.paths, &target, "2026-05-24 12:00", "2026-05-24")
        .await
        .unwrap();
    let file = std::fs::File::open(&res.zip_path).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    assert!(zip.by_name("auskunft.pdf").unwrap().size() > 0);
    let mut json = String::new();
    zip.by_name("auskunft.json")
        .unwrap()
        .read_to_string(&mut json)
        .unwrap();
    let report: serde_json::Value = serde_json::from_str(&json).unwrap();

    let templates = report["recurringInvoices"].as_array().unwrap();
    assert_eq!(templates.len(), 1, "nur die Vorlage dieses Kontakts");
    assert_eq!(templates[0]["label"], "Wartung Ziel");
    assert_eq!(templates[0]["items"][0]["description"], "Server-Wartung");
    assert_eq!(report["subscriptions"][0]["label"], "Hosting beim Ziel");
    assert_eq!(report["subscriptions"].as_array().unwrap().len(), 1);
    assert_eq!(report["assets"][0]["assetNumber"], "AV-2026-0001");
    assert!(!json.contains("Fremdvorlage"), "fremde Vorlage");
    assert!(!json.contains("INTERN"), "interne Notizen");

    let voided = report["invoices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["invoiceNumber"] == "RE-2026-0002")
        .expect("verworfener Entwurf in der Auskunft");
    assert_eq!(voided["status"], "voided");
    assert_eq!(voided["voidedAt"], "2026-03-06 08:00:00");
    assert_eq!(voided["voidReason"], "Doppelt angelegt, neu erstellt");

    let audit: Vec<(String, String)> = report["auditEvents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| {
            (
                a["entityType"].as_str().unwrap_or_default().to_string(),
                a["entityId"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    for want in [
        ("recurring_invoice", "rt-1"),
        ("recurring", "rs-1"),
        ("asset", "a-1"),
    ] {
        assert!(
            audit.iter().any(|(t, i)| t == want.0 && i == want.1),
            "Audit-Bezug {want:?} fehlt: {audit:?}"
        );
    }
    assert!(
        !audit.iter().any(|(_, i)| i == "rt-2"),
        "fremde Vorlage im Audit"
    );
    assert!(
        !audit.iter().any(|(t, i)| t == "invoice" && i == "a-1"),
        "nur passendes Paar Typ/ID"
    );

    let details: String =
        sqlx::query_scalar("SELECT details_json FROM audit_log WHERE action = 'dsgvo.export'")
            .fetch_one(pool)
            .await
            .unwrap();
    let details: serde_json::Value = serde_json::from_str(&details).unwrap();
    assert_eq!(details["recurringInvoices"], 1);
    assert_eq!(details["subscriptions"], 1);
    assert_eq!(details["assets"], 1);
}
