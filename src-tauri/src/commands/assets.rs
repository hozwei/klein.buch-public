//! Commands für Anlagen (Block 12, Phase 2C).
//!
//! Orchestriert:
//! - Domain-Validation ([`crate::domain::asset::validate_asset`]) — Hard-Block.
//! - AfA-Methoden-Auflösung (Nutzungsdauer je Methode) + Start-Restbuchwert
//!   (anteilig Privatanteil).
//! - Lieferanten-/Quell-Kosten-Verknüpfung (inkl. Schutz gegen Doppel-Aktivierung).
//! - Counter-Allokation `AV-{YYYY}-{NNNN}` ([`crate::db::numbering`]).
//! - Persistenz ([`crate::db::repo::assets`]) + Audit + Auto-Backup (best-effort).
//!
//! ## Lebenszyklus
//!
//! Anlegen → (bei Bedarf korrigieren, solange unlocked) → AfA buchen (lockt) →
//! ggf. veräußern. Gelöscht wird nie (GoBD-Hardline).

use crate::app_ctx::AppCtx;
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::assets::afa_tabellen::{self, AfaTabellen};
use crate::backup;
use crate::db::models::{AssetDetail, AssetRow, DepreciationEntryRow};
use crate::db::numbering;
use crate::db::repo::{assets, audit_log, depreciation as depreciation_repo, expenses};
use crate::domain::asset::{
    self, business_book_value_start_cents, AssetInput, DepreciationMethod, DisposalType,
    MethodSuggestion,
};
use crate::domain::depreciation::{
    compute_disposal_year_partial, plan_disposal, BookedYear, DepreciationAsset,
};
use crate::domain::numbering::DocType;
use crate::error::{Error, Result};
use crate::fiscal_year::guard;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetDisposeArgs {
    pub asset_id: String,
    /// Veräußerungsdatum (YYYY-MM-DD).
    pub disposal_date: NaiveDate,
    /// 'sale' | 'scrap' | 'given_away'.
    pub disposal_type: String,
    /// Verkaufserlös in Cent (nur bei 'sale' relevant; sonst 0).
    pub proceeds_cents: i64,
}

// =============================================================================
// Read
// =============================================================================

/// Argumente von [`assets_list`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetsListArgs {
    #[serde(default)]
    pub filter: Option<assets::ListFilter>,
}

pub async fn assets_list(
    ctx: &Arc<AppCtx>,
    args: AssetsListArgs,
) -> Result<Vec<crate::db::models::AssetListItem>> {
    let AssetsListArgs { filter } = args;
    let pool = ctx.pool()?;

    assets::list(&pool, &filter.unwrap_or_default()).await
}

/// Argumente von [`assets_get`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetsGetArgs {
    pub id: String,
}

pub async fn assets_get(ctx: &Arc<AppCtx>, args: AssetsGetArgs) -> Result<Option<AssetDetail>> {
    let AssetsGetArgs { id } = args;
    let pool = ctx.pool()?;

    assets::get_detail(&pool, &id).await
}

/// Argumente von [`assets_afa_table`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetsAfaTableArgs {}

/// Liefert die geladene BMF-AfA-Tabelle (Kategorien + GWG-Grenze) fürs Formular.
pub async fn assets_afa_table(ctx: &Arc<AppCtx>, _args: AssetsAfaTableArgs) -> Result<AfaTabellen> {
    let paths = ctx.paths.clone();
    afa_tabellen::load(&paths.inputs_dir)
}

/// Argumente von [`assets_suggest_method`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetsSuggestMethodArgs {
    #[serde(default)]
    pub expense_category: Option<String>,
    pub acquisition_cost_cents: i64,
    /// Lauf F8: Quell-Kosten beim Aktivieren. Dann gilt für die GWG-Grenze deren
    /// Nettobetrag, und die Antwort enthält die Vorbelegung aus den Kosten.
    #[serde(default)]
    pub expense_id: Option<String>,
}

/// Vorbelegung einer Anlage aus ihren Quell-Kosten (Lauf F8).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetSourceDefaults {
    /// Belegdatum der Kosten.
    pub acquisition_date: String,
    /// Anschaffungskosten: brutto bei §19, netto bei Regelbesteuerung
    /// (§ 9b Abs. 1 EStG; Status bei Leistungsbezug = Jahr des Belegdatums,
    /// Lauf F8d).
    pub acquisition_cost_cents: i64,
    /// Betrag ohne Umsatzsteuer — Grundlage der GWG-Grenze.
    pub gwg_basis_cents: i64,
    pub is_kleinunternehmer: bool,
    /// Kosten mit §13b-Schalter (Lauf F8c): bei §19 gehört die selbst geschuldete
    /// Steuer in eine eigene Kostenposition und Anlage (ADR 0039 Nr. 147).
    pub reverse_charge_13b: bool,
}

/// Antwort von [`assets_suggest_method`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetSuggestion {
    #[serde(flatten)]
    pub suggestion: MethodSuggestion,
    pub source: Option<AssetSourceDefaults>,
}

/// Schlägt AfA-Methode + Nutzungsdauer vor (PRD §6.17). `expense_category` ist die
/// EÜR-Kategorie der Quell-Kosten (falls vorhanden). Mit `expense_id` (Lauf F8)
/// prüft der Vorschlag die GWG-Grenze am Nettobetrag der Kosten und liefert die
/// Vorbelegung (Belegdatum, Anschaffungskosten brutto bei §19).
pub async fn assets_suggest_method(
    ctx: &Arc<AppCtx>,
    args: AssetsSuggestMethodArgs,
) -> Result<AssetSuggestion> {
    let AssetsSuggestMethodArgs {
        expense_category,
        acquisition_cost_cents,
        expense_id,
    } = args;

    let paths = ctx.paths.clone();
    let table = afa_tabellen::load(&paths.inputs_dir)?;
    let source = match expense_id.as_deref() {
        Some(eid) => {
            let pool = ctx.pool()?;
            let expense = expenses::get(&pool, eid)
                .await?
                .ok_or_else(|| Error::Domain(format!("Quell-Kosten nicht gefunden: {eid}")))?;
            let is_klein = expense_is_klein(&pool, &expense).await?;
            Some(AssetSourceDefaults {
                acquisition_date: expense.expense_date.chars().take(10).collect(),
                acquisition_cost_cents: asset::acquisition_cost_from_expense(
                    expense.net_amount_cents,
                    expense.gross_amount_cents,
                    is_klein,
                ),
                gwg_basis_cents: expense.net_amount_cents,
                is_kleinunternehmer: is_klein,
                reverse_charge_13b: expense.reverse_charge_13b == 1,
            })
        }
        None => None,
    };
    let gwg_basis = source
        .as_ref()
        .map_or(acquisition_cost_cents, |s| s.gwg_basis_cents);
    Ok(AssetSuggestion {
        suggestion: asset::suggest_method(
            expense_category.as_deref(),
            gwg_basis,
            table.gwg_threshold_cents,
        ),
        source,
    })
}

/// §19-Status, unter dem Kosten steuerlich zu behandeln sind (Lauf F8d, ADR 0039
/// Nr. 148): Status bei Leistungsbezug, d. h. im Jahr des Belegdatums (§ 15
/// Abs. 1 UStG, Abschn. 19.5 UStAE) — nicht im Zahlungsjahr. Dasselbe Kriterium
/// nutzen EÜR, Ausfüllhilfe und DATEV-Stapel
/// ([`crate::db::repo::seller_profile::is_klein_for_expense`]). Das Belegdatum
/// ist ein festgeschriebenes Kernfeld (`trg_expenses_immutable`); weder ein
/// Zahldatum noch eine andere Änderung der Kosten kann den Status verschieben.
async fn expense_is_klein(
    pool: &sqlx::SqlitePool,
    expense: &crate::db::models::ExpenseRow,
) -> Result<bool> {
    let profile = crate::db::repo::seller_profile::get(pool).await?;
    crate::db::repo::seller_profile::is_klein_for_expense(profile.as_ref(), &expense.expense_date)
}

/// §19-Status der Quell-Kosten für `assets_create`/`assets_update` (Lauf F8e,
/// ADR 0039 Nr. 148 (5)). Bei einer Anlage ist der Leistungsbezug die
/// Lieferung (§ 9a EStDV), also das eingegebene Anschaffungsdatum; Klein.Buch
/// bestimmt den Status der Kosten aber nach dem Belegdatum. Liegen beide Daten
/// in Jahren mit unterschiedlichem Status, lässt sich der Fall nicht abbilden:
/// Ablehnung mit beiden Daten und beiden Status und Verweis an die
/// Steuerberatung. Sonst der gemeinsame Status.
async fn expense_status_for_asset(
    pool: &sqlx::SqlitePool,
    expense: &crate::db::models::ExpenseRow,
    acquisition_date: NaiveDate,
) -> Result<bool> {
    let profile = crate::db::repo::seller_profile::get(pool).await?;
    let doc_klein = crate::db::repo::seller_profile::is_klein_for_expense(
        profile.as_ref(),
        &expense.expense_date,
    )?;
    let acq_klein = crate::db::repo::seller_profile::is_klein_for_year(
        profile.as_ref(),
        acquisition_date.year(),
    );
    if doc_klein == acq_klein {
        return Ok(doc_klein);
    }
    let label = |klein: bool| {
        if klein {
            "Kleinunternehmer nach §19"
        } else {
            "Regelbesteuerung"
        }
    };
    let head = expense
        .expense_date
        .get(..10)
        .unwrap_or(&expense.expense_date);
    let doc = NaiveDate::parse_from_str(head, "%Y-%m-%d")
        .map(|d| d.format("%d.%m.%Y").to_string())
        .unwrap_or_else(|_| expense.expense_date.clone());
    Err(Error::Domain(format!(
        "Anschaffungsdatum {} ({}) und Belegdatum {doc} der Kosten {} ({}) liegen in \
         Jahren mit unterschiedlichem Steuerstatus. Für Vorsteuerabzug und \
         Anschaffungskosten ist der Status bei Leistungsbezug maßgeblich, bei einer \
         Anlage die Lieferung (§ 9a EStDV, § 15 Abs. 1 UStG, § 9b Abs. 1 EStG). \
         Klein.Buch bestimmt den Status von Kosten nach dem Belegdatum und kann diesen \
         Fall nicht abbilden. Prüfe das Anschaffungsdatum; stimmt es, klärst Du die \
         Erfassung mit Deiner Steuerberatung.",
        acquisition_date.format("%d.%m.%Y"),
        label(acq_klein),
        expense.expense_number,
        label(doc_klein),
    )))
}

/// (Belegjahr, Zahlungsjahr oder Belegjahr) einer Kostenposition — die Jahre,
/// deren EÜR das Aktivieren ändert.
fn expense_booking_years(expense: &crate::db::models::ExpenseRow) -> (i64, i64) {
    let paid_year = expense
        .paid_date
        .as_deref()
        .and_then(|d| d.get(..4))
        .and_then(|y| y.parse::<i64>().ok());
    (
        expense.fiscal_year,
        paid_year.unwrap_or(expense.fiscal_year),
    )
}

/// Aus Kosten übernommene Anlage, deren Anschaffungskosten nicht (mehr) zum
/// §19-Status der Kosten bei Leistungsbezug passen (Lauf F8d) — typischerweise
/// nach einem Verzicht auf §19, der ab Jahresbeginn wirkt, oder einer Rückkehr.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapitalizedCostMismatch {
    pub asset_number: String,
    pub expense_number: String,
    pub acquisition_cost_cents: i64,
    /// Anschaffungskosten nach § 9b Abs. 1 EStG unter dem heutigen Status.
    pub expected_cents: i64,
    /// `true` = Korrektur in Klein.Buch möglich (Anlage nicht abgegangen,
    /// Anschaffungsjahr offen, keine AfA in einem abgeschlossenen Jahr,
    /// Anschaffungs- und Belegdatum unter demselben Status, Lauf F8e);
    /// `false` = nur mit der Steuerberatung.
    pub correctable: bool,
}

/// Alle aus (nicht stornierten) Kosten übernommenen Anlagen, deren
/// Anschaffungskosten unter dem §19-Status der Kosten bei Leistungsbezug
/// [`asset::check_cost_against_expense`] nicht genügen (Lauf F8d). Lesend;
/// Klein.Buch ändert keine Anlage still.
pub async fn capitalized_cost_mismatches(
    pool: &sqlx::SqlitePool,
) -> Result<Vec<CapitalizedCostMismatch>> {
    #[allow(clippy::type_complexity)]
    let rows: Vec<(
        String,
        String,
        i64,
        i64,
        String,
        String,
        i64,
        i64,
        i64,
        i64,
        i64,
    )> = sqlx::query_as(
        "SELECT a.id, a.asset_number, a.acquisition_cost_cents, a.disposed,
                e.expense_number, e.expense_date, e.net_amount_cents, e.gross_amount_cents,
                e.reverse_charge_13b, a.acquisition_fiscal_year,
                CAST(substr(a.acquisition_date, 1, 4) AS INTEGER)
           FROM assets a
           JOIN expenses e ON e.id = a.expense_id AND e.capitalized_as_asset_id = a.id
          WHERE e.status = 'recorded'
          ORDER BY a.asset_number",
    )
    .fetch_all(pool)
    .await?;
    let profile = crate::db::repo::seller_profile::get(pool).await?;
    let mut out = Vec::new();
    for (
        id,
        asset_number,
        cost,
        disposed,
        expense_number,
        expense_date,
        net,
        gross,
        rc,
        acq_year,
        acq_date_year,
    ) in rows
    {
        let klein =
            crate::db::repo::seller_profile::is_klein_for_expense(profile.as_ref(), &expense_date)?;
        if asset::check_cost_against_expense(cost, net, gross, klein, rc == 1).is_none() {
            continue;
        }
        // Lauf F8e: Liegt das Anschaffungsdatum (Lieferung, § 9a EStDV) in einem
        // Jahr mit anderem Status als das Belegdatum, wäre der Soll-Betrag nach
        // dem Belegdatum kein sicherer Korrekturweg (`expense_status_for_asset`
        // lehnt ihn ab): nur mit der Steuerberatung.
        let acq_klein = i32::try_from(acq_date_year)
            .ok()
            .map(|y| crate::db::repo::seller_profile::is_klein_for_year(profile.as_ref(), y));
        let mut correctable =
            disposed == 0 && acq_klein == Some(klein) && !guard::is_closed(pool, acq_year).await?;
        if correctable {
            let years: Vec<(i64,)> = sqlx::query_as(
                "SELECT DISTINCT fiscal_year FROM depreciation_entries WHERE asset_id = ?",
            )
            .bind(&id)
            .fetch_all(pool)
            .await?;
            for (y,) in years {
                if guard::is_closed(pool, y).await? {
                    correctable = false;
                    break;
                }
            }
        }
        out.push(CapitalizedCostMismatch {
            asset_number,
            expense_number,
            acquisition_cost_cents: cost,
            expected_cents: asset::acquisition_cost_from_expense(net, gross, klein),
            correctable,
        });
    }
    Ok(out)
}

/// Inbox-Hinweis nach einem Wechsel des §19-Status in den Einstellungen
/// (Verzicht oder Rückkehr, Lauf F8d): nennt jede betroffene Anlage mit dem
/// Handlungsweg. Der Wechsel selbst wird nicht blockiert (gesetzliches Recht,
/// § 19 Abs. 1 und 3 UStG). Best-effort; ohne betroffene Anlage kein Hinweis.
pub async fn notify_capitalized_status_change(pool: &sqlx::SqlitePool) {
    let list = match capitalized_cost_mismatches(pool).await {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!("Prüfung aktivierter Kosten nach §19-Wechsel fehlgeschlagen: {e}");
            return;
        }
    };
    if list.is_empty() {
        return;
    }
    use crate::euer::detail::eur_grouped as eur;
    let lines: Vec<String> = list
        .iter()
        .map(|m| {
            let way = if m.correctable {
                "Anschaffungskosten in der Anlage korrigieren (eine gebuchte AfA vorher im \
                 offenen Geschäftsjahr zurücksetzen)"
            } else {
                "Korrektur in Klein.Buch nicht möglich (AfA in einem abgeschlossenen Jahr \
                 festgeschrieben, Anlage abgegangen oder Anschaffungsdatum in einem Jahr mit \
                 anderem Steuerstatus als das Belegdatum): mit der Steuerberatung klären"
            };
            format!(
                "Anlage {} (Kosten {}): {} statt {} — {way}.",
                m.asset_number,
                m.expense_number,
                eur(m.acquisition_cost_cents),
                eur(m.expected_cents)
            )
        })
        .collect();
    let body = format!(
        "Mit dem geänderten Steuerstatus passen die Anschaffungskosten dieser aus Kosten \
         übernommenen Anlagen nicht mehr. Maßgeblich ist der Status bei Leistungsbezug \
         (Belegdatum der Kosten): als Kleinunternehmer der Bruttobetrag, bei \
         Regelbesteuerung der Nettobetrag (§ 9b Abs. 1 EStG, § 15 Abs. 1 UStG). Klein.Buch \
         wendet einen Wechsel immer ab Beginn des Kalenderjahres an; das passt zum Verzicht \
         auf §19 und zum Wegfall, weil der Vorjahresumsatz über 25.000 € lag. {} Ist §19 \
         dagegen im laufenden Jahr durch Überschreiten der 100.000-€-Grenze weggefallen \
         (§ 19 Abs. 1 UStG), bleiben Kosten mit Leistungsbezug vor dem Überschreiten beim \
         Bruttobetrag ohne Vorsteuer: Dann diese Anlagen nicht korrigieren und die \
         Erfassung des Jahres mit der Steuerberatung klären; einen Wechsel mitten im Jahr \
         bildet Klein.Buch nicht ab. Eine Vorsteuerberichtigung nach § 15a UStG für Anlagen \
         aus früheren Jahren berechnet Klein.Buch nicht.",
        lines.join(" ")
    );
    let numbers: Vec<&str> = list.iter().map(|m| m.asset_number.as_str()).collect();
    let dedup = format!("capitalized_status_change:{}", numbers.join(","));
    let n = crate::notify::NewNotification {
        title: "Anschaffungskosten nach §19-Wechsel prüfen",
        body: &body,
        severity: "warning",
        related_entity_type: Some("asset"),
        action_url: Some("/assets"),
        dedup_key: Some(&dedup),
        ..Default::default()
    };
    if let Err(e) = crate::notify::emit(pool, n).await {
        tracing::warn!("Hinweis zu aktivierten Kosten nicht angelegt: {e}");
    }
}

// =============================================================================
// Create
// =============================================================================

/// Argumente von [`assets_create`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetsCreateArgs {
    pub input: AssetInput,
}

pub async fn assets_create(ctx: &Arc<AppCtx>, args: AssetsCreateArgs) -> Result<AssetDetail> {
    let AssetsCreateArgs { input } = args;
    let pool = ctx.pool()?;
    let session = &ctx.session;

    let pool = &pool;
    let paths = ctx.paths.clone();

    // Hard-Block bei Validierungsfehlern.
    if let Err(errs) = asset::validate_asset(&input, today_berlin()) {
        return Err(Error::Domain(format!(
            "Anlage kann nicht angelegt werden: {}",
            errs.iter()
                .map(asset::message)
                .collect::<Vec<_>>()
                .join("; ")
        )));
    }

    let (method_db, useful_life) = resolve_method(&input)?;
    let book_value =
        business_book_value_start_cents(input.acquisition_cost_cents, input.business_share_percent);
    let fiscal_year = input.acquisition_date.year() as i64;

    // R2-022 (GJ-Guard): Anlage darf nicht in ein festgeschriebenes GJ
    // eingebucht werden — sonst würde die AfA dieses Jahres nachträglich
    // den Lock-Snapshot verbiegen.
    let fy = guard::ensure_year_open(pool, fiscal_year).await?;

    // Quell-Kosten prüfen (Lauf F8). Seit F5b nimmt die EÜR aktivierte Kosten
    // heraus; das Aktivieren ändert also das Jahr der Kosten. Beleg- und
    // Zahlungsjahr müssen offen sein (unter derselben Erlaubnis), stornierte
    // Kosten lassen sich nicht aktivieren, und die Anschaffungskosten folgen
    // § 9b Abs. 1 EStG (bei §19 brutto).
    let mut source: Option<crate::db::models::ExpenseRow> = None;
    if let Some(eid) = input.expense_id.as_deref() {
        let expense = expenses::get(pool, eid)
            .await?
            .ok_or_else(|| Error::Domain(format!("Quell-Kosten nicht gefunden: {eid}")))?;
        if let Some(existing) = expense.capitalized_as_asset_id.as_deref() {
            return Err(Error::Domain(format!(
                "Diese Kosten wurden bereits als Anlage aktiviert ({existing})."
            )));
        }
        if expense.status != "recorded" {
            return Err(Error::Domain(
                "Stornierte Kosten lassen sich nicht als Anlage aktivieren.".into(),
            ));
        }
        let (doc_year, paid_year) = expense_booking_years(&expense);
        for year in [doc_year, paid_year] {
            if guard::is_closed(pool, year).await? {
                return Err(Error::Domain(format!(
                    "Die Kosten {} gehören zum abgeschlossenen Geschäftsjahr {year}. \
                     Sie stecken dort schon in der EÜR; ein Aktivieren würde das \
                     abgeschlossene Jahr nachträglich ändern. Eine Korrektur klärst \
                     Du mit Deiner Steuerberatung.",
                    expense.expense_number
                )));
            }
        }
        let is_klein = expense_status_for_asset(pool, &expense, input.acquisition_date).await?;
        if let Some(msg) = asset::check_cost_against_expense(
            input.acquisition_cost_cents,
            expense.net_amount_cents,
            expense.gross_amount_cents,
            is_klein,
            expense.reverse_charge_13b == 1,
        ) {
            return Err(Error::Domain(msg));
        }
        source = Some(expense);
    }

    // Lauf F8: Nummer, Anlage, Verweis der Quell-Kosten und Audit in **einer**
    // Transaktion — keine Nummer ohne Spur, keine doppelte Aktivierung (die
    // Bedingung im UPDATE greift auch bei zwei gleichzeitigen Aufrufen).
    let mut tx = crate::db::begin_immediate(pool).await?;
    let asset_number =
        numbering::next_number_in(&mut tx, DocType::Asset, fiscal_year as i32).await?;
    let row = assets::create_in(
        &mut tx,
        &input,
        &asset_number,
        fiscal_year,
        &method_db,
        useful_life,
        book_value,
    )
    .await?;
    if let Some(expense) = source.as_ref() {
        let res = sqlx::query(
            "UPDATE expenses SET capitalized_as_asset_id = ?
              WHERE id = ? AND capitalized_as_asset_id IS NULL AND status = 'recorded'
                AND paid_date IS ?",
        )
        .bind(&row.id)
        .bind(&expense.id)
        .bind(expense.paid_date.as_deref())
        .execute(&mut *tx)
        .await?;
        if res.rows_affected() != 1 {
            return Err(Error::Domain(
                "Diese Kosten wurden inzwischen als Anlage aktiviert, storniert oder ihr \
                 Zahldatum wurde geändert. Bitte neu laden."
                    .into(),
            ));
        }
    }
    audit_log::append_in(
        &mut tx,
        "asset.create",
        "asset",
        &row.id,
        Some(serde_json::json!({ "number": &asset_number, "cost": row.acquisition_cost_cents, "method": &method_db, "business_share": row.business_share_percent, "book_value": book_value, "expense_id": input.expense_id.as_deref() })),
    )
    .await?;
    tx.commit().await?;
    drop(fy);

    backup::request_auto_backup(pool, &paths, session, "asset.create", ctx.clock().now_utc())
        .await
        .ok();

    assets::get_detail(pool, &row.id)
        .await?
        .ok_or_else(|| Error::Domain("create: detail-load leer".into()))
}

// =============================================================================
// Update (nur solange unlocked / nicht veräußert)
// =============================================================================

/// Argumente von [`assets_update`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetsUpdateArgs {
    pub id: String,
    pub input: AssetInput,
}

pub async fn assets_update(ctx: &Arc<AppCtx>, args: AssetsUpdateArgs) -> Result<AssetDetail> {
    let AssetsUpdateArgs { id, input } = args;
    let pool = ctx.pool()?;
    let session = &ctx.session;

    let pool = &pool;
    let paths = ctx.paths.clone();

    if let Err(errs) = asset::validate_asset(&input, today_berlin()) {
        return Err(Error::Domain(format!(
            "Anlage kann nicht geändert werden: {}",
            errs.iter()
                .map(asset::message)
                .collect::<Vec<_>>()
                .join("; ")
        )));
    }

    // Lauf F2c: Eine veräußerte Anlage ist abgeschlossen — vor der Prüfung der
    // AfA-Buchungen, damit die Meldung nicht zum Zurücksetzen rät (DB-Backstop
    // `trg_assets_disposal_final`).
    let existing = assets::get(pool, &id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Anlage nicht gefunden: {id}")))?;
    if existing.disposed == 1 {
        return Err(Error::Domain(
            "Anlage ist bereits veräußert — keine Stammdaten-Änderung möglich.".into(),
        ));
    }

    // Solange AfA gebucht ist, sind die Stammdaten nicht änderbar (sonst würden die
    // Buchungen inkonsistent). Erst die Abschreibung des offenen GJ zurücksetzen.
    if depreciation_repo::count_for_asset(pool, &id).await? > 0 {
        return Err(Error::Domain(
            "Für diese Anlage ist bereits eine Abschreibung gebucht. Bitte erst die \
             Abschreibung zurücksetzen (offenes Geschäftsjahr), dann bearbeiten."
                .into(),
        ));
    }

    // R2-022 (GJ-Guard): existierender Anschaffungs-GJ und neu gewähltes
    // Anschaffungs-Jahr müssen offen sein — sonst lässt sich rückwirkend
    // in ein festgeschriebenes GJ schreiben.
    let fy = guard::ensure_year_open(pool, existing.acquisition_fiscal_year).await?;
    guard::check_open(pool, &fy, input.acquisition_date.year() as i64).await?;

    // Lauf F8: Die Quell-Kosten einer Anlage bleiben fest (der Verweis
    // `capitalized_as_asset_id` entsteht nur beim Anlegen), und die
    // Anschaffungskosten folgen auch beim Korrigieren § 9b Abs. 1 EStG.
    if input.expense_id != existing.expense_id {
        return Err(Error::Domain(
            "Die Quell-Kosten einer Anlage lassen sich nicht nachträglich ändern.".into(),
        ));
    }
    if let Some(eid) = existing.expense_id.as_deref() {
        if let Some(expense) = expenses::get(pool, eid).await? {
            let is_klein = expense_status_for_asset(pool, &expense, input.acquisition_date).await?;
            if let Some(msg) = asset::check_cost_against_expense(
                input.acquisition_cost_cents,
                expense.net_amount_cents,
                expense.gross_amount_cents,
                is_klein,
                expense.reverse_charge_13b == 1,
            ) {
                return Err(Error::Domain(msg));
            }
        }
    }

    let (method_db, useful_life) = resolve_method(&input)?;
    let book_value =
        business_book_value_start_cents(input.acquisition_cost_cents, input.business_share_percent);

    let row = assets::update(pool, &id, &input, &method_db, useful_life, book_value).await?;

    audit_log::append(
        pool,
        "asset.update",
        "asset",
        &row.id,
        Some(serde_json::json!({ "number": &row.asset_number, "cost": row.acquisition_cost_cents, "method": &method_db, "business_share": row.business_share_percent })),
    )
    .await?;

    backup::request_auto_backup(pool, &paths, session, "asset.update", ctx.clock().now_utc())
        .await
        .ok();

    assets::get_detail(pool, &row.id)
        .await?
        .ok_or_else(|| Error::Domain("update: detail-load leer".into()))
}

// =============================================================================
// Dispose (Veräußerung/Verschrottung — kein Löschen)
// =============================================================================

/// Argumente von [`assets_dispose`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetsDisposeArgs {
    pub args: AssetDisposeArgs,
}

pub async fn assets_dispose(ctx: &Arc<AppCtx>, args: AssetsDisposeArgs) -> Result<AssetDetail> {
    let AssetsDisposeArgs { args } = args;
    let pool = ctx.pool()?;
    let session = &ctx.session;

    let pool = &pool;
    let paths = ctx.paths.clone();
    let clock = ctx.clock();

    let dtype = DisposalType::from_db(&args.disposal_type).ok_or_else(|| {
        Error::Domain(format!("Ungültige Veräußerungsart: {}", args.disposal_type))
    })?;
    let existing = assets::get(pool, &args.asset_id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Anlage nicht gefunden: {}", args.asset_id)))?;
    if existing.disposed == 1 {
        return Err(Error::Domain("Anlage ist bereits veräußert.".into()));
    }
    // R1-020 / Lauf F3b: Datum (nicht in der Zukunft, nicht vor der Anschaffung —
    // sonst fehlte die anteilige AfA und der Restbuchwert ginge als Verlust in ein
    // Jahr vor dem Zugang) und Erlös (nur beim Verkauf, nie negativ) prüft der
    // Functional Core.
    let acquisition_date = parse_db_date(&existing.acquisition_date)?;
    let proceeds = crate::domain::asset::validate_disposal(
        dtype,
        args.proceeds_cents,
        args.disposal_date,
        acquisition_date,
        clock.today_berlin(),
    )
    .map_err(|e| Error::Domain(crate::domain::asset::disposal_message(&e)))?;

    // R2-023 (GJ-Guard): Disposal-Datum muss in einem offenen GJ liegen —
    // die Veräußerungs-Buchung kippt sonst rückwirkend Restbuchwert und
    // Erlös ins festgeschriebene Jahr. Die Erlaubnis hält bis nach dem Commit.
    let disposal_year = args.disposal_date.year() as i64;
    let _fy = guard::ensure_year_open(pool, disposal_year).await?;

    // Lauf F3b: AfA bis zum Abgang und der Abgang in **einer** Transaktion.
    let mut tx = crate::db::begin_immediate(pool).await?;
    let asset: AssetRow = sqlx::query_as("SELECT * FROM assets WHERE id = ?")
        .bind(&args.asset_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| Error::Domain(format!("Anlage nicht gefunden: {}", args.asset_id)))?;
    if asset.disposed == 1 {
        return Err(Error::Domain("Anlage ist bereits veräußert.".into()));
    }

    // 1. AfA ab dem Abgangsjahr (Lauf F3b). Nach dem Abgang gibt es für die
    //    Anlage keine AfA mehr, und im Abgangsjahr nur die anteilige bis zum
    //    Abgangsmonat (R 7.4 Abs. 8 EStR, §7 Abs. 1 Satz 4 EStG). Eine schon
    //    gebuchte AfA für spätere Jahre (oder eine volle Jahres-AfA im
    //    Abgangsjahr) wäre danach nicht mehr zu korrigieren — das Zurücksetzen
    //    lehnt veräußerte Anlagen ab (F2c). Festgeschriebene Einträge (Jahr
    //    abgeschlossen) lehnen den Abgang ab; offene werden hier entfernt und
    //    mit ihren Werten auditiert, der Restbuchwert steht danach wieder auf
    //    dem Stand vor dem Abgangsjahr.
    let from_disposal_year: Vec<DepreciationEntryRow> = sqlx::query_as(
        "SELECT * FROM depreciation_entries WHERE asset_id = ? AND fiscal_year >= ?
          ORDER BY fiscal_year",
    )
    .bind(&asset.id)
    .bind(disposal_year)
    .fetch_all(&mut *tx)
    .await?;
    // Auch ohne AfA-Eintrag (z. B. schon voll abgeschrieben) führt ein später
    // abgeschlossenes Jahr die Anlage festgeschrieben im Bestand.
    let closed_after: Option<i64> = sqlx::query_scalar(
        "SELECT MAX(fiscal_year) FROM fiscal_year_locks WHERE fiscal_year > ? AND fiscal_year >= ?",
    )
    .bind(disposal_year)
    .bind(asset.acquisition_fiscal_year)
    .fetch_one(&mut *tx)
    .await?;
    let locked_after = from_disposal_year
        .iter()
        .filter(|e| e.locked_at.is_some())
        .map(|e| e.fiscal_year)
        .max();
    if let Some(year) = closed_after.max(locked_after) {
        return Err(Error::Domain(closed_conflict_message(
            &asset.asset_number,
            year,
            disposal_year,
        )));
    }
    if !from_disposal_year.is_empty() {
        sqlx::query(
            "DELETE FROM depreciation_entries
              WHERE asset_id = ? AND fiscal_year >= ? AND locked_at IS NULL",
        )
        .bind(&asset.id)
        .bind(disposal_year)
        .execute(&mut *tx)
        .await?;
        let before: Option<(i64, i64)> = sqlx::query_as(
            "SELECT book_value_after_cents, fiscal_year FROM depreciation_entries
              WHERE asset_id = ? ORDER BY fiscal_year DESC LIMIT 1",
        )
        .bind(&asset.id)
        .fetch_optional(&mut *tx)
        .await?;
        let (book_value, last_year) = match before {
            Some((value, year)) => (value, Some(year)),
            None => (
                business_book_value_start_cents(
                    asset.acquisition_cost_cents,
                    asset.business_share_percent,
                ),
                None,
            ),
        };
        assets::set_book_value_in(&mut tx, &asset.id, book_value, last_year).await?;
        let removed: Vec<serde_json::Value> = from_disposal_year
            .iter()
            .map(|e| {
                serde_json::json!({
                    "fiscal_year": e.fiscal_year,
                    "amount": e.depreciation_amount_cents,
                    "months": e.months_in_year,
                })
            })
            .collect();
        audit_log::append_in(
            &mut tx,
            "depreciation.reset_for_disposal",
            "asset",
            &asset.id,
            Some(serde_json::json!({
                "asset_number": &asset.asset_number,
                "disposal_date": args.disposal_date,
                "removed": removed,
                "book_value_restored": book_value,
            })),
        )
        .await?;
    }

    // 2. Lauf F2c: Vorjahre ohne gebuchte AfA nachholen. Nach dem Abgang bucht
    //    der AfA-Lauf die Anlage nicht mehr; die AfA der Vorjahre fehlte sonst
    //    endgültig, und die anteilige AfA des Abgangsjahres rechnete vom zu
    //    hohen Restbuchwert. Abgeschlossene Jahre sind gebucht (der Abschluss
    //    bucht vorher) und bleiben unberührt.
    if disposal_year > asset.acquisition_fiscal_year {
        let caught_up = crate::depreciation::accrue_yearly::accrue_asset_through_in(
            &mut tx,
            &asset,
            disposal_year - 1,
        )
        .await?;
        if caught_up > 0 {
            tracing::info!(
                "Anlage {}: {caught_up} AfA-Jahr(e) vor dem Abgang nachgebucht",
                asset.asset_number
            );
        }
    }

    // 3. R2-021: anteilige AfA im Abgangsjahr (M/12 bis einschließlich
    //    Abgangsmonat) vom Restbuchwert davor; der gekürzte Restbuchwert
    //    wandert als Snapshot in den Abgang.
    let running: i64 = sqlx::query_scalar("SELECT book_value_cents FROM assets WHERE id = ?")
        .bind(&asset.id)
        .fetch_one(&mut *tx)
        .await?;
    let method = DepreciationMethod::from_db(&asset.depreciation_method).ok_or_else(|| {
        Error::Domain(format!(
            "Unbekannte AfA-Methode: {}",
            asset.depreciation_method
        ))
    })?;
    let dep_asset = DepreciationAsset {
        depreciation_method: method,
        acquisition_date,
        acquisition_cost_cents: asset.acquisition_cost_cents,
        business_share_percent: asset.business_share_percent,
        useful_life_years: asset.useful_life_years,
    };
    let calc = compute_disposal_year_partial(&dep_asset, running, args.disposal_date);
    if !calc.is_noop() {
        depreciation_repo::book_entry_in(&mut tx, &asset.id, disposal_year, &calc).await?;
        assets::set_book_value_in(
            &mut tx,
            &asset.id,
            calc.book_value_after_cents,
            Some(disposal_year),
        )
        .await?;
        audit_log::append_in(
            &mut tx,
            "depreciation.disposal_partial",
            "asset",
            &asset.id,
            Some(serde_json::json!({ "fiscal_year": disposal_year, "months": calc.months_in_year, "amount": calc.depreciation_amount_cents, "book_value_after": calc.book_value_after_cents })),
        )
        .await?;
    }
    let residual = calc.book_value_after_cents;

    // 4. Abgang (Status-Bedingung im UPDATE; danach hält
    //    `trg_assets_disposal_final` alles fest).
    assets::dispose_in(
        &mut tx,
        &asset.id,
        &args.disposal_date.to_string(),
        dtype.as_db(),
        proceeds,
        residual,
    )
    .await?;
    audit_log::append_in(
        &mut tx,
        "asset.dispose",
        "asset",
        &asset.id,
        Some(serde_json::json!({ "number": &asset.asset_number, "type": dtype.as_db(), "date": args.disposal_date, "proceeds": proceeds, "residual_book_value": residual })),
    )
    .await?;
    tx.commit().await?;

    backup::request_auto_backup(pool, &paths, session, "asset.dispose", clock.now_utc())
        .await
        .ok();

    assets::get_detail(pool, &asset.id)
        .await?
        .ok_or_else(|| Error::Domain("dispose: detail-load leer".into()))
}

// =============================================================================
// Vorschau des Abgangs (Lauf 2026.11 U4, Warteliste 5) — liest nur
// =============================================================================

/// Argumente von [`assets_disposal_preview`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetsDisposalPreviewArgs {
    pub id: String,
    /// Geplantes Abgangsdatum (YYYY-MM-DD).
    pub disposal_date: NaiveDate,
}

/// Was [`assets_dispose`] zum gewählten Datum buchen würde (Restbuchwert,
/// AfA bis zum Abgang). Gewinn bzw. Verlust = Erlös − `residual_book_value_cents`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetDisposalPreview {
    pub asset_id: String,
    pub disposal_date: NaiveDate,
    /// Aktueller Restbuchwert der Anlage (Stand vor dem Abgang).
    pub current_book_value_cents: i64,
    /// Offene AfA ab dem Abgangsjahr, die der Abgang ersetzt (Summe, Jahre).
    pub replaced_depreciation_cents: i64,
    pub replaced_years: Vec<i64>,
    /// AfA der Vorjahre ohne Buchung, die der Abgang nachholt (Summe, Jahre).
    pub catch_up_depreciation_cents: i64,
    pub catch_up_years: Vec<i64>,
    /// Restbuchwert zu Beginn des Abgangsjahres (nach Ersetzen und Nachholen).
    pub book_value_before_cents: i64,
    /// Anteilige AfA im Abgangsjahr bis einschließlich Abgangsmonat.
    pub disposal_year_depreciation_cents: i64,
    pub disposal_year_months: i32,
    /// Restbuchwert zum Abgang (geht als Ausgabe in die EÜR).
    pub residual_book_value_cents: i64,
    /// AfA insgesamt von der Anschaffung bis zum Abgang (betrieblicher
    /// Anschaffungswert − Restbuchwert).
    pub depreciation_to_date_cents: i64,
    /// Grund, aus dem der Abgang zu diesem Datum abgelehnt würde (Datum,
    /// abgeschlossenes Jahr); `None` = möglich.
    pub blocked_reason: Option<String>,
}

/// Vorschau des Abgangs einer Anlage zum Datum: dieselbe Rechnung wie
/// [`assets_dispose`] ([`plan_disposal`]), ohne zu buchen.
pub async fn assets_disposal_preview(
    ctx: &Arc<AppCtx>,
    args: AssetsDisposalPreviewArgs,
) -> Result<AssetDisposalPreview> {
    let AssetsDisposalPreviewArgs { id, disposal_date } = args;
    let pool = ctx.pool()?;

    let asset = assets::get(&pool, &id)
        .await?
        .ok_or_else(|| Error::Domain(format!("Anlage nicht gefunden: {id}")))?;
    if asset.disposed == 1 {
        return Err(Error::Domain("Anlage ist bereits veräußert.".into()));
    }
    let acquisition_date = parse_db_date(&asset.acquisition_date)?;
    let method = DepreciationMethod::from_db(&asset.depreciation_method).ok_or_else(|| {
        Error::Domain(format!(
            "Unbekannte AfA-Methode: {}",
            asset.depreciation_method
        ))
    })?;
    let dep_asset = DepreciationAsset {
        depreciation_method: method,
        acquisition_date,
        acquisition_cost_cents: asset.acquisition_cost_cents,
        business_share_percent: asset.business_share_percent,
        useful_life_years: asset.useful_life_years,
    };
    let entries: Vec<DepreciationEntryRow> = sqlx::query_as(
        "SELECT * FROM depreciation_entries WHERE asset_id = ? ORDER BY fiscal_year",
    )
    .bind(&asset.id)
    .fetch_all(&pool)
    .await?;
    let booked: Vec<BookedYear> = entries
        .iter()
        .map(|e| BookedYear {
            fiscal_year: e.fiscal_year,
            amount_cents: e.depreciation_amount_cents,
            book_value_after_cents: e.book_value_after_cents,
            locked: e.locked_at.is_some(),
        })
        .collect();
    let closed_years: Vec<i64> = sqlx::query_scalar("SELECT fiscal_year FROM fiscal_year_locks")
        .fetch_all(&pool)
        .await?;

    let plan = plan_disposal(
        &dep_asset,
        asset.acquisition_fiscal_year,
        asset.book_value_cents,
        &booked,
        &closed_years,
        disposal_date,
    );
    let disposal_year = i64::from(disposal_date.year());
    // Gleiche Reihenfolge der Prüfungen wie `assets_dispose`.
    let blocked_reason = if let Err(e) = crate::domain::asset::validate_disposal(
        DisposalType::Scrap,
        0,
        disposal_date,
        acquisition_date,
        ctx.clock().today_berlin(),
    ) {
        Some(crate::domain::asset::disposal_message(&e))
    } else if closed_years.contains(&disposal_year) {
        Some(guard::closed_message(disposal_year))
    } else {
        plan.closed_conflict_year
            .map(|year| closed_conflict_message(&asset.asset_number, year, disposal_year))
    };

    let residual = plan.residual_book_value_cents();
    let start =
        business_book_value_start_cents(asset.acquisition_cost_cents, asset.business_share_percent);
    Ok(AssetDisposalPreview {
        asset_id: asset.id,
        disposal_date,
        current_book_value_cents: asset.book_value_cents,
        replaced_depreciation_cents: plan.replaced.iter().map(|(_, c)| c).sum(),
        replaced_years: plan.replaced.iter().map(|(y, _)| *y).collect(),
        catch_up_depreciation_cents: plan.catch_up.iter().map(|(_, c)| c).sum(),
        catch_up_years: plan.catch_up.iter().map(|(y, _)| *y).collect(),
        book_value_before_cents: plan.book_value_before_cents,
        disposal_year_depreciation_cents: plan.disposal_year.depreciation_amount_cents,
        disposal_year_months: plan.disposal_year.months_in_year,
        residual_book_value_cents: residual,
        depreciation_to_date_cents: start - residual,
        blocked_reason,
    })
}

// ---- intern ----------------------------------------------------------------

/// Ablehnung eines Abgangs, wenn ein späteres abgeschlossenes Jahr die Anlage
/// mit festgeschriebener AfA bzw. im Bestand führt (Lauf F3b; auch Vorschau).
fn closed_conflict_message(asset_number: &str, year: i64, disposal_year: i64) -> String {
    format!(
        "Das Geschäftsjahr {year} ist abgeschlossen und führt die Anlage {asset_number} mit \
         festgeschriebener AfA bzw. im Bestand. Ein Abgang im Jahr {disposal_year} \
         widerspräche dem abgeschlossenen Jahr — erfassen lässt er sich frühestens {}. \
         Eine frühere Veräußerung klärst Du mit Deiner Steuerberatung.",
        year + 1
    )
}

/// Leitet aus der (validierten) Eingabe den DB-Methoden-Slug + die effektive
/// Nutzungsdauer ab: linear = Eingabe, Computer-Sonderregel = 1 Jahr, GWG = keine.
fn resolve_method(input: &AssetInput) -> Result<(String, Option<f64>)> {
    let method = DepreciationMethod::from_db(&input.depreciation_method).ok_or_else(|| {
        Error::Domain(format!(
            "Ungültige AfA-Methode: {}",
            input.depreciation_method
        ))
    })?;
    let useful_life = match method {
        DepreciationMethod::Linear => input.useful_life_years,
        DepreciationMethod::ComputerSpecial2021 => Some(1.0),
        DepreciationMethod::GwgSofort => None,
    };
    Ok((method.as_db().to_string(), useful_life))
}

/// Heutiges Datum in Europe/Berlin (`crate::clock`, Plan W1.8).
fn today_berlin() -> NaiveDate {
    crate::clock::today_berlin()
}

/// Datum aus der DB (`YYYY-MM-DD`).
fn parse_db_date(s: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| Error::Domain(format!("Ungültiges Anschaffungs-Datum in DB: {s}")))
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
