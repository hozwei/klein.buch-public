//! Tauri-Commands für den Paket-Katalog (Block P2).
//!
//! Pakete sind **Stammdaten**, kein §14-Beleg → **kein** GoBD-Auto-Backup-Hook
//! (normaler Audit-Log-Eintrag genügt). Revisionen sind append-only (DB-Trigger);
//! „Bearbeiten" erzeugt eine neue Revision, „Rollback" ebenfalls.

use crate::config::Paths;
use crate::db::models::{PackageCategoryRow, PackageRevisionRow, PackageRow, SellerProfileRow};
use crate::db::repo::{audit_log, contacts, mail_accounts, packages, seller_profile};
use crate::domain::invoice::SellerView;
use crate::domain::kleinunternehmer;
use crate::domain::package::{self, PackageRevisionInput};
use crate::error::{Error, Result};
use crate::pdf::{templates, typst_render};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

/// §19-Hardline + Validierung: bei Kleinunternehmer wird die USt-Kategorie auf
/// `'E'` (Exempt) gezwungen; danach strukturelle Validierung. Preis bleibt Netto.
async fn prepare_revision(
    pool: &SqlitePool,
    mut input: PackageRevisionInput,
) -> Result<PackageRevisionInput> {
    if let Some(seller) = seller_profile::get(pool).await? {
        if seller.is_kleinunternehmer == 1 {
            input.tax_category_code = "E".to_string();
        }
    }
    let issues = package::validate_revision(&input);
    if !issues.is_empty() {
        return Err(Error::Domain(format!(
            "Paket-Angaben unvollständig: {}",
            issues
                .iter()
                .map(|i| i.message.clone())
                .collect::<Vec<_>>()
                .join(" ")
        )));
    }
    Ok(input)
}

// ---- Kategorien ------------------------------------------------------------

#[tauri::command]
pub async fn package_categories_list(
    pool: State<'_, SqlitePool>,
) -> Result<Vec<PackageCategoryRow>> {
    packages::categories_list(pool.inner()).await
}

#[tauri::command]
pub async fn package_category_create(
    pool: State<'_, SqlitePool>,
    name: String,
) -> Result<PackageCategoryRow> {
    if name.trim().is_empty() {
        return Err(Error::Domain(
            "Der Kategorie-Name darf nicht leer sein.".into(),
        ));
    }
    let row = packages::category_create(pool.inner(), name.trim()).await?;
    audit_log::append(
        pool.inner(),
        "package_category.create",
        "package_category",
        &row.id,
        None,
    )
    .await
    .ok();
    Ok(row)
}

#[tauri::command]
pub async fn package_category_update(
    pool: State<'_, SqlitePool>,
    id: String,
    name: String,
) -> Result<()> {
    if name.trim().is_empty() {
        return Err(Error::Domain(
            "Der Kategorie-Name darf nicht leer sein.".into(),
        ));
    }
    packages::category_update(pool.inner(), &id, name.trim()).await?;
    audit_log::append(
        pool.inner(),
        "package_category.update",
        "package_category",
        &id,
        None,
    )
    .await
    .ok();
    Ok(())
}

#[tauri::command]
pub async fn package_categories_reorder(
    pool: State<'_, SqlitePool>,
    ordered_ids: Vec<String>,
) -> Result<()> {
    packages::categories_reorder(pool.inner(), &ordered_ids).await
}

// ---- Pakete ----------------------------------------------------------------

#[tauri::command]
pub async fn packages_list(pool: State<'_, SqlitePool>) -> Result<Vec<PackageRow>> {
    packages::list(pool.inner()).await
}

#[tauri::command]
pub async fn packages_get(pool: State<'_, SqlitePool>, id: String) -> Result<Option<PackageRow>> {
    packages::get(pool.inner(), &id).await
}

#[tauri::command]
pub async fn packages_create(
    pool: State<'_, SqlitePool>,
    category_id: Option<String>,
    name: String,
    revision: PackageRevisionInput,
) -> Result<PackageRow> {
    let pool = pool.inner();
    if name.trim().is_empty() {
        return Err(Error::Domain("Der Paket-Name darf nicht leer sein.".into()));
    }
    let prepared = prepare_revision(pool, revision).await?;
    let row = packages::create(pool, category_id.as_deref(), name.trim(), &prepared).await?;
    audit_log::append(
        pool,
        "package.create",
        "package",
        &row.id,
        Some(&format!(r#"{{"name":"{}"}}"#, escape(name.trim()))),
    )
    .await
    .ok();
    Ok(row)
}

#[tauri::command]
pub async fn packages_update_as_new_revision(
    pool: State<'_, SqlitePool>,
    id: String,
    category_id: Option<String>,
    name: String,
    revision: PackageRevisionInput,
) -> Result<PackageRow> {
    let pool = pool.inner();
    if name.trim().is_empty() {
        return Err(Error::Domain("Der Paket-Name darf nicht leer sein.".into()));
    }
    let prepared = prepare_revision(pool, revision).await?;
    let row =
        packages::update_as_new_revision(pool, &id, category_id.as_deref(), name.trim(), &prepared)
            .await?;
    audit_log::append(
        pool,
        "package.new_revision",
        "package",
        &id,
        Some(&format!(
            r#"{{"revision":{}}}"#,
            row.current_revision.unwrap_or(0)
        )),
    )
    .await
    .ok();
    Ok(row)
}

#[tauri::command]
pub async fn packages_archive(pool: State<'_, SqlitePool>, id: String) -> Result<()> {
    packages::set_status(pool.inner(), &id, "archived").await?;
    // R2-026: Pflicht-Audit (Status-Wechsel auf Paket = fachlicher Vorgang).
    audit_log::append(pool.inner(), "package.archive", "package", &id, None).await?;
    Ok(())
}

#[tauri::command]
pub async fn packages_reactivate(pool: State<'_, SqlitePool>, id: String) -> Result<()> {
    packages::set_status(pool.inner(), &id, "active").await?;
    // R2-026: Pflicht-Audit (Status-Wechsel auf Paket = fachlicher Vorgang).
    audit_log::append(pool.inner(), "package.reactivate", "package", &id, None).await?;
    Ok(())
}

// ---- Revisionen ------------------------------------------------------------

#[tauri::command]
pub async fn package_revisions_list(
    pool: State<'_, SqlitePool>,
    package_id: String,
) -> Result<Vec<PackageRevisionRow>> {
    packages::revisions_list(pool.inner(), &package_id).await
}

#[tauri::command]
pub async fn package_revisions_get(
    pool: State<'_, SqlitePool>,
    package_id: String,
    revision: i64,
) -> Result<Option<PackageRevisionRow>> {
    packages::revision_get(pool.inner(), &package_id, revision).await
}

#[tauri::command]
pub async fn package_revisions_rollback(
    pool: State<'_, SqlitePool>,
    package_id: String,
    to_revision: i64,
) -> Result<PackageRow> {
    let pool = pool.inner();
    let row = packages::rollback(pool, &package_id, to_revision).await?;
    audit_log::append(
        pool,
        "package.rollback",
        "package",
        &package_id,
        Some(&format!(
            r#"{{"to_revision":{},"new_revision":{}}}"#,
            to_revision,
            row.current_revision.unwrap_or(0)
        )),
    )
    .await
    .ok();
    Ok(row)
}

// ---- Vorschau (PDF) --------------------------------------------------------

/// Rendert eine Beispiel-Position als PDF (Editor-„PDF-Vorschau"). Reine Vorschau:
/// schreibt nach `data/preview/`, nichts wird archiviert oder validiert. Der
/// formatierte Block kommt aus `domain::package::to_typst` und wird im Template
/// via `eval(mode: "markup")` eingebettet.
#[tauri::command]
pub async fn package_preview(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    title: String,
    body_markup: String,
    default_unit_price_cents: i64,
) -> Result<()> {
    let pool = pool.inner();
    let body_typst = package::to_typst(&package::parse_markup(&body_markup));
    let is_klein = seller_profile::get(pool)
        .await?
        .map(|s| s.is_kleinunternehmer == 1)
        .unwrap_or(true);
    let title_show = if title.trim().is_empty() {
        "(ohne Titel)"
    } else {
        title.trim()
    };
    let data = serde_json::json!({
        "title": title_show,
        "body_typst": body_typst,
        "price": fmt_eur(default_unit_price_cents),
        "is_klein": is_klein,
        "klausel": kleinunternehmer::hinweis_text(),
    });
    let json =
        serde_json::to_string(&data).map_err(|e| Error::Config(format!("Vorschau-JSON: {e}")))?;

    let paths = Paths::from_handle(&app)?;
    let branding_dir = paths.inputs_dir.join("branding");
    let branding_opt = if branding_dir.is_dir() {
        Some(branding_dir.as_path())
    } else {
        None
    };
    let pdf = typst_render::render_pdf(templates::PACKAGE_PREVIEW_TEMPLATE, &json, branding_opt)?;

    let dir = paths.data_dir.join("preview");
    std::fs::create_dir_all(&dir).map_err(|e| Error::Config(format!("Vorschau-Ordner: {e}")))?;
    let file = dir.join("paket-vorschau.pdf");
    std::fs::write(&file, &pdf).map_err(|e| Error::Config(format!("Vorschau schreiben: {e}")))?;
    app.opener()
        .open_path(file.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| Error::Config(format!("Vorschau öffnen: {e}")))?;
    Ok(())
}

// ---- Materialisierung in eine Beleg-Position (Block P3) ---------------------

/// Eine aus einem Paket materialisierte Beleg-Position. Reine Aufbereitung —
/// das Frontend übernimmt sie in die Positions-Liste (Angebot/Rechnung/
/// Umwandeln) und persistiert sie erst beim Speichern des Entwurfs.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterializedPackageItem {
    /// Klartext für XRechnung-BT-154 UND PDF-Fallback (= Klartext des Body,
    /// ohne Titel; Fallback Titel, falls Body leer).
    pub description: String,
    /// Positions-Titel — steht in der PDF-Zeile (Beschreibungs-Spalte), nicht im XML.
    pub description_title: String,
    /// Body-Markdown (ohne Titel) — treibt den formatierten PDF-Block.
    pub description_markup: String,
    pub quantity: f64,
    pub unit_code: String,
    pub unit_price_cents: i64,
    pub tax_rate_percent: f64,
    pub tax_category_code: String,
    pub source_package_id: String,
    pub source_package_revision: i64,
    /// Katalog-Name des Pakets (für das „aus Paket: … (V…)"-Badge im UI).
    pub package_name: String,
}

/// Materialisiert eine Beleg-Position aus der **aktiven** Revision eines Pakets
/// (Block P3). Reine Lese-/Aufbereitungs-Operation — schreibt nichts.
///
/// Semantik (Manuel-Entscheidung 2026-05-23 — „Titel + Body nur PDF, XML = Body"):
/// - `description`        = Klartext(Body) → XRechnung-BT-154 (ohne Titel).
///   Fallback = Titel, falls der Body leer ist (BT-154 darf nicht leer sein).
/// - `description_markup` = fetter Titel + Body-Markup → der formatierte PDF-Block.
/// - §19-Hardline: bei Kleinunternehmer `tax_category_code = 'E'`, Rate 0.
#[tauri::command]
pub async fn package_materialize_item(
    pool: State<'_, SqlitePool>,
    package_id: String,
) -> Result<MaterializedPackageItem> {
    let pool = pool.inner();
    let pkg = packages::get(pool, &package_id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Paket nicht gefunden: {package_id}")))?;
    let rev = packages::current_revision(pool, &package_id)
        .await?
        .ok_or_else(|| {
            Error::Domain("Dieses Paket hat noch keine veröffentlichte Revision.".into())
        })?;

    // XRechnung-BT-154 = Klartext des Body (ohne Titel); Fallback Titel, falls
    // der Body leer ist (BT-154 darf nicht leer sein). Der Body-Markup treibt
    // den PDF-Block; der Titel steht separat in der Positionszeile.
    let body_plain = package::to_plaintext(&package::parse_markup(&rev.body_markup));
    let description = if body_plain.trim().is_empty() {
        rev.title.clone()
    } else {
        body_plain
    };
    let description_markup = rev.body_markup.clone();

    // §19-Hardline: Kleinunternehmer → Category 'E'. Rate immer 0 (Pakete sind
    // Netto-Katalogpreise; Revisionen führen keinen USt-Satz).
    let is_klein = seller_profile::get(pool)
        .await?
        .map(|s| s.is_kleinunternehmer == 1)
        .unwrap_or(true);
    let tax_category_code = if is_klein {
        "E".to_string()
    } else {
        rev.tax_category_code.clone()
    };

    Ok(MaterializedPackageItem {
        description,
        description_title: rev.title.clone(),
        description_markup,
        quantity: 1.0,
        unit_code: rev.unit_code.clone(),
        unit_price_cents: rev.default_unit_price_cents,
        tax_rate_percent: 0.0,
        tax_category_code,
        source_package_id: package_id,
        source_package_revision: rev.revision,
        package_name: pkg.name,
    })
}

// ---- Paket-Katalog-Broschüre (Block P4) ------------------------------------
//
// KEIN §14-Beleg: render-on-demand aus den aktuellen Revisionen der gewählten
// Pakete, kein Nummernkreis, KEIN write-once-Archiv. Druck/Vorschau öffnet ein
// Plain-PDF; der Versand läuft über den bestehenden `send_and_log`-Pfad und wird
// ausschließlich im append-only `email_log` (`related_kind = 'package_catalog'`)
// protokolliert.

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendPackageCatalogArgs {
    pub account_id: String,
    pub contact_id: String,
    pub package_ids: Vec<String>,
    /// Optionaler Betreff-Override; sonst Standard-Betreff.
    pub subject: Option<String>,
    /// Optionaler Body-Override; sonst Standard-Text.
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendPackageCatalogResult {
    pub to: String,
    pub subject: String,
    pub package_count: u32,
    pub attachment_count: u32,
}

/// Baut einen `SellerView` aus dem Stammdaten-Row (geliehen) — wie
/// `commands::quotes::make_seller_view`, hier lokal für den Broschüren-Render.
fn seller_view_from_row(row: &SellerProfileRow) -> SellerView<'_> {
    SellerView {
        name: &row.name,
        street: &row.street,
        postal_code: &row.postal_code,
        city: &row.city,
        country_code: &row.country_code,
        tax_number: row.tax_number.as_deref(),
        vat_id: row.vat_id.as_deref(),
        email: &row.email,
        iban: row.iban.as_deref(),
        bic: row.bic.as_deref(),
        is_kleinunternehmer: row.is_kleinunternehmer == 1,
        waived_since: row
            .waived_paragraph_19_since
            .as_deref()
            .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()),
    }
}

/// Lädt die **aktiven** Revisionen der gewählten Pakete. Pakete ohne
/// veröffentlichte Revision werden übersprungen; ist am Ende nichts übrig,
/// ist das ein Fehler (leere Broschüre macht keinen Sinn).
async fn load_catalog_revisions(
    pool: &SqlitePool,
    package_ids: &[String],
) -> Result<Vec<PackageRevisionRow>> {
    let mut revs = Vec::new();
    for id in package_ids {
        if let Some(rev) = packages::current_revision(pool, id).await? {
            revs.push(rev);
        }
    }
    if revs.is_empty() {
        return Err(Error::Domain(
            "Bitte mindestens ein Paket mit veröffentlichter Version auswählen.".into(),
        ));
    }
    Ok(revs)
}

/// Rendert die Broschüre als PDF-Bytes (Logo + Firmen-Impressum + §19-Hinweis).
/// Gemeinsamer Kern für Druck/Vorschau und Versand. `contact_name` setzt die
/// optionale persönliche Anrede.
async fn render_catalog_pdf(
    pool: &SqlitePool,
    paths: &Paths,
    package_ids: &[String],
    contact_name: Option<&str>,
) -> Result<Vec<u8>> {
    let seller_row = seller_profile::get(pool).await?.ok_or_else(|| {
        Error::Domain("Firmendaten fehlen — bitte zuerst Stammdaten anlegen.".into())
    })?;
    let revs = load_catalog_revisions(pool, package_ids).await?;
    let entries: Vec<typst_render::CatalogEntry> = revs
        .iter()
        .map(|r| typst_render::CatalogEntry {
            title: &r.title,
            body_markup: &r.body_markup,
            price_cents: r.default_unit_price_cents,
        })
        .collect();
    let seller_view = seller_view_from_row(&seller_row);

    // Inhaber-Name (Einzelunternehmer §14) — fürs Impressum, leere Werte → None.
    let owner_name = crate::db::repo::app_settings::get(pool, "seller_owner_name")
        .await?
        .filter(|s| !s.trim().is_empty());

    // Branding: Fonts aus inputs/branding (optional), Logo aus data/branding.
    let branding_dir = paths.inputs_dir.join("branding");
    let branding_opt = if branding_dir.is_dir() {
        Some(branding_dir.as_path())
    } else {
        None
    };
    let logo = crate::branding::find_logo(&crate::branding::branding_dir(&paths.data_dir));
    let logo_vpath = logo.as_ref().map(|(name, _)| format!("/branding/{name}"));
    let logo_opt = match (logo_vpath.as_deref(), logo.as_ref()) {
        (Some(vp), Some((_, bytes))) => Some((vp, bytes.as_slice())),
        _ => None,
    };

    let generated_at = chrono::Local::now().date_naive().to_string();

    typst_render::render_package_catalog(
        templates::PACKAGE_CATALOG_TEMPLATE,
        &entries,
        &seller_view,
        contact_name,
        owner_name.as_deref(),
        &generated_at,
        branding_opt,
        logo_opt,
    )
}

/// Druck/Vorschau: rendert die Broschüre für die gewählten Pakete und öffnet sie.
/// Schreibt nach `data/preview/` (reine Vorschau, kein Archiv).
#[tauri::command]
pub async fn package_catalog_render(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    package_ids: Vec<String>,
) -> Result<()> {
    let pool = pool.inner();
    let paths = Paths::from_handle(&app)?;
    let pdf = render_catalog_pdf(pool, &paths, &package_ids, None).await?;

    let dir = paths.data_dir.join("preview");
    std::fs::create_dir_all(&dir).map_err(|e| Error::Config(format!("Broschüren-Ordner: {e}")))?;
    let file = dir.join("paket-broschuere.pdf");
    std::fs::write(&file, &pdf).map_err(|e| Error::Config(format!("Broschüre schreiben: {e}")))?;
    app.opener()
        .open_path(file.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| Error::Config(format!("Broschüre öffnen: {e}")))?;

    audit_log::append(
        pool,
        "package_catalog.render",
        "package_catalog",
        "catalog",
        Some(&format!(r#"{{"packages":{}}}"#, package_ids.len())),
    )
    .await
    .ok();
    Ok(())
}

/// Versand: rendert die Broschüre (mit persönlicher Anrede) und mailt sie als
/// **einzelnen** PDF-Anhang an den gewählten Kontakt. Protokoll ausschließlich
/// im append-only `email_log` (`related_kind = 'package_catalog'`) — KEIN
/// §14-Status, KEIN write-once-Archiv.
#[tauri::command]
pub async fn package_catalog_send(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    args: SendPackageCatalogArgs,
) -> Result<SendPackageCatalogResult> {
    let paths = Paths::from_handle(&app)?;
    send_catalog_core(pool.inner(), &paths, &args).await
}

/// Kern-Pipeline für den Broschüren-Versand (ohne Tauri-State). Public, damit
/// Integrationstests sie ohne `AppHandle` treiben können (Paths aus tempdir) —
/// analog `commands::mail::send_invoice_core`/`send_quote_core`. Rendert die
/// Broschüre, mailt sie als einzelnen PDF-Anhang und protokolliert den Versuch
/// genau einmal im append-only `email_log` (`related_kind = 'package_catalog'`).
pub async fn send_catalog_core(
    pool: &SqlitePool,
    paths: &Paths,
    args: &SendPackageCatalogArgs,
) -> Result<SendPackageCatalogResult> {
    let account = mail_accounts::get(pool, &args.account_id)
        .await?
        .ok_or_else(|| Error::Mail(format!("Mail-Account nicht gefunden: {}", args.account_id)))?;
    let contact = contacts::get(pool, &args.contact_id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Kontakt nicht gefunden: {}", args.contact_id)))?;
    let recipient = contact
        .email
        .as_deref()
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            Error::Domain("Beim gewählten Kontakt ist keine E-Mail hinterlegt.".into())
        })?;

    let pdf =
        render_catalog_pdf(pool, paths, &args.package_ids, Some(contact.name.as_str())).await?;
    let package_count = load_catalog_revisions(pool, &args.package_ids).await?.len() as u32;

    let subject = args
        .subject
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "Unsere Leistungspakete".to_string());
    let body = args
        .body
        .as_deref()
        .map(str::trim)
        .filter(|b| !b.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            format!(
                "Guten Tag {},\n\nanbei unsere aktuelle Paket-Übersicht als PDF.\n\n\
                 Bei Fragen oder für ein individuelles Angebot melden Sie sich gern.\n\n\
                 Mit freundlichen Grüßen\n{}",
                contact.name, account.from_name
            )
        });

    let mail = crate::mail::smtp::OutgoingMail {
        from_name: account.from_name.clone(),
        from_email: account.from_email.clone(),
        to: recipient.clone(),
        subject: subject.clone(),
        body_text: body,
        attachments: vec![crate::mail::smtp::MailAttachment {
            filename: "Paket-Uebersicht.pdf".to_string(),
            mime_type: "application/pdf".to_string(),
            bytes: pdf,
        }],
    };

    // Versand + append-only Protokoll-Eintrag (kein Beleg-Bezug → related_id None).
    crate::commands::mail::send_and_log(
        pool,
        &account,
        &mail,
        crate::commands::mail::SendContext {
            related_kind: "package_catalog",
            related_id: None,
            related_number: None,
        },
    )
    .await?;
    mail_accounts::touch_last_used(pool, &args.account_id)
        .await
        .ok();

    audit_log::append(
        pool,
        "package_catalog.sent",
        "package_catalog",
        "catalog",
        Some(&format!(
            r#"{{"account":"{}","to":"{}","packages":{}}}"#,
            escape(&args.account_id),
            escape(&recipient),
            package_count
        )),
    )
    .await
    .ok();

    Ok(SendPackageCatalogResult {
        to: recipient,
        subject,
        package_count,
        attachment_count: 1,
    })
}

/// Netto-Cent → „1.234,56 €" (deutsches Format, ohne Tausenderpunkt für die Vorschau).
fn fmt_eur(cents: i64) -> String {
    let euros = cents / 100;
    let rem = (cents % 100).abs();
    format!("{euros},{rem:02} €")
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
