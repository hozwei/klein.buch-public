//! Commands für den Paket-Katalog (Block P2).
//!
//! Pakete sind **Stammdaten**, kein §14-Beleg → **kein** GoBD-Auto-Backup-Hook
//! (normaler Audit-Log-Eintrag genügt). Revisionen sind append-only (DB-Trigger);
//! „Bearbeiten" erzeugt eine neue Revision, „Rollback" ebenfalls.

use crate::app_ctx::AppCtx;
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
use std::sync::Arc;

/// §19-Hardline + Validierung: gilt heute §19, wird die USt-Kategorie auf `'E'`
/// (Exempt) gezwungen; danach strukturelle Validierung. Preis bleibt Netto. Die
/// Steuerart einer eingefügten Position bestimmt der Status am Datum des
/// Zielbelegs ([`package_tax_defaults`], ADR 0042 Nr. 3).
async fn prepare_revision(
    pool: &SqlitePool,
    mut input: PackageRevisionInput,
) -> Result<PackageRevisionInput> {
    if crate::db::repo::tax_status::is_klein_at(pool, crate::clock::today_berlin()).await? {
        input.tax_category_code = "E".to_string();
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

// ---- Provenienz eingefügter Paket-Positionen ---------------------------------

/// Felder einer Beleg-Position, die über die Paket-Provenienz entscheiden
/// (Rechnung, Angebot, Abo-Vorlage).
pub(crate) trait PackageProvenance {
    fn source(&self) -> (Option<&str>, Option<i64>);
    /// `true`, wenn die Position die unveränderte Kopie der Revision ist.
    fn matches_revision(&self, rev: &PackageRevisionRow) -> bool;
    fn clear_source(&mut self);
}

impl PackageProvenance for crate::domain::invoice::InvoiceItemInput {
    fn source(&self) -> (Option<&str>, Option<i64>) {
        (
            self.source_package_id.as_deref(),
            self.source_package_revision,
        )
    }
    fn matches_revision(&self, r: &PackageRevisionRow) -> bool {
        self.description_title.as_deref() == Some(r.title.as_str())
            && self.description_markup.as_deref() == Some(r.body_markup.as_str())
            && self.unit_price_cents == r.default_unit_price_cents
            && self.unit_code == r.unit_code
    }
    fn clear_source(&mut self) {
        self.source_package_id = None;
        self.source_package_revision = None;
    }
}

impl PackageProvenance for crate::domain::quote::QuoteItemInput {
    fn source(&self) -> (Option<&str>, Option<i64>) {
        (
            self.source_package_id.as_deref(),
            self.source_package_revision,
        )
    }
    fn matches_revision(&self, r: &PackageRevisionRow) -> bool {
        self.description_title.as_deref() == Some(r.title.as_str())
            && self.description_markup.as_deref() == Some(r.body_markup.as_str())
            && self.unit_price_cents == r.default_unit_price_cents
            && self.unit_code == r.unit_code
    }
    fn clear_source(&mut self) {
        self.source_package_id = None;
        self.source_package_revision = None;
    }
}

/// Paket-Provenienz (Hardline Phase 3): Eine Position behält
/// `source_package_*` nur, solange sie die unveränderte Kopie der genannten
/// Revision ist (Titel, Markup, Einzelpreis, Einheit). Wurde sie bearbeitet —
/// oder gibt es die Revision nicht —, wird sie zur reinen Custom-Position
/// („Paket anpassen = sauberer Bruch“, kein Rest-Hinweis). Revisionen sind
/// unveränderlich, eine spätere neue Revision ändert am Vergleich also nichts.
/// Gilt für Rechnungs-Entwürfe, Angebote und Abo-Vorlagen (KB-0046, F5-fix).
pub(crate) async fn enforce_package_provenance<T: PackageProvenance>(
    pool: &SqlitePool,
    items: &mut [T],
) -> Result<()> {
    for it in items.iter_mut() {
        let keep = match it.source() {
            (None, _) => continue,
            (Some(pid), Some(rev)) => match packages::revision_get(pool, pid, rev).await? {
                Some(r) => it.matches_revision(&r),
                None => false,
            },
            (Some(_), None) => false,
        };
        if !keep {
            it.clear_source();
        }
    }
    Ok(())
}

// ---- Kategorien ------------------------------------------------------------

/// Argumente von [`package_categories_list`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageCategoriesListArgs {}

pub async fn package_categories_list(
    ctx: &Arc<AppCtx>,
    _args: PackageCategoriesListArgs,
) -> Result<Vec<PackageCategoryRow>> {
    let pool = ctx.pool()?;

    packages::categories_list(&pool).await
}

/// Argumente von [`package_category_create`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageCategoryCreateArgs {
    pub name: String,
}

pub async fn package_category_create(
    ctx: &Arc<AppCtx>,
    args: PackageCategoryCreateArgs,
) -> Result<PackageCategoryRow> {
    let PackageCategoryCreateArgs { name } = args;
    let pool = ctx.pool()?;

    if name.trim().is_empty() {
        return Err(Error::Domain(
            "Der Kategorie-Name darf nicht leer sein.".into(),
        ));
    }
    let row = packages::category_create(&pool, name.trim()).await?;
    audit_log::append(
        &pool,
        "package_category.create",
        "package_category",
        &row.id,
        None,
    )
    .await
    .ok();
    Ok(row)
}

/// Argumente von [`package_category_update`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageCategoryUpdateArgs {
    pub id: String,
    pub name: String,
}

pub async fn package_category_update(
    ctx: &Arc<AppCtx>,
    args: PackageCategoryUpdateArgs,
) -> Result<()> {
    let PackageCategoryUpdateArgs { id, name } = args;
    let pool = ctx.pool()?;

    if name.trim().is_empty() {
        return Err(Error::Domain(
            "Der Kategorie-Name darf nicht leer sein.".into(),
        ));
    }
    packages::category_update(&pool, &id, name.trim()).await?;
    audit_log::append(
        &pool,
        "package_category.update",
        "package_category",
        &id,
        None,
    )
    .await
    .ok();
    Ok(())
}

/// Argumente von [`package_categories_reorder`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageCategoriesReorderArgs {
    pub ordered_ids: Vec<String>,
}

pub async fn package_categories_reorder(
    ctx: &Arc<AppCtx>,
    args: PackageCategoriesReorderArgs,
) -> Result<()> {
    let PackageCategoriesReorderArgs { ordered_ids } = args;
    let pool = ctx.pool()?;

    packages::categories_reorder(&pool, &ordered_ids).await
}

// ---- Pakete ----------------------------------------------------------------

/// Argumente von [`packages_list`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackagesListArgs {}

pub async fn packages_list(ctx: &Arc<AppCtx>, _args: PackagesListArgs) -> Result<Vec<PackageRow>> {
    let pool = ctx.pool()?;

    packages::list(&pool).await
}

/// Argumente von [`packages_get`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackagesGetArgs {
    pub id: String,
}

pub async fn packages_get(ctx: &Arc<AppCtx>, args: PackagesGetArgs) -> Result<Option<PackageRow>> {
    let PackagesGetArgs { id } = args;
    let pool = ctx.pool()?;

    packages::get(&pool, &id).await
}

/// Argumente von [`packages_create`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackagesCreateArgs {
    #[serde(default)]
    pub category_id: Option<String>,
    pub name: String,
    pub revision: PackageRevisionInput,
}

pub async fn packages_create(ctx: &Arc<AppCtx>, args: PackagesCreateArgs) -> Result<PackageRow> {
    let PackagesCreateArgs {
        category_id,
        name,
        revision,
    } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
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
        Some(serde_json::json!({ "name": name.trim() })),
    )
    .await
    .ok();
    Ok(row)
}

/// Argumente von [`packages_update_as_new_revision`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackagesUpdateAsNewRevisionArgs {
    pub id: String,
    #[serde(default)]
    pub category_id: Option<String>,
    pub name: String,
    pub revision: PackageRevisionInput,
}

pub async fn packages_update_as_new_revision(
    ctx: &Arc<AppCtx>,
    args: PackagesUpdateAsNewRevisionArgs,
) -> Result<PackageRow> {
    let PackagesUpdateAsNewRevisionArgs {
        id,
        category_id,
        name,
        revision,
    } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
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
        Some(serde_json::json!({ "revision": row.current_revision.unwrap_or(0) })),
    )
    .await
    .ok();
    Ok(row)
}

/// Argumente von [`packages_archive`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackagesArchiveArgs {
    pub id: String,
}

pub async fn packages_archive(ctx: &Arc<AppCtx>, args: PackagesArchiveArgs) -> Result<()> {
    let PackagesArchiveArgs { id } = args;
    let pool = ctx.pool()?;

    packages::set_status(&pool, &id, "archived").await?;
    // R2-026: Pflicht-Audit (Status-Wechsel auf Paket = fachlicher Vorgang).
    audit_log::append(&pool, "package.archive", "package", &id, None).await?;
    Ok(())
}

/// Argumente von [`packages_reactivate`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackagesReactivateArgs {
    pub id: String,
}

pub async fn packages_reactivate(ctx: &Arc<AppCtx>, args: PackagesReactivateArgs) -> Result<()> {
    let PackagesReactivateArgs { id } = args;
    let pool = ctx.pool()?;

    packages::set_status(&pool, &id, "active").await?;
    // R2-026: Pflicht-Audit (Status-Wechsel auf Paket = fachlicher Vorgang).
    audit_log::append(&pool, "package.reactivate", "package", &id, None).await?;
    Ok(())
}

// ---- Revisionen ------------------------------------------------------------

/// Argumente von [`package_revisions_list`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageRevisionsListArgs {
    pub package_id: String,
}

pub async fn package_revisions_list(
    ctx: &Arc<AppCtx>,
    args: PackageRevisionsListArgs,
) -> Result<Vec<PackageRevisionRow>> {
    let PackageRevisionsListArgs { package_id } = args;
    let pool = ctx.pool()?;

    packages::revisions_list(&pool, &package_id).await
}

/// Argumente von [`package_revisions_get`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageRevisionsGetArgs {
    pub package_id: String,
    pub revision: i64,
}

pub async fn package_revisions_get(
    ctx: &Arc<AppCtx>,
    args: PackageRevisionsGetArgs,
) -> Result<Option<PackageRevisionRow>> {
    let PackageRevisionsGetArgs {
        package_id,
        revision,
    } = args;
    let pool = ctx.pool()?;

    packages::revision_get(&pool, &package_id, revision).await
}

/// Argumente von [`package_revisions_rollback`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageRevisionsRollbackArgs {
    pub package_id: String,
    pub to_revision: i64,
}

pub async fn package_revisions_rollback(
    ctx: &Arc<AppCtx>,
    args: PackageRevisionsRollbackArgs,
) -> Result<PackageRow> {
    let PackageRevisionsRollbackArgs {
        package_id,
        to_revision,
    } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
    let row = packages::rollback(pool, &package_id, to_revision).await?;
    audit_log::append(
        pool,
        "package.rollback",
        "package",
        &package_id,
        Some(serde_json::json!({ "to_revision": to_revision, "new_revision": row.current_revision.unwrap_or(0) })),
    )
    .await
    .ok();
    Ok(row)
}

// ---- Vorschau (PDF) --------------------------------------------------------

/// Argumente von [`package_preview`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackagePreviewArgs {
    pub title: String,
    pub body_markup: String,
    pub default_unit_price_cents: i64,
}

/// Rendert eine Beispiel-Position als PDF (Editor-„PDF-Vorschau"). Reine Vorschau:
/// schreibt nach `data/preview/`, nichts wird archiviert oder validiert. Der
/// formatierte Block kommt aus `domain::package::to_typst` und wird im Template
/// via `eval(mode: "markup")` eingebettet.
pub async fn package_preview(
    ctx: &Arc<AppCtx>,
    args: PackagePreviewArgs,
) -> Result<crate::downloads::DownloadTicket> {
    let PackagePreviewArgs {
        title,
        body_markup,
        default_unit_price_cents,
    } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
    let body_typst = package::to_typst(&package::parse_markup(&body_markup));
    // Vorschau: Status heute (ADR 0042 Nr. 3; kein Beleg).
    let is_klein =
        crate::db::repo::tax_status::is_klein_at(pool, crate::clock::today_berlin()).await?;
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

    let paths = ctx.paths.clone();
    let branding_dir = paths.inputs_dir.join("branding");
    let branding_opt = if branding_dir.is_dir() {
        Some(branding_dir.as_path())
    } else {
        None
    };
    let pdf = typst_render::plain_job(templates::PACKAGE_PREVIEW_TEMPLATE, &json, branding_opt)
        .compile_async()
        .await?;

    // Plan W7: Vorschau als Download-Ticket (inline im Browser).
    ctx.downloads.put_bytes(
        "Paket-Vorschau.pdf",
        &pdf,
        crate::downloads::DownloadKind::Preview,
    )
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

/// Argumente von [`package_materialize_item`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageMaterializeItemArgs {
    pub package_id: String,
    /// Maßgebliches Datum des Zielbelegs (Leistungsdatum, sonst Beleg-
    /// datum; ADR 0042 Nr. 3). Ohne Angabe: heute.
    #[serde(default)]
    pub document_date: Option<chrono::NaiveDate>,
}

/// Materialisiert eine Beleg-Position aus der **aktiven** Revision eines Pakets
/// (Block P3). Reine Lese-/Aufbereitungs-Operation — schreibt nichts.
///
/// Semantik (Manuel-Entscheidung 2026-05-23 — „Titel + Body nur PDF, XML = Body"):
/// - `description`        = Klartext(Body) → XRechnung-BT-154 (ohne Titel).
///   Fallback = Titel, falls der Body leer ist (BT-154 darf nicht leer sein).
/// - `description_markup` = fetter Titel + Body-Markup → der formatierte PDF-Block.
/// - §19-Hardline: bei Kleinunternehmer `tax_category_code = 'E'`, Rate 0.
pub async fn package_materialize_item(
    ctx: &Arc<AppCtx>,
    args: PackageMaterializeItemArgs,
) -> Result<MaterializedPackageItem> {
    let PackageMaterializeItemArgs {
        package_id,
        document_date,
    } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
    let pkg = packages::get(pool, &package_id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Paket nicht gefunden: {package_id}")))?;
    let rev = packages::current_revision(pool, &package_id)
        .await?
        .ok_or_else(|| Error::Domain("Dieses Paket hat noch keine Version.".into()))?;

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

    // §19-Hardline: Kleinunternehmer → Category 'E', Rate 0. Regelbesteuerung
    // (KB-0047): Pakete sind Netto-Katalogpreise ohne eigenen Satz; die
    // Steuerart der Revision gilt, 'E' gibt es ohne §19 nicht (KB-0014) und wird
    // zu 'S'. Bei 'S' gilt der Regelsatz 19 % (§12 Abs. 1 UStG) — ein anderer
    // Satz (z. B. 7 %) wird an der Position gesetzt; Satz und Steuerart gehören
    // nicht zum Paket-Inhalt, die Provenienz bleibt.
    // ADR 0042 Nr. 3: Status am Datum des Zielbelegs, ohne Angabe heute.
    let is_klein = crate::db::repo::tax_status::is_klein_at(
        pool,
        document_date.unwrap_or_else(crate::clock::today_berlin),
    )
    .await?;
    let (tax_category_code, tax_rate_percent) =
        package_tax_defaults(is_klein, &rev.tax_category_code);

    Ok(MaterializedPackageItem {
        description,
        description_title: rev.title.clone(),
        description_markup,
        quantity: 1.0,
        unit_code: rev.unit_code.clone(),
        unit_price_cents: rev.default_unit_price_cents,
        tax_rate_percent,
        tax_category_code,
        source_package_id: package_id,
        source_package_revision: rev.revision,
        package_name: pkg.name,
    })
}

/// Steuerart und Satz einer aus einem Paket eingefügten Position (KB-0047).
pub fn package_tax_defaults(is_kleinunternehmer: bool, revision_code: &str) -> (String, f64) {
    if is_kleinunternehmer {
        return ("E".into(), 0.0);
    }
    let code = if revision_code == crate::domain::kleinunternehmer::EXEMPT_CATEGORY {
        "S"
    } else {
        revision_code
    };
    let rate = if code == "S" {
        crate::domain::money::STANDARD_VAT_RATE_PERCENT
    } else {
        0.0
    };
    (code.to_string(), rate)
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
    /// Versandprotokoll unvollständig (R4-S2-EMAILLOG, Lauf F5c).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

/// Baut einen `SellerView` aus dem Stammdaten-Row (geliehen) — wie
/// `commands::quotes::make_seller_view`, hier lokal für den Broschüren-Render.
/// Die Broschüre ist kein Beleg; sie zeigt den Status von heute.
fn seller_view_from_row(row: &SellerProfileRow, is_kleinunternehmer: bool) -> SellerView<'_> {
    SellerView {
        name: &row.name,
        street: &row.street,
        postal_code: &row.postal_code,
        city: &row.city,
        country_code: &row.country_code,
        tax_number: row.tax_number.as_deref(),
        vat_id: row.vat_id.as_deref(),
        email: &row.email,
        phone: row.phone.as_deref(),
        contact_name: None,
        iban: row.iban.as_deref(),
        bic: row.bic.as_deref(),
        is_kleinunternehmer,
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
            "Bitte mindestens ein Paket auswählen, das schon eine Version hat.".into(),
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
    let seller_view = seller_view_from_row(
        &seller_row,
        crate::db::repo::tax_status::is_klein_at(pool, crate::clock::today_berlin()).await?,
    );

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

    let generated_at = crate::clock::today_berlin().to_string();

    typst_render::package_catalog_job(
        templates::PACKAGE_CATALOG_TEMPLATE,
        &entries,
        &seller_view,
        contact_name,
        owner_name.as_deref(),
        &generated_at,
        branding_opt,
        logo_opt,
    )?
    .compile_async()
    .await
}

/// Argumente von [`package_catalog_render`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageCatalogRenderArgs {
    pub package_ids: Vec<String>,
}

/// Druck/Vorschau: rendert die Broschüre für die gewählten Pakete als
/// Download-Ticket (Plan W7; reine Vorschau, kein Archiv).
pub async fn package_catalog_render(
    ctx: &Arc<AppCtx>,
    args: PackageCatalogRenderArgs,
) -> Result<crate::downloads::DownloadTicket> {
    let PackageCatalogRenderArgs { package_ids } = args;
    let pool = ctx.pool()?;

    let pool = &pool;
    let paths = ctx.paths.clone();
    let pdf = render_catalog_pdf(pool, &paths, &package_ids, None).await?;

    let ticket = ctx.downloads.put_bytes(
        "Paket-Broschuere.pdf",
        &pdf,
        crate::downloads::DownloadKind::Preview,
    )?;

    audit_log::append(
        pool,
        "package_catalog.render",
        "package_catalog",
        "catalog",
        Some(serde_json::json!({ "packages": package_ids.len() })),
    )
    .await
    .ok();
    Ok(ticket)
}

/// Argumente von [`package_catalog_send`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageCatalogSendArgs {
    pub args: SendPackageCatalogArgs,
}

/// Versand: rendert die Broschüre (mit persönlicher Anrede) und mailt sie als
/// **einzelnen** PDF-Anhang an den gewählten Kontakt. Protokoll ausschließlich
/// im append-only `email_log` (`related_kind = 'package_catalog'`) — KEIN
/// §14-Status, KEIN write-once-Archiv.
pub async fn package_catalog_send(
    ctx: &Arc<AppCtx>,
    args: PackageCatalogSendArgs,
) -> Result<SendPackageCatalogResult> {
    let PackageCatalogSendArgs { args } = args;
    let pool = ctx.pool()?;

    let paths = ctx.paths.clone();
    send_catalog_core(&pool, &paths, &args).await
}

/// Kern-Pipeline für den Broschüren-Versand (ohne `AppCtx`). Public, damit
/// Integrationstests sie ohne Server treiben können (Paths aus tempdir) —
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

    // Versand + append-only Protokoll-Eintrag. Kein Beleg-Bezug; `related_id`
    // ist der Kontakt (Lauf F, KB-0049: die DSGVO-Auskunft findet den Versand
    // auch, wenn sich die Adresse später ändert).
    let receipt = crate::commands::mail::send_and_log(
        pool,
        &account,
        &mail,
        crate::commands::mail::SendContext {
            related_kind: "package_catalog",
            related_id: Some(&contact.id),
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
        Some(serde_json::json!({ "account": &args.account_id, "to": &recipient, "packages": package_count })),
    )
    .await
    .ok();

    Ok(SendPackageCatalogResult {
        to: recipient,
        subject,
        package_count,
        attachment_count: 1,
        warning: receipt.warning(),
    })
}

/// Netto-Cent → „1.234,56 €" (deutsches Format, ohne Tausenderpunkt für die Vorschau).
fn fmt_eur(cents: i64) -> String {
    let euros = cents / 100;
    let rem = (cents % 100).abs();
    format!("{euros},{rem:02} €")
}

#[cfg(test)]
mod tests {
    use super::package_tax_defaults;

    // KB-0047: §19 → E/0; Regelbesteuerung → Steuerart der Revision, E wird S,
    // S mit 19 %.
    #[test]
    fn package_tax_defaults_follow_status() {
        assert_eq!(package_tax_defaults(true, "S"), ("E".into(), 0.0));
        assert_eq!(package_tax_defaults(false, "E"), ("S".into(), 19.0));
        assert_eq!(package_tax_defaults(false, "S"), ("S".into(), 19.0));
        assert_eq!(package_tax_defaults(false, "Z"), ("Z".into(), 0.0));
    }
}
