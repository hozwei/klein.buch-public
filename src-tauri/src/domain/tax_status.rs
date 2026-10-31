//! Steuerstatus nach § 19 UStG als Historie (ADR 0042 Nr. 1–6). Pure, kein I/O.
//!
//! Der Status steht nicht mehr als Schalter im Verkäuferprofil, sondern als
//! append-only Folge von Einträgen (`tax_status_changes`, Migration 0033): ab
//! einem Wirkungsdatum gilt ein Status, mit einem Grund. Hier liegen
//!
//! - die Abfrage des Status an einem Datum ([`TaxStatusHistory::status_at`])
//!   und je Geschäftsjahr ([`TaxStatusHistory::year_status`]),
//! - die Prüfung eines neuen Eintrags bzw. Widerrufs ([`validate_change`],
//!   [`validate_revoke`]): Grund ↔ Status ↔ Wirkungsdatum, Bindung nach einem
//!   Verzicht, keine Überschneidung, keine Wirkung in einem abgeschlossenen
//!   Geschäftsjahr,
//! - der Gesamtumsatz nach vereinnahmten Entgelten und die Grenzprüfung
//!   ([`gesamtumsatz`], [`grenzpruefung`], ADR 0042 Nr. 5).
//!
//! Ohne Eintrag gilt §19 (Zielgruppe von Klein.Buch, wie bisher ohne Profil).

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

/// Steuerstatus an einem Tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaxStatus {
    Kleinunternehmer,
    Regelbesteuerung,
}

impl TaxStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Kleinunternehmer => "kleinunternehmer",
            Self::Regelbesteuerung => "regelbesteuerung",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "kleinunternehmer" => Some(Self::Kleinunternehmer),
            "regelbesteuerung" => Some(Self::Regelbesteuerung),
            _ => None,
        }
    }

    pub fn is_klein(self) -> bool {
        self == Self::Kleinunternehmer
    }

    pub fn from_klein(is_klein: bool) -> Self {
        if is_klein {
            Self::Kleinunternehmer
        } else {
            Self::Regelbesteuerung
        }
    }

    /// Bezeichnung für Texte.
    pub fn label(self) -> &'static str {
        match self {
            Self::Kleinunternehmer => "Kleinunternehmer nach §19 UStG",
            Self::Regelbesteuerung => "Regelbesteuerung",
        }
    }
}

/// Grund eines Eintrags (ADR 0042 Nr. 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeReason {
    /// Stand zu Beginn der Aufzeichnungen (Wirkung „seit Beginn“).
    Initial,
    /// Verzicht auf §19 (§ 19 Abs. 3 UStG): ab 1. Januar, Bindung fünf
    /// Kalenderjahre einschließlich des Wirkungsjahres.
    Verzicht,
    /// Gesamtumsatz des Vorjahres über 25.000 € (§ 19 Abs. 1 Satz 1 UStG):
    /// ab 1. Januar, ohne Bindung.
    WegfallVorjahr,
    /// Gesamtumsatz im laufenden Jahr über 100.000 € (§ 19 Abs. 1 Satz 1
    /// UStG): ab dem Umsatz, der die Grenze überschreitet, ohne Bindung.
    WegfallLaufend,
    /// Gründungsjahr über 25.000 € (§ 19 Abs. 2 UStG n. F.): ab dem Umsatz,
    /// der die Grenze überschreitet, ohne Bindung.
    WegfallGruendungsjahr,
    /// Rückkehr zu §19: zum 1. Januar; nach einem Verzicht erst nach Ablauf
    /// der Bindung.
    Rueckkehr,
    /// Aus dem bisherigen Schalter übernommen (Migration 0033); Grund unbekannt.
    Legacy,
}

impl ChangeReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Initial => "initial",
            Self::Verzicht => "verzicht",
            Self::WegfallVorjahr => "wegfall_vorjahr",
            Self::WegfallLaufend => "wegfall_laufend",
            Self::WegfallGruendungsjahr => "wegfall_gruendungsjahr",
            Self::Rueckkehr => "rueckkehr",
            Self::Legacy => "legacy",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "initial" => Self::Initial,
            "verzicht" => Self::Verzicht,
            "wegfall_vorjahr" => Self::WegfallVorjahr,
            "wegfall_laufend" => Self::WegfallLaufend,
            "wegfall_gruendungsjahr" => Self::WegfallGruendungsjahr,
            "rueckkehr" => Self::Rueckkehr,
            "legacy" => Self::Legacy,
            _ => return None,
        })
    }

    /// Status, den der Grund vorschreibt; `None` = frei (Anfangsstand, Bestand).
    pub fn fixed_status(self) -> Option<TaxStatus> {
        match self {
            Self::Verzicht
            | Self::WegfallVorjahr
            | Self::WegfallLaufend
            | Self::WegfallGruendungsjahr => Some(TaxStatus::Regelbesteuerung),
            Self::Rueckkehr => Some(TaxStatus::Kleinunternehmer),
            Self::Initial | Self::Legacy => None,
        }
    }

    /// Wirkung nur zum 1. Januar (ADR 0042 Nr. 2).
    pub fn requires_jan_first(self) -> bool {
        matches!(
            self,
            Self::Verzicht | Self::WegfallVorjahr | Self::Rueckkehr | Self::Legacy
        )
    }

    /// Bezeichnung für Texte.
    pub fn label(self) -> &'static str {
        match self {
            Self::Initial => "Stand zu Beginn",
            Self::Verzicht => "Verzicht auf §19 (§ 19 Abs. 3 UStG)",
            Self::WegfallVorjahr => "Wegfall: Vorjahresumsatz über 25.000 €",
            Self::WegfallLaufend => "Wegfall: Umsatz im laufenden Jahr über 100.000 €",
            Self::WegfallGruendungsjahr => "Wegfall: Umsatz im Gründungsjahr über 25.000 €",
            Self::Rueckkehr => "Rückkehr zu §19",
            Self::Legacy => "Wechsel aus dem bisherigen Schalter übernommen",
        }
    }
}

/// Wirkungsdatum „seit Beginn“ des Anfangsstands (Grund `initial`).
pub const ANFANG: &str = "0001-01-01";

pub fn anfang() -> NaiveDate {
    NaiveDate::from_ymd_opt(1, 1, 1).expect("gültiges Datum")
}

fn jan_first(year: i32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, 1, 1).expect("gültiges Jahr")
}

fn dec_31(year: i32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, 12, 31).expect("gültiges Jahr")
}

/// Ende der Bindung nach einem Verzicht (ADR 0042 Nr. 2): fünf Kalenderjahre
/// einschließlich des Wirkungsjahres; zurückgegeben wird der erste Tag, an dem
/// die Rückkehr möglich ist. Verzicht ab 2026 → Rückkehr frühestens 01.01.2031.
pub fn verzicht_binding_until(effective_from: NaiveDate) -> NaiveDate {
    jan_first(effective_from.year() + 5)
}

/// Maßgebliches Datum eines Ausgangsbelegs (ADR 0042 Nr. 3): Leistungsdatum,
/// bei einem Leistungszeitraum dessen Ende (ADR 0042 Nr. 10), ersatzweise das
/// Rechnungs- bzw. Angebotsdatum (§ 13 Abs. 1 Nr. 1 Buchst. a UStG).
pub fn document_date(
    delivery_date: Option<NaiveDate>,
    service_period_end: Option<NaiveDate>,
    document_date: NaiveDate,
) -> NaiveDate {
    delivery_date
        .or(service_period_end)
        .unwrap_or(document_date)
}

/// Ein Eintrag der Historie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaxStatusEntry {
    pub id: String,
    pub effective_from: NaiveDate,
    pub status: TaxStatus,
    pub reason: ChangeReason,
    pub binding_until: Option<NaiveDate>,
    pub revoked: bool,
}

/// Zeitabschnitt mit einheitlichem Status innerhalb eines Jahres.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusSegment {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub status: TaxStatus,
}

/// Status eines Geschäftsjahres (dreiwertig, ADR 0042 Nr. 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum YearStatus {
    Kleinunternehmer,
    Regelbesteuerung,
    /// Wechsel im Jahr.
    Wechsel,
}

impl YearStatus {
    /// §19 das ganze Jahr.
    pub fn is_klein(self) -> bool {
        self == Self::Kleinunternehmer
    }
}

/// Die gültigen (nicht widerrufenen) Einträge, nach Wirkungsdatum sortiert.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaxStatusHistory {
    active: Vec<TaxStatusEntry>,
}

impl TaxStatusHistory {
    /// Aus allen Einträgen; widerrufene fallen heraus.
    pub fn new(entries: impl IntoIterator<Item = TaxStatusEntry>) -> Self {
        let mut active: Vec<TaxStatusEntry> = entries.into_iter().filter(|e| !e.revoked).collect();
        active.sort_by_key(|e| e.effective_from);
        Self { active }
    }

    /// Gültige Einträge, aufsteigend nach Wirkungsdatum.
    pub fn entries(&self) -> &[TaxStatusEntry] {
        &self.active
    }

    /// Jüngster gültiger Eintrag (nach Wirkungsdatum).
    pub fn last(&self) -> Option<&TaxStatusEntry> {
        self.active.last()
    }

    /// Eintrag, der am `date` gilt (letzter mit Wirkungsdatum ≤ `date`).
    pub fn entry_at(&self, date: NaiveDate) -> Option<&TaxStatusEntry> {
        self.active.iter().rev().find(|e| e.effective_from <= date)
    }

    /// Status am `date`; ohne Eintrag §19.
    pub fn status_at(&self, date: NaiveDate) -> TaxStatus {
        self.entry_at(date)
            .map_or(TaxStatus::Kleinunternehmer, |e| e.status)
    }

    /// §19 am `date`?
    pub fn is_klein_at(&self, date: NaiveDate) -> bool {
        self.status_at(date).is_klein()
    }

    /// Abschnitte gleichen Status im Kalenderjahr `year` (lückenlos 01.01.–31.12.).
    pub fn segments(&self, year: i32) -> Vec<StatusSegment> {
        let start = jan_first(year);
        let end = dec_31(year);
        let mut out = vec![StatusSegment {
            from: start,
            to: end,
            status: self.status_at(start),
        }];
        for e in &self.active {
            if e.effective_from <= start || e.effective_from > end {
                continue;
            }
            let cur = out.last_mut().expect("mindestens ein Abschnitt");
            if cur.status == e.status {
                continue;
            }
            cur.to = e.effective_from.pred_opt().expect("gültiges Datum");
            out.push(StatusSegment {
                from: e.effective_from,
                to: end,
                status: e.status,
            });
        }
        out
    }

    /// Status des Geschäftsjahres (Kalenderjahr).
    pub fn year_status(&self, year: i32) -> YearStatus {
        let segs = self.segments(year);
        match (segs.len(), segs[0].status) {
            (1, TaxStatus::Kleinunternehmer) => YearStatus::Kleinunternehmer,
            (1, TaxStatus::Regelbesteuerung) => YearStatus::Regelbesteuerung,
            _ => YearStatus::Wechsel,
        }
    }

    /// Bindung nach einem Verzicht, die am `date` noch läuft: erster Tag, an dem
    /// die Rückkehr möglich ist.
    pub fn binding_at(&self, date: NaiveDate) -> Option<NaiveDate> {
        self.active
            .iter()
            .filter(|e| e.effective_from <= date)
            .filter_map(|e| e.binding_until)
            .filter(|until| *until > date)
            .max()
    }

    /// Frühestes Datum einer Rückkehr zu §19, wenn heute Regelbesteuerung gilt:
    /// nach einem Verzicht das Bindungsende, sonst der nächste 1. Januar nach
    /// dem letzten Eintrag bzw. nach heute.
    pub fn earliest_return(&self, today: NaiveDate) -> Option<NaiveDate> {
        let last = self.last()?;
        if last.status.is_klein() {
            return None;
        }
        let after_last = jan_first(last.effective_from.year() + 1);
        let this_year = jan_first(today.year());
        let base = after_last.max(this_year);
        Some(match last.binding_until {
            Some(until) => until.max(base),
            None => base,
        })
    }

    fn with(&self, extra: TaxStatusEntry) -> Self {
        Self::new(self.active.iter().cloned().chain(std::iter::once(extra)))
    }

    fn without(&self, id: &str) -> Self {
        Self::new(self.active.iter().filter(|e| e.id != id).cloned())
    }
}

/// Ein neuer Eintrag, wie ihn der Nutzer erfasst.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewChange {
    pub effective_from: NaiveDate,
    pub status: TaxStatus,
    pub reason: ChangeReason,
}

/// Grund, aus dem ein Eintrag oder Widerruf abgelehnt wird.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeError {
    /// `legacy` entsteht nur in der Migration.
    LegacyNotAllowed,
    ReasonStatusMismatch {
        reason: ChangeReason,
        status: TaxStatus,
    },
    /// Anfangsstand nur als erster Eintrag.
    InitialNotFirst,
    /// Anfangsstand wirkt „seit Beginn“.
    InitialDateNotStart,
    /// Wirkungsdatum vor 1900 (Tippfehler).
    DateOutOfRange(NaiveDate),
    NotJanFirst {
        reason: ChangeReason,
        date: NaiveDate,
    },
    /// Wegfall erst, wenn der Umsatz vorliegt.
    InFuture {
        reason: ChangeReason,
        date: NaiveDate,
    },
    /// Am Wirkungsdatum gilt der Status schon.
    NoChange {
        status: TaxStatus,
        date: NaiveDate,
    },
    /// Neue Einträge nur nach dem jüngsten gültigen Eintrag (keine Überschneidung).
    NotAfterLast {
        last: NaiveDate,
    },
    /// Rückkehr vor Ablauf der Bindung nach einem Verzicht.
    Binding {
        until: NaiveDate,
    },
    /// Der Eintrag änderte den Status in einem abgeschlossenen Geschäftsjahr.
    ClosedYear(i32),
    /// Der Eintrag änderte die Vorsteuer aktivierter Kosten, die in einem
    /// abgeschlossenen Geschäftsjahr gezahlt wurden ([`CapitalizedInputTax`]).
    ClosedYearInputTax(i32),
    /// Widerruf: Eintrag unbekannt oder schon widerrufen.
    NotFound,
    AlreadyRevoked,
    /// Widerruf nur des jüngsten gültigen Eintrags.
    NotLatest,
    /// Widerruf ohne Grund.
    ReasonMissing,
}

fn de(d: NaiveDate) -> String {
    d.format("%d.%m.%Y").to_string()
}

/// Deutscher Klartext eines Ablehnungsgrundes.
pub fn message(e: &ChangeError) -> String {
    match e {
        ChangeError::LegacyNotAllowed => {
            "Der Grund „übernommen“ entsteht nur bei der Übernahme des Bestands.".into()
        }
        ChangeError::ReasonStatusMismatch { reason, status } => format!(
            "„{}“ passt nicht zum Status „{}“.",
            reason.label(),
            status.label()
        ),
        ChangeError::InitialNotFirst => {
            "Den Stand zu Beginn gibt es schon; trage einen Wechsel mit Grund ein.".into()
        }
        ChangeError::InitialDateNotStart => {
            "Der Stand zu Beginn gilt seit Beginn der Aufzeichnungen, ohne eigenes Datum.".into()
        }
        ChangeError::DateOutOfRange(d) => format!("Das Wirkungsdatum {} ist ungültig.", de(*d)),
        ChangeError::NotJanFirst { reason, date } => format!(
            "„{}“ wirkt nur zum 1. Januar, nicht zum {} (§ 19 Abs. 1 und 3 UStG).",
            reason.label(),
            de(*date)
        ),
        ChangeError::InFuture { reason, date } => format!(
            "„{}“ lässt sich erst eintragen, wenn der Umsatz vorliegt; das Wirkungsdatum {} liegt in der Zukunft.",
            reason.label(),
            de(*date)
        ),
        ChangeError::NoChange { status, date } => format!(
            "Am {} gilt bereits „{}“.",
            de(*date),
            status.label()
        ),
        ChangeError::NotAfterLast { last } => format!(
            "Ein neuer Eintrag muss nach dem jüngsten Eintrag ({}) wirken. Einen späteren Eintrag widerrufst du vorher.",
            de(*last)
        ),
        ChangeError::Binding { until } => format!(
            "Nach dem Verzicht auf §19 bindet die Regelbesteuerung fünf Kalenderjahre; die Rückkehr ist frühestens zum {} möglich (§ 19 Abs. 3 UStG).",
            de(*until)
        ),
        ChangeError::ClosedYear(y) => format!(
            "Der Eintrag änderte den Steuerstatus im abgeschlossenen Geschäftsjahr {y}. Das ist nicht möglich; eine Korrektur des Jahres klärst du mit deiner Steuerberatung."
        ),
        ChangeError::ClosedYearInputTax(y) => format!(
            "Der Eintrag änderte die Vorsteuer von Kosten, die als Anlage übernommen und im abgeschlossenen Geschäftsjahr {y} bezahlt wurden (Belegdatum danach). Das ist nicht möglich; eine Korrektur des Jahres klärst du mit deiner Steuerberatung."
        ),
        ChangeError::NotFound => "Eintrag nicht gefunden.".into(),
        ChangeError::AlreadyRevoked => "Der Eintrag ist bereits widerrufen.".into(),
        ChangeError::NotLatest => {
            "Widerrufen lässt sich nur der jüngste gültige Eintrag.".into()
        }
        ChangeError::ReasonMissing => "Bitte einen Grund für den Widerruf angeben.".into(),
    }
}

/// Jahre unter `closed_years`, in denen sich der Status zwischen `old` und
/// `new` unterscheidet. Der Jahres-Snapshot hängt über die Vorsteuer
/// aktivierter Kosten vom Status ab (ADR 0042 Nr. 4).
fn changed_closed_years(
    old: &TaxStatusHistory,
    new: &TaxStatusHistory,
    closed_years: &[i32],
) -> Vec<i32> {
    let mut out: Vec<i32> = closed_years
        .iter()
        .copied()
        .filter(|y| old.segments(*y) != new.segments(*y))
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// Aktivierte Kosten mit Vorsteuer (ADR 0042 Nr. 4): Ihr Status richtet sich
/// nach dem Belegdatum, die Vorsteuer zählt aber im Jahr der Zahlung (§ 11
/// Abs. 2 EStG, `repo::euer::capitalized_input_tax`). Liegt die Zahlung in
/// einem abgeschlossenen Jahr und das Belegdatum danach, ändert ein Eintrag im
/// offenen Jahr den Snapshot des abgeschlossenen Jahres.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapitalizedInputTax {
    pub expense_date: NaiveDate,
    pub paid_year: i32,
}

/// Abgeschlossene Jahre, deren Vorsteuer aktivierter Kosten sich zwischen
/// `old` und `new` ändert (Zahljahr abgeschlossen, Status am Belegdatum
/// verschieden).
fn changed_closed_input_tax_years(
    old: &TaxStatusHistory,
    new: &TaxStatusHistory,
    closed_years: &[i32],
    capitalized: &[CapitalizedInputTax],
) -> Vec<i32> {
    let mut out: Vec<i32> = capitalized
        .iter()
        .filter(|c| closed_years.contains(&c.paid_year))
        .filter(|c| old.status_at(c.expense_date) != new.status_at(c.expense_date))
        .map(|c| c.paid_year)
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// Prüft einen neuen Eintrag gegen die Historie (ADR 0042 Nr. 2 und 4).
/// Liefert das Bindungsende (nur beim Verzicht) oder alle Ablehnungsgründe.
/// Ohne aktivierte Kosten; die Commands rufen [`validate_change_with`].
pub fn validate_change(
    hist: &TaxStatusHistory,
    new: &NewChange,
    today: NaiveDate,
    closed_years: &[i32],
) -> Result<Option<NaiveDate>, Vec<ChangeError>> {
    validate_change_with(hist, new, today, closed_years, &[])
}

/// Wie [`validate_change`], zusätzlich mit der Vorsteuer aktivierter Kosten
/// abgeschlossener Jahre ([`CapitalizedInputTax`]).
pub fn validate_change_with(
    hist: &TaxStatusHistory,
    new: &NewChange,
    today: NaiveDate,
    closed_years: &[i32],
    capitalized: &[CapitalizedInputTax],
) -> Result<Option<NaiveDate>, Vec<ChangeError>> {
    use ChangeError as E;
    let mut errs = Vec::new();
    let reason = new.reason;
    let date = new.effective_from;

    if reason == ChangeReason::Legacy {
        return Err(vec![E::LegacyNotAllowed]);
    }
    if let Some(fixed) = reason.fixed_status() {
        if fixed != new.status {
            errs.push(E::ReasonStatusMismatch {
                reason,
                status: new.status,
            });
        }
    }
    if reason == ChangeReason::Initial {
        if !hist.entries().is_empty() {
            errs.push(E::InitialNotFirst);
        }
        if date != anfang() {
            errs.push(E::InitialDateNotStart);
        }
    } else {
        if date.year() < 1900 {
            errs.push(E::DateOutOfRange(date));
        }
        if reason.requires_jan_first() && (date.month(), date.day()) != (1, 1) {
            errs.push(E::NotJanFirst { reason, date });
        }
        let future = match reason {
            ChangeReason::WegfallLaufend | ChangeReason::WegfallGruendungsjahr => date > today,
            ChangeReason::WegfallVorjahr => date.year() > today.year(),
            _ => false,
        };
        if future {
            errs.push(E::InFuture { reason, date });
        }
        if let Some(last) = hist.last() {
            if date <= last.effective_from {
                errs.push(E::NotAfterLast {
                    last: last.effective_from,
                });
            }
        }
        if hist.status_at(date) == new.status {
            errs.push(E::NoChange {
                status: new.status,
                date,
            });
        }
        if reason == ChangeReason::Rueckkehr {
            if let Some(until) = hist.binding_at(date) {
                errs.push(E::Binding { until });
            }
        }
    }
    let binding_until = (reason == ChangeReason::Verzicht).then(|| verzicht_binding_until(date));
    if errs.is_empty() {
        let next = hist.with(TaxStatusEntry {
            id: String::new(),
            effective_from: date,
            status: new.status,
            reason,
            binding_until,
            revoked: false,
        });
        errs.extend(
            changed_closed_years(hist, &next, closed_years)
                .into_iter()
                .map(E::ClosedYear),
        );
        if errs.is_empty() {
            errs.extend(
                changed_closed_input_tax_years(hist, &next, closed_years, capitalized)
                    .into_iter()
                    .map(E::ClosedYearInputTax),
            );
        }
    }
    if errs.is_empty() {
        Ok(binding_until)
    } else {
        Err(errs)
    }
}

/// Prüft den Widerruf des Eintrags `id` (ADR 0042 Nr. 1): nur der jüngste
/// gültige Eintrag, kein abgeschlossenes Geschäftsjahr betroffen. Ohne
/// aktivierte Kosten; die Commands rufen [`validate_revoke_with`].
pub fn validate_revoke(
    hist: &TaxStatusHistory,
    id: &str,
    closed_years: &[i32],
) -> Result<(), ChangeError> {
    validate_revoke_with(hist, id, closed_years, &[])
}

/// Wie [`validate_revoke`], zusätzlich mit der Vorsteuer aktivierter Kosten
/// abgeschlossener Jahre ([`CapitalizedInputTax`]).
pub fn validate_revoke_with(
    hist: &TaxStatusHistory,
    id: &str,
    closed_years: &[i32],
    capitalized: &[CapitalizedInputTax],
) -> Result<(), ChangeError> {
    let Some(last) = hist.last() else {
        return Err(ChangeError::NotFound);
    };
    if last.id != id {
        return Err(if hist.entries().iter().any(|e| e.id == id) {
            ChangeError::NotLatest
        } else {
            ChangeError::NotFound
        });
    }
    let next = hist.without(id);
    if let Some(y) = changed_closed_years(hist, &next, closed_years).first() {
        return Err(ChangeError::ClosedYear(*y));
    }
    match changed_closed_input_tax_years(hist, &next, closed_years, capitalized).first() {
        Some(y) => Err(ChangeError::ClosedYearInputTax(*y)),
        None => Ok(()),
    }
}

/// Letzter Tag, an dem der Verzicht für das Jahr `year` erklärt werden kann:
/// Ende Februar des übernächsten Jahres (§ 19 Abs. 3 Satz 1 UStG, ADR 0042
/// Nr. 4). Verzicht für 2026 → bis 28.02.2028.
pub fn verzicht_deadline(year: i32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year + 2, 3, 1)
        .and_then(|d| d.pred_opt())
        .expect("gültiges Jahr")
}

/// Hinweis zu einem geplanten Eintrag, der ihn nicht sperrt (ADR 0042 Nr. 4
/// und 5). Klein.Buch kennt die Erklärung beim Finanzamt und den vollständigen
/// Gesamtumsatz nicht sicher; darum Warnung statt Ablehnung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeWarning {
    /// Verzicht für ein Jahr, dessen Erklärungsfrist abgelaufen ist.
    VerzichtLate { year: i32, deadline: NaiveDate },
    /// Rückkehr, obwohl der Gesamtumsatz des Vorjahres nach den in Klein.Buch
    /// erfassten Zahlungen über 25.000 € lag.
    RueckkehrTurnover { year: i32, turnover_cents: i64 },
}

/// Warnungen zu einem geplanten Eintrag. `turnover_before`: Gesamtumsatz des
/// Jahres vor dem Wirkungsjahr (nur für die Rückkehr gebraucht).
pub fn change_warnings(
    new: &NewChange,
    today: NaiveDate,
    turnover_before: Option<i64>,
) -> Vec<ChangeWarning> {
    let year = new.effective_from.year();
    let mut out = Vec::new();
    match new.reason {
        ChangeReason::Verzicht => {
            let deadline = verzicht_deadline(year);
            if today > deadline {
                out.push(ChangeWarning::VerzichtLate { year, deadline });
            }
        }
        ChangeReason::Rueckkehr => {
            if let Some(t) = turnover_before.filter(|t| *t > GRENZE_VORJAHR_CENTS) {
                out.push(ChangeWarning::RueckkehrTurnover {
                    year: year - 1,
                    turnover_cents: t,
                });
            }
        }
        _ => {}
    }
    out
}

/// Deutscher Klartext einer Warnung.
pub fn warning_message(w: &ChangeWarning) -> String {
    match w {
        ChangeWarning::VerzichtLate { year, deadline } => format!(
            "Den Verzicht für {year} konnte man nur bis zum {} gegenüber dem Finanzamt erklären \
             (§ 19 Abs. 3 Satz 1 UStG). Trage ihn nur ein, wenn du ihn rechtzeitig erklärt hast.",
            de(*deadline)
        ),
        ChangeWarning::RueckkehrTurnover {
            year,
            turnover_cents,
        } => format!(
            "Nach den in Klein.Buch erfassten Zahlungen lag dein Gesamtumsatz {year} bei {} und \
             damit über 25.000 €. Dann ist die Rückkehr zu §19 zum 01.01.{} nicht möglich \
             (§ 19 Abs. 1 UStG). Trage sie nur ein, wenn dein Gesamtumsatz tatsächlich höchstens \
             25.000 € betrug.",
            crate::euer::detail::eur_grouped(*turnover_cents),
            year + 1
        ),
    }
}

/// Erster Tag, an dem sich der Status zwischen `old` und `new` unterscheidet
/// (für die Hinweise zu betroffenen Belegen); `None` = gleich.
pub fn first_difference(old: &TaxStatusHistory, new: &TaxStatusHistory) -> Option<NaiveDate> {
    let mut points: Vec<NaiveDate> = old
        .entries()
        .iter()
        .chain(new.entries())
        .map(|e| e.effective_from)
        .collect();
    points.sort_unstable();
    points.dedup();
    points
        .into_iter()
        .find(|d| old.status_at(*d) != new.status_at(*d))
}

impl TaxStatusHistory {
    /// Historie nach einem neuen Eintrag (für Folgen-Vorschau und Hinweise).
    pub fn after_change(&self, new: &NewChange, binding_until: Option<NaiveDate>) -> Self {
        self.with(TaxStatusEntry {
            id: String::new(),
            effective_from: new.effective_from,
            status: new.status,
            reason: new.reason,
            binding_until,
            revoked: false,
        })
    }

    /// Gilt §19 heute oder ab einem künftigen Eintrag? (Vorlagenwahl: eine
    /// Vorlage ohne Klausel wäre dann für künftige Belege unbrauchbar.)
    pub fn klein_today_or_later(&self, today: NaiveDate) -> bool {
        self.is_klein_at(today)
            || self
                .active
                .iter()
                .any(|e| e.effective_from > today && e.status.is_klein())
    }

    /// Historie nach dem Widerruf des Eintrags `id`.
    pub fn after_revoke(&self, id: &str) -> Self {
        self.without(id)
    }
}

// ============================================================================
// Gesamtumsatz und Grenzprüfung (ADR 0042 Nr. 5)
// ============================================================================

/// Grenze Vorjahr (§ 19 Abs. 1 Satz 1 UStG): 25.000 €.
pub const GRENZE_VORJAHR_CENTS: i64 = 2_500_000;
/// Grenze laufendes Jahr (§ 19 Abs. 1 Satz 1 UStG): 100.000 €.
pub const GRENZE_LAUFEND_CENTS: i64 = 10_000_000;
/// Grenze im Gründungsjahr (§ 19 Abs. 2 UStG n. F.): 25.000 €.
pub const GRENZE_GRUENDUNG_CENTS: i64 = 2_500_000;

/// Ein vereinnahmtes Entgelt (Zahlungseingang, netto; Storno-Erstattung
/// negativ). Umsätze mit Anlagevermögen gehören nicht hinein (§ 19 Abs. 2
/// UStG); Klein.Buch erfasst Anlagenabgänge ohnehin nicht als Rechnung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UmsatzPosten {
    /// Tag der Vereinnahmung.
    pub paid_date: NaiveDate,
    pub net_cents: i64,
    /// Belegnummer der Rechnung (bzw. des Stornos).
    pub document: String,
    /// Maßgebliches Datum des Umsatzes (Leistungsdatum, sonst Rechnungsdatum).
    pub document_date: NaiveDate,
}

/// Gesamtumsatz nach vereinnahmten Entgelten im Kalenderjahr `year` (netto).
pub fn gesamtumsatz(posten: &[UmsatzPosten], year: i32) -> i64 {
    posten
        .iter()
        .filter(|p| p.paid_date.year() == year)
        .map(|p| p.net_cents)
        .sum()
}

/// Art der Umsatzgrenze.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Grenzart {
    Vorjahr,
    Laufend,
    Gruendungsjahr,
}

impl Grenzart {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Vorjahr => "vorjahr",
            Self::Laufend => "laufend",
            Self::Gruendungsjahr => "gruendungsjahr",
        }
    }
}

/// Hinweis zu einer erreichten oder überschrittenen Umsatzgrenze.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Grenzhinweis {
    pub kind: Grenzart,
    /// Jahr, dessen Umsatz geprüft wurde.
    pub year: i32,
    pub turnover_cents: i64,
    pub limit_cents: i64,
    /// `true` = überschritten (Wegfall), `false` = genau erreicht (§19 gilt weiter).
    pub exceeded: bool,
    /// Beleg, mit dem die Grenze überschritten wurde (laufendes Jahr, Gründungsjahr).
    pub trigger_document: Option<String>,
    /// Vorschlag für das Wirkungsdatum.
    pub suggested_effective_from: Option<NaiveDate>,
    /// Vorgeschlagener Grund.
    pub suggested_reason: ChangeReason,
    /// `true` = nur vermutet (Gründungsjahr: in Klein.Buch kein früherer Umsatz).
    pub presumed: bool,
}

/// Erster Posten des Jahres, mit dem die kumulierte Summe `limit` übersteigt.
fn trigger(posten: &[UmsatzPosten], year: i32, limit: i64) -> Option<&UmsatzPosten> {
    let mut list: Vec<&UmsatzPosten> = posten
        .iter()
        .filter(|p| p.paid_date.year() == year)
        .collect();
    list.sort_by_key(|p| p.paid_date);
    let mut sum = 0i64;
    for p in list {
        sum += p.net_cents;
        if sum > limit {
            return Some(p);
        }
    }
    None
}

/// Prüft die Umsatzgrenzen für das Jahr von `today` (ADR 0042 Nr. 5). Nur
/// solange §19 gilt; den Wechsel trägt der Nutzer selbst ein. `earlier_turnover`:
/// Klein.Buch kennt Umsätze vor dem Vorjahr (sonst gilt ein Jahr ohne
/// Vorjahresumsatz als vermutetes Gründungsjahr).
pub fn grenzpruefung(
    hist: &TaxStatusHistory,
    posten: &[UmsatzPosten],
    today: NaiveDate,
    earlier_turnover: bool,
) -> Vec<Grenzhinweis> {
    let year = today.year();
    let mut out = Vec::new();
    let prev = gesamtumsatz(posten, year - 1);
    let current = gesamtumsatz(posten, year);
    let earlier_turnover = earlier_turnover || posten.iter().any(|p| p.paid_date.year() < year);

    if hist.is_klein_at(jan_first(year)) && prev >= GRENZE_VORJAHR_CENTS {
        let exceeded = prev > GRENZE_VORJAHR_CENTS;
        out.push(Grenzhinweis {
            kind: Grenzart::Vorjahr,
            year: year - 1,
            turnover_cents: prev,
            limit_cents: GRENZE_VORJAHR_CENTS,
            exceeded,
            trigger_document: None,
            suggested_effective_from: exceeded.then(|| jan_first(year)),
            suggested_reason: ChangeReason::WegfallVorjahr,
            presumed: false,
        });
    }
    let laufend = |kind: Grenzart, limit: i64, presumed: bool| -> Option<Grenzhinweis> {
        if current < limit {
            return None;
        }
        let t = trigger(posten, year, limit);
        // Status am Umsatz, der die Grenze überschreitet; ohne Überschreiten heute.
        let at = t.map_or(today, |p| p.document_date.max(jan_first(year)));
        if !hist.is_klein_at(at) {
            return None;
        }
        Some(Grenzhinweis {
            kind,
            year,
            turnover_cents: current,
            limit_cents: limit,
            exceeded: t.is_some(),
            trigger_document: t.map(|p| p.document.clone()),
            suggested_effective_from: t.map(|_| at),
            suggested_reason: if kind == Grenzart::Gruendungsjahr {
                ChangeReason::WegfallGruendungsjahr
            } else {
                ChangeReason::WegfallLaufend
            },
            presumed,
        })
    };
    if !earlier_turnover {
        out.extend(laufend(
            Grenzart::Gruendungsjahr,
            GRENZE_GRUENDUNG_CENTS,
            true,
        ));
    }
    out.extend(laufend(Grenzart::Laufend, GRENZE_LAUFEND_CENTS, false));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn entry(
        id: &str,
        from: NaiveDate,
        status: TaxStatus,
        reason: ChangeReason,
        binding: Option<NaiveDate>,
    ) -> TaxStatusEntry {
        TaxStatusEntry {
            id: id.into(),
            effective_from: from,
            status,
            reason,
            binding_until: binding,
            revoked: false,
        }
    }

    use ChangeReason as R;
    use TaxStatus::{Kleinunternehmer as K, Regelbesteuerung as G};

    fn initial(status: TaxStatus) -> TaxStatusEntry {
        entry("a", anfang(), status, R::Initial, None)
    }

    fn change(from: NaiveDate, status: TaxStatus, reason: ChangeReason) -> NewChange {
        NewChange {
            effective_from: from,
            status,
            reason,
        }
    }

    #[test]
    fn empty_history_is_kleinunternehmer() {
        let h = TaxStatusHistory::default();
        assert!(h.is_klein_at(d(2026, 5, 1)));
        assert_eq!(h.year_status(2026), YearStatus::Kleinunternehmer);
        assert_eq!(h.binding_at(d(2026, 5, 1)), None);
        assert_eq!(h.earliest_return(d(2026, 5, 1)), None);
    }

    #[test]
    fn status_at_is_inclusive_at_effective_date() {
        let h = TaxStatusHistory::new([
            initial(K),
            entry("b", d(2026, 8, 15), G, R::WegfallLaufend, None),
        ]);
        assert!(h.is_klein_at(d(2026, 8, 14)));
        assert!(!h.is_klein_at(d(2026, 8, 15)));
        assert_eq!(h.year_status(2025), YearStatus::Kleinunternehmer);
        assert_eq!(h.year_status(2026), YearStatus::Wechsel);
        assert_eq!(h.year_status(2027), YearStatus::Regelbesteuerung);
        assert_eq!(
            h.segments(2026),
            vec![
                StatusSegment {
                    from: d(2026, 1, 1),
                    to: d(2026, 8, 14),
                    status: K
                },
                StatusSegment {
                    from: d(2026, 8, 15),
                    to: d(2026, 12, 31),
                    status: G
                },
            ]
        );
    }

    /// ADR 0042 Nr. 3 mit Nr. 10 (V3-R01): maßgeblich ist das Leistungsdatum,
    /// bei einem Leistungszeitraum dessen Ende, ersatzweise das Belegdatum.
    #[test]
    fn document_date_prefers_delivery_then_period_end_then_document() {
        let rechnung = d(2026, 9, 10);
        let ende = d(2026, 9, 30);
        assert_eq!(document_date(None, Some(ende), rechnung), ende);
        assert_eq!(
            document_date(Some(d(2026, 9, 5)), Some(ende), rechnung),
            d(2026, 9, 5),
            "Leistungsdatum geht dem Ende des Zeitraums vor"
        );
        assert_eq!(document_date(None, None, rechnung), rechnung);
        // Wegfall am 15.09.: Zeitraum bis 30.09. → Regel, bis 14.09. → §19.
        let h = TaxStatusHistory::new([
            initial(K),
            entry("b", d(2026, 9, 15), G, R::WegfallLaufend, None),
        ]);
        assert!(!h.is_klein_at(document_date(None, Some(ende), rechnung)));
        assert!(h.is_klein_at(document_date(None, Some(d(2026, 9, 14)), rechnung)));
    }

    #[test]
    fn revoked_entries_are_ignored() {
        let mut e = entry("b", d(2026, 1, 1), G, R::Verzicht, Some(d(2031, 1, 1)));
        e.revoked = true;
        let h = TaxStatusHistory::new([initial(K), e]);
        assert!(h.is_klein_at(d(2027, 1, 1)));
        assert_eq!(h.binding_at(d(2027, 1, 1)), None);
    }

    #[test]
    fn verzicht_binds_five_years_including_the_effective_year() {
        assert_eq!(verzicht_binding_until(d(2026, 1, 1)), d(2031, 1, 1));
        let hist = TaxStatusHistory::new([initial(K)]);
        let b = validate_change(
            &hist,
            &change(d(2026, 1, 1), G, R::Verzicht),
            d(2026, 3, 1),
            &[],
        )
        .unwrap();
        assert_eq!(b, Some(d(2031, 1, 1)));
        let h = hist.after_change(&change(d(2026, 1, 1), G, R::Verzicht), b);
        // Rückkehr 2031 erlaubt, 2030 nicht.
        assert!(validate_change(
            &h,
            &change(d(2031, 1, 1), K, R::Rueckkehr),
            d(2030, 12, 1),
            &[]
        )
        .is_ok());
        let err = validate_change(
            &h,
            &change(d(2030, 1, 1), K, R::Rueckkehr),
            d(2029, 12, 1),
            &[],
        )
        .unwrap_err();
        assert_eq!(
            err,
            vec![ChangeError::Binding {
                until: d(2031, 1, 1)
            }]
        );
        assert_eq!(h.earliest_return(d(2026, 6, 1)), Some(d(2031, 1, 1)));
    }

    #[test]
    fn wegfall_vorjahr_allows_return_next_year_without_lock() {
        let hist = TaxStatusHistory::new([initial(K)]);
        let b = validate_change(
            &hist,
            &change(d(2026, 1, 1), G, R::WegfallVorjahr),
            d(2026, 2, 1),
            &[],
        )
        .unwrap();
        assert_eq!(b, None);
        let h = hist.after_change(&change(d(2026, 1, 1), G, R::WegfallVorjahr), b);
        assert_eq!(h.earliest_return(d(2026, 6, 1)), Some(d(2027, 1, 1)));
        assert!(validate_change(
            &h,
            &change(d(2027, 1, 1), K, R::Rueckkehr),
            d(2027, 1, 5),
            &[]
        )
        .is_ok());
    }

    #[test]
    fn jan_first_rules_per_reason() {
        let hist = TaxStatusHistory::new([initial(K)]);
        let today = d(2026, 9, 1);
        for reason in [R::Verzicht, R::WegfallVorjahr] {
            let err =
                validate_change(&hist, &change(d(2026, 3, 1), G, reason), today, &[]).unwrap_err();
            assert!(err.contains(&ChangeError::NotJanFirst {
                reason,
                date: d(2026, 3, 1)
            }));
        }
        for reason in [R::WegfallLaufend, R::WegfallGruendungsjahr] {
            assert!(validate_change(&hist, &change(d(2026, 8, 15), G, reason), today, &[]).is_ok());
        }
        let regel = TaxStatusHistory::new([initial(G)]);
        let err = validate_change(&regel, &change(d(2027, 3, 1), K, R::Rueckkehr), today, &[])
            .unwrap_err();
        assert!(matches!(err[0], ChangeError::NotJanFirst { .. }));
    }

    #[test]
    fn reason_must_match_status_and_change_something() {
        let hist = TaxStatusHistory::new([initial(K)]);
        let today = d(2026, 9, 1);
        let err =
            validate_change(&hist, &change(d(2027, 1, 1), K, R::Verzicht), today, &[]).unwrap_err();
        assert!(err.contains(&ChangeError::ReasonStatusMismatch {
            reason: R::Verzicht,
            status: K
        }));
        let err = validate_change(&hist, &change(d(2027, 1, 1), K, R::Rueckkehr), today, &[])
            .unwrap_err();
        assert_eq!(
            err,
            vec![ChangeError::NoChange {
                status: K,
                date: d(2027, 1, 1)
            }]
        );
        assert_eq!(
            validate_change(&hist, &change(d(2027, 1, 1), G, R::Legacy), today, &[]),
            Err(vec![ChangeError::LegacyNotAllowed])
        );
    }

    #[test]
    fn wegfall_only_with_turnover_not_in_future() {
        let hist = TaxStatusHistory::new([initial(K)]);
        let today = d(2026, 9, 1);
        let err = validate_change(
            &hist,
            &change(d(2026, 9, 2), G, R::WegfallLaufend),
            today,
            &[],
        )
        .unwrap_err();
        assert!(matches!(err[0], ChangeError::InFuture { .. }));
        let err = validate_change(
            &hist,
            &change(d(2027, 1, 1), G, R::WegfallVorjahr),
            today,
            &[],
        )
        .unwrap_err();
        assert!(matches!(err[0], ChangeError::InFuture { .. }));
        // Verzicht für das Folgejahr ist erlaubt (Wirkung in der Zukunft).
        assert!(validate_change(&hist, &change(d(2027, 1, 1), G, R::Verzicht), today, &[]).is_ok());
    }

    #[test]
    fn initial_only_first_and_from_start() {
        let empty = TaxStatusHistory::default();
        let today = d(2026, 9, 1);
        assert!(validate_change(&empty, &change(anfang(), G, R::Initial), today, &[]).is_ok());
        let err =
            validate_change(&empty, &change(d(2026, 1, 1), G, R::Initial), today, &[]).unwrap_err();
        assert_eq!(err, vec![ChangeError::InitialDateNotStart]);
        let hist = TaxStatusHistory::new([initial(K)]);
        let err = validate_change(&hist, &change(anfang(), G, R::Initial), today, &[]).unwrap_err();
        assert_eq!(err, vec![ChangeError::InitialNotFirst]);
        // Anfangsstand Regel bei abgeschlossenem Jahr kippt das Jahr → abgelehnt.
        let err =
            validate_change(&empty, &change(anfang(), G, R::Initial), today, &[2025]).unwrap_err();
        assert_eq!(err, vec![ChangeError::ClosedYear(2025)]);
    }

    #[test]
    fn no_overlap_with_later_entries() {
        let hist = TaxStatusHistory::new([
            initial(K),
            entry("b", d(2027, 1, 1), G, R::Verzicht, Some(d(2032, 1, 1))),
        ]);
        let err = validate_change(
            &hist,
            &change(d(2026, 8, 15), G, R::WegfallLaufend),
            d(2026, 9, 1),
            &[],
        )
        .unwrap_err();
        assert!(err.contains(&ChangeError::NotAfterLast {
            last: d(2027, 1, 1)
        }));
    }

    #[test]
    fn retroactive_entry_in_closed_year_is_rejected() {
        let hist = TaxStatusHistory::new([initial(K)]);
        // Rückwirkend in das abgeschlossene 2025: abgelehnt; ins offene 2026: erlaubt.
        let err = validate_change(
            &hist,
            &change(d(2025, 11, 3), G, R::WegfallLaufend),
            d(2026, 3, 1),
            &[2025],
        )
        .unwrap_err();
        assert_eq!(err, vec![ChangeError::ClosedYear(2025)]);
        assert!(validate_change(
            &hist,
            &change(d(2026, 1, 1), G, R::WegfallVorjahr),
            d(2026, 3, 1),
            &[2025]
        )
        .is_ok());
    }

    #[test]
    fn revoke_only_latest_and_not_in_closed_year() {
        let hist = TaxStatusHistory::new([
            initial(K),
            entry("b", d(2026, 1, 1), G, R::Verzicht, Some(d(2031, 1, 1))),
        ]);
        assert_eq!(
            validate_revoke(&hist, "a", &[]),
            Err(ChangeError::NotLatest)
        );
        assert_eq!(validate_revoke(&hist, "x", &[]), Err(ChangeError::NotFound));
        assert_eq!(validate_revoke(&hist, "b", &[2025]), Ok(()));
        assert_eq!(
            validate_revoke(&hist, "b", &[2026]),
            Err(ChangeError::ClosedYear(2026))
        );
        let after = hist.after_revoke("b");
        assert!(after.is_klein_at(d(2026, 6, 1)));
        assert_eq!(first_difference(&hist, &after), Some(d(2026, 1, 1)));
    }

    #[test]
    fn multiple_changes_klein_regel_klein_regel() {
        let h = TaxStatusHistory::new([
            initial(K),
            entry("b", d(2026, 1, 1), G, R::WegfallVorjahr, None),
            entry("c", d(2027, 1, 1), K, R::Rueckkehr, None),
            entry("d", d(2027, 10, 1), G, R::WegfallLaufend, None),
        ]);
        assert!(h.is_klein_at(d(2025, 12, 31)));
        assert!(!h.is_klein_at(d(2026, 6, 1)));
        assert!(h.is_klein_at(d(2027, 9, 30)));
        assert!(!h.is_klein_at(d(2027, 10, 1)));
        assert_eq!(h.year_status(2027), YearStatus::Wechsel);
        assert_eq!(h.binding_at(d(2027, 10, 1)), None);
    }

    fn posten(paid: NaiveDate, net: i64, doc: &str) -> UmsatzPosten {
        UmsatzPosten {
            paid_date: paid,
            net_cents: net,
            document: doc.into(),
            document_date: paid,
        }
    }

    #[test]
    fn gesamtumsatz_sums_net_of_the_year_incl_refunds() {
        let p = [
            posten(d(2025, 12, 31), 1_000_000, "RE-2025-0001"),
            posten(d(2026, 1, 2), 2_000_000, "RE-2026-0001"),
            posten(d(2026, 3, 2), -500_000, "ST-2026-0001"),
        ];
        assert_eq!(gesamtumsatz(&p, 2025), 1_000_000);
        assert_eq!(gesamtumsatz(&p, 2026), 1_500_000);
    }

    #[test]
    fn grenzen_vorjahr_laufend_gruendung() {
        let hist = TaxStatusHistory::default();
        // Vorjahr genau 25.000 € erreicht: Hinweis ohne Wegfall.
        let p = [posten(d(2025, 6, 1), 2_500_000, "RE-2025-0001")];
        let h = grenzpruefung(&hist, &p, d(2026, 2, 1), false);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].kind, Grenzart::Vorjahr);
        assert!(!h[0].exceeded);
        assert_eq!(h[0].suggested_effective_from, None);
        // Vorjahr überschritten → Vorschlag 01.01.
        let p = [posten(d(2025, 6, 1), 2_500_001, "RE-2025-0001")];
        let h = grenzpruefung(&hist, &p, d(2026, 2, 1), false);
        assert!(h[0].exceeded);
        assert_eq!(h[0].suggested_effective_from, Some(d(2026, 1, 1)));
        assert_eq!(h[0].suggested_reason, R::WegfallVorjahr);

        // Laufend über 100.000 €: auslösender Beleg; mit Vorjahresumsatz keine
        // Gründungsjahr-Vermutung.
        let p = [
            posten(d(2025, 6, 1), 1_000, "RE-2025-0001"),
            posten(d(2026, 3, 1), 6_000_000, "RE-2026-0001"),
            posten(d(2026, 8, 15), 4_000_001, "RE-2026-0002"),
        ];
        let h = grenzpruefung(&hist, &p, d(2026, 9, 1), false);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].kind, Grenzart::Laufend);
        assert_eq!(h[0].trigger_document.as_deref(), Some("RE-2026-0002"));
        assert_eq!(h[0].suggested_effective_from, Some(d(2026, 8, 15)));

        // Ohne früheren Umsatz: Gründungsjahr über 25.000 € als Vermutung.
        let p = [
            posten(d(2026, 3, 1), 2_000_000, "RE-2026-0001"),
            posten(d(2026, 4, 1), 600_000, "RE-2026-0002"),
        ];
        let h = grenzpruefung(&hist, &p, d(2026, 9, 1), false);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].kind, Grenzart::Gruendungsjahr);
        assert!(h[0].presumed && h[0].exceeded);
        assert_eq!(h[0].trigger_document.as_deref(), Some("RE-2026-0002"));

        // Bei Regelbesteuerung keine Hinweise.
        let regel = TaxStatusHistory::new([initial(G)]);
        assert!(grenzpruefung(&regel, &p, d(2026, 9, 1), false).is_empty());
    }

    /// V1-R03: Vorsteuer aktivierter Kosten, gezahlt im abgeschlossenen Jahr,
    /// Belegdatum im offenen Jahr.
    #[test]
    fn input_tax_of_capitalized_costs_in_closed_year_blocks_change_and_revoke() {
        let hist = TaxStatusHistory::new([initial(K)]);
        let cap = [CapitalizedInputTax {
            expense_date: d(2026, 1, 5),
            paid_year: 2025,
        }];
        let v = change(d(2026, 1, 1), G, R::Verzicht);
        assert!(validate_change(&hist, &v, d(2026, 3, 1), &[2025]).is_ok());
        assert_eq!(
            validate_change_with(&hist, &v, d(2026, 3, 1), &[2025], &cap).unwrap_err(),
            vec![ChangeError::ClosedYearInputTax(2025)]
        );
        // Zahljahr offen oder Belegdatum vor der Wirkung: keine Sperre.
        assert!(validate_change_with(&hist, &v, d(2026, 3, 1), &[], &cap).is_ok());
        assert!(validate_change_with(
            &hist,
            &change(d(2027, 1, 1), G, R::Verzicht),
            d(2026, 3, 1),
            &[2025],
            &cap
        )
        .is_ok());
        assert!(message(&ChangeError::ClosedYearInputTax(2025)).contains("2025"));

        // Widerruf derselben Wirkung.
        let with = TaxStatusHistory::new([
            initial(K),
            entry("v", d(2026, 1, 1), G, R::Verzicht, Some(d(2031, 1, 1))),
        ]);
        assert!(validate_revoke(&with, "v", &[2025]).is_ok());
        assert_eq!(
            validate_revoke_with(&with, "v", &[2025], &cap),
            Err(ChangeError::ClosedYearInputTax(2025))
        );
    }

    /// V1-R07: Erklärungsfrist des Verzichts und Vorjahresumsatz vor einer
    /// Rückkehr als Warnung.
    #[test]
    fn warnings_for_late_verzicht_and_return_above_threshold() {
        assert_eq!(verzicht_deadline(2026), d(2028, 2, 29));
        assert_eq!(verzicht_deadline(2027), d(2029, 2, 28));
        let v = change(d(2026, 1, 1), G, R::Verzicht);
        assert!(change_warnings(&v, d(2028, 2, 29), None).is_empty());
        assert_eq!(
            change_warnings(&v, d(2028, 3, 1), None),
            vec![ChangeWarning::VerzichtLate {
                year: 2026,
                deadline: d(2028, 2, 29)
            }]
        );
        let r = change(d(2027, 1, 1), K, R::Rueckkehr);
        assert!(change_warnings(&r, d(2026, 12, 1), Some(GRENZE_VORJAHR_CENTS)).is_empty());
        assert!(change_warnings(&r, d(2026, 12, 1), None).is_empty());
        let w = change_warnings(&r, d(2026, 12, 1), Some(10_000_000));
        assert_eq!(
            w,
            vec![ChangeWarning::RueckkehrTurnover {
                year: 2026,
                turnover_cents: 10_000_000
            }]
        );
        let text = warning_message(&w[0]);
        assert!(
            text.contains("100.000,00 €") && text.contains("01.01.2027"),
            "{text}"
        );
    }
}
