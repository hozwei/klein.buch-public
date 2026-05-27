//! Repository für die EÜR-Aggregation (Block 13).
//!
//! Schicht: **Imperative Shell**. Lädt die rohen Bewegungs-Daten aus der DB und
//! mappt sie auf die DB-agnostischen Views des Functional Core
//! ([`crate::euer::aggregate`]). Die eigentliche Periodenzuordnung + Summierung
//! passiert dort (pure, getestet).
//!
//! ## Cash-Basis-Quellen (siehe `euer::aggregate` für die Rechtsgrundlage)
//!
//! - **Einnahmen** = einzelne Zahlungseingänge ausgehender, NICHT-Storno-
//!   Rechnungen. Teilzahlungen werden aus `payment_history_json` aufgelöst, damit
//!   jede Zahlung ihrem tatsächlichen Zuflussjahr zugeordnet wird (§11 EStG).
//!   Fallback ohne Historie: `paid_amount_cents` am `paid_at`.
//! - **Storno-Erstattungen** = der auf dem stornierten Original gezahlte Betrag,
//!   erfasst zum `invoice_date` des Storno-Belegs (Abfluss-Jahr).
//! - **Ausgaben** = bezahlte (`paid_date IS NOT NULL`), nicht stornierte Kosten,
//!   brutto, am `paid_date`.
//! - **AfA** = `depreciation_entries` (Jahres-Größe über `fiscal_year`).
//! - **Veräußerungen** = veräußerte Anlagen mit Erlös + Restbuchwert-Abgang am
//!   `disposal_date`.

use chrono::NaiveDate;
use sqlx::SqlitePool;

use crate::db::repo::invoices::PaymentEntry;
use crate::error::{Error, Result};
use crate::euer::aggregate::{
    DepreciationView, DisposalView, EuerInputs, ExpenseView, PaymentView, StornoReversalView,
};
use crate::euer::detail::{
    AveeurItem, DisposalItem, ExpenseItem, IncomeItem, PrivateMovementItem, StornoItem,
};
use crate::euer::elster_csv::AfaSplit;

/// Parst ein in der DB gespeichertes ISO-Datum (`YYYY-MM-DD`, ggf. mit
/// Zeitanteil) zu `NaiveDate`. Die ersten 10 Zeichen genügen.
fn parse_date(s: &str) -> Result<NaiveDate> {
    let head = &s[..s.len().min(10)];
    NaiveDate::parse_from_str(head, "%Y-%m-%d")
        .map_err(|e| Error::Domain(format!("EÜR: ungültiges Datum '{s}': {e}")))
}

/// Einzelne Zahlungseingänge ausgehender, NICHT-Storno-Rechnungen.
///
/// Teilzahlungen aus `payment_history_json` werden je Zahlung als eigener
/// [`PaymentView`] geliefert. Fehlt die Historie (ältere Daten), greift der
/// Fallback `paid_amount_cents` @ `paid_at`.
pub async fn income_payments(pool: &SqlitePool) -> Result<Vec<PaymentView>> {
    // R2-008: defensiver Lock-Guard. Drafts dürfen niemals als Einnahme
    // gezählt werden. **NICHT** zusätzlich `status != 'canceled'` filtern —
    // eine später stornierte Rechnung war zum Zahlungs-Datum eingenommen
    // (Cash-Basis §11 EStG); der Storno-Refund wird separat im Storno-Jahr
    // verbucht (`storno_reversals`). Rückwirkendes Ausblenden der Original-
    // Einnahme würde die Periodengetreuheit der GoBD brechen.
    let rows: Vec<(i64, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT paid_amount_cents, paid_at, payment_history_json
           FROM invoices
          WHERE direction = 'issued'
            AND is_storno_for IS NULL
            AND locked_at IS NOT NULL
            AND paid_amount_cents > 0",
    )
    .fetch_all(pool)
    .await?;

    let mut out = Vec::new();
    for (paid_amount_cents, paid_at, history_json) in rows {
        let history: Vec<PaymentEntry> = match history_json.as_deref() {
            Some(s) => serde_json::from_str(s).unwrap_or_default(),
            None => Vec::new(),
        };
        if !history.is_empty() {
            for entry in history {
                if entry.amount_cents == 0 {
                    continue;
                }
                out.push(PaymentView {
                    paid_date: parse_date(&entry.paid_date)?,
                    amount_cents: entry.amount_cents,
                });
            }
        } else if let Some(at) = paid_at.as_deref() {
            // Fallback: keine Historie, aber als bezahlt markiert.
            out.push(PaymentView {
                paid_date: parse_date(at)?,
                amount_cents: paid_amount_cents,
            });
        }
        // Wenn weder Historie noch paid_at gesetzt sind, ist die Zahlung nicht
        // datierbar → konservativ ausgelassen (kein Zuflussdatum bekannt).
    }
    Ok(out)
}

/// Storno-Erstattungen: der auf dem Original tatsächlich gezahlte Betrag,
/// erfasst zum Datum des Storno-Belegs.
pub async fn storno_reversals(pool: &SqlitePool) -> Result<Vec<StornoReversalView>> {
    // R2-007: Storno-Beleg nur einrechnen, wenn er **gelockt** ist. Im
    // Crash-Window zwischen `create_draft(storno)` und dem atomaren
    // `lock_with_pair_cancel` (R1-003) kann sonst ein Draft-Storno hängen
    // bleiben, während der Original noch nicht `canceled` ist — die EÜR
    // würde dann denselben Refund doppelt rechnen (Original positiv +
    // Draft-Storno negativ). Filter blockt das.
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT s.invoice_date AS storno_date, o.paid_amount_cents AS refunded
           FROM invoices s
           JOIN invoices o ON o.id = s.is_storno_for
          WHERE s.is_storno_for IS NOT NULL
            AND s.locked_at IS NOT NULL
            AND o.paid_amount_cents > 0",
    )
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|(storno_date, refunded_cents)| {
            Ok(StornoReversalView {
                storno_date: parse_date(&storno_date)?,
                refunded_cents,
            })
        })
        .collect()
}

/// Bezahlte, nicht stornierte Kosten (brutto) — am Zahlungsausgang.
pub async fn expense_payments(pool: &SqlitePool) -> Result<Vec<ExpenseView>> {
    let rows: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT paid_date, category, gross_amount_cents
           FROM expenses
          WHERE status = 'recorded'
            AND paid_date IS NOT NULL",
    )
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|(paid_date, category, gross_cents)| {
            Ok(ExpenseView {
                paid_date: parse_date(&paid_date)?,
                category,
                gross_cents,
            })
        })
        .collect()
}

/// Alle AfA-Buchungen (Periodenzuordnung über `fiscal_year` im Core).
pub async fn depreciation_views(pool: &SqlitePool) -> Result<Vec<DepreciationView>> {
    let rows: Vec<(i64, i64)> =
        sqlx::query_as("SELECT fiscal_year, depreciation_amount_cents FROM depreciation_entries")
            .fetch_all(pool)
            .await?;
    Ok(rows
        .into_iter()
        .map(|(fiscal_year, amount_cents)| DepreciationView {
            fiscal_year: fiscal_year as i32,
            amount_cents,
        })
        .collect())
}

/// AfA des Geschäftsjahres, aufgeteilt nach Anlage-EÜR-Zeile (Block 14a).
///
/// Klassifiziert die Jahres-AfA über die `depreciation_method` der zugehörigen
/// Anlage: `gwg_sofort` → Zeile 36 (GWG-Sofortabschreibung), alles andere
/// (linear, Computer-Sonderregel) → Zeile 33 (AfA auf bewegliche
/// Wirtschaftsgüter). Die Summe entspricht der aggregierten Jahres-AfA.
pub async fn depreciation_split_for_year(pool: &SqlitePool, year: i32) -> Result<AfaSplit> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT a.depreciation_method AS method,
                COALESCE(SUM(e.depreciation_amount_cents), 0) AS amount
           FROM depreciation_entries e
           JOIN assets a ON a.id = e.asset_id
          WHERE e.fiscal_year = ?
          GROUP BY a.depreciation_method",
    )
    .bind(year as i64)
    .fetch_all(pool)
    .await?;

    let mut split = AfaSplit::default();
    for (method, amount) in rows {
        if method == "gwg_sofort" {
            split.gwg_cents += amount;
        } else {
            split.beweglich_cents += amount;
        }
    }
    Ok(split)
}

/// Schneidet ein Datum auf `YYYY-MM-DD` (erste 10 Zeichen).
fn trim_date(s: &str) -> String {
    s.chars().take(10).collect()
}

// ----------------------------------------------------------------------------
// Einzelaufstellung + Anlageverzeichnis (Block 14a)
// ----------------------------------------------------------------------------

/// Einzelne Zahlungseingänge (Betriebseinnahmen) eines Geschäftsjahres —
/// Teilzahlungen je eigene Zeile, mit Rechnungsnummer + Kunde.
pub async fn income_detail(pool: &SqlitePool, year: i32) -> Result<Vec<IncomeItem>> {
    #[allow(clippy::type_complexity)]
    let rows: Vec<(
        String,
        Option<String>,
        i64,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        // R2-008: defensiver Lock-Guard, siehe income_payments. `status` wird
        // bewusst NICHT gefiltert — Periodengetreuheit (Original-Einnahme bleibt
        // im Zahlungsjahr, Storno-Refund im Storno-Jahr).
        "SELECT i.invoice_number, i.buyer_name, i.paid_amount_cents, i.paid_at,
                    i.payment_history_json,
                    (SELECT group_concat(d, '; ') FROM (
                         SELECT description AS d FROM invoice_items
                          WHERE invoice_id = i.id ORDER BY position
                     )) AS description
               FROM invoices i
              WHERE i.direction = 'issued'
                AND i.is_storno_for IS NULL
                AND i.locked_at IS NOT NULL
                AND i.paid_amount_cents > 0",
    )
    .fetch_all(pool)
    .await?;

    let yp = format!("{year:04}");
    let mut out = Vec::new();
    for (invoice_number, buyer_name, paid_amount_cents, paid_at, history_json, description) in rows
    {
        let customer = buyer_name.unwrap_or_default();
        let description = description.unwrap_or_default();
        let history: Vec<PaymentEntry> = match history_json.as_deref() {
            Some(s) => serde_json::from_str(s).unwrap_or_default(),
            None => Vec::new(),
        };
        if !history.is_empty() {
            for entry in history {
                if entry.amount_cents == 0 || !entry.paid_date.starts_with(&yp) {
                    continue;
                }
                out.push(IncomeItem {
                    paid_date: trim_date(&entry.paid_date),
                    invoice_number: invoice_number.clone(),
                    customer: customer.clone(),
                    description: description.clone(),
                    amount_cents: entry.amount_cents,
                });
            }
        } else if let Some(at) = paid_at {
            if at.starts_with(&yp) {
                out.push(IncomeItem {
                    paid_date: trim_date(&at),
                    invoice_number,
                    customer,
                    description,
                    amount_cents: paid_amount_cents,
                });
            }
        }
    }
    out.sort_by(|a, b| a.paid_date.cmp(&b.paid_date));
    Ok(out)
}

/// Storno-Erstattungen eines Geschäftsjahres (negative Einnahmen).
pub async fn storno_detail(pool: &SqlitePool, year: i32) -> Result<Vec<StornoItem>> {
    let yp = format!("{year:04}");
    let rows: Vec<(String, String, String, i64)> = sqlx::query_as(
        // R2-007: Storno-Beleg muss gelockt sein (siehe storno_reversals).
        "SELECT s.invoice_number, s.invoice_date, o.invoice_number, o.paid_amount_cents
           FROM invoices s
           JOIN invoices o ON o.id = s.is_storno_for
          WHERE s.is_storno_for IS NOT NULL
            AND s.locked_at IS NOT NULL
            AND o.paid_amount_cents > 0
            AND substr(s.invoice_date, 1, 4) = ?
          ORDER BY s.invoice_date",
    )
    .bind(&yp)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(storno_number, storno_date, original_number, refunded)| StornoItem {
                storno_date: trim_date(&storno_date),
                storno_number,
                original_number,
                refunded_cents: refunded,
            },
        )
        .collect())
}

/// Einzelne bezahlte Kostenpositionen eines Geschäftsjahres (brutto).
pub async fn expense_detail(pool: &SqlitePool, year: i32) -> Result<Vec<ExpenseItem>> {
    let yp = format!("{year:04}");
    let rows: Vec<(String, String, String, String, String, i64)> = sqlx::query_as(
        "SELECT paid_date, expense_number, vendor_name_snapshot, category, description,
                gross_amount_cents
           FROM expenses
          WHERE status = 'recorded'
            AND paid_date IS NOT NULL
            AND substr(paid_date, 1, 4) = ?
          ORDER BY paid_date, expense_number",
    )
    .bind(&yp)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(paid_date, expense_number, vendor, category, description, gross)| ExpenseItem {
                paid_date: trim_date(&paid_date),
                expense_number,
                vendor,
                category,
                description,
                gross_cents: gross,
            },
        )
        .collect())
}

/// Privatbewegungen (Entnahmen/Einlagen) eines Geschäftsjahres — EÜR-neutral,
/// aber für den DATEV-Buchungsstapel relevant (R2-009). Gefiltert nach dem
/// `fiscal_year`-Feld (Bewegung läuft immer übers Geschäftsjahr).
pub async fn private_movement_detail(
    pool: &SqlitePool,
    year: i32,
) -> Result<Vec<PrivateMovementItem>> {
    let rows: Vec<(String, String, String, String, i64)> = sqlx::query_as(
        "SELECT movement_date, movement_number, movement_type, description, amount_cents
           FROM private_movements
          WHERE fiscal_year = ?
          ORDER BY movement_date, movement_number",
    )
    .bind(year as i64)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(movement_date, movement_number, movement_type, description, amount_cents)| {
                PrivateMovementItem {
                    movement_date: trim_date(&movement_date),
                    movement_number,
                    movement_type,
                    description,
                    amount_cents,
                }
            },
        )
        .collect())
}

/// Anlagen-Veräußerungen eines Geschäftsjahres (Einzelaufstellung).
pub async fn disposal_detail(pool: &SqlitePool, year: i32) -> Result<Vec<DisposalItem>> {
    let yp = format!("{year:04}");
    #[allow(clippy::type_complexity)]
    let rows: Vec<(String, String, String, Option<i64>, Option<i64>)> = sqlx::query_as(
        "SELECT disposal_date, asset_number, label, disposal_proceeds_cents,
                disposal_residual_book_value_cents
           FROM assets
          WHERE disposed = 1 AND disposal_date IS NOT NULL
            AND substr(disposal_date, 1, 4) = ?
          ORDER BY disposal_date, asset_number",
    )
    .bind(&yp)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(disposal_date, asset_number, label, proceeds, residual)| {
            let p = proceeds.unwrap_or(0);
            let r = residual.unwrap_or(0);
            DisposalItem {
                disposal_date: trim_date(&disposal_date),
                asset_number,
                label,
                proceeds_cents: p,
                residual_book_value_cents: r,
                gain_loss_cents: p - r,
            }
        })
        .collect())
}

#[derive(sqlx::FromRow)]
struct AveeurRow {
    asset_number: String,
    label: String,
    acquisition_date: String,
    acquisition_cost_cents: i64,
    depreciation_method: String,
    useful_life_years: Option<f64>,
    business_share_percent: f64,
    disposed: i64,
    disposal_date: Option<String>,
    disposal_proceeds_cents: Option<i64>,
    book_value_cents: i64,
    depreciation_amount_cents: Option<i64>,
    book_value_before_cents: Option<i64>,
    book_value_after_cents: Option<i64>,
}

/// Anlageverzeichnis (AVEÜR) für ein Geschäftsjahr: alle im Jahr gehaltenen
/// Anlagen mit der Jahres-AfA + Restbuchwerten (LEFT JOIN auf die AfA-Buchung
/// des Jahres). Restwert-Spalten fallen ohne gebuchte AfA auf den aktuellen
/// Restbuchwert der Anlage zurück.
pub async fn aveeur_items(pool: &SqlitePool, year: i32) -> Result<Vec<AveeurItem>> {
    let yp = format!("{year:04}");
    let rows: Vec<AveeurRow> = sqlx::query_as(
        "SELECT a.asset_number, a.label, a.acquisition_date, a.acquisition_cost_cents,
                a.depreciation_method, a.useful_life_years, a.business_share_percent,
                a.disposed, a.disposal_date, a.disposal_proceeds_cents, a.book_value_cents,
                e.depreciation_amount_cents, e.book_value_before_cents, e.book_value_after_cents
           FROM assets a
           LEFT JOIN depreciation_entries e
                  ON e.asset_id = a.id AND e.fiscal_year = ?
          WHERE a.acquisition_fiscal_year <= ?
            AND (a.disposed = 0 OR substr(a.disposal_date, 1, 4) >= ?)
          ORDER BY a.asset_number",
    )
    .bind(year as i64)
    .bind(year as i64)
    .bind(&yp)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let afa = r.depreciation_amount_cents.unwrap_or(0);
            let bv_end = r.book_value_after_cents.unwrap_or(r.book_value_cents);
            let bv_start = r.book_value_before_cents.unwrap_or(bv_end);
            let disposed_in_year = r.disposed == 1
                && r.disposal_date
                    .as_deref()
                    .map(|d| d.starts_with(&yp))
                    .unwrap_or(false);
            AveeurItem {
                asset_number: r.asset_number,
                label: r.label,
                acquisition_date: trim_date(&r.acquisition_date),
                acquisition_cost_cents: r.acquisition_cost_cents,
                depreciation_method: r.depreciation_method,
                useful_life_years: r.useful_life_years,
                business_share_percent: r.business_share_percent,
                afa_year_cents: afa,
                book_value_start_cents: bv_start,
                book_value_end_cents: bv_end,
                disposed_in_year,
                disposal_date: if disposed_in_year {
                    r.disposal_date.as_deref().map(trim_date)
                } else {
                    None
                },
                disposal_proceeds_cents: if disposed_in_year {
                    r.disposal_proceeds_cents
                } else {
                    None
                },
            }
        })
        .collect())
}

/// Original-Beleg-Archive für alle festgeschriebenen Rechnungen eines GJ
/// (R2-014). Tuple = (`invoice_number`, `pdf_archive_id`, `xml_archive_id`).
/// Storno-Belege sind eingeschlossen, damit der STB die Generalumkehr-Belege
/// auch im Paket hat. Drafts sind ausgeschlossen — sie sind keine §14-Belege.
pub async fn invoice_archives_for_year(
    pool: &SqlitePool,
    year: i32,
) -> Result<Vec<(String, Option<String>, Option<String>)>> {
    let rows: Vec<(String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT invoice_number, pdf_archive_id, xml_archive_id
           FROM invoices
          WHERE direction = 'issued'
            AND fiscal_year = ?
            AND locked_at IS NOT NULL
          ORDER BY invoice_number",
    )
    .bind(year as i64)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Original-Beleg-Archive für alle festgeschriebenen Kostenbelege eines GJ
/// (R2-014). Tuple = (`expense_number`, `receipt_archive_id`). Stornierte
/// Belege bleiben drin — der Steuerberater muss den Vorgang prüfen können.
pub async fn expense_archives_for_year(
    pool: &SqlitePool,
    year: i32,
) -> Result<Vec<(String, Option<String>)>> {
    let rows: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT expense_number, receipt_archive_id
           FROM expenses
          WHERE fiscal_year = ?
            AND locked_at IS NOT NULL
          ORDER BY expense_number",
    )
    .bind(year as i64)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Anzahl aktiver Anlagen (Anschaffung ≤ Jahr, nicht veräußert, Restbuchwert > 0),
/// für die im Geschäftsjahr noch KEINE AfA gebucht ist. > 0 ⇒ die EÜR dieses
/// Jahres würde Abschreibungen still auslassen (Safeguard für den Export).
pub async fn afa_pending_count(pool: &SqlitePool, year: i32) -> Result<i64> {
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM assets a
          WHERE a.disposed = 0
            AND a.acquisition_fiscal_year <= ?
            AND a.book_value_cents > 0
            AND NOT EXISTS (
                SELECT 1 FROM depreciation_entries e
                 WHERE e.asset_id = a.id AND e.fiscal_year = ?
            )",
    )
    .bind(year as i64)
    .bind(year as i64)
    .fetch_one(pool)
    .await?;
    Ok(n)
}

/// Veräußerte Anlagen mit Erlös + Restbuchwert-Abgang.
pub async fn disposals(pool: &SqlitePool) -> Result<Vec<DisposalView>> {
    let rows: Vec<(String, Option<i64>, Option<i64>)> = sqlx::query_as(
        "SELECT disposal_date, disposal_proceeds_cents, disposal_residual_book_value_cents
           FROM assets
          WHERE disposed = 1
            AND disposal_date IS NOT NULL",
    )
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|(disposal_date, proceeds, residual)| {
            Ok(DisposalView {
                disposal_date: parse_date(&disposal_date)?,
                proceeds_cents: proceeds.unwrap_or(0),
                residual_book_value_cents: residual.unwrap_or(0),
            })
        })
        .collect()
}

/// Geschäftsjahre, in denen überhaupt EÜR-relevante Bewegungen liegen — für den
/// Jahres-Selector im UI (absteigend).
pub async fn available_years(pool: &SqlitePool) -> Result<Vec<i32>> {
    let rows: Vec<(i64,)> = sqlx::query_as(
        // R2-007/R2-008: nur **gelockte** Belege erzeugen Jahres-Einträge im
        // Selector. `status='canceled'` wird bei Einnahmen NICHT gefiltert
        // (Periodengetreuheit, siehe income_payments).
        "SELECT DISTINCT y FROM (
            SELECT CAST(substr(paid_at, 1, 4) AS INTEGER) AS y
              FROM invoices
             WHERE paid_at IS NOT NULL AND direction = 'issued'
               AND locked_at IS NOT NULL
            UNION
            SELECT CAST(substr(invoice_date, 1, 4) AS INTEGER)
              FROM invoices
             WHERE is_storno_for IS NOT NULL
               AND locked_at IS NOT NULL
            UNION
            SELECT CAST(substr(paid_date, 1, 4) AS INTEGER)
              FROM expenses WHERE paid_date IS NOT NULL AND status = 'recorded'
            UNION
            SELECT fiscal_year FROM depreciation_entries
            UNION
            SELECT CAST(substr(disposal_date, 1, 4) AS INTEGER)
              FROM assets WHERE disposed = 1 AND disposal_date IS NOT NULL
         )
         WHERE y IS NOT NULL
         ORDER BY y DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(y,)| y as i32).collect())
}

/// Lädt alle Eingaben für die EÜR-Aggregation in einem Rutsch.
pub async fn load_inputs(pool: &SqlitePool) -> Result<EuerInputs> {
    Ok(EuerInputs {
        payments: income_payments(pool).await?,
        storno_reversals: storno_reversals(pool).await?,
        expenses: expense_payments(pool).await?,
        depreciation: depreciation_views(pool).await?,
        disposals: disposals(pool).await?,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
