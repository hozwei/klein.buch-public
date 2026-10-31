//! Commands für Settings.
//!
//! Verkäuferprofil (`seller_profile_{get,upsert}`), Logo, Vorlagen und
//! Anfahrt. Den Steuerstatus nach §19 führt seit ADR 0042 die Historie
//! ([`crate::commands::tax_status`]); das Profil liefert nur noch den Status
//! von heute zur Anzeige mit.

use crate::app_ctx::AppCtx;
use crate::branding;
use crate::db::repo::seller_profile::{self, SellerProfileInput, SellerProfileView};
use crate::domain::kleinunternehmer::hinweis_text;
use crate::error::{Error, Result};
use serde::Serialize;
use std::sync::Arc;

/// Argumente von [`seller_profile_get`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SellerProfileGetArgs {}

pub async fn seller_profile_get(
    ctx: &Arc<AppCtx>,
    _args: SellerProfileGetArgs,
) -> Result<Option<SellerProfileView>> {
    let pool = ctx.pool()?;

    seller_profile::get_view(&pool).await
}

/// Argumente von [`seller_profile_upsert`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SellerProfileUpsertArgs {
    pub input: SellerProfileInput,
}

/// Speichert das Verkäuferprofil. Bedingte Re-Auth (W14-038, Lauf F4a):
/// Ändern sich IBAN oder BIC gegenüber dem gespeicherten Profil, verlangt der
/// Kern eine frische Bestätigung ([`crate::commands::reauth::require_fresh`];
/// als frisch gilt auch eine Anmeldung der letzten 5 min). Prüfung und
/// Speichern laufen unter [`crate::locks::SELLER_PROFILE`] am Stück. Den
/// §19-Status ändert das Profil nicht mehr (ADR 0042 Nr. 1).
pub async fn seller_profile_upsert(
    ctx: &Arc<AppCtx>,
    args: SellerProfileUpsertArgs,
) -> Result<SellerProfileView> {
    let SellerProfileUpsertArgs { input } = args;
    let pool = ctx.pool()?;

    let _profile = crate::locks::SELLER_PROFILE.lock("1").await;
    let prev = seller_profile::get(&pool).await?;
    if seller_profile::reauth_relevant_change(prev.as_ref(), &input) {
        crate::commands::reauth::require_fresh()?;
    }
    seller_profile::upsert(&pool, &input).await?;
    seller_profile::get_view(&pool)
        .await?
        .ok_or_else(|| Error::Domain("Profil nach dem Speichern nicht gefunden.".into()))
}

/// Argumente von [`seller_logo_set`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SellerLogoSetArgs {
    /// Aus `POST /api/upload/logo` (Plan W7).
    pub upload_id: String,
}

/// Logo hochladen: Bytes nach `data/branding/logo.<ext>` schreiben (NICHT
/// `inputs/` — Hard-Rule), Dateinamen im Profil merken. Profil muss existieren.
pub async fn seller_logo_set(
    ctx: &Arc<AppCtx>,
    args: SellerLogoSetArgs,
) -> Result<SellerProfileView> {
    let SellerLogoSetArgs { upload_id } = args;
    let pool = ctx.pool()?;

    let upload = ctx
        .uploads
        .read(&upload_id, &[crate::uploads::UploadKind::Logo])?;
    let paths = ctx.paths.clone();
    let dir = branding::branding_dir(&paths.data_dir);
    // Endung aus der Inhaltsprüfung, nicht aus dem Dateinamen (Plan W7).
    let stored = branding::store_logo(
        &dir,
        &upload.bytes,
        &format!("logo.{}", upload.file_type.ext()),
    )?;
    seller_profile::set_logo_filename(&pool, Some(&stored)).await?;
    ctx.uploads.consume(&upload_id);
    seller_profile::get_view(&pool)
        .await?
        .ok_or_else(|| Error::Domain("Profil nach Logo-Upload nicht gefunden.".into()))
}

/// Argumente von [`seller_logo_clear`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SellerLogoClearArgs {}

/// Logo entfernen: Datei löschen + `logo_filename` auf NULL.
pub async fn seller_logo_clear(
    ctx: &Arc<AppCtx>,
    _args: SellerLogoClearArgs,
) -> Result<SellerProfileView> {
    let pool = ctx.pool()?;

    let paths = ctx.paths.clone();
    let dir = branding::branding_dir(&paths.data_dir);
    if let Some(p) = seller_profile::get(&pool).await? {
        // Ein ungültiger Altwert wird nur aus dem Profil entfernt, nie als
        // Pfad benutzt (Plan W7, Gegenprüfung).
        if let Some(fname) = p
            .logo_filename
            .as_deref()
            .filter(|f| branding::is_asset_name(f))
        {
            branding::clear_logo(&dir, fname)?;
        }
    }
    seller_profile::set_logo_filename(&pool, None).await?;
    seller_profile::get_view(&pool)
        .await?
        .ok_or_else(|| Error::Domain("Profil nach Logo-Entfernen nicht gefunden.".into()))
}

/// Argumente von [`seller_logo_data`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SellerLogoDataArgs {}

/// Liefert das Logo als Data-URL für die In-App-Vorschau (oder `None`).
pub async fn seller_logo_data(
    ctx: &Arc<AppCtx>,
    _args: SellerLogoDataArgs,
) -> Result<Option<String>> {
    let pool = ctx.pool()?;

    let paths = ctx.paths.clone();
    let dir = branding::branding_dir(&paths.data_dir);
    let Some(p) = seller_profile::get(&pool).await? else {
        return Ok(None);
    };
    match p.logo_filename.as_deref() {
        Some(fname) => branding::read_logo_data_url(&dir, fname),
        None => Ok(None),
    }
}

/// Argumente von [`pdf_templates_list`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfTemplatesListArgs {}

/// Listet alle wählbaren PDF-Vorlagen (eingebettete Built-ins + `inputs/`-
/// Overrides) inkl. §19-Kompatibilität. Block 17a.
pub async fn pdf_templates_list(
    ctx: &Arc<AppCtx>,
    _args: PdfTemplatesListArgs,
) -> Result<Vec<crate::pdf::templates::TemplateMeta>> {
    let paths = ctx.paths.clone();
    crate::pdf::templates::list_templates(&paths.inputs_dir)
}

/// Dummy-Firmenlogo für die Vorlagen-Vorschau. Rein geometrisches SVG (kein
/// `<text>`) — rendert in Typst font-unabhängig zuverlässig. Petrol-Dokument-Icon.
const PREVIEW_LOGO_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="120" viewBox="0 0 120 120">
  <rect width="120" height="120" rx="24" fill="#176b87"/>
  <rect x="34" y="28" width="52" height="64" rx="6" fill="#ffffff"/>
  <rect x="44" y="44" width="32" height="6" rx="3" fill="#176b87"/>
  <rect x="44" y="57" width="32" height="6" rx="3" fill="#10aeb8"/>
  <rect x="44" y="70" width="20" height="6" rx="3" fill="#176b87"/>
</svg>"##;

/// Argumente von [`pdf_template_preview`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfTemplatePreviewArgs {
    pub name: String,
}

/// Rendert eine Beispiel-Rechnung (Dummy-Daten „Max Mustermann“ + Dummy-Logo) mit
/// der gewählten Vorlage und öffnet sie im Standard-PDF-Viewer. Reine Vorschau:
/// schreibt nach `data/preview/`, nichts wird validiert oder archiviert. Block 17a.
pub async fn pdf_template_preview(
    ctx: &Arc<AppCtx>,
    args: PdfTemplatePreviewArgs,
) -> Result<crate::downloads::DownloadTicket> {
    let PdfTemplatePreviewArgs { name } = args;
    let pool = ctx.pool()?;

    use crate::domain::invoice::{
        BuyerView, InvoiceDirection, InvoiceInput, InvoiceItemInput, SellerView,
    };

    let paths = ctx.paths.clone();
    let source = crate::pdf::templates::resolve_invoice_template(&paths.inputs_dir, &name)?;

    // R3-004: Vorschau ist der einzige Render-Pfad, der den §19-Klausel-Check
    // bisher ausgelassen hat. `seller_default_template_set` blockiert eine
    // klausel-lose Vorlage beim Speichern, **wenn** der User §19-Kleinunter-
    // nehmer ist (analoge Seller-aware-Logik). Die Vorschau hat es bisher still
    // gerendert. Symmetrisch zur Speichern-Logik machen: bei §19 vorab prüfen,
    // bei Regelbesteuerung weiterhin frei vorab-render-bar. §19 zählt, wenn es
    // heute oder ab einem künftigen Eintrag gilt (ADR 0042 Nr. 3).
    let seller_is_klein = crate::db::repo::tax_status::history(&pool)
        .await?
        .klein_today_or_later(crate::clock::berlin_date(ctx.clock().now_utc()));
    if seller_is_klein {
        if let Err(check_err) = crate::pdf::klausel_check::verify_for_kleinunternehmer(&source) {
            return Err(Error::Domain(format!(
                "Vorlage ist nicht §19-kompatibel: {check_err}. \
                 Sie würde auch beim Speichern als Standard-Vorlage blockiert werden."
            )));
        }
    }

    let seller = SellerView {
        name: "Max Mustermann IT-Service",
        street: "Musterstraße 1",
        postal_code: "84028",
        city: "Landshut",
        country_code: "DE",
        tax_number: Some("133/456/7890"),
        vat_id: None,
        email: "kontakt@mustermann-it.example",
        phone: None,
        contact_name: None,
        iban: Some("DE02 1203 0000 0000 2020 51"),
        bic: Some("BYLADEM1001"),
        is_kleinunternehmer: true,
    };
    let buyer = BuyerView {
        name: "Beispielkunde GmbH",
        street: Some("Beispielweg 12"),
        postal_code: Some("80331"),
        city: Some("München"),
        country_code: "DE",
        vat_id: None,
        email: Some("einkauf@beispielkunde.example"),
    };
    let input = InvoiceInput {
        service_period_start: None,
        service_period_end: None,
        buyer_reference: None,
        direction: InvoiceDirection::Issued,
        invoice_date: chrono::NaiveDate::from_ymd_opt(2026, 1, 15).unwrap(),
        delivery_date: chrono::NaiveDate::from_ymd_opt(2026, 1, 12),
        due_date: chrono::NaiveDate::from_ymd_opt(2026, 1, 29),
        currency_code: "EUR".into(),
        items: vec![
            InvoiceItemInput {
                position: 1,
                description: "Beratung & Einrichtung Arbeitsplatz".into(),
                quantity: 3.0,
                unit_code: "Std.".into(),
                unit_price_cents: 8500,
                tax_rate_percent: 0.0,
                tax_category_code: "E".into(),
                description_title: None,
                description_markup: None,
                source_package_id: None,
                source_package_revision: None,
            },
            InvoiceItemInput {
                position: 2,
                description: "Netzwerk-Switch 8-Port (Hardware)".into(),
                quantity: 1.0,
                unit_code: "Stk.".into(),
                unit_price_cents: 7900,
                tax_rate_percent: 0.0,
                tax_category_code: "E".into(),
                description_title: None,
                description_markup: None,
                source_package_id: None,
                source_package_revision: None,
            },
            InvoiceItemInput {
                position: 3,
                description: "Monatliche Fernwartung".into(),
                quantity: 1.0,
                unit_code: "Monat".into(),
                unit_price_cents: 2900,
                tax_rate_percent: 0.0,
                tax_category_code: "E".into(),
                description_title: None,
                description_markup: None,
                source_package_id: None,
                source_package_revision: None,
            },
        ],
        notes: None,
        payment_note: None,
        pdf_template: name.clone(),
        is_storno_for: None,
        cancel_reason: None,
    };

    let logo_bytes = PREVIEW_LOGO_SVG.as_bytes();
    let logo_opt = Some(("/preview-logo.svg", logo_bytes));

    // Beispiel-Konten für die Vorschau: ein Bank-Konto (IBAN/BIC) + PayPal,
    // damit das Impressum die Mehr-Konten-Darstellung zeigt.
    let accounts_json = serde_json::json!([
        {
            "type": "bank",
            "label": "Geschäftskonto",
            "holder": "Max Mustermann",
            "iban": "DE02 1203 0000 0000 2020 51",
            "bic": "BYLADEM1001",
            "details": null,
        },
        {
            "type": "paypal",
            "label": "PayPal",
            "holder": "Max Mustermann",
            "iban": null,
            "bic": null,
            "details": "paypal.me/mustermann-it",
        }
    ]);

    let pdf = crate::pdf::typst_render::invoice_job(
        &source,
        "MUSTER-2026-0001",
        &input,
        &seller,
        &buyer,
        None,
        logo_opt,
        Some("Max Mustermann"),
        &accounts_json,
        true,
    )?
    .compile_async()
    .await?;

    // Plan W7: Vorschau als Download-Ticket (inline im Browser), keine Datei
    // mit festem Namen.
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    ctx.downloads.put_bytes(
        &format!("Vorschau-{safe}.pdf"),
        &pdf,
        crate::downloads::DownloadKind::Preview,
    )
}

/// Argumente von [`seller_default_template_set`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SellerDefaultTemplateSetArgs {
    pub name: String,
}

/// Setzt die globale Standard-PDF-Vorlage (gilt für neu gerenderte Rechnungen
/// und Angebote). §19-Schutz: bei Kleinunternehmer darf nur eine §19-kompatible
/// Vorlage gewählt werden (Marker + Hinweis-Text). Block 17a.
pub async fn seller_default_template_set(
    ctx: &Arc<AppCtx>,
    args: SellerDefaultTemplateSetArgs,
) -> Result<SellerProfileView> {
    let SellerDefaultTemplateSetArgs { name } = args;
    let pool = ctx.pool()?;

    let paths = ctx.paths.clone();
    let templates = crate::pdf::templates::list_templates(&paths.inputs_dir)?;
    let meta = templates
        .iter()
        .find(|t| t.name == name)
        .ok_or_else(|| Error::Domain(format!("Unbekannte Vorlage: {name}")))?;

    seller_profile::get(&pool)
        .await?
        .ok_or_else(|| Error::Domain("Bitte zuerst die Firmendaten speichern.".into()))?;
    // §19 heute oder ab einem künftigen Eintrag (ADR 0042 Nr. 3): Die
    // Standardvorlage muss dann die Klausel tragen.
    let klein = crate::db::repo::tax_status::history(&pool)
        .await?
        .klein_today_or_later(crate::clock::berlin_date(ctx.clock().now_utc()));
    if klein && !meta.klausel_status.is_klein_compatible {
        return Err(Error::Domain(format!(
            "Vorlage „{name}“ ist nicht §19-konform (ohne Kleinunternehmer-Klausel) und kann nicht gewählt werden, solange du Kleinunternehmer bist."
        )));
    }

    seller_profile::set_default_pdf_template(&pool, &name).await?;
    seller_profile::get_view(&pool)
        .await?
        .ok_or_else(|| Error::Domain("Profil nach Vorlagen-Wahl nicht gefunden.".into()))
}

/// Liefert dem Frontend die §19-Eckdaten — wortgleicher Hinweis-Text und
/// (falls relevant) das frühestmögliche Rückkehr-Datum aus der Historie (nach
/// einem Verzicht das Bindungsende, sonst der nächste 1. Januar; ADR 0042 Nr. 2). Verhindert, dass das Frontend die
/// Klausel selbst formuliert.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Paragraph19Info {
    pub hinweis_text: &'static str,
    pub return_date_after_waiver: Option<String>,
}

/// Argumente von [`paragraph_19_info`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Paragraph19InfoArgs {}

pub async fn paragraph_19_info(
    ctx: &Arc<AppCtx>,
    _args: Paragraph19InfoArgs,
) -> Result<Paragraph19Info> {
    let pool = ctx.pool()?;

    let today = crate::clock::berlin_date(ctx.clock().now_utc());
    let return_date = crate::db::repo::tax_status::history(&pool)
        .await?
        .earliest_return(today)
        .map(|d| d.to_string());
    Ok(Paragraph19Info {
        hinweis_text: hinweis_text(),
        return_date_after_waiver: return_date,
    })
}

// ---- Anfahrtspauschale (Block P1) ------------------------------------------

/// Aktueller Kilometersatz (Netto-Cent) + UI-Default für Hin & Rück.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelSettings {
    pub cost_per_km_cents: i64,
    pub round_trip_default: bool,
}

/// Argumente von [`travel_settings_get`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelSettingsGetArgs {}

pub async fn travel_settings_get(
    ctx: &Arc<AppCtx>,
    _args: TravelSettingsGetArgs,
) -> Result<TravelSettings> {
    let pool = ctx.pool()?;

    let cost_per_km_cents = crate::db::repo::app_settings::get(&pool, "travel_cost_per_km_cents")
        .await?
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0);
    let round_trip_default =
        crate::db::repo::app_settings::get_bool(&pool, "travel_round_trip_default", false).await?;
    Ok(TravelSettings {
        cost_per_km_cents,
        round_trip_default,
    })
}

/// Argumente von [`travel_settings_set`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelSettingsSetArgs {
    pub cost_per_km_cents: i64,
    pub round_trip_default: bool,
}

pub async fn travel_settings_set(ctx: &Arc<AppCtx>, args: TravelSettingsSetArgs) -> Result<()> {
    let TravelSettingsSetArgs {
        cost_per_km_cents,
        round_trip_default,
    } = args;
    let pool = ctx.pool()?;

    if cost_per_km_cents < 0 {
        return Err(Error::Domain(
            "Der Kilometersatz darf nicht negativ sein.".into(),
        ));
    }
    crate::db::repo::app_settings::set(
        &pool,
        "travel_cost_per_km_cents",
        &cost_per_km_cents.to_string(),
    )
    .await?;
    crate::db::repo::app_settings::set_bool(&pool, "travel_round_trip_default", round_trip_default)
        .await?;
    // R2-026: Audit-Schreiben hier rein kosmetisch (Reisesatz-Update ist
    // kein GoBD-Beleg, sondern Settings). Fehler werden geloggt, sperren
    // den Settings-Save aber nicht.
    if let Err(e) = crate::db::repo::audit_log::append(
        &pool,
        "settings.travel.update",
        "settings",
        "travel",
        Some(serde_json::json!({ "cost_per_km_cents": cost_per_km_cents, "round_trip_default": round_trip_default })),
    )
    .await
    {
        tracing::warn!("audit_log.settings.travel.update fehlgeschlagen: {e}");
    }
    Ok(())
}

/// Argumente von [`travel_compute`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TravelComputeArgs {
    pub km: f64,
    pub round_trip: bool,
}

/// Berechnet eine Anfahrts-Position aus km + gespeichertem Satz. Geld-Mathematik
/// bleibt in Rust (autoritativ), das Frontend fügt das Ergebnis als Position ein.
pub async fn travel_compute(
    ctx: &Arc<AppCtx>,
    args: TravelComputeArgs,
) -> Result<crate::domain::travel::TravelLine> {
    let TravelComputeArgs { km, round_trip } = args;
    let pool = ctx.pool()?;

    let cents = crate::db::repo::app_settings::get(&pool, "travel_cost_per_km_cents")
        .await?
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0);
    if cents <= 0 {
        return Err(Error::Domain(
            "Bitte zuerst den Kilometersatz unter Einstellungen → Anfahrtspauschale festlegen."
                .into(),
        ));
    }
    // Verhaltensgleich zu `!(km > 0.0)`: lehnt km <= 0 UND NaN ab (clippy-clean).
    if km <= 0.0 || km.is_nan() {
        return Err(Error::Domain(
            "Bitte eine Kilometerzahl größer als 0 eingeben.".into(),
        ));
    }
    Ok(crate::domain::travel::compute_travel(km, cents, round_trip))
}

// ---- Angebots-Unterschrift (Block „Angebots-Signatur") ---------------------

/// Argumente von [`seller_signature_set`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SellerSignatureSetArgs {
    /// Aus `POST /api/upload/unterschrift` (Plan W7).
    pub upload_id: String,
}

/// Unterschrift-Bild hochladen → `data/branding/signature.<ext>` (NICHT inputs/).
/// Gibt die Data-URL für die sofortige Vorschau zurück.
pub async fn seller_signature_set(
    ctx: &Arc<AppCtx>,
    args: SellerSignatureSetArgs,
) -> Result<Option<String>> {
    let SellerSignatureSetArgs { upload_id } = args;

    let upload = ctx
        .uploads
        .read(&upload_id, &[crate::uploads::UploadKind::Signature])?;
    let paths = ctx.paths.clone();
    let dir = branding::branding_dir(&paths.data_dir);
    // Endung aus der Inhaltsprüfung, nicht aus dem Dateinamen (Plan W7).
    let stored = branding::store_signature(
        &dir,
        &upload.bytes,
        &format!("signature.{}", upload.file_type.ext()),
    )?;
    ctx.uploads.consume(&upload_id);
    branding::read_logo_data_url(&dir, &stored)
}

/// Argumente von [`seller_signature_clear`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SellerSignatureClearArgs {}

/// Unterschrift-Bild entfernen (idempotent).
pub async fn seller_signature_clear(
    ctx: &Arc<AppCtx>,
    _args: SellerSignatureClearArgs,
) -> Result<()> {
    let paths = ctx.paths.clone();
    let dir = branding::branding_dir(&paths.data_dir);
    if let Some((name, _)) = branding::find_signature(&dir) {
        branding::clear_logo(&dir, &name)?;
    }
    Ok(())
}

/// Argumente von [`seller_signature_data`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SellerSignatureDataArgs {}

/// Unterschrift als Data-URL für die In-App-Vorschau (oder `None`).
pub async fn seller_signature_data(
    ctx: &Arc<AppCtx>,
    _args: SellerSignatureDataArgs,
) -> Result<Option<String>> {
    let paths = ctx.paths.clone();
    let dir = branding::branding_dir(&paths.data_dir);
    match branding::find_signature(&dir) {
        Some((name, _)) => branding::read_logo_data_url(&dir, &name),
        None => Ok(None),
    }
}

/// Argumente von [`quote_signature_get`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteSignatureGetArgs {}

/// Globaler Schalter „Unterschriftenfelder auf Angeboten".
pub async fn quote_signature_get(ctx: &Arc<AppCtx>, _args: QuoteSignatureGetArgs) -> Result<bool> {
    let pool = ctx.pool()?;

    crate::db::repo::app_settings::get_bool(&pool, "quote_signature_enabled", false).await
}

/// Argumente von [`quote_signature_set`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteSignatureSetArgs {
    pub enabled: bool,
}

pub async fn quote_signature_set(ctx: &Arc<AppCtx>, args: QuoteSignatureSetArgs) -> Result<()> {
    let QuoteSignatureSetArgs { enabled } = args;
    let pool = ctx.pool()?;

    crate::db::repo::app_settings::set_bool(&pool, "quote_signature_enabled", enabled).await
}

// ---- Inhaber-Name + Standard-Fristen ---------------------------------------

/// Argumente von [`seller_owner_get`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SellerOwnerGetArgs {}

/// Inhaber (Vor- und Nachname) — §14-Pflicht bei Einzelunternehmern/Freiberuflern.
/// Leerer Wert → None.
pub async fn seller_owner_get(
    ctx: &Arc<AppCtx>,
    _args: SellerOwnerGetArgs,
) -> Result<Option<String>> {
    let pool = ctx.pool()?;

    Ok(
        crate::db::repo::app_settings::get(&pool, "seller_owner_name")
            .await?
            .filter(|s| !s.trim().is_empty()),
    )
}

/// Argumente von [`seller_owner_set`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SellerOwnerSetArgs {
    #[serde(default)]
    pub owner_name: Option<String>,
}

pub async fn seller_owner_set(ctx: &Arc<AppCtx>, args: SellerOwnerSetArgs) -> Result<()> {
    let SellerOwnerSetArgs { owner_name } = args;
    let pool = ctx.pool()?;

    let v = owner_name
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("");
    crate::db::repo::app_settings::set(&pool, "seller_owner_name", v).await
}

/// Standard-Fristen: wie viele Tage ein Angebot gültig ist und in wie vielen
/// Tagen eine Rechnung fällig wird (Vorbelegung in den Formularen).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentTerms {
    pub quote_valid_days: i64,
    pub invoice_due_days: i64,
}

/// Argumente von [`document_terms_get`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentTermsGetArgs {}

pub async fn document_terms_get(
    ctx: &Arc<AppCtx>,
    _args: DocumentTermsGetArgs,
) -> Result<DocumentTerms> {
    let pool = ctx.pool()?;

    let quote_valid_days = crate::db::repo::app_settings::get(&pool, "quote_valid_days")
        .await?
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(30);
    let invoice_due_days = crate::db::repo::app_settings::get(&pool, "invoice_due_days")
        .await?
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(14);
    Ok(DocumentTerms {
        quote_valid_days,
        invoice_due_days,
    })
}

/// Argumente von [`document_terms_set`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentTermsSetArgs {
    pub quote_valid_days: i64,
    pub invoice_due_days: i64,
}

pub async fn document_terms_set(ctx: &Arc<AppCtx>, args: DocumentTermsSetArgs) -> Result<()> {
    let DocumentTermsSetArgs {
        quote_valid_days,
        invoice_due_days,
    } = args;
    let pool = ctx.pool()?;

    crate::db::repo::app_settings::set(
        &pool,
        "quote_valid_days",
        &quote_valid_days.max(0).to_string(),
    )
    .await?;
    crate::db::repo::app_settings::set(
        &pool,
        "invoice_due_days",
        &invoice_due_days.max(0).to_string(),
    )
    .await?;
    Ok(())
}

// ---- Versionsprüfung (Plan W12, ADR 0039 E14) -------------------------------

/// Opt-in Versionsprüfung gegen die GitHub-Releases (Standard aus).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckSettings {
    pub enabled: bool,
    /// Letzte Abfrage (RFC 3339, UTC); `None`, solange keine lief.
    pub last_check_at: Option<String>,
}

/// Argumente von [`update_check_get`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckGetArgs {}

pub async fn update_check_get(
    ctx: &Arc<AppCtx>,
    _args: UpdateCheckGetArgs,
) -> Result<UpdateCheckSettings> {
    use crate::scheduler::update_check::{SETTING_ENABLED, SETTING_LAST_CHECK};
    let pool = ctx.pool()?;

    Ok(UpdateCheckSettings {
        enabled: crate::db::repo::app_settings::get_bool(&pool, SETTING_ENABLED, false).await?,
        last_check_at: crate::db::repo::app_settings::get(&pool, SETTING_LAST_CHECK).await?,
    })
}

/// Argumente von [`update_check_set`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckSetArgs {
    pub enabled: bool,
}

/// Schaltet die Versionsprüfung ein oder aus; die erste Abfrage nach dem
/// Einschalten macht der nächste Scheduler-Tick (höchstens täglich).
pub async fn update_check_set(ctx: &Arc<AppCtx>, args: UpdateCheckSetArgs) -> Result<()> {
    let UpdateCheckSetArgs { enabled } = args;
    let pool = ctx.pool()?;

    crate::db::repo::app_settings::set_bool(
        &pool,
        crate::scheduler::update_check::SETTING_ENABLED,
        enabled,
    )
    .await
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
