//! Abgleich eines abgeschlossenen Geschäftsjahres mit seinem
//! Festschreibungs-Snapshot (Lauf F, KB-0031).
//!
//! Beim Abschluss hält `fiscal_year_locks` die EÜR-Eckwerte fest (Einnahmen,
//! Ausgaben, AfA, Überschuss). Auswertungen und Exporte eines abgeschlossenen
//! Jahres rechnen weiter aus den Einzelbuchungen (nur daraus entstehen
//! Einzelaufstellung, DATEV-Stapel und Belege); weichen die live gerechneten
//! Eckwerte vom Snapshot ab, hat sich entweder der Datenbestand eines
//! festgeschriebenen Jahres verändert (Eingriff von außen, Fehler) oder die
//! Rechenregel einer neueren Version. Beides muss sichtbar sein (GoBD Rz. 107 ff.
//! Unveränderbarkeit, Rz. 145 ff. Nachvollziehbarkeit): die Anzeige meldet es,
//! jeder Export trägt es ins Audit-Protokoll und in die Begleitdatei, und es
//! entsteht ein Hinweis. Blockiert wird nichts — der Steuerberater braucht die
//! Unterlagen gerade dann.

use crate::db::repo::{audit_log, euer as euer_repo};
use crate::error::Result;
use crate::euer::aggregate::{aggregate, EuerReport};
use crate::fiscal_year::lock::{self, FiscalYearLock};
use crate::notify::{self, NewNotification};
use serde::Serialize;
use sqlx::SqlitePool;

/// EÜR-Eckwerte in Cent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Figures {
    pub income_cents: i64,
    pub expense_cents: i64,
    pub afa_cents: i64,
    pub surplus_cents: i64,
}

impl Figures {
    pub fn of_lock(lock: &FiscalYearLock) -> Self {
        Self {
            income_cents: lock.income_total_cents,
            expense_cents: lock.expense_total_cents,
            afa_cents: lock.afa_total_cents,
            surplus_cents: lock.surplus_cents,
        }
    }

    pub fn of_report(report: &EuerReport) -> Self {
        Self {
            income_cents: report.total_income_cents,
            expense_cents: report.total_expenses_cents,
            afa_cents: report.depreciation_total_cents,
            surplus_cents: report.surplus_cents,
        }
    }
}

/// Ergebnis des Abgleichs für ein abgeschlossenes Jahr.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotCheck {
    pub fiscal_year: i64,
    pub closed_at: String,
    /// Version, die das Jahr abgeschlossen hat.
    pub closed_with_version: String,
    pub snapshot: Figures,
    pub live: Figures,
    pub matches: bool,
    /// Abweichende Eckwerte (`income`, `expenses`, `afa`, `surplus`).
    pub differences: Vec<&'static str>,
}

impl SnapshotCheck {
    /// Kurzform für Audit-Details: `match` bzw. `mismatch`.
    pub fn status(&self) -> &'static str {
        if self.matches {
            "match"
        } else {
            "mismatch"
        }
    }
}

/// Vergleicht Snapshot und live gerechneten Bericht (pure).
pub fn compare(lock: &FiscalYearLock, report: &EuerReport) -> SnapshotCheck {
    let snapshot = Figures::of_lock(lock);
    let live = Figures::of_report(report);
    let mut differences = Vec::new();
    if snapshot.income_cents != live.income_cents {
        differences.push("income");
    }
    if snapshot.expense_cents != live.expense_cents {
        differences.push("expenses");
    }
    if snapshot.afa_cents != live.afa_cents {
        differences.push("afa");
    }
    if snapshot.surplus_cents != live.surplus_cents {
        differences.push("surplus");
    }
    SnapshotCheck {
        fiscal_year: lock.fiscal_year,
        closed_at: lock.closed_at.clone(),
        closed_with_version: lock.app_version.clone(),
        snapshot,
        live,
        matches: differences.is_empty(),
        differences,
    }
}

/// Abgleich für `year`; `None`, wenn das Jahr nicht abgeschlossen ist.
pub async fn check(pool: &SqlitePool, year: i64) -> Result<Option<SnapshotCheck>> {
    let Some(lock) = lock::get(pool, year).await? else {
        return Ok(None);
    };
    let inputs = euer_repo::load_inputs(pool).await?;
    Ok(Some(compare(&lock, &aggregate(year as i32, &inputs))))
}

/// Status für Audit-Details eines Exports: `open` (Jahr nicht abgeschlossen),
/// sonst `match`/`mismatch`.
pub fn audit_status(check: Option<&SnapshotCheck>) -> &'static str {
    check.map_or("open", SnapshotCheck::status)
}

/// Meldet eine Abweichung, die ein Export festgestellt hat: Audit
/// `euer.snapshot_mismatch` (Eckwerte, keine Inhalte) und ein dringender
/// Hinweis (einmal je Jahr, `dedup_key`). Ohne Abweichung nichts.
pub async fn report_mismatch(pool: &SqlitePool, check: &SnapshotCheck, export: &str) -> Result<()> {
    if check.matches {
        return Ok(());
    }
    audit_log::append(
        pool,
        "euer.snapshot_mismatch",
        "fiscal_year",
        &check.fiscal_year.to_string(),
        Some(serde_json::json!({
            "export": export,
            "differences": check.differences,
            "snapshot": check.snapshot,
            "live": check.live,
            "closed_with_version": check.closed_with_version,
        })),
    )
    .await?;
    let body = format!(
        "Die heute gerechneten EÜR-Eckwerte des abgeschlossenen Geschäftsjahres {} weichen \
         vom festgeschriebenen Stand ab ({}). Maßgeblich für die abgegebene Erklärung ist der \
         festgeschriebene Stand; die Exporte weisen die Abweichung aus. Bitte die Ursache klären \
         (Änderung am Datenbestand oder an der Rechenregel einer neueren Version).",
        check.fiscal_year,
        check.differences.join(", ")
    );
    notify::emit(
        pool,
        NewNotification {
            title: "EÜR weicht vom Jahresabschluss ab",
            body: &body,
            severity: "urgent",
            related_entity_type: Some("fiscal_year"),
            related_entity_id: Some(&check.fiscal_year.to_string()),
            action_url: Some("/euer"),
            dedup_key: Some(&format!("euer_snapshot_mismatch:{}", check.fiscal_year)),
            ..Default::default()
        },
    )
    .await?;
    Ok(())
}

/// Daten für den Hinweiskasten im EÜR-PDF (Lauf F2c): `null` ohne Abweichung
/// (oder für ein offenes Jahr), sonst Abschluss, Felder und je Eckwert
/// Snapshot und heutiger Wert als formatierte Beträge.
pub fn pdf_block(check: Option<&SnapshotCheck>) -> serde_json::Value {
    let Some(c) = check.filter(|c| !c.matches) else {
        return serde_json::Value::Null;
    };
    let eur = |v: i64| crate::euer::detail::eur_grouped(v);
    let row = |label: &str, s: i64, l: i64| serde_json::json!({ "label": label, "snapshot": eur(s), "live": eur(l) });
    serde_json::json!({
        "closedAt": c.closed_at,
        "version": c.closed_with_version,
        "fields": c.differences.iter().map(|d| field_label(d)).collect::<Vec<_>>().join(", "),
        "rows": [
            row("Einnahmen", c.snapshot.income_cents, c.live.income_cents),
            row("Ausgaben", c.snapshot.expense_cents, c.live.expense_cents),
            row("AfA", c.snapshot.afa_cents, c.live.afa_cents),
            row("Überschuss", c.snapshot.surplus_cents, c.live.surplus_cents),
        ],
    })
}

/// Deutsche Bezeichnung eines abweichenden Eckwerts.
fn field_label(field: &str) -> &str {
    match field {
        "income" => "Einnahmen",
        "expenses" => "Ausgaben",
        "afa" => "AfA",
        "surplus" => "Überschuss",
        other => other,
    }
}

/// Kommentarzeilen für die ELSTER-Ausfüllhilfe (CSV) bei einer Abweichung
/// (Lauf F2c): stehen nach der Titelzeile, beginnen wie diese mit `#`.
pub fn csv_note_lines(check: Option<&SnapshotCheck>) -> Vec<String> {
    let Some(c) = check.filter(|c| !c.matches) else {
        return Vec::new();
    };
    let eur = |v: i64| crate::euer::detail::eur_grouped(v);
    vec![
        format!(
            "ACHTUNG: Abweichung vom Jahresabschluss vom {} (Klein.Buch {}) — {} weichen ab.",
            c.closed_at,
            c.closed_with_version,
            c.differences
                .iter()
                .map(|d| field_label(d))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        format!(
            "Festgeschrieben: Überschuss {}; heute: Überschuss {}. Maßgeblich für die \
             abgegebene Erklärung ist der festgeschriebene Stand.",
            eur(c.snapshot.surplus_cents),
            eur(c.live.surplus_cents)
        ),
    ]
}

/// Textblock für Begleitdateien (LIESMICH) bei einer Abweichung.
pub fn readme_block(check: &SnapshotCheck) -> String {
    let eur = |c: i64| crate::euer::detail::eur_grouped(c);
    format!(
        "ACHTUNG — Abweichung vom Festschreibungs-Snapshot:\n\
         \x20   Das Geschäftsjahr {year} wurde am {closed} (Klein.Buch {version}) abgeschlossen.\n\
         \x20   Die heute aus den Einzelbuchungen gerechneten Eckwerte weichen vom damals\n\
         \x20   festgeschriebenen Stand ab ({fields}).\n\
         \x20                 Snapshot        heute\n\
         \x20   Einnahmen   {si:>12}  {li:>12}\n\
         \x20   Ausgaben    {se:>12}  {le:>12}\n\
         \x20   AfA         {sa:>12}  {la:>12}\n\
         \x20   Überschuss  {ss:>12}  {ls:>12}\n\
         \x20   Maßgeblich für die abgegebene Erklärung ist der festgeschriebene Stand.\n\
         \x20   Die Ursache (Änderung am Datenbestand oder an der Rechenregel einer neueren\n\
         \x20   Version) bitte klären; der Vorgang steht im Audit-Protokoll.\n\n",
        year = check.fiscal_year,
        closed = check.closed_at,
        version = check.closed_with_version,
        fields = check.differences.join(", "),
        si = eur(check.snapshot.income_cents),
        li = eur(check.live.income_cents),
        se = eur(check.snapshot.expense_cents),
        le = eur(check.live.expense_cents),
        sa = eur(check.snapshot.afa_cents),
        la = eur(check.live.afa_cents),
        ss = eur(check.snapshot.surplus_cents),
        ls = eur(check.live.surplus_cents),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lock(income: i64, expense: i64, afa: i64) -> FiscalYearLock {
        FiscalYearLock {
            fiscal_year: 2026,
            closed_at: "2027-01-10 09:00:00".into(),
            income_total_cents: income,
            expense_total_cents: expense,
            afa_total_cents: afa,
            surplus_cents: income - expense - afa,
            assets_locked: 0,
            depreciation_entries_locked: 0,
            app_version: "2026.10.0".into(),
            schema_version: 31,
            notes: None,
            open_drafts: None,
        }
    }

    fn report(income: i64, expense: i64, afa: i64) -> EuerReport {
        let inputs = crate::euer::aggregate::EuerInputs::default();
        let mut r = crate::euer::aggregate::aggregate(2026, &inputs);
        r.total_income_cents = income;
        r.total_expenses_cents = expense;
        r.depreciation_total_cents = afa;
        r.surplus_cents = income - expense - afa;
        r
    }

    #[test]
    fn equal_figures_match() {
        let c = compare(&lock(10_000, 4_000, 1_000), &report(10_000, 4_000, 1_000));
        assert!(c.matches);
        assert!(c.differences.is_empty());
        assert_eq!(c.status(), "match");
    }

    #[test]
    fn differences_are_named() {
        let c = compare(&lock(10_000, 4_000, 1_000), &report(12_000, 4_000, 1_000));
        assert!(!c.matches);
        assert_eq!(c.differences, vec!["income", "surplus"]);
        assert_eq!(c.status(), "mismatch");
        let text = readme_block(&c);
        assert!(
            text.contains("Abweichung vom Festschreibungs-Snapshot"),
            "{text}"
        );
        assert!(text.contains("income, surplus"), "{text}");
    }

    /// Lauf F2c: Hinweiskasten im EÜR-PDF nur bei Abweichung, mit deutschen
    /// Feldnamen und beiden Eckwerten.
    #[test]
    fn pdf_block_only_on_mismatch() {
        let ok = compare(&lock(10_000, 4_000, 1_000), &report(10_000, 4_000, 1_000));
        assert!(pdf_block(Some(&ok)).is_null());
        assert!(pdf_block(None).is_null());
        assert!(csv_note_lines(Some(&ok)).is_empty());
        let bad = compare(&lock(10_000, 4_000, 1_000), &report(12_000, 4_000, 1_000));
        let block = pdf_block(Some(&bad));
        assert_eq!(block["fields"], "Einnahmen, Überschuss");
        assert_eq!(block["version"], "2026.10.0");
        assert_eq!(block["rows"][0]["label"], "Einnahmen");
        assert_eq!(block["rows"][0]["snapshot"], "100,00 €");
        assert_eq!(block["rows"][0]["live"], "120,00 €");
        assert_eq!(block["rows"][3]["live"], "70,00 €");
        let lines = csv_note_lines(Some(&bad));
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("ACHTUNG: Abweichung vom Jahresabschluss"));
        assert!(lines[0].contains("Einnahmen, Überschuss"), "{}", lines[0]);
        assert!(lines[1].contains("50,00 €") && lines[1].contains("70,00 €"));
    }
}
