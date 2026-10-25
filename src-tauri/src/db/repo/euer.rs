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
//! - **Ausgaben** = bezahlte (`paid_date IS NOT NULL`), nicht stornierte und
//!   nicht aktivierte Kosten, brutto, am `paid_date`. Aktivierte Kosten
//!   (`capitalized_as_asset_id`) wirken nur über die AfA der Anlage
//!   (§ 4 Abs. 3 Satz 3 EStG) — sonst stünden die Anschaffungskosten doppelt in
//!   der EÜR (Lauf F5b). Ausnahme (Lauf F8b): Bei Regelbesteuerung im
//!   Zeitpunkt des Leistungsbezugs (Jahr des Belegdatums, Lauf F8d) bleibt die
//!   gezahlte **Vorsteuer** einer aktivierten Position Betriebsausgabe des
//!   Zahlungsjahres (Anlage EÜR „gezahlte Vorsteuerbeträge“); aktiviert ist nur
//!   der Nettobetrag (§ 9b Abs. 1 EStG).
//! - **Steuerstatus der Kosten** (Lauf F8d, ADR 0039 Nr. 148): brutto bzw.
//!   netto mit Vorsteuer nach dem §19-Status bei Leistungsbezug (§ 15 Abs. 1
//!   UStG, Abschn. 19.5 UStAE), nicht nach dem Zahlungsjahr; die zeitliche
//!   Zuordnung bleibt das Zahlungsjahr (§ 11 Abs. 2 EStG).
//! - **AfA** = `depreciation_entries` (Jahres-Größe über `fiscal_year`).
//! - **Veräußerungen** = veräußerte Anlagen mit Erlös + Restbuchwert-Abgang am
//!   `disposal_date`.

use chrono::NaiveDate;
use sqlx::SqlitePool;

use crate::db::repo::invoices::parse_payment_history;
use crate::error::{Error, Result};
use crate::euer::aggregate::{
    DepreciationView, DisposalView, EuerInputs, ExpenseView, PaymentView, StornoReversalView,
};
use crate::euer::detail::{
    split_payment, AveeurItem, DisposalItem, ExpenseItem, IncomeItem, InvoiceTaxGroup,
    PrivateMovementItem, StornoItem, UnissuedNumberItem,
};
use crate::euer::elster_csv::AfaSplit;

/// Parst ein in der DB gespeichertes ISO-Datum (`YYYY-MM-DD`, ggf. mit
/// Zeitanteil) zu `NaiveDate`. Die ersten 10 Zeichen genügen.
fn parse_date(s: &str) -> Result<NaiveDate> {
    // R1-S2-PAYHIST: `get` statt Byte-Slice — keine Panik an einer
    // Nicht-ASCII-Grenze, ein kaputter Wert wird zum Fehler.
    let head = s.get(..10).unwrap_or(s);
    NaiveDate::parse_from_str(head, "%Y-%m-%d")
        .map_err(|e| Error::Domain(format!("EÜR: ungültiges Datum '{s}': {e}")))
}

/// Einzelne Zahlungseingänge ausgehender, NICHT-Storno-Rechnungen.
///
/// Teilzahlungen aus `payment_history_json` werden je Zahlung als eigener
/// [`PaymentView`] geliefert. Fehlt die Historie (ältere Daten), greift der
/// Fallback `paid_amount_cents` @ `paid_at`.
pub async fn income_payments<'e, E: sqlx::SqliteExecutor<'e>>(pool: E) -> Result<Vec<PaymentView>> {
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
        let history = parse_payment_history(history_json.as_deref())?;
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
pub async fn storno_reversals<'e, E: sqlx::SqliteExecutor<'e>>(
    pool: E,
) -> Result<Vec<StornoReversalView>> {
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
pub async fn expense_payments<'e, E: sqlx::SqliteExecutor<'e>>(
    pool: E,
) -> Result<Vec<ExpenseView>> {
    let rows: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT paid_date, category, gross_amount_cents
           FROM expenses
          WHERE status = 'recorded'
            AND paid_date IS NOT NULL
            AND capitalized_as_asset_id IS NULL",
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

/// Gezahlte Vorsteuer aktivierter Kosten (Lauf F8b). Bei Regelbesteuerung im
/// Zeitpunkt des Leistungsbezugs ist nur der Nettobetrag Anschaffungskosten
/// (§ 9b Abs. 1 EStG); die Vorsteuer bleibt Betriebsausgabe im Jahr der
/// Zahlung (§ 11 Abs. 2 EStG, Anlage EÜR „gezahlte Vorsteuerbeträge“). Bei §19
/// gibt es keine abziehbare Vorsteuer — der Bruttobetrag wirkt ganz über die
/// AfA. Maßgeblich ist der Status im Jahr des Belegdatums, nicht des
/// Zahlungsjahres (Lauf F8d, § 15 Abs. 1 UStG, Abschn. 19.5 UStAE). Kategorie
/// wie die Kosten (bei Regelbesteuerung stehen auch sonst Kosten brutto in
/// ihrer Kategorie; die Ausfüllhilfe weist die Vorsteuer gesondert aus).
pub async fn capitalized_input_tax(conn: &mut sqlx::SqliteConnection) -> Result<Vec<ExpenseView>> {
    let profile = crate::db::repo::seller_profile::get_in(&mut *conn).await?;
    let rows: Vec<(String, String, String, i64)> = sqlx::query_as(
        "SELECT paid_date, expense_date, category, tax_amount_cents
           FROM expenses
          WHERE status = 'recorded'
            AND paid_date IS NOT NULL
            AND capitalized_as_asset_id IS NOT NULL
            AND tax_amount_cents > 0",
    )
    .fetch_all(&mut *conn)
    .await?;

    let mut out = Vec::new();
    for (paid_date, expense_date, category, tax_cents) in rows {
        if crate::db::repo::seller_profile::is_klein_for_expense(profile.as_ref(), &expense_date)? {
            continue;
        }
        out.push(ExpenseView {
            paid_date: parse_date(&paid_date)?,
            category,
            gross_cents: tax_cents,
        });
    }
    Ok(out)
}

/// Alle AfA-Buchungen (Periodenzuordnung über `fiscal_year` im Core).
pub async fn depreciation_views<'e, E: sqlx::SqliteExecutor<'e>>(
    pool: E,
) -> Result<Vec<DepreciationView>> {
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

/// Steuergruppen (Steuerart + Satz) je festgeschriebener ausgehender Rechnung
/// mit Zahlung (Lauf F5b). Die Steuer je Gruppe entsteht wie auf der Rechnung
/// aus der Gruppensumme (BR-CO-17, [`crate::domain::invoice::tax_groups`]); eine
/// Rundungsdifferenz zur gespeicherten Rechnungssteuer (ältere Belege mit
/// Rundung je Position) trägt die größte Gruppe, damit die Summe dem Beleg
/// entspricht.
async fn invoice_tax_groups(
    pool: &SqlitePool,
) -> Result<std::collections::HashMap<String, Vec<InvoiceTaxGroup>>> {
    let items: Vec<(String, String, f64, i64)> = sqlx::query_as(
        "SELECT it.invoice_id, it.tax_category_code, it.tax_rate_percent, it.net_amount_cents
           FROM invoice_items it
           JOIN invoices i ON i.id = it.invoice_id
          WHERE i.direction = 'issued'
            AND i.locked_at IS NOT NULL
          ORDER BY it.invoice_id, it.position",
    )
    .fetch_all(pool)
    .await?;
    let totals: Vec<(String, i64)> = sqlx::query_as(
        "SELECT id, tax_amount_cents FROM invoices
          WHERE direction = 'issued' AND locked_at IS NOT NULL",
    )
    .fetch_all(pool)
    .await?;
    let stored_tax: std::collections::HashMap<String, i64> = totals.into_iter().collect();

    let mut lines: std::collections::HashMap<String, Vec<(String, f64, i64)>> =
        std::collections::HashMap::new();
    for (invoice_id, code, rate, net) in items {
        lines.entry(invoice_id).or_default().push((code, rate, net));
    }
    let mut out = std::collections::HashMap::new();
    for (invoice_id, ls) in lines {
        let mut groups: Vec<InvoiceTaxGroup> =
            crate::domain::invoice::tax_groups(ls.iter().map(|(c, r, n)| (c.as_str(), *r, *n)))
                .into_iter()
                .map(|g| InvoiceTaxGroup {
                    category_code: g.category_code,
                    rate_basis_points: (g.rate * 100.0).round() as i64,
                    net_cents: g.taxable_amount_cents,
                    tax_cents: g.tax_amount_cents,
                })
                .collect();
        if let Some(stored) = stored_tax.get(&invoice_id) {
            let computed: i64 = groups.iter().map(|g| g.tax_cents).sum();
            let diff = stored - computed;
            if diff != 0 {
                if let Some(g) = groups
                    .iter_mut()
                    .filter(|g| g.tax_cents != 0)
                    .max_by_key(|g| g.net_cents.abs())
                {
                    g.tax_cents += diff;
                }
            }
        }
        out.insert(invoice_id, groups);
    }
    Ok(out)
}

/// Einzelne Zahlungseingänge (Betriebseinnahmen) eines Geschäftsjahres —
/// Teilzahlungen je eigene Zeile, mit Rechnungsnummer, Kunde, eingefrorenem
/// §19-Status, Aufteilung auf die Steuergruppen und Zahlungskonto (Lauf F5b).
pub async fn income_detail(pool: &SqlitePool, year: i32) -> Result<Vec<IncomeItem>> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: String,
        invoice_number: String,
        buyer_name: Option<String>,
        paid_amount_cents: i64,
        paid_at: Option<String>,
        payment_history_json: Option<String>,
        description: Option<String>,
        is_kleinunternehmer: i64,
        account_type: Option<String>,
    }
    let rows: Vec<Row> = sqlx::query_as(
        // R2-008: defensiver Lock-Guard, siehe income_payments. `status` wird
        // bewusst NICHT gefiltert — Periodengetreuheit (Original-Einnahme bleibt
        // im Zahlungsjahr, Storno-Refund im Storno-Jahr).
        "SELECT i.id, i.invoice_number, i.buyer_name, i.paid_amount_cents, i.paid_at,
                i.payment_history_json, i.is_kleinunternehmer,
                (SELECT group_concat(d, '; ') FROM (
                     SELECT description AS d FROM invoice_items
                      WHERE invoice_id = i.id ORDER BY position
                 )) AS description,
                pa.type AS account_type
           FROM invoices i
           LEFT JOIN payment_accounts pa ON pa.id = i.paid_to_account_id
          WHERE i.direction = 'issued'
            AND i.is_storno_for IS NULL
            AND i.locked_at IS NOT NULL
            AND i.paid_amount_cents > 0",
    )
    .fetch_all(pool)
    .await?;
    let groups = invoice_tax_groups(pool).await?;
    // R2-011: Konto je Zahlungseingang aus der Zahlungshistorie.
    let account_types: std::collections::HashMap<String, String> =
        sqlx::query_as::<_, (String, String)>("SELECT id, type FROM payment_accounts")
            .fetch_all(pool)
            .await?
            .into_iter()
            .collect();

    let yp = format!("{year:04}");
    let mut out = Vec::new();
    for r in rows {
        let customer = r.buyer_name.unwrap_or_default();
        let description = r.description.unwrap_or_default();
        let is_klein = r.is_kleinunternehmer == 1;
        let g = groups.get(&r.id).map(Vec::as_slice).unwrap_or(&[]);
        let mk = |paid_date: &str, amount: i64, account: Option<&str>| IncomeItem {
            paid_date: trim_date(paid_date),
            invoice_number: r.invoice_number.clone(),
            customer: customer.clone(),
            description: description.clone(),
            amount_cents: amount,
            is_kleinunternehmer: is_klein,
            shares: split_payment(g, amount),
            account_type: account
                .and_then(|a| account_types.get(a).cloned())
                .or_else(|| r.account_type.clone()),
        };
        let history = parse_payment_history(r.payment_history_json.as_deref())?;
        if !history.is_empty() {
            for entry in history {
                if entry.amount_cents == 0 || !entry.paid_date.starts_with(&yp) {
                    continue;
                }
                out.push(mk(
                    &entry.paid_date,
                    entry.amount_cents,
                    entry.account_id.as_deref(),
                ));
            }
        } else if let Some(at) = r.paid_at.as_deref() {
            if at.starts_with(&yp) {
                out.push(mk(at, r.paid_amount_cents, None));
            }
        }
    }
    out.sort_by(|a, b| a.paid_date.cmp(&b.paid_date));
    Ok(out)
}

/// Storno-Erstattungen eines Geschäftsjahres (negative Einnahmen) — mit dem
/// §19-Status und den Steuergruppen des Originals. Geldkonto der Erstattung:
/// das Konto des letzten Zahlungseingangs mit Konto, sonst Bank (R2-011).
pub async fn storno_detail(pool: &SqlitePool, year: i32) -> Result<Vec<StornoItem>> {
    #[derive(sqlx::FromRow)]
    struct Row {
        storno_number: String,
        storno_date: String,
        original_id: String,
        original_number: String,
        refunded: i64,
        is_kleinunternehmer: i64,
        account_type: Option<String>,
        history: Option<String>,
    }
    let yp = format!("{year:04}");
    let rows: Vec<Row> = sqlx::query_as(
        // R2-007: Storno-Beleg muss gelockt sein (siehe storno_reversals).
        "SELECT s.invoice_number AS storno_number, s.invoice_date AS storno_date,
                o.id AS original_id, o.invoice_number AS original_number,
                o.paid_amount_cents AS refunded, o.is_kleinunternehmer,
                pa.type AS account_type, o.payment_history_json AS history
           FROM invoices s
           JOIN invoices o ON o.id = s.is_storno_for
           LEFT JOIN payment_accounts pa ON pa.id = o.paid_to_account_id
          WHERE s.is_storno_for IS NOT NULL
            AND s.locked_at IS NOT NULL
            AND o.paid_amount_cents > 0
            AND substr(s.invoice_date, 1, 4) = ?
          ORDER BY s.invoice_date",
    )
    .bind(&yp)
    .fetch_all(pool)
    .await?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let groups = invoice_tax_groups(pool).await?;
    let account_types: std::collections::HashMap<String, String> =
        sqlx::query_as::<_, (String, String)>("SELECT id, type FROM payment_accounts")
            .fetch_all(pool)
            .await?
            .into_iter()
            .collect();
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let g = groups.get(&r.original_id).map(Vec::as_slice).unwrap_or(&[]);
        let history_account = parse_payment_history(r.history.as_deref())?
            .into_iter()
            .rev()
            .find_map(|e| e.account_id)
            .and_then(|a| account_types.get(&a).cloned());
        out.push(StornoItem {
            storno_date: trim_date(&r.storno_date),
            storno_number: r.storno_number,
            original_number: r.original_number,
            refunded_cents: r.refunded,
            is_kleinunternehmer: r.is_kleinunternehmer == 1,
            shares: split_payment(g, r.refunded),
            account_type: history_account.or(r.account_type),
        });
    }
    Ok(out)
}

#[derive(sqlx::FromRow)]
struct ExpenseDetailRow {
    paid_date: String,
    expense_date: String,
    expense_number: String,
    vendor_name_snapshot: String,
    category: String,
    description: String,
    gross_amount_cents: i64,
    net_amount_cents: i64,
    tax_amount_cents: i64,
    account_type: Option<String>,
    asset_method: Option<String>,
    asset_cost: Option<i64>,
    asset_number: Option<String>,
}

impl ExpenseDetailRow {
    /// `is_klein` = §19-Status bei Leistungsbezug (Lauf F8d,
    /// [`crate::db::repo::seller_profile::is_klein_for_expense`]).
    fn into_item(self, is_klein: bool) -> ExpenseItem {
        ExpenseItem {
            paid_date: trim_date(&self.paid_date),
            expense_number: self.expense_number,
            vendor: self.vendor_name_snapshot,
            category: self.category,
            description: self.description,
            gross_cents: self.gross_amount_cents,
            net_cents: self.net_amount_cents,
            tax_cents: self.tax_amount_cents,
            is_kleinunternehmer: is_klein,
            account_type: self.account_type,
            capitalized_asset_method: self.asset_method,
            capitalized_asset_cost_cents: self.asset_cost,
            capitalized_asset_number: self.asset_number,
        }
    }
}

async fn expense_rows(pool: &SqlitePool, year: i32, capitalized: bool) -> Result<Vec<ExpenseItem>> {
    let yp = format!("{year:04}");
    let filter = if capitalized {
        "e.capitalized_as_asset_id IS NOT NULL"
    } else {
        "e.capitalized_as_asset_id IS NULL"
    };
    let sql = format!(
        "SELECT e.paid_date, e.expense_date, e.expense_number, e.vendor_name_snapshot,
                e.category, e.description,
                e.gross_amount_cents, e.net_amount_cents, e.tax_amount_cents,
                pa.type AS account_type, a.depreciation_method AS asset_method,
                a.acquisition_cost_cents AS asset_cost, a.asset_number AS asset_number
           FROM expenses e
           LEFT JOIN payment_accounts pa ON pa.id = e.paid_from_account_id
           LEFT JOIN assets a ON a.id = e.capitalized_as_asset_id
          WHERE e.status = 'recorded'
            AND e.paid_date IS NOT NULL
            AND substr(e.paid_date, 1, 4) = ?
            AND {filter}
          ORDER BY e.paid_date, e.expense_number"
    );
    let rows: Vec<ExpenseDetailRow> = sqlx::query_as(&sql).bind(&yp).fetch_all(pool).await?;
    let profile = crate::db::repo::seller_profile::get(pool).await?;
    rows.into_iter()
        .map(|r| {
            let is_klein = crate::db::repo::seller_profile::is_klein_for_expense(
                profile.as_ref(),
                &r.expense_date,
            )?;
            Ok(r.into_item(is_klein))
        })
        .collect()
}

/// Einzelne bezahlte Kostenpositionen eines Geschäftsjahres (brutto, mit
/// Netto/Steuer und Zahlungskonto). Aktivierte Kosten sind ausgenommen — sie
/// sind keine Betriebsausgabe (Lauf F5b).
pub async fn expense_detail(pool: &SqlitePool, year: i32) -> Result<Vec<ExpenseItem>> {
    expense_rows(pool, year, false).await
}

/// Bezahlte, als Anlage aktivierte Kostenpositionen eines Geschäftsjahres —
/// nur für den DATEV-Stapel (Zugang auf dem Anlagekonto, damit der Geldbestand
/// stimmt); in der EÜR wirken sie über die AfA.
pub async fn capitalized_expense_detail(pool: &SqlitePool, year: i32) -> Result<Vec<ExpenseItem>> {
    expense_rows(pool, year, true).await
}

/// Privatbewegungen (Entnahmen/Einlagen) eines Geschäftsjahres — EÜR-neutral,
/// aber für den DATEV-Buchungsstapel relevant (R2-009). Gefiltert nach dem
/// `fiscal_year`-Feld (Bewegung läuft immer übers Geschäftsjahr).
pub async fn private_movement_detail(
    pool: &SqlitePool,
    year: i32,
) -> Result<Vec<PrivateMovementItem>> {
    #[allow(clippy::type_complexity)]
    let rows: Vec<(String, String, String, String, i64, Option<String>)> = sqlx::query_as(
        "SELECT m.movement_date, m.movement_number, m.movement_type, m.description,
                m.amount_cents, pa.type
           FROM private_movements m
           LEFT JOIN payment_accounts pa ON pa.id = m.account_id
          WHERE m.fiscal_year = ?
          ORDER BY m.movement_date, m.movement_number",
    )
    .bind(year as i64)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(movement_date, movement_number, movement_type, description, amount_cents, acct)| {
                PrivateMovementItem {
                    movement_date: trim_date(&movement_date),
                    movement_number,
                    movement_type,
                    description,
                    amount_cents,
                    account_type: acct,
                }
            },
        )
        .collect())
}

/// Anlagen-Veräußerungen eines Geschäftsjahres (Einzelaufstellung).
pub async fn disposal_detail(pool: &SqlitePool, year: i32) -> Result<Vec<DisposalItem>> {
    let yp = format!("{year:04}");
    #[allow(clippy::type_complexity)]
    let rows: Vec<(
        String,
        String,
        String,
        Option<i64>,
        Option<i64>,
        String,
        Option<String>,
    )> = sqlx::query_as(
        "SELECT disposal_date, asset_number, label, disposal_proceeds_cents,
                disposal_residual_book_value_cents, depreciation_method, disposal_type
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
        .map(
            |(disposal_date, asset_number, label, proceeds, residual, method, dtype)| {
                let p = proceeds.unwrap_or(0);
                let r = residual.unwrap_or(0);
                DisposalItem {
                    disposal_date: trim_date(&disposal_date),
                    asset_number,
                    label,
                    proceeds_cents: p,
                    residual_book_value_cents: r,
                    gain_loss_cents: p - r,
                    depreciation_method: method,
                    disposal_type: dtype.unwrap_or_default(),
                }
            },
        )
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

/// Rechnungs- und Storno-Nummern eines GJ, die nie festgeschrieben wurden
/// (Lauf F2c): verworfene Entwürfe mit Zeitpunkt (Europe/Berlin) und Grund und
/// noch offene Entwürfe — für die Steuerberater-Übergabe, die sonst nur die
/// festgeschriebenen Belege enthält und Lücken nicht erklärt.
pub async fn unissued_numbers_for_year(
    pool: &SqlitePool,
    year: i32,
) -> Result<Vec<UnissuedNumberItem>> {
    #[allow(clippy::type_complexity)]
    let rows: Vec<(String, bool, String, Option<String>, Option<String>, i64)> = sqlx::query_as(
        "SELECT invoice_number, is_storno_for IS NOT NULL, invoice_date, voided_at,
                void_reason, gross_amount_cents
           FROM invoices
          WHERE direction = 'issued'
            AND fiscal_year = ?
            AND locked_at IS NULL
          ORDER BY invoice_number",
    )
    .bind(year as i64)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(number, is_storno, invoice_date, voided_at, void_reason, gross_cents)| {
                UnissuedNumberItem {
                    number,
                    is_storno,
                    invoice_date,
                    voided_at: voided_at.map(|ts| {
                        crate::clock::db_utc_to_berlin(&ts)
                            .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                            .unwrap_or(ts)
                    }),
                    void_reason,
                    gross_cents,
                }
            },
        )
        .collect())
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
pub async fn disposals<'e, E: sqlx::SqliteExecutor<'e>>(pool: E) -> Result<Vec<DisposalView>> {
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
    let mut conn = pool.acquire().await?;
    load_inputs_in(&mut conn).await
}

/// Wie [`load_inputs`], auf einer Verbindung bzw. in einer laufenden
/// Transaktion (Jahresabschluss: Snapshot und Protokoll in derselben
/// Transaktion, Lauf F).
pub async fn load_inputs_in(conn: &mut sqlx::SqliteConnection) -> Result<EuerInputs> {
    Ok(EuerInputs {
        payments: income_payments(&mut *conn).await?,
        storno_reversals: storno_reversals(&mut *conn).await?,
        expenses: {
            let mut v = expense_payments(&mut *conn).await?;
            v.extend(capitalized_input_tax(&mut *conn).await?);
            v
        },
        depreciation: depreciation_views(&mut *conn).await?,
        disposals: disposals(&mut *conn).await?,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
