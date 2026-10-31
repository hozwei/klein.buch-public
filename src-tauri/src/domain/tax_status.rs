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
    /// Gründungsjahr über 25.000 € (§ 19 Abs. 1 UStG i. V. m. BMF-Schreiben
    /// vom 18.03.2025, Abschn. 19.1 UStAE): ab dem Umsatz, der die Grenze
    /// überschreitet, ohne Bindung.
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

/// Kleinstes Jahr eines Wirkungsdatums (Tippfehler-Schutz).
pub const MIN_YEAR: i32 = crate::domain::fiscal_year::MIN_YEAR;
/// Größtes Jahr eines Wirkungsdatums bzw. Bindungsendes: vierstellig wie der
/// CHECK `date(x) IS x` aus Migration 0033 (SEC-2026.11.1-01).
pub const MAX_YEAR: i32 = crate::domain::fiscal_year::MAX_YEAR;

/// Der 1. Januar des Jahres; außerhalb des Bereichs von `NaiveDate` auf dessen Grenze
/// geklemmt, statt in Panik zu geraten (SEC-2026.11.1-01). Die Commands lehnen
/// solche Jahre vorher ab ([`crate::domain::fiscal_year::checked_year`]).
fn jan_first(year: i32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, 1, 1).unwrap_or(if year < 0 {
        NaiveDate::MIN
    } else {
        NaiveDate::MAX
    })
}

/// 31. Dezember, geklemmt wie [`jan_first`].
fn dec_31(year: i32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, 12, 31).unwrap_or(if year < 0 {
        NaiveDate::MIN
    } else {
        NaiveDate::MAX
    })
}

/// Ende der Bindung nach einem Verzicht (ADR 0042 Nr. 2): fünf Kalenderjahre
/// einschließlich des Wirkungsjahres; zurückgegeben wird der erste Tag, an dem
/// die Rückkehr möglich ist. Verzicht ab 2026 → Rückkehr frühestens 01.01.2031.
pub fn verzicht_binding_until(effective_from: NaiveDate) -> NaiveDate {
    jan_first(effective_from.year().saturating_add(5))
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

    /// Frühestes Datum einer Rückkehr zu §19, wenn heute Regelbesteuerung gilt
    /// (ADR 0042 Nr. 2, DOC-02):
    ///
    /// - nach einem Verzicht (und einem übernommenen Wechsel) das Bindungsende;
    /// - nach einem Wegfall im laufenden Jahr bzw. im Gründungsjahr Y erst der
    ///   01.01.(Y+2): Wer ihn einträgt, hatte in Y einen Gesamtumsatz über
    ///   25.000 €, also gilt nach § 19 Abs. 1 Satz 1 UStG auch Y+1 die
    ///   Regelbesteuerung (Vorjahresumsatz über 25.000 €);
    /// - nach einem Wegfall wegen des Vorjahres Y der 01.01.(Y+1), sofern der
    ///   Gesamtumsatz von Y höchstens 25.000 € beträgt (das prüft Klein.Buch nur
    ///   als Warnung, [`change_warnings`]);
    /// - sonst der nächste 1. Januar nach dem letzten Eintrag bzw. nach heute.
    pub fn earliest_return(&self, today: NaiveDate) -> Option<NaiveDate> {
        let last = self.last()?;
        if last.status.is_klein() {
            return None;
        }
        let years_after = match last.reason {
            ChangeReason::WegfallLaufend | ChangeReason::WegfallGruendungsjahr => 2,
            _ => 1,
        };
        let after_last = jan_first(last.effective_from.year().saturating_add(years_after));
        let this_year = jan_first(today.year());
        let base = after_last.max(this_year);
        Some(match last.binding_until {
            Some(until) => until.max(base),
            None => base,
        })
    }

    /// Jüngster gültiger Wegfall im laufenden Jahr bzw. Gründungsjahr vor
    /// `date` (für die Prüfung der Rückkehr, DOC-02, V8a-R-04).
    fn wegfall_in_year_before(&self, date: NaiveDate) -> Option<&TaxStatusEntry> {
        self.active.iter().rev().find(|e| {
            e.effective_from < date
                && matches!(
                    e.reason,
                    ChangeReason::WegfallLaufend | ChangeReason::WegfallGruendungsjahr
                )
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
    /// Wirkungsdatum vor 1900 oder nach 9999, beim Verzicht auch ein
    /// Bindungsende nach 9999 (Tippfehler; SEC-2026.11.1-01).
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
    /// Rückkehr vor dem 01.01.(Y+2) nach einem eigenen, gültigen Wegfall im
    /// laufenden Jahr bzw. Gründungsjahr Y (DOC-02, V8a-R-04): Der Eintrag
    /// besagt einen Gesamtumsatz über 25.000 € in Y, also gilt Y+1 die
    /// Regelbesteuerung (§ 19 Abs. 1 Satz 1 UStG). Kein unsicherer Umsatz,
    /// darum Ablehnung statt Warnung; war der Wegfall falsch, wird er widerrufen.
    ReturnAfterWegfall {
        wegfall: ChangeReason,
        year: i32,
        earliest: NaiveDate,
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
        ChangeError::DateOutOfRange(d) => format!(
            "Das Wirkungsdatum {} ist ungültig: Klein.Buch nimmt Jahre von {MIN_YEAR} bis \
             {MAX_YEAR} an (beim Verzicht einschließlich der fünf Jahre Bindung).",
            de(*d)
        ),
        ChangeError::NotJanFirst { reason, date } => format!(
            "„{}“ wirkt nur zum 1. Januar, nicht zum {} (§ 19 Abs. 1 und 3 UStG).",
            reason.label(),
            de(*date)
        ),
        ChangeError::InFuture { reason, date } if *reason == ChangeReason::WegfallVorjahr => {
            format!(
                "„{}“ lässt sich höchstens zum 1. Januar des nächsten Jahres im Voraus eintragen, \
                 nicht zum {}.",
                reason.label(),
                de(*date)
            )
        }
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
        ChangeError::ReturnAfterWegfall {
            wegfall,
            year,
            earliest,
        } => format!(
            "Für {year} ist „{}“ eingetragen; dein Gesamtumsatz {year} lag damit über 25.000 €. \
             Für {} gilt deshalb die Regelbesteuerung, die Rückkehr zu §19 ist frühestens zum {} \
             möglich (§ 19 Abs. 1 Satz 1 UStG). Widerrufe zuerst den Wegfall-Eintrag, wenn er \
             falsch war.",
            wegfall.label(),
            year + 1,
            de(*earliest)
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
        // SEC-2026.11.1-01: vierstellige Jahre wie der CHECK aus 0033; beim
        // Verzicht auch das Bindungsende (Wirkungsjahr + 5).
        let last_year = if reason == ChangeReason::Verzicht {
            date.year().saturating_add(5)
        } else {
            date.year()
        };
        if date.year() < MIN_YEAR || last_year > MAX_YEAR {
            errs.push(E::DateOutOfRange(date));
        }
        if reason.requires_jan_first() && (date.month(), date.day()) != (1, 1) {
            errs.push(E::NotJanFirst { reason, date });
        }
        let future = match reason {
            ChangeReason::WegfallLaufend | ChangeReason::WegfallGruendungsjahr => date > today,
            // L-GOBD-02: Der Wegfall wegen des Vorjahres lässt sich schon im
            // laufenden Jahr zum 01.01. des Folgejahres eintragen, sobald der
            // Gesamtumsatz 25.000 € überschreitet (§ 19 Abs. 1 Satz 1 UStG).
            // Sonst bekämen Abo-Rechnungen und Vorausrechnungen, deren
            // maßgebliches Datum im Folgejahr liegt (ADR 0042 Nr. 3, 11, 22),
            // zwingend §19. Ob der Umsatz die Grenze überschreitet, meldet
            // [`change_warnings`] nur als Warnung (ADR 0042 Nr. 18).
            ChangeReason::WegfallVorjahr => date.year() > today.year().saturating_add(1),
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
            // V8a-R-04: nach eigenem Wegfall im Jahr Y frühestens 01.01.(Y+2).
            if let Some(w) = hist.wegfall_in_year_before(date) {
                let year = w.effective_from.year();
                let earliest = jan_first(year.saturating_add(2));
                if date < earliest {
                    errs.push(E::ReturnAfterWegfall {
                        wegfall: w.reason,
                        year,
                        earliest,
                    });
                }
            }
        }
    }
    if !errs.is_empty() {
        return Err(errs);
    }
    // Bindung erst nach der Prüfung des Datums (SEC-2026.11.1-01).
    let binding_until = (reason == ChangeReason::Verzicht).then(|| verzicht_binding_until(date));
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
/// Nr. 4). Verzicht für 2026 → bis 29.02.2028 (Schaltjahr). Außerhalb des
/// Datumsbereichs geklemmt wie [`jan_first`] (SEC-2026.11.1-01).
pub fn verzicht_deadline(year: i32) -> NaiveDate {
    jan_first(year.saturating_add(2))
        .with_month(3)
        .and_then(|d| d.pred_opt())
        .unwrap_or_else(|| dec_31(year.saturating_add(2)))
}

/// Hinweis zu einem geplanten Eintrag, der ihn nicht sperrt (ADR 0042 Nr. 4,
/// 5 und 18). Klein.Buch kennt die Erklärung beim Finanzamt und den
/// vollständigen Gesamtumsatz nicht sicher; darum Warnung statt Ablehnung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeWarning {
    /// Verzicht für ein Jahr, dessen Erklärungsfrist abgelaufen ist.
    VerzichtLate { year: i32, deadline: NaiveDate },
    /// Rückkehr, obwohl der Gesamtumsatz des Vorjahres nach den in Klein.Buch
    /// erfassten Zahlungen über 25.000 € lag.
    RueckkehrTurnover { year: i32, turnover_cents: i64 },
    /// Rückkehr im Voraus: Das Vorjahr der Rückkehr läuft noch, sein
    /// Gesamtumsatz steht nicht fest (ADR 0042 Nr. 18).
    RueckkehrYearRunning { year: i32 },
    /// Wegfall wegen des Vorjahres, obwohl Klein.Buch für das Vorjahr keinen
    /// Gesamtumsatz über 25.000 € kennt (L-GOBD-02); `running` = das Jahr läuft
    /// noch (Eintrag im Voraus).
    WegfallVorjahrTurnover {
        year: i32,
        turnover_cents: i64,
        running: bool,
    },
}

/// Warnungen zu einem geplanten Eintrag. `turnover_before`: Gesamtumsatz des
/// Jahres vor dem Wirkungsjahr nach den erfassten Zahlungen (für Rückkehr und
/// Wegfall wegen des Vorjahres; `None` = nicht ermittelt). `_hist` wirkt derzeit
/// nicht: Was aus der Historie folgt, lehnt [`validate_change_with`] ab.
pub fn change_warnings(
    _hist: &TaxStatusHistory,
    new: &NewChange,
    today: NaiveDate,
    turnover_before: Option<i64>,
) -> Vec<ChangeWarning> {
    let year = new.effective_from.year();
    let prev = year.saturating_sub(1);
    let running = today.year() <= prev;
    let mut out = Vec::new();
    match new.reason {
        ChangeReason::Verzicht => {
            let deadline = verzicht_deadline(year);
            if today > deadline {
                out.push(ChangeWarning::VerzichtLate { year, deadline });
            }
        }
        ChangeReason::Rueckkehr => {
            // Eine Rückkehr nach eigenem Wegfall im Vorjahr lehnt schon
            // [`validate_change_with`] ab (V8a-R-04).
            if let Some(t) = turnover_before.filter(|t| *t > GRENZE_VORJAHR_CENTS) {
                out.push(ChangeWarning::RueckkehrTurnover {
                    year: prev,
                    turnover_cents: t,
                });
            } else if running {
                out.push(ChangeWarning::RueckkehrYearRunning { year: prev });
            }
        }
        ChangeReason::WegfallVorjahr => {
            if let Some(t) = turnover_before.filter(|t| *t <= GRENZE_VORJAHR_CENTS) {
                out.push(ChangeWarning::WegfallVorjahrTurnover {
                    year: prev,
                    turnover_cents: t,
                    running,
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
        ChangeWarning::RueckkehrYearRunning { year } => format!(
            "Das Jahr {year} läuft noch. Die Rückkehr zu §19 zum 01.01.{} setzt voraus, dass dein \
             Gesamtumsatz {year} am Jahresende höchstens 25.000 € beträgt (§ 19 Abs. 1 Satz 1 \
             UStG); liegt er darüber, widerrufe den Eintrag.",
            year + 1
        ),
        ChangeWarning::WegfallVorjahrTurnover {
            year,
            turnover_cents,
            running,
        } => {
            let (verb, then) = if *running {
                (
                    "liegt bisher",
                    " Das Jahr läuft noch; bleibt dein Gesamtumsatz am Jahresende bei höchstens \
                     25.000 €, widerrufe den Eintrag.",
                )
            } else {
                ("lag", "")
            };
            format!(
                "Nach den in Klein.Buch erfassten Zahlungen {verb} dein Gesamtumsatz {year} bei {} \
                 und damit nicht über 25.000 €. Trage den Wegfall zum 01.01.{} nur ein, wenn dein \
                 Gesamtumsatz einschließlich der Umsätze außerhalb von Klein.Buch über 25.000 € \
                 liegt (§ 19 Abs. 1 Satz 1 UStG).{then}",
                crate::euer::detail::eur_grouped(*turnover_cents),
                year + 1
            )
        }
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
/// Grenze im Gründungsjahr (§ 19 Abs. 1 UStG i. V. m. BMF-Schreiben vom
/// 18.03.2025, Abschn. 19.1 UStAE; § 19 Abs. 2 definiert nur den
/// Gesamtumsatz): 25.000 €.
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
    /// Gesamtumsatz des Vorjahres über 25.000 € → Wegfall ab 1. Januar.
    Vorjahr,
    Laufend,
    Gruendungsjahr,
    /// Gesamtumsatz des laufenden Jahres schon über 25.000 € → Wegfall wegen
    /// des Vorjahres ab 1. Januar des Folgejahres (L-GOBD-02), damit
    /// Abo-Vorlagen und Vorausrechnungen rechtzeitig umgestellt werden.
    Folgejahr,
}

impl Grenzart {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Vorjahr => "vorjahr",
            Self::Laufend => "laufend",
            Self::Gruendungsjahr => "gruendungsjahr",
            Self::Folgejahr => "folgejahr",
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
    /// Beleg, mit dem die Grenze überschritten wurde (laufendes Jahr,
    /// Gründungsjahr, Folgejahr).
    pub trigger_document: Option<String>,
    /// Jahr der Leistung (Leistungs-, sonst Belegdatum) des auslösenden Belegs
    /// (V8a-R-02): Liegt es vor `year`, bleibt dieser Umsatz unter dem Status
    /// seines Jahres (ADR 0042 Nr. 3); steuerpflichtig sind erst die Umsätze ab
    /// der Zahlung.
    pub trigger_service_year: Option<i32>,
    /// Vorschlag für das Wirkungsdatum.
    pub suggested_effective_from: Option<NaiveDate>,
    /// Vorgeschlagener Grund.
    pub suggested_reason: ChangeReason,
    /// `true` = nur vermutet (Gründungsjahr: in Klein.Buch kein früherer Umsatz).
    pub presumed: bool,
}

/// Erster Posten des Jahres, mit dem die kumulierte Summe `limit` übersteigt.
/// Maßgeblich ist der Zufluss (Gesamtumsatz nach vereinnahmten Entgelten,
/// § 19 Abs. 2 UStG), also die Reihenfolge der Zahldaten.
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

/// Vorschlag für das Wirkungsdatum eines Wegfalls im laufenden Jahr bzw. im
/// Gründungsjahr `year` (ADR 0042 Nr. 2, L-GOBD-01): der Tag des Umsatzes, mit
/// dem die Grenze überschritten wird (§ 19 Abs. 1 Satz 1 UStG n. F., BMF-Schreiben
/// vom 18.03.2025). Welcher Umsatz das ist, bestimmt der Zufluss ([`trigger`]);
/// der Vorschlag ist der früheste vertretbare Tag, an dem dieser Umsatz schon
/// steuerpflichtig ist:
///
/// - Leistungsdatum im geprüften Jahr und nicht nach der Zahlung: das
///   Leistungsdatum (sein Status richtet sich danach, ADR 0042 Nr. 3);
/// - Leistung im Vorjahr: die Zahlung. Eine Leistung des Vorjahres bleibt
///   unter dem Status ihres Jahres; die Umsätze des Jahres vor der
///   Überschreitung bleiben steuerfrei, darum nicht der 1. Januar;
/// - Leistung nach der Zahlung (Anzahlung, Vorausrechnung, ADR 0042 Nr. 11):
///   die Zahlung. Mit ihr ist die Grenze überschritten; die spätere Leistung
///   ist dann schon steuerpflichtig.
///
/// Begrenzt auf das Jahr und auf `today`.
fn wegfall_suggestion(p: &UmsatzPosten, year: i32, today: NaiveDate) -> NaiveDate {
    let day = if p.document_date.year() == year {
        p.document_date.min(p.paid_date)
    } else {
        p.paid_date
    };
    day.min(today).max(jan_first(year))
}

/// `true`, wenn Klein.Buch für `year` ein Gründungsjahr vermutet: keine
/// Zahlung vor `year` in `posten` und `earlier_turnover` (Umsätze vor dem
/// Vorjahr) nicht gesetzt. Grundlage des Hinweises „Gründungsjahr“ und der
/// Grenzangabe in der Übersicht (UI-08, V8a-R-06).
pub fn gruendungsjahr_vermutet(posten: &[UmsatzPosten], year: i32, earlier_turnover: bool) -> bool {
    !(earlier_turnover || posten.iter().any(|p| p.paid_date.year() < year))
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
    let presumed_founding = gruendungsjahr_vermutet(posten, year, earlier_turnover);

    if hist.is_klein_at(jan_first(year)) && prev >= GRENZE_VORJAHR_CENTS {
        let exceeded = prev > GRENZE_VORJAHR_CENTS;
        out.push(Grenzhinweis {
            kind: Grenzart::Vorjahr,
            year: year - 1,
            turnover_cents: prev,
            limit_cents: GRENZE_VORJAHR_CENTS,
            exceeded,
            trigger_document: None,
            trigger_service_year: None,
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
        // Status am vorgeschlagenen Wirkungsdatum (L-GOBD-01: derselbe Tag wie
        // der Vorschlag); ohne Überschreiten heute.
        let at = t.map_or(today, |p| wegfall_suggestion(p, year, today));
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
            trigger_service_year: t.map(|p| p.document_date.year()),
            suggested_effective_from: t.map(|_| at),
            suggested_reason: if kind == Grenzart::Gruendungsjahr {
                ChangeReason::WegfallGruendungsjahr
            } else {
                ChangeReason::WegfallLaufend
            },
            presumed,
        })
    };
    if presumed_founding {
        out.extend(laufend(
            Grenzart::Gruendungsjahr,
            GRENZE_GRUENDUNG_CENTS,
            true,
        ));
    }
    out.extend(laufend(Grenzart::Laufend, GRENZE_LAUFEND_CENTS, false));

    // L-GOBD-02: Gesamtumsatz des laufenden Jahres schon über 25.000 € → ab
    // 01.01. des Folgejahres keine §19 (§ 19 Abs. 1 Satz 1 UStG). Der Hinweis
    // kommt im laufenden Jahr, damit Abo-Rechnungen und Vorausrechnungen mit
    // Leistung im Folgejahr (ADR 0042 Nr. 3, 11, 22) den richtigen Status
    // bekommen. Entfällt, wenn schon ein Wegfall über 100.000 € im laufenden
    // Jahr oder wegen des Vorjahres zum 01.01. des laufenden Jahres ansteht: Er
    // wirkt über den Jahreswechsel fort, und ein Eintrag zum 01.01. des
    // Folgejahres sperrte den dringenderen zum 01.01. des laufenden Jahres
    // (`NotAfterLast`, V8a-R-03).
    let next_jan = jan_first(year.saturating_add(1));
    let wegfall_pending = out
        .iter()
        .any(|h| matches!(h.kind, Grenzart::Laufend | Grenzart::Vorjahr) && h.exceeded);
    if current > GRENZE_VORJAHR_CENTS && hist.is_klein_at(next_jan) && !wegfall_pending {
        let t = trigger(posten, year, GRENZE_VORJAHR_CENTS);
        out.push(Grenzhinweis {
            kind: Grenzart::Folgejahr,
            year,
            turnover_cents: current,
            limit_cents: GRENZE_VORJAHR_CENTS,
            exceeded: true,
            trigger_document: t.map(|p| p.document.clone()),
            trigger_service_year: t.map(|p| p.document_date.year()),
            suggested_effective_from: Some(next_jan),
            suggested_reason: ChangeReason::WegfallVorjahr,
            presumed: false,
        });
    }
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
        // L-GOBD-02: Wegfall wegen des Vorjahres höchstens zum 01.01. des
        // Folgejahres im Voraus, nicht weiter.
        assert!(validate_change(
            &hist,
            &change(d(2027, 1, 1), G, R::WegfallVorjahr),
            today,
            &[],
        )
        .is_ok());
        let err = validate_change(
            &hist,
            &change(d(2028, 1, 1), G, R::WegfallVorjahr),
            today,
            &[],
        )
        .unwrap_err();
        assert_eq!(
            err,
            vec![ChangeError::InFuture {
                reason: R::WegfallVorjahr,
                date: d(2028, 1, 1)
            }]
        );
        assert!(message(&err[0]).contains("im Voraus"));
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
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].kind, Grenzart::Gruendungsjahr);
        assert!(h[0].presumed && h[0].exceeded);
        assert_eq!(h[0].trigger_document.as_deref(), Some("RE-2026-0002"));
        // Dazu der Wegfall wegen des Vorjahres für 2027 (L-GOBD-02).
        assert_eq!(h[1].kind, Grenzart::Folgejahr);
        assert_eq!(h[1].suggested_effective_from, Some(d(2027, 1, 1)));

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
        let none = TaxStatusHistory::new([initial(K)]);
        let v = change(d(2026, 1, 1), G, R::Verzicht);
        assert!(change_warnings(&none, &v, d(2028, 2, 29), None).is_empty());
        assert_eq!(
            change_warnings(&none, &v, d(2028, 3, 1), None),
            vec![ChangeWarning::VerzichtLate {
                year: 2026,
                deadline: d(2028, 2, 29)
            }]
        );
        let regel = TaxStatusHistory::new([
            initial(K),
            entry("b", d(2026, 1, 1), G, R::WegfallVorjahr, None),
        ]);
        let r = change(d(2027, 1, 1), K, R::Rueckkehr);
        // Nach Ablauf des Vorjahres und mit Umsatz bis 25.000 €: keine Warnung.
        assert!(change_warnings(&regel, &r, d(2027, 1, 5), Some(GRENZE_VORJAHR_CENTS)).is_empty());
        assert!(change_warnings(&regel, &r, d(2027, 1, 5), None).is_empty());
        let w = change_warnings(&regel, &r, d(2026, 12, 1), Some(10_000_000));
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

    fn posten_mit(paid: NaiveDate, leistung: NaiveDate, net: i64, doc: &str) -> UmsatzPosten {
        UmsatzPosten {
            paid_date: paid,
            net_cents: net,
            document: doc.into(),
            document_date: leistung,
        }
    }

    /// L-GOBD-01 (a): Der auslösende Zufluss gehört zu einer Leistung des
    /// Vorjahres. Vorschlag ist der Tag der Zahlung, nicht der 01.01.: Die
    /// Umsätze davor bleiben steuerfrei (§ 19 Abs. 1 Satz 1 UStG n. F.).
    #[test]
    fn wegfall_suggestion_with_service_in_previous_year_is_the_payment() {
        let hist = TaxStatusHistory::new([initial(K)]);
        let p = [
            posten(d(2025, 6, 1), 1_000, "RE-2025-0001"),
            posten(d(2026, 3, 1), 9_900_000, "RE-2026-0001"),
            posten_mit(d(2026, 11, 5), d(2025, 12, 10), 200_000, "RE-2025-0099"),
        ];
        let h = grenzpruefung(&hist, &p, d(2026, 11, 20), false);
        let l = h.iter().find(|x| x.kind == Grenzart::Laufend).unwrap();
        assert_eq!(l.trigger_document.as_deref(), Some("RE-2025-0099"));
        assert_eq!(l.suggested_effective_from, Some(d(2026, 11, 5)));
        // V8a-R-02: Leistung 2025, der Hinweistext unterscheidet danach.
        assert_eq!(l.trigger_service_year, Some(2025));
        // UI-08: Umsatz 2025 bekannt, kein vermutetes Gründungsjahr.
        assert!(!gruendungsjahr_vermutet(&p, 2026, false));
        assert!(gruendungsjahr_vermutet(&p[1..], 2026, false));
        assert!(!gruendungsjahr_vermutet(&p[1..], 2026, true));
        // Der Vorschlag besteht die Prüfung des Eintrags.
        assert!(validate_change(
            &hist,
            &change(d(2026, 11, 5), G, R::WegfallLaufend),
            d(2026, 11, 20),
            &[]
        )
        .is_ok());
    }

    /// L-GOBD-01: Leistung im Jahr vor der Zahlung → Leistungsdatum (der
    /// auslösende Umsatz ist damit schon steuerpflichtig).
    #[test]
    fn wegfall_suggestion_with_service_before_payment_is_the_service_date() {
        let hist = TaxStatusHistory::new([initial(K)]);
        let p = [
            posten(d(2025, 6, 1), 1_000, "RE-2025-0001"),
            posten(d(2026, 3, 1), 9_900_000, "RE-2026-0001"),
            posten_mit(d(2026, 11, 5), d(2026, 9, 20), 200_000, "RE-2026-0050"),
        ];
        let h = grenzpruefung(&hist, &p, d(2026, 11, 20), false);
        assert_eq!(h[0].kind, Grenzart::Laufend);
        assert_eq!(h[0].suggested_effective_from, Some(d(2026, 9, 20)));
    }

    /// L-GOBD-01 (b): Anzahlung bzw. Vorausrechnung, Leistung nach der Zahlung
    /// (im selben Jahr oder im Folgejahr) → Tag der Zahlung; ein schon
    /// eingetragener Wechsel im Folgejahr unterdrückt den Hinweis nicht.
    #[test]
    fn wegfall_suggestion_with_service_after_payment_is_the_payment() {
        let p = [
            posten(d(2026, 2, 1), 2_400_000, "RE-2026-0001"),
            posten_mit(d(2026, 11, 5), d(2027, 2, 15), 200_000, "RE-2026-0120"),
        ];
        let hist = TaxStatusHistory::new([initial(K)]);
        let h = grenzpruefung(&hist, &p, d(2026, 11, 20), false);
        assert_eq!(h[0].kind, Grenzart::Gruendungsjahr);
        assert_eq!(h[0].trigger_document.as_deref(), Some("RE-2026-0120"));
        assert_eq!(h[0].suggested_effective_from, Some(d(2026, 11, 5)));
        assert!(validate_change(
            &hist,
            &change(d(2026, 11, 5), G, R::WegfallGruendungsjahr),
            d(2026, 11, 20),
            &[]
        )
        .is_ok());
        // Wegfall wegen des Vorjahres zum 01.01.2027 schon eingetragen: Für
        // 2026 gilt am Vorschlagstag noch §19, der Hinweis bleibt.
        let mit_2027 = TaxStatusHistory::new([
            initial(K),
            entry("b", d(2027, 1, 1), G, R::WegfallVorjahr, None),
        ]);
        let h = grenzpruefung(&mit_2027, &p, d(2026, 11, 20), false);
        assert_eq!(h.len(), 1, "{h:?}");
        assert_eq!(h[0].kind, Grenzart::Gruendungsjahr);
        // Leistung später im selben Jahr: ebenfalls die Zahlung.
        let p = [
            posten(d(2026, 2, 1), 2_400_000, "RE-2026-0001"),
            posten_mit(d(2026, 6, 5), d(2026, 9, 1), 200_000, "RE-2026-0002"),
        ];
        let h = grenzpruefung(&hist, &p, d(2026, 11, 20), false);
        assert_eq!(h[0].suggested_effective_from, Some(d(2026, 6, 5)));
    }

    /// L-GOBD-02: Gesamtumsatz des laufenden Jahres über 25.000 € → Hinweis
    /// auf den Wegfall zum 01.01. des Folgejahres; nicht bei genau 25.000 €,
    /// nicht bei schon eingetragenem Wegfall und nicht neben einem Wegfall über
    /// 100.000 €.
    #[test]
    fn threshold_for_next_year_in_running_year() {
        let hist = TaxStatusHistory::new([initial(K)]);
        let p = [
            posten(d(2025, 6, 1), 1_000_000, "RE-2025-0001"),
            posten(d(2026, 3, 1), 2_000_000, "RE-2026-0001"),
            posten(d(2026, 10, 1), 1_000_000, "RE-2026-0002"),
        ];
        let h = grenzpruefung(&hist, &p, d(2026, 10, 20), false);
        assert_eq!(h.len(), 1, "{h:?}");
        assert_eq!(h[0].kind, Grenzart::Folgejahr);
        assert_eq!(h[0].year, 2026);
        assert_eq!(h[0].turnover_cents, 3_000_000);
        assert_eq!(h[0].suggested_reason, R::WegfallVorjahr);
        assert_eq!(h[0].suggested_effective_from, Some(d(2027, 1, 1)));
        assert_eq!(h[0].trigger_document.as_deref(), Some("RE-2026-0002"));
        assert_eq!(Grenzart::Folgejahr.as_str(), "folgejahr");

        let genau = [
            posten(d(2025, 6, 1), 1_000_000, "RE-2025-0001"),
            posten(d(2026, 3, 1), 2_500_000, "RE-2026-0001"),
        ];
        assert!(grenzpruefung(&hist, &genau, d(2026, 10, 20), false).is_empty());

        let eingetragen = TaxStatusHistory::new([
            initial(K),
            entry("b", d(2027, 1, 1), G, R::WegfallVorjahr, None),
        ]);
        assert!(grenzpruefung(&eingetragen, &p, d(2026, 10, 20), false).is_empty());

        let gross = [
            posten(d(2025, 6, 1), 1_000_000, "RE-2025-0001"),
            posten(d(2026, 3, 1), 10_000_001, "RE-2026-0001"),
        ];
        let h = grenzpruefung(&hist, &gross, d(2026, 10, 20), false);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].kind, Grenzart::Laufend);
    }

    /// L-GOBD-02: Wegfall wegen des Vorjahres im Voraus mit Warnung, wenn
    /// Klein.Buch keinen Umsatz über 25.000 € kennt; ohne Warnung darüber.
    #[test]
    fn wegfall_vorjahr_in_advance_warns_without_known_turnover() {
        let hist = TaxStatusHistory::new([initial(K)]);
        let w = change(d(2027, 1, 1), G, R::WegfallVorjahr);
        let today = d(2026, 12, 10);
        assert!(validate_change(&hist, &w, today, &[]).is_ok());
        assert!(change_warnings(&hist, &w, today, Some(3_000_000)).is_empty());
        let warn = change_warnings(&hist, &w, today, Some(1_200_000));
        assert_eq!(
            warn,
            vec![ChangeWarning::WegfallVorjahrTurnover {
                year: 2026,
                turnover_cents: 1_200_000,
                running: true
            }]
        );
        let text = warning_message(&warn[0]);
        assert!(
            text.contains("liegt bisher")
                && text.contains("01.01.2027")
                && text.contains("widerrufe"),
            "{text}"
        );
        // Im betroffenen Jahr eingetragen: Vorjahr abgeschlossen.
        let w = change(d(2026, 1, 1), G, R::WegfallVorjahr);
        let warn = change_warnings(&hist, &w, d(2026, 2, 1), Some(2_500_000));
        assert_eq!(
            warn,
            vec![ChangeWarning::WegfallVorjahrTurnover {
                year: 2025,
                turnover_cents: 2_500_000,
                running: false
            }]
        );
        assert!(!warning_message(&warn[0]).contains("bisher"));
    }

    /// DOC-02: Nach einem Wegfall im laufenden Jahr bzw. im Gründungsjahr Y ist
    /// die Rückkehr frühestens zum 01.01.(Y+2) möglich; eine Rückkehr zum
    /// 01.01.(Y+1) lehnt Klein.Buch unabhängig vom erfassten Umsatz ab
    /// (V8a-R-04: der eigene Eintrag besagt einen Gesamtumsatz Y über 25.000 €).
    #[test]
    fn return_after_wegfall_in_year_is_two_years_later() {
        for reason in [R::WegfallLaufend, R::WegfallGruendungsjahr] {
            let h =
                TaxStatusHistory::new([initial(K), entry("b", d(2026, 9, 10), G, reason, None)]);
            assert_eq!(h.earliest_return(d(2026, 10, 1)), Some(d(2028, 1, 1)));
            assert_eq!(h.earliest_return(d(2029, 3, 1)), Some(d(2029, 1, 1)));
            let r = change(d(2027, 1, 1), K, R::Rueckkehr);
            let err = validate_change(&h, &r, d(2026, 12, 1), &[]).unwrap_err();
            assert_eq!(
                err,
                vec![ChangeError::ReturnAfterWegfall {
                    wegfall: reason,
                    year: 2026,
                    earliest: d(2028, 1, 1)
                }]
            );
            let text = message(&err[0]);
            assert!(
                text.contains("01.01.2028") && text.contains("2027") && text.contains("Widerrufe"),
                "{text}"
            );
            // Auch ohne erfassten Umsatz keine bloße Warnung mehr.
            assert!(change_warnings(&h, &r, d(2026, 12, 1), Some(2_000_000))
                .iter()
                .all(|w| !matches!(w, ChangeWarning::RueckkehrTurnover { .. })));
            // Rückkehr zum 01.01.2028: zulässig, keine Warnung aus dem Wegfall.
            let r = change(d(2028, 1, 1), K, R::Rueckkehr);
            assert!(validate_change(&h, &r, d(2028, 1, 2), &[]).is_ok());
            assert!(change_warnings(&h, &r, d(2028, 1, 2), Some(2_000_000)).is_empty());
            // Nach dem Widerruf des Wegfalls ist der Weg frei.
            let h2 = h.without("b");
            let r = change(d(2027, 1, 1), K, R::Rueckkehr);
            assert_eq!(
                validate_change(&h2, &r, d(2026, 12, 1), &[]).unwrap_err(),
                vec![ChangeError::NoChange {
                    status: K,
                    date: d(2027, 1, 1)
                }]
            );
        }
    }

    /// V8a-R-03: Liegen die Zahlungseingänge des Vorjahres über 25.000 € und
    /// ist noch kein Wegfall eingetragen, bietet nur der Hinweis „Vorjahr“ den
    /// Wegfall an (zum 01.01. des laufenden Jahres, wirkt fort); der Hinweis
    /// „Folgejahr“ entfällt, sonst sperrte ein Eintrag zum 01.01. des
    /// Folgejahres den dringenderen.
    #[test]
    fn folgejahr_hint_is_suppressed_while_vorjahr_exceeded() {
        let hist = TaxStatusHistory::new([initial(K)]);
        let ps = vec![
            posten(d(2025, 5, 1), 3_000_000, "RE-2025-0001"),
            posten(d(2026, 4, 1), 3_000_000, "RE-2026-0001"),
        ];
        let h = grenzpruefung(&hist, &ps, d(2026, 6, 1), true);
        let kinds: Vec<Grenzart> = h.iter().map(|x| x.kind).collect();
        assert_eq!(kinds, vec![Grenzart::Vorjahr]);
        assert!(h[0].exceeded);
        assert_eq!(h[0].suggested_effective_from, Some(d(2026, 1, 1)));
        // Vorjahr nur genau erreicht: kein Wegfall offen, Folgejahr bleibt.
        let ps = vec![
            posten(d(2025, 5, 1), 2_500_000, "RE-2025-0001"),
            posten(d(2026, 4, 1), 3_000_000, "RE-2026-0001"),
        ];
        let h = grenzpruefung(&hist, &ps, d(2026, 6, 1), true);
        let kinds: Vec<Grenzart> = h.iter().map(|x| x.kind).collect();
        assert_eq!(kinds, vec![Grenzart::Vorjahr, Grenzart::Folgejahr]);
        assert_eq!(h[1].trigger_service_year, Some(2026));
        // Nach dem Eintrag zum 01.01.2026 entfallen beide Hinweise.
        let hist = TaxStatusHistory::new([
            initial(K),
            entry("b", d(2026, 1, 1), G, R::WegfallVorjahr, None),
        ]);
        let ps = vec![
            posten(d(2025, 5, 1), 3_000_000, "RE-2025-0001"),
            posten(d(2026, 4, 1), 3_000_000, "RE-2026-0001"),
        ];
        assert!(grenzpruefung(&hist, &ps, d(2026, 6, 1), true).is_empty());
    }

    /// DOC-02: Nach einem Wegfall wegen des Vorjahres Y bleibt die Rückkehr zum
    /// 01.01.(Y+1) möglich, nur wenn der Gesamtumsatz Y höchstens 25.000 €
    /// beträgt: Warnung, solange das Jahr läuft oder der erfasste Umsatz
    /// darüber liegt.
    #[test]
    fn return_after_wegfall_vorjahr_warns_while_year_runs() {
        let h = TaxStatusHistory::new([
            initial(K),
            entry("b", d(2026, 1, 1), G, R::WegfallVorjahr, None),
        ]);
        assert_eq!(h.earliest_return(d(2026, 6, 1)), Some(d(2027, 1, 1)));
        let r = change(d(2027, 1, 1), K, R::Rueckkehr);
        let w = change_warnings(&h, &r, d(2026, 6, 1), Some(1_000_000));
        assert_eq!(w, vec![ChangeWarning::RueckkehrYearRunning { year: 2026 }]);
        assert!(warning_message(&w[0]).contains("läuft noch"));
        let w = change_warnings(&h, &r, d(2027, 1, 3), Some(3_000_000));
        assert!(matches!(
            w[0],
            ChangeWarning::RueckkehrTurnover { year: 2026, .. }
        ));
        assert!(change_warnings(&h, &r, d(2027, 1, 3), Some(1_000_000)).is_empty());
    }

    /// SEC-2026.11.1-01: Jahre außerhalb 1900–9999 lehnt die Prüfung ab, ohne
    /// Panik; beim Verzicht zählt auch das Bindungsende.
    #[test]
    fn out_of_range_years_are_rejected_without_panic() {
        let hist = TaxStatusHistory::new([initial(K)]);
        let today = d(2026, 9, 1);
        let far = NaiveDate::from_ymd_opt(262_140, 1, 1).unwrap();
        for date in [d(10_000, 1, 1), far, d(1899, 1, 1)] {
            for reason in [R::Verzicht, R::WegfallVorjahr, R::WegfallLaufend] {
                let err = validate_change(&hist, &change(date, G, reason), today, &[]).unwrap_err();
                assert!(
                    err.contains(&ChangeError::DateOutOfRange(date)),
                    "{reason:?} {date}"
                );
            }
        }
        // Verzicht 9999 bände bis 10004: abgelehnt; 9994 bindet bis 9999.
        let err =
            validate_change(&hist, &change(d(9999, 1, 1), G, R::Verzicht), today, &[]).unwrap_err();
        assert_eq!(err, vec![ChangeError::DateOutOfRange(d(9999, 1, 1))]);
        assert!(message(&err[0]).contains("9999"));
        assert_eq!(
            validate_change(&hist, &change(d(9994, 1, 1), G, R::Verzicht), today, &[]),
            Ok(Some(d(9999, 1, 1)))
        );
        // Abfragen für extreme Jahre geraten nicht in Panik.
        let h = TaxStatusHistory::new([
            initial(K),
            entry("b", d(2026, 1, 1), G, R::Verzicht, Some(d(2031, 1, 1))),
        ]);
        assert_eq!(h.year_status(300_000), YearStatus::Regelbesteuerung);
        assert_eq!(h.year_status(-300_000), YearStatus::Kleinunternehmer);
        assert_eq!(h.segments(262_143).len(), 1);
        let _ = verzicht_binding_until(far);
        let _ = verzicht_deadline(262_143);
        let _ = verzicht_deadline(i32::MAX);
        let last = TaxStatusHistory::new([
            initial(K),
            entry("b", NaiveDate::MAX, G, R::WegfallLaufend, None),
        ]);
        assert!(last.earliest_return(d(2026, 1, 1)).is_some());
    }
}
