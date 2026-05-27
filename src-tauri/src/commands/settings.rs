//! Tauri-Commands für Settings.
//!
//! Block 2 implementiert nur `seller_profile_{get,upsert}` inkl. §19-Toggle
//! mit 5-Jahres-Bindungs-Schutz. Weitere Settings (mail, payment-accounts, pdf,
//! backup) folgen in Blöcken 4, 5, 9.

use crate::backup::BackupSession;
use crate::branding;
use crate::config::Paths;
use crate::db::models::SellerProfileRow;
use crate::db::repo::seller_profile::{self, SellerProfileInput};
use crate::domain::kleinunternehmer::{hinweis_text, waiver_deadline};
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn seller_profile_get(pool: State<'_, SqlitePool>) -> Result<Option<SellerProfileRow>> {
    seller_profile::get(pool.inner()).await
}

#[tauri::command]
pub async fn seller_profile_upsert(
    pool: State<'_, SqlitePool>,
    input: SellerProfileInput,
) -> Result<SellerProfileRow> {
    seller_profile::upsert(pool.inner(), &input).await
}

/// Logo hochladen: Bytes nach `data/branding/logo.<ext>` schreiben (NICHT
/// `inputs/` — Hard-Rule), Dateinamen im Profil merken. Profil muss existieren.
#[tauri::command]
pub async fn seller_logo_set(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    file_bytes: Vec<u8>,
    file_name: String,
) -> Result<SellerProfileRow> {
    let paths = Paths::from_handle(&app)?;
    let dir = branding::branding_dir(&paths.data_dir);
    let stored = branding::store_logo(&dir, &file_bytes, &file_name)?;
    seller_profile::set_logo_filename(pool.inner(), Some(&stored)).await?;
    seller_profile::get(pool.inner())
        .await?
        .ok_or_else(|| Error::Domain("Profil nach Logo-Upload nicht gefunden.".into()))
}

/// Logo entfernen: Datei löschen + `logo_filename` auf NULL.
#[tauri::command]
pub async fn seller_logo_clear(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
) -> Result<SellerProfileRow> {
    let paths = Paths::from_handle(&app)?;
    let dir = branding::branding_dir(&paths.data_dir);
    if let Some(p) = seller_profile::get(pool.inner()).await? {
        if let Some(fname) = p.logo_filename.as_deref() {
            branding::clear_logo(&dir, fname)?;
        }
    }
    seller_profile::set_logo_filename(pool.inner(), None).await?;
    seller_profile::get(pool.inner())
        .await?
        .ok_or_else(|| Error::Domain("Profil nach Logo-Entfernen nicht gefunden.".into()))
}

/// Liefert das Logo als Data-URL für die In-App-Vorschau (oder `None`).
#[tauri::command]
pub async fn seller_logo_data(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
) -> Result<Option<String>> {
    let paths = Paths::from_handle(&app)?;
    let dir = branding::branding_dir(&paths.data_dir);
    let Some(p) = seller_profile::get(pool.inner()).await? else {
        return Ok(None);
    };
    match p.logo_filename.as_deref() {
        Some(fname) => branding::read_logo_data_url(&dir, fname),
        None => Ok(None),
    }
}

/// Listet alle wählbaren PDF-Vorlagen (eingebettete Built-ins + `inputs/`-
/// Overrides) inkl. §19-Kompatibilität. Block 17a.
#[tauri::command]
pub async fn pdf_templates_list(
    app: AppHandle,
) -> Result<Vec<crate::pdf::templates::TemplateMeta>> {
    let paths = Paths::from_handle(&app)?;
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

/// Rendert eine Beispiel-Rechnung (Dummy-Daten „Max Mustermann“ + Dummy-Logo) mit
/// der gewählten Vorlage und öffnet sie im Standard-PDF-Viewer. Reine Vorschau:
/// schreibt nach `data/preview/`, nichts wird validiert oder archiviert. Block 17a.
#[tauri::command]
pub async fn pdf_template_preview(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    name: String,
) -> Result<()> {
    use crate::domain::invoice::{
        BuyerView, InvoiceDirection, InvoiceInput, InvoiceItemInput, SellerView,
    };

    let paths = Paths::from_handle(&app)?;
    let source = crate::pdf::templates::resolve_invoice_template(&paths.inputs_dir, &name)?;

    // R3-004: Vorschau ist der einzige Render-Pfad, der den §19-Klausel-Check
    // bisher ausgelassen hat. `seller_default_template_set` blockiert eine
    // klausel-lose Vorlage beim Speichern, **wenn** der User §19-Kleinunter-
    // nehmer ist (analoge Seller-aware-Logik). Die Vorschau hat es bisher still
    // gerendert. Symmetrisch zur Speichern-Logik machen: bei §19 vorab prüfen,
    // bei Regelbesteuerung weiterhin frei vorab-render-bar.
    let seller_is_klein = seller_profile::get(pool.inner())
        .await?
        .map(|p| p.is_kleinunternehmer == 1)
        .unwrap_or(true);
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
        iban: Some("DE02 1203 0000 0000 2020 51"),
        bic: Some("BYLADEM1001"),
        is_kleinunternehmer: true,
        waived_since: None,
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

    let pdf = crate::pdf::typst_render::render_invoice(
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
    )?;

    let dir = paths.data_dir.join("preview");
    std::fs::create_dir_all(&dir).map_err(|e| Error::Config(format!("preview-Ordner: {e}")))?;
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
    let file = dir.join(format!("vorschau-{safe}.pdf"));
    std::fs::write(&file, &pdf).map_err(|e| Error::Config(format!("Vorschau schreiben: {e}")))?;

    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(file.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| Error::Config(format!("Vorschau öffnen: {e}")))?;
    Ok(())
}

/// Setzt die globale Standard-PDF-Vorlage (gilt für neu gerenderte Rechnungen
/// und Angebote). §19-Schutz: bei Kleinunternehmer darf nur eine §19-kompatible
/// Vorlage gewählt werden (Marker + Hinweis-Text). Block 17a.
#[tauri::command]
pub async fn seller_default_template_set(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    name: String,
) -> Result<SellerProfileRow> {
    let paths = Paths::from_handle(&app)?;
    let templates = crate::pdf::templates::list_templates(&paths.inputs_dir)?;
    let meta = templates
        .iter()
        .find(|t| t.name == name)
        .ok_or_else(|| Error::Domain(format!("Unbekannte Vorlage: {name}")))?;

    let profile = seller_profile::get(pool.inner())
        .await?
        .ok_or_else(|| Error::Domain("Bitte zuerst die Firmendaten speichern.".into()))?;
    if profile.is_kleinunternehmer == 1 && !meta.klausel_status.is_klein_compatible {
        return Err(Error::Domain(format!(
            "Vorlage „{name}“ ist nicht §19-konform (ohne Kleinunternehmer-Klausel) und kann nicht gewählt werden, solange du Kleinunternehmer bist."
        )));
    }

    seller_profile::set_default_pdf_template(pool.inner(), &name).await?;
    seller_profile::get(pool.inner())
        .await?
        .ok_or_else(|| Error::Domain("Profil nach Vorlagen-Wahl nicht gefunden.".into()))
}

/// Liefert dem Frontend die §19-Eckdaten — wortgleicher Hinweis-Text und
/// (falls relevant) das Rückkehr-Datum. Verhindert, dass das Frontend die
/// Klausel selbst formuliert.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Paragraph19Info {
    pub hinweis_text: &'static str,
    pub return_date_after_waiver: Option<String>,
}

#[tauri::command]
pub async fn paragraph_19_info(pool: State<'_, SqlitePool>) -> Result<Paragraph19Info> {
    let profile = seller_profile::get(pool.inner()).await?;
    let return_date = profile
        .and_then(|p| p.waived_paragraph_19_since)
        .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
        .map(|d| waiver_deadline(d).to_string());
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

#[tauri::command]
pub async fn travel_settings_get(pool: State<'_, SqlitePool>) -> Result<TravelSettings> {
    let cost_per_km_cents =
        crate::db::repo::app_settings::get(pool.inner(), "travel_cost_per_km_cents")
            .await?
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0);
    let round_trip_default =
        crate::db::repo::app_settings::get_bool(pool.inner(), "travel_round_trip_default", false)
            .await?;
    Ok(TravelSettings {
        cost_per_km_cents,
        round_trip_default,
    })
}

#[tauri::command]
pub async fn travel_settings_set(
    pool: State<'_, SqlitePool>,
    cost_per_km_cents: i64,
    round_trip_default: bool,
) -> Result<()> {
    if cost_per_km_cents < 0 {
        return Err(Error::Domain(
            "Der Kilometersatz darf nicht negativ sein.".into(),
        ));
    }
    crate::db::repo::app_settings::set(
        pool.inner(),
        "travel_cost_per_km_cents",
        &cost_per_km_cents.to_string(),
    )
    .await?;
    crate::db::repo::app_settings::set_bool(
        pool.inner(),
        "travel_round_trip_default",
        round_trip_default,
    )
    .await?;
    // R2-026: Audit-Schreiben hier rein kosmetisch (Reisesatz-Update ist
    // kein GoBD-Beleg, sondern Settings). Fehler werden geloggt, sperren
    // den Settings-Save aber nicht.
    if let Err(e) = crate::db::repo::audit_log::append(
        pool.inner(),
        "settings.travel.update",
        "settings",
        "travel",
        Some(&format!(
            r#"{{"cost_per_km_cents":{cost_per_km_cents},"round_trip_default":{round_trip_default}}}"#
        )),
    )
    .await
    {
        tracing::warn!("audit_log.settings.travel.update fehlgeschlagen: {e}");
    }
    Ok(())
}

/// Berechnet eine Anfahrts-Position aus km + gespeichertem Satz. Geld-Mathematik
/// bleibt in Rust (autoritativ), das Frontend fügt das Ergebnis als Position ein.
#[tauri::command]
pub async fn travel_compute(
    pool: State<'_, SqlitePool>,
    km: f64,
    round_trip: bool,
) -> Result<crate::domain::travel::TravelLine> {
    let cents = crate::db::repo::app_settings::get(pool.inner(), "travel_cost_per_km_cents")
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

/// Unterschrift-Bild hochladen → `data/branding/signature.<ext>` (NICHT inputs/).
/// Gibt die Data-URL für die sofortige Vorschau zurück.
#[tauri::command]
pub async fn seller_signature_set(
    app: AppHandle,
    file_bytes: Vec<u8>,
    file_name: String,
) -> Result<Option<String>> {
    let paths = Paths::from_handle(&app)?;
    let dir = branding::branding_dir(&paths.data_dir);
    let stored = branding::store_signature(&dir, &file_bytes, &file_name)?;
    branding::read_logo_data_url(&dir, &stored)
}

/// Unterschrift-Bild entfernen (idempotent).
#[tauri::command]
pub async fn seller_signature_clear(app: AppHandle) -> Result<()> {
    let paths = Paths::from_handle(&app)?;
    let dir = branding::branding_dir(&paths.data_dir);
    if let Some((name, _)) = branding::find_signature(&dir) {
        branding::clear_logo(&dir, &name)?;
    }
    Ok(())
}

/// Unterschrift als Data-URL für die In-App-Vorschau (oder `None`).
#[tauri::command]
pub async fn seller_signature_data(app: AppHandle) -> Result<Option<String>> {
    let paths = Paths::from_handle(&app)?;
    let dir = branding::branding_dir(&paths.data_dir);
    match branding::find_signature(&dir) {
        Some((name, _)) => branding::read_logo_data_url(&dir, &name),
        None => Ok(None),
    }
}

/// Globaler Schalter „Unterschriftenfelder auf Angeboten".
#[tauri::command]
pub async fn quote_signature_get(pool: State<'_, SqlitePool>) -> Result<bool> {
    crate::db::repo::app_settings::get_bool(pool.inner(), "quote_signature_enabled", false).await
}

#[tauri::command]
pub async fn quote_signature_set(pool: State<'_, SqlitePool>, enabled: bool) -> Result<()> {
    crate::db::repo::app_settings::set_bool(pool.inner(), "quote_signature_enabled", enabled).await
}

// ---- Inhaber-Name + Standard-Fristen ---------------------------------------

/// Inhaber (Vor- und Nachname) — §14-Pflicht bei Einzelunternehmern/Freiberuflern.
/// Leerer Wert → None.
#[tauri::command]
pub async fn seller_owner_get(pool: State<'_, SqlitePool>) -> Result<Option<String>> {
    Ok(
        crate::db::repo::app_settings::get(pool.inner(), "seller_owner_name")
            .await?
            .filter(|s| !s.trim().is_empty()),
    )
}

#[tauri::command]
pub async fn seller_owner_set(
    pool: State<'_, SqlitePool>,
    owner_name: Option<String>,
) -> Result<()> {
    let v = owner_name
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("");
    crate::db::repo::app_settings::set(pool.inner(), "seller_owner_name", v).await
}

/// Standard-Fristen: wie viele Tage ein Angebot gültig ist und in wie vielen
/// Tagen eine Rechnung fällig wird (Vorbelegung in den Formularen).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentTerms {
    pub quote_valid_days: i64,
    pub invoice_due_days: i64,
}

#[tauri::command]
pub async fn document_terms_get(pool: State<'_, SqlitePool>) -> Result<DocumentTerms> {
    let quote_valid_days = crate::db::repo::app_settings::get(pool.inner(), "quote_valid_days")
        .await?
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(30);
    let invoice_due_days = crate::db::repo::app_settings::get(pool.inner(), "invoice_due_days")
        .await?
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(14);
    Ok(DocumentTerms {
        quote_valid_days,
        invoice_due_days,
    })
}

#[tauri::command]
pub async fn document_terms_set(
    pool: State<'_, SqlitePool>,
    quote_valid_days: i64,
    invoice_due_days: i64,
) -> Result<()> {
    crate::db::repo::app_settings::set(
        pool.inner(),
        "quote_valid_days",
        &quote_valid_days.max(0).to_string(),
    )
    .await?;
    crate::db::repo::app_settings::set(
        pool.inner(),
        "invoice_due_days",
        &invoice_due_days.max(0).to_string(),
    )
    .await?;
    Ok(())
}

// ---- Drop-Folder fuer eingehende E-Rechnungen (Block PV1-DROP) -------------

/// Konfiguration des Watched-Folders fuer eingehende E-Rechnungen.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DropFolderSettings {
    /// Ist der periodische Sync (Tick + App-Start) aktiv?
    pub enabled: bool,
    /// Absoluter Ordner-Pfad. Leer = noch nicht gewaehlt.
    pub path: String,
}

#[tauri::command]
pub async fn drop_folder_settings_get(pool: State<'_, SqlitePool>) -> Result<DropFolderSettings> {
    let enabled =
        crate::db::repo::app_settings::get_bool(pool.inner(), "drop_folder_enabled", false).await?;
    let path = crate::db::repo::app_settings::get(pool.inner(), "drop_folder_path")
        .await?
        .unwrap_or_default();
    Ok(DropFolderSettings { enabled, path })
}

#[tauri::command]
pub async fn drop_folder_settings_set(
    pool: State<'_, SqlitePool>,
    args: DropFolderSettings,
) -> Result<DropFolderSettings> {
    let pool = pool.inner();
    let trimmed = args.path.trim().to_string();

    // Pre-Check beim Aktivieren: leerer Pfad oder fehlender/unzugaenglicher
    // Ordner blockt das Einschalten. Das verhindert den haeufigsten Fehler
    // (Toggle on, Pfad noch nicht gepickt) und macht den Sync danach
    // deterministisch — kein silent-disable mehr.
    if args.enabled {
        if trimmed.is_empty() {
            return Err(Error::Domain(
                "Bitte zuerst einen Ordner auswaehlen, bevor der Drop-Folder aktiviert wird."
                    .into(),
            ));
        }
        let p = std::path::Path::new(&trimmed);
        if !p.is_dir() {
            return Err(Error::Domain(format!(
                "Der gewaehlte Ordner existiert nicht oder ist kein Verzeichnis: {trimmed}"
            )));
        }
        // Schreibbarkeit testen: temporaere Datei anlegen + sofort loeschen.
        // `processed/`/`failed/` werden beim ersten Sync erst angelegt — wir
        // wollen den Fehler aber jetzt im Settings-Save sehen, nicht erst
        // beim Tick.
        let probe = p.join(".klein-buch-write-probe");
        match std::fs::write(&probe, b"probe") {
            Ok(()) => {
                let _ = std::fs::remove_file(&probe);
            }
            Err(e) => {
                return Err(Error::Domain(format!(
                    "Im gewaehlten Ordner kann nicht geschrieben werden ({trimmed}): {e}"
                )));
            }
        }
    }

    crate::db::repo::app_settings::set_bool(pool, "drop_folder_enabled", args.enabled).await?;
    crate::db::repo::app_settings::set(pool, "drop_folder_path", &trimmed).await?;

    // Settings-Update protokollieren (kein GoBD-Beleg, aber audit-relevant fuer
    // „wer hat den Drop-Folder umgestellt?"). Fehler werden geloggt, sperren
    // den Save nicht — Pattern wie bei travel_settings_set.
    if let Err(e) = crate::db::repo::audit_log::append(
        pool,
        "settings.drop_folder.update",
        "settings",
        "drop_folder",
        Some(&format!(
            r#"{{"enabled":{},"path":"{}"}}"#,
            args.enabled,
            trimmed.replace('\\', "\\\\").replace('"', "\\\"")
        )),
    )
    .await
    {
        tracing::warn!("audit_log.settings.drop_folder.update fehlgeschlagen: {e}");
    }

    Ok(DropFolderSettings {
        enabled: args.enabled,
        path: trimmed,
    })
}

/// Manueller Trigger fuer den Drop-Folder-Sync — nuetzlich, wenn Manuel nicht
/// 5 Minuten auf den naechsten Tick warten will (Settings-Page-Button „Jetzt
/// synchronisieren"). Liefert dem Frontend Kurz-Counter zurueck, damit es
/// einen Toast schreiben kann.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DropFolderSyncResult {
    pub skipped_disabled: bool,
    pub imported: usize,
    pub failed: usize,
    pub ignored_hidden: usize,
}

#[tauri::command]
pub async fn drop_folder_sync_now(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    session: State<'_, BackupSession>,
) -> Result<DropFolderSyncResult> {
    let paths = Paths::from_handle(&app)?;
    let today = chrono::Local::now().date_naive();
    let report =
        crate::scheduler::drop_folder::run_sync(pool.inner(), &paths, session.inner(), today)
            .await?;
    Ok(DropFolderSyncResult {
        skipped_disabled: report.skipped_disabled,
        imported: report.imported,
        failed: report.failed,
        ignored_hidden: report.ignored_hidden,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
