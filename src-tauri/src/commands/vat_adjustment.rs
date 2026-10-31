//! Übersicht der Vorsteuerberichtigung nach § 15a UStG (ADR 0042 Nr. 14).
//!
//! Lesend: rechnet je Anlage mit bekannter Vorsteuer (aus Kosten übernommen)
//! über [`crate::domain::vat_adjustment::compute`] und der Steuerstatus-Historie.
//! Nichts wird gebucht oder geändert (GoBD: festgeschriebene Anlagen und Kosten
//! bleiben unberührt); die Berichtigung gehört in die Umsatzsteuererklärung und
//! zur Steuerberatung. Anlagen ohne bekannte Vorsteuer weist die Übersicht mit
//! dem Grund aus, statt zu raten.

use std::sync::Arc;

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::app_ctx::AppCtx;
use crate::db::repo::tax_status as tax_repo;
use crate::domain::tax_status::TaxStatusHistory;
use crate::domain::vat_adjustment::{
    self, AssetAdjustment, AssetFacts, Direction, DisposalKind, Exemption, YearShare,
};
use crate::error::Result;

/// Hinweis, den jede Übersicht trägt (ADR 0042 Nr. 14).
pub const NOTICE: &str = "Nur eine Übersicht für die Umsatzsteuererklärung: Klein.Buch bucht die \
     Berichtigung nicht. Beginn ist das Anschaffungsdatum, weil Klein.Buch den Tag der ersten \
     Verwendung nicht kennt. Bitte mit der Steuerberatung klären.";

/// Ziel des Inbox-Hinweises (Übersicht unter Anschaffungen, `routes/assets/vat-adjustment`).
pub const OVERVIEW_URL: &str = "/assets/vat-adjustment";

/// Argumente von [`vat_adjustment_overview`] (JSON-Objekt, Schlüssel in camelCase wie bei `invoke`).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VatAdjustmentOverviewArgs {
    pub year: i32,
}

/// Eine Anlage mit bekannter Vorsteuer.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VatAdjustmentAsset {
    pub asset_id: String,
    pub asset_number: String,
    pub label: String,
    pub expense_number: String,
    /// Beginn der Verwendung (ersatzweise Anschaffungsdatum, ISO).
    pub first_use: String,
    pub depreciation_method: String,
    /// Rechnung über den ganzen Zeitraum.
    pub adjustment: AssetAdjustment,
    /// Zeile des angefragten Jahres (`None`: Jahr außerhalb des Zeitraums).
    pub year_share: Option<YearShare>,
    pub has_change: bool,
    /// Prüfhinweise (Privatanteil, Verwendungsdauer).
    pub hints: Vec<String>,
}

/// Grund, aus dem eine Anlage nicht gerechnet wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnknownReason {
    /// Ohne Kostenbeleg angelegt (oder Kosten storniert).
    NoExpense,
    /// § 13b UStG: die Vorsteuer steht nicht als Steuerbetrag am Beleg.
    ReverseCharge,
    /// Kostenbeleg ohne Umsatzsteuer.
    NoInputTax,
}

/// Anlage ohne bekannte Vorsteuer.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VatAdjustmentUnknown {
    pub asset_id: String,
    pub asset_number: String,
    pub label: String,
    pub reason: UnknownReason,
    pub message: String,
}

/// Ergebnis von [`vat_adjustment_overview`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VatAdjustmentOverview {
    pub year: i32,
    pub notice: &'static str,
    /// Anlagen, deren Berichtigungszeitraum das Jahr berührt.
    pub assets: Vec<VatAdjustmentAsset>,
    /// Anlagen im Zeitraum ohne bekannte Vorsteuer.
    pub unknown: Vec<VatAdjustmentUnknown>,
    /// Summe der zu berichtigenden Beträge des Jahres (positiv zugunsten).
    pub total_adjustment_cents: i64,
}

type AssetRow = (
    String,         // id
    String,         // asset_number
    String,         // label
    String,         // acquisition_date
    String,         // depreciation_method
    Option<f64>,    // useful_life_years
    f64,            // business_share_percent
    i64,            // disposed
    Option<String>, // disposal_date
    Option<String>, // disposal_type
    Option<String>, // expense_number (nur nicht stornierte, verknüpfte Kosten)
    Option<String>, // expense_date
    Option<i64>,    // tax_amount_cents
    Option<i64>,    // reverse_charge_13b
);

/// Eine Anlage, gerechnet oder mit Grund ausgewiesen.
enum Computed {
    Known(Box<VatAdjustmentAsset>),
    /// Mit dem Fünfjahreszeitraum ab Anschaffung (für die Auswahl je Jahr).
    Unknown(VatAdjustmentUnknown, vat_adjustment::Period),
}

fn parse(date: &str, what: &str) -> Result<NaiveDate> {
    tax_repo::parse_document_date(date, what)
}

/// Kürzere Verwendungsdauer (§ 15a Abs. 5 Satz 2 UStG) aus der
/// betriebsgewöhnlichen Nutzungsdauer der linearen AfA (UStAE 15a.3 Abs. 2).
/// Die Sonderregel für Computer (ein Jahr, BMF-Schreiben 2021) und die
/// GWG-Sofortabschreibung verkürzen den Zeitraum nicht; die Übersicht weist
/// bei Computern darauf hin.
fn use_months(method: &str, useful_life_years: Option<f64>) -> Option<u32> {
    match (method, useful_life_years) {
        ("linear", Some(y)) if y > 0.0 && y < 5.0 => Some((y * 12.0).round().max(1.0) as u32),
        _ => None,
    }
}

fn compute_row(
    row: AssetRow,
    hist: &TaxStatusHistory,
    today: NaiveDate,
    year: i32,
) -> Result<Computed> {
    let (
        id,
        asset_number,
        label,
        acquisition_date,
        method,
        useful_life,
        business_share,
        disposed,
        disposal_date,
        disposal_type,
        expense_number,
        expense_date,
        tax,
        rc,
    ) = row;
    let first_use = parse(&acquisition_date, "Anlage")?;
    let unknown = |reason: UnknownReason, message: &str| {
        Computed::Unknown(
            VatAdjustmentUnknown {
                asset_id: id.clone(),
                asset_number: asset_number.clone(),
                label: label.clone(),
                reason,
                message: message.to_string(),
            },
            vat_adjustment::period(first_use, None),
        )
    };
    let (Some(expense_number), Some(expense_date), Some(tax)) = (expense_number, expense_date, tax)
    else {
        return Ok(unknown(
            UnknownReason::NoExpense,
            "Ohne Kostenbeleg angelegt oder Kosten storniert: Die Vorsteuer kennt Klein.Buch nicht.",
        ));
    };
    if rc == Some(1) {
        return Ok(unknown(
            UnknownReason::ReverseCharge,
            "Reverse-Charge (§ 13b UStG): Die selbst geschuldete Steuer steht nicht als \
             Steuerbetrag auf dem Kostenbeleg.",
        ));
    }
    if tax <= 0 {
        return Ok(unknown(
            UnknownReason::NoInputTax,
            "Kostenbeleg ohne Umsatzsteuer (z. B. Kauf von privat oder von einem \
             Kleinunternehmer). Wurde hier die bei § 13b selbst geschuldete Steuer als eigene \
             Anlage übernommen, gilt sie als Vorsteuer: mit der Steuerberatung klären.",
        ));
    }
    let original_klein = tax_repo::is_klein_for_expense(hist, &expense_date)?;
    let disposal = match (disposed, disposal_date.as_deref(), disposal_type.as_deref()) {
        (1, Some(date), Some(kind)) => DisposalKind::from_db(kind)
            .map(|k| parse(date, "Anlagenabgang").map(|d| (d, k)))
            .transpose()?,
        _ => None,
    };
    let facts = AssetFacts {
        first_use,
        input_tax_cents: tax,
        original_klein,
        use_months: use_months(&method, useful_life),
        disposal,
    };
    let adjustment = vat_adjustment::compute(&facts, hist, today);
    let mut hints = Vec::new();
    if business_share < 100.0 {
        hints.push(format!(
            "Privatanteil {} %: Gerechnet ist mit der ganzen Vorsteuer des Belegs; ob und wie \
             weit die Anlage dem Unternehmen zugeordnet ist, klärt die Steuerberatung.",
            business_share.to_string().replace('.', ",")
        ));
    }
    if method == "computer_special_2021" {
        hints.push(
            "Computer mit Nutzungsdauer ein Jahr (BMF-Schreiben 2021): Gerechnet ist mit fünf \
             Jahren; eine kürzere Verwendungsdauer (§ 15a Abs. 5 Satz 2 UStG) mit der \
             Steuerberatung klären."
                .to_string(),
        );
    }
    let year_share = adjustment.year(year).cloned();
    Ok(Computed::Known(Box::new(VatAdjustmentAsset {
        asset_id: id,
        asset_number,
        label,
        expense_number,
        first_use: acquisition_date.chars().take(10).collect(),
        depreciation_method: method,
        has_change: adjustment.has_change(),
        adjustment,
        year_share,
        hints,
    })))
}

async fn rows(pool: &SqlitePool) -> Result<Vec<AssetRow>> {
    Ok(sqlx::query_as(
        "SELECT a.id, a.asset_number, a.label, a.acquisition_date, a.depreciation_method,
                a.useful_life_years, a.business_share_percent, a.disposed, a.disposal_date,
                a.disposal_type, e.expense_number, e.expense_date, e.tax_amount_cents,
                e.reverse_charge_13b
           FROM assets a
           LEFT JOIN expenses e ON e.id = a.expense_id AND e.capitalized_as_asset_id = a.id
                AND e.status = 'recorded'
          ORDER BY a.asset_number",
    )
    .fetch_all(pool)
    .await?)
}

/// Alle Anlagen gerechnet (für Übersicht und Hinweis).
async fn compute_all(
    pool: &SqlitePool,
    hist: &TaxStatusHistory,
    today: NaiveDate,
    year: i32,
) -> Result<Vec<Computed>> {
    let mut out = Vec::new();
    for row in rows(pool).await? {
        out.push(compute_row(row, hist, today, year)?);
    }
    Ok(out)
}

/// Kern von [`vat_adjustment_overview`] (testbar mit eigenem `today`).
pub async fn overview_core(
    pool: &SqlitePool,
    year: i32,
    today: NaiveDate,
) -> Result<VatAdjustmentOverview> {
    let hist = tax_repo::history(pool).await?;
    let mut assets = Vec::new();
    let mut unknown = Vec::new();
    for c in compute_all(pool, &hist, today, year).await? {
        match c {
            Computed::Known(a) if a.year_share.is_some() => assets.push(*a),
            Computed::Known(_) => {}
            // Nur Anlagen, deren Fünfjahreszeitraum das Jahr berührt.
            Computed::Unknown(u, p) if p.from.year() <= year && year <= p.to.year() => {
                unknown.push(u)
            }
            Computed::Unknown(..) => {}
        }
    }
    let total = assets
        .iter()
        .filter_map(|a| a.year_share.as_ref())
        .map(|y| y.adjustment_cents)
        .sum();
    Ok(VatAdjustmentOverview {
        year,
        notice: NOTICE,
        assets,
        unknown,
        total_adjustment_cents: total,
    })
}

/// Übersicht der Vorsteuerberichtigung nach § 15a UStG für ein Kalenderjahr
/// (ADR 0042 Nr. 14). Liest nur.
pub async fn vat_adjustment_overview(
    ctx: &Arc<AppCtx>,
    args: VatAdjustmentOverviewArgs,
) -> Result<VatAdjustmentOverview> {
    let pool = ctx.pool()?;
    overview_core(&pool, args.year, ctx.clock().today_berlin()).await
}

fn eur(cents: i64) -> String {
    crate::euer::detail::eur_grouped(cents)
}

/// Text einer Jahreszeile für den Hinweis.
fn year_text(y: &YearShare) -> String {
    let direction = match y.direction {
        Some(Direction::Zugunsten) => "zugunsten",
        Some(Direction::Zulasten) => "zulasten",
        None => "",
    };
    match y.exemption {
        Some(Exemption::InputTaxBagatelle) => {
            format!(
                "{} entfällt (Vorsteuer bis 1.000 €, § 44 Abs. 1 UStDV)",
                y.year
            )
        }
        Some(Exemption::BelowTenPoints) => format!(
            "{} entfällt (Änderung unter 10 Prozentpunkten, § 44 Abs. 2 UStDV)",
            y.year
        ),
        None => format!("{} {} {direction}", y.year, eur(y.adjustment_cents.abs())),
    }
}

/// Inbox-Hinweis nach einem Eintrag oder Widerruf des Steuerstatus: nennt die
/// Anlagen, bei denen sich die Verhältnisse im Berichtigungszeitraum ändern,
/// und verweist auf die Übersicht (ADR 0042 Nr. 14). Best-effort; ohne
/// betroffene Anlage kein Hinweis.
pub async fn notify_after_status_change(pool: &SqlitePool, dedup: &str, today: NaiveDate) {
    let hist = match tax_repo::history(pool).await {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!("Übersicht § 15a nach Statuswechsel nicht gerechnet: {e}");
            return;
        }
    };
    let computed = match compute_all(pool, &hist, today, today.year()).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Übersicht § 15a nach Statuswechsel nicht gerechnet: {e}");
            return;
        }
    };
    let lines: Vec<String> = computed
        .iter()
        .filter_map(|c| match c {
            Computed::Known(a) if a.has_change => {
                let years: Vec<String> = a
                    .adjustment
                    .years
                    .iter()
                    .filter(|y| y.months_changed > 0)
                    .map(year_text)
                    .collect();
                Some(format!(
                    "Anlage {} ({}): {}.",
                    a.asset_number,
                    a.label,
                    years.join(", ")
                ))
            }
            _ => None,
        })
        .collect();
    if lines.is_empty() {
        return;
    }
    let body = format!(
        "Mit dem geänderten Steuerstatus ändern sich bei diesen Anlagen die Verhältnisse für den \
         Vorsteuerabzug im Berichtigungszeitraum (fünf Jahre, § 15a UStG). {} Die Übersicht \
         „Vorsteuerberichtigung (§ 15a)“ unter Anschaffungen zeigt Zeitraum, Jahresanteile und die \
         Grenzen nach § 44 UStDV. {NOTICE}",
        lines.join(" ")
    );
    let n = crate::notify::NewNotification {
        title: "Vorsteuerberichtigung nach § 15a UStG prüfen",
        body: &body,
        severity: "warning",
        related_entity_type: Some("asset"),
        action_url: Some(OVERVIEW_URL),
        dedup_key: Some(dedup),
        ..Default::default()
    };
    if let Err(e) = crate::notify::emit(pool, n).await {
        tracing::warn!("Hinweis zur Vorsteuerberichtigung nicht angelegt: {e}");
    }
}
