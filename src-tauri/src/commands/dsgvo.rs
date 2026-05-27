//! Tauri-Command für die **DSGVO-Auskunft nach Art. 15** (Block 18).
//!
//! Reiner **Lese-/Export-Pfad**: sammelt alle Daten zu einem Kontakt
//! ([`crate::db::repo::dsgvo`]), baut den Report ([`crate::domain::dsgvo`]) und
//! schreibt ein **Komplett-ZIP** nach `data/export/dsgvo/`:
//!
//! - `auskunft.pdf`   — lesbare Auskunft (Typst, Art. 15)
//! - `auskunft.json`  — maschinenlesbar inkl. SHA-256 (Art. 20)
//! - `dokumente/`     — die archivierten Original-Dateien (Manuel-Entscheidung „Komplett-ZIP"); jede wird beim Lesen Hash-re-verifiziert
//! - `LIESMICH.txt`   — Kurzbeschreibung + rechtlicher Hinweis
//!
//! Die Erstellung wird **prüfungssicher** im append-only `audit_log`
//! protokolliert (`action = 'dsgvo.export'`) — ohne personenbezogene Inhalte,
//! nur Kontakt-ID + Zählwerte. Keine DB-Mutation an Belegen, keine Migration.
//!
//! Testbarer Kern: [`export_core`] (nimmt `&Paths` statt `AppHandle`, wie
//! `send_catalog_core`). Der Command [`dsgvo_export`] ist nur die dünne
//! Tauri-/Opener-Hülle.

use chrono::Local;
use serde::Serialize;
use sqlx::SqlitePool;
use std::io::Write;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::config::Paths;
use crate::db::repo::{audit_log, dsgvo as dsgvo_repo, seller_profile};
use crate::domain::dsgvo::{self, ControllerInfo, RawData, RawInvoice, RawQuote};
use crate::error::{Error, Result};
use crate::pdf::{templates, typst_render};

/// Ergebnis des Auskunft-Exports.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DsgvoExportResult {
    pub zip_path: String,
    pub file_name: String,
    pub invoice_count: usize,
    pub quote_count: usize,
    pub expense_count: usize,
    pub document_count: usize,
    pub bundled_document_count: usize,
    pub email_count: usize,
}

/// Baut die `ControllerInfo` (verantwortliche Stelle) aus dem Verkäuferprofil.
async fn build_controller(pool: &SqlitePool) -> Result<ControllerInfo> {
    let profile = seller_profile::get(pool).await?;
    Ok(match profile {
        Some(s) => ControllerInfo {
            name: s.name,
            address: format!("{}, {} {}", s.street, s.postal_code, s.city),
            tax_number: s.tax_number,
            vat_id: s.vat_id,
            email: Some(s.email),
            phone: s.phone,
        },
        None => ControllerInfo {
            name: String::new(),
            address: String::new(),
            tax_number: None,
            vat_id: None,
            email: None,
            phone: None,
        },
    })
}

/// Testbarer Kern: sammeln → Report → ZIP schreiben → Audit. Kein Opener, kein
/// AppHandle. `generated_at` (Anzeige) und `file_date` (Dateiname) kommen vom
/// Aufrufer, damit der Test deterministisch ist.
pub async fn export_core(
    pool: &SqlitePool,
    paths: &Paths,
    contact_id: &str,
    generated_at: &str,
    file_date: &str,
) -> Result<DsgvoExportResult> {
    let gathered = dsgvo_repo::gather(pool, contact_id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Kontakt nicht gefunden: {contact_id}")))?;

    let controller = build_controller(pool).await?;

    let invoices: Vec<RawInvoice> = gathered
        .invoices
        .iter()
        .map(|(inv, items)| RawInvoice {
            invoice: inv,
            items,
        })
        .collect();
    let quotes: Vec<RawQuote> = gathered
        .quotes
        .iter()
        .map(|(q, items)| RawQuote { quote: q, items })
        .collect();

    let raw = RawData {
        contact: &gathered.contact,
        invoices: &invoices,
        quotes: &quotes,
        expenses: &gathered.expenses,
        documents: &gathered.documents,
        emails: &gathered.emails,
        audit: &gathered.audit,
        controller,
        generated_at: generated_at.to_string(),
    };
    let mut report = dsgvo::assemble(&raw);

    // Zielpfad (Maschinen schreiben nur nach data/).
    let out_dir = paths.data_dir.join("export").join("dsgvo");
    std::fs::create_dir_all(&out_dir)?;
    let slug = slugify(&gathered.contact.name);
    let file_name = format!("auskunft-{slug}-{file_date}.zip");
    let zip_path = out_dir.join(&file_name);

    let file = std::fs::File::create(&zip_path)?;
    let mut zw = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // 1) Original-Dateien beilegen (Komplett-ZIP). Beim Lesen wird der SHA-256
    //    gegen den DB-Eintrag re-verifiziert (Tamper-Detection); schlägt das
    //    fehl, wird die Datei übersprungen und `bundled = false` markiert.
    let mut bundled_count = 0usize;
    for (i, d) in gathered.documents.iter().enumerate() {
        match crate::archive::read_and_verify(pool, &d.archive_id).await {
            Ok(bytes) => {
                let entry = format!(
                    "dokumente/{:02}-{}",
                    i + 1,
                    crate::commands::attachments::sanitize_filename(&d.file_name)
                );
                zw.start_file(entry, opts)?;
                zw.write_all(&bytes)?;
                report.documents[i].bundled = true;
                bundled_count += 1;
            }
            Err(e) => {
                tracing::warn!(
                    "DSGVO-Export: Dokument {} nicht beilegbar: {e}",
                    d.archive_id
                );
                report.documents[i].bundled = false;
            }
        }
    }

    // 2) PDF (lesbare Auskunft) — nutzt den final aktualisierten Report.
    let data_json = serde_json::to_string(&report)?;
    let source = templates::load_dsgvo_source(&paths.inputs_dir);
    let branding_dir = paths.inputs_dir.join("branding");
    let branding_opt = if branding_dir.is_dir() {
        Some(branding_dir.as_path())
    } else {
        None
    };
    let pdf_bytes = typst_render::render_pdf(&source, &data_json, branding_opt)?;
    zw.start_file("auskunft.pdf", opts)?;
    zw.write_all(&pdf_bytes)?;

    // 3) JSON (maschinenlesbar, Art. 20) — derselbe Report, pretty.
    let json_pretty = serde_json::to_vec_pretty(&report)?;
    zw.start_file("auskunft.json", opts)?;
    zw.write_all(&json_pretty)?;

    // 4) LIESMICH.
    zw.start_file("LIESMICH.txt", opts)?;
    zw.write_all(readme_text(&gathered.contact.name, generated_at).as_bytes())?;

    zw.finish()?;

    // R2-029: prüfungssicher protokollieren — nur Zählwerte + Kontakt-ID
    // (= Hash-Ersatz, da UUIDv7), **kein** Klarnamen-Slug. Der Datei-Name
    // enthielt früher den slugifizierten Kontakt-Namen → PII-Leak im append-
    // only audit_log. Datum + entity_id reichen, um den Export zu identifizieren.
    audit_log::append(
        pool,
        "dsgvo.export",
        "contact",
        contact_id,
        Some(&format!(
            r#"{{"invoices":{},"quotes":{},"expenses":{},"documents":{},"bundled":{},"emails":{},"generatedAt":"{}"}}"#,
            report.invoices.len(),
            report.quotes.len(),
            report.expenses.len(),
            report.documents.len(),
            bundled_count,
            report.emails.len(),
            generated_at,
        )),
    )
    .await?;

    Ok(DsgvoExportResult {
        zip_path: zip_path.to_string_lossy().to_string(),
        file_name,
        invoice_count: report.invoices.len(),
        quote_count: report.quotes.len(),
        expense_count: report.expenses.len(),
        document_count: report.documents.len(),
        bundled_document_count: bundled_count,
        email_count: report.emails.len(),
    })
}

/// Erzeugt die DSGVO-Auskunft (Art. 15) zu einem Kontakt als ZIP und öffnet den
/// Ablage-Ordner. Read-only.
#[tauri::command]
pub async fn dsgvo_export(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    contact_id: String,
) -> Result<DsgvoExportResult> {
    let paths = Paths::from_handle(&app)?;
    let now = Local::now();
    let generated_at = now.format("%Y-%m-%d %H:%M").to_string();
    let file_date = now.format("%Y-%m-%d").to_string();

    let res = export_core(pool.inner(), &paths, &contact_id, &generated_at, &file_date).await?;

    app.opener()
        .reveal_item_in_dir(&res.zip_path)
        .map_err(|e| Error::Other(anyhow::anyhow!("Ordner konnte nicht geöffnet werden: {e}")))?;
    Ok(res)
}

// ---- Helpers ----------------------------------------------------------------

/// Reduziert einen Namen auf einen dateinamen-sicheren Slug (ASCII, kebab).
fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    let base = if trimmed.is_empty() {
        "kontakt".to_string()
    } else {
        trimmed
    };
    base.chars().take(40).collect()
}

fn readme_text(name: &str, generated_at: &str) -> String {
    format!(
        "DSGVO-Auskunft nach Art. 15 DSGVO\n\
         =================================\n\n\
         Betroffene Person/Firma: {name}\n\
         Erstellt am: {generated_at} (Europe/Berlin)\n\n\
         Inhalt dieses Pakets:\n\
         - auskunft.pdf   Lesbare Auskunft (Art. 15 DSGVO).\n\
         - auskunft.json  Maschinenlesbare Fassung inkl. SHA-256-Prüfsummen (Art. 20 Datenübertragbarkeit).\n\
         - dokumente/     Beigefügte Originaldateien (Rechnungen, Belege, Anhänge), soweit vorhanden.\n\n\
         Hinweise:\n\
         - Diese Auskunft ist keine Rechtsberatung.\n\
         - Teile der Daten unterliegen gesetzlichen Aufbewahrungspflichten (u. a. § 147 AO, § 14b UStG, 10 Jahre).\n\
         - Angaben Dritter in beigefügten Dokumenten sind vor einer Herausgabe zu prüfen (Art. 15 Abs. 4 DSGVO).\n\
         - Erstellt mit Klein.Buch — einem Werkzeug, keinem Steuerberater.\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_handles_umlauts_and_punctuation() {
        assert_eq!(slugify("Müller & Söhne GmbH"), "m-ller-s-hne-gmbh");
        assert_eq!(slugify("   "), "kontakt");
        assert_eq!(slugify("ACME"), "acme");
    }
}
