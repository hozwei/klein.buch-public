// Steuerstatus nach § 19 UStG in der Oberfläche (ADR 0042 Nr. 1–6).
//
// Reine Helfer: maßgebliches Datum eines Belegs, Texte zu Gründen und Status,
// Auswahl und Vorschläge im Dialog „Statuswechsel eintragen“, Feldprüfungen, die
// Server-Regeln nachbilden (domain::tax_status, der Server prüft erneut), und ein
// Zwischenspeicher für `taxStatusAt` je Datum. Maßgeblich ist immer der Server.

import { addDaysIso, isIsoDate } from "#lib/date.ts";
import { date as fmtDate, euro } from "#lib/format.ts";
import type {
  TaxStatus,
  TaxStatusChange,
  TaxStatusConsequences,
  TaxStatusReason,
  TaxThresholdHint,
  YearStatus,
} from "#lib/types.ts";

/** Wirkungsdatum „seit Beginn“ des Anfangsstands (Grund `initial`). */
export const ANFANG = "0001-01-01";

/**
 * Maßgebliches Datum eines Ausgangsbelegs (ADR 0042 Nr. 3): Leistungsdatum,
 * ersatzweise Rechnungs- bzw. Angebotsdatum, sonst `fallback` (heute). Nur gültige
 * Kalenderdaten zählen; ein halb getipptes Datum fällt auf das nächste zurück.
 */
export function relevantTaxDate(
  deliveryDate: string | null | undefined,
  documentDate: string | null | undefined,
  fallback: string,
): string {
  for (const d of [deliveryDate, documentDate]) {
    const s = (d ?? "").trim().slice(0, 10);
    if (isIsoDate(s)) return s;
  }
  return fallback;
}

/**
 * Maßgebliches Datum einer Rechnung (ADR 0042 Nr. 3/10, wie
 * `domain::tax_status::document_date`): Leistungsdatum, sonst Ende des
 * Leistungszeitraums, sonst Rechnungsdatum, sonst `fallback`.
 */
export function invoiceTaxDate(
  inv: {
    deliveryDate?: string | null;
    servicePeriodEnd?: string | null;
    invoiceDate?: string | null;
  },
  fallback: string,
): string {
  const service = relevantTaxDate(inv.deliveryDate, inv.servicePeriodEnd, "");
  return relevantTaxDate(service, inv.invoiceDate, fallback);
}

export const TAX_STATUS_LABEL: Record<TaxStatus, string> = {
  kleinunternehmer: "Kleinunternehmer nach §19 UStG",
  regelbesteuerung: "Regelbesteuerung",
};

export const YEAR_STATUS_LABEL: Record<YearStatus, string> = {
  kleinunternehmer: "Kleinunternehmer nach §19 UStG",
  regelbesteuerung: "Regelbesteuerung",
  wechsel: "Wechsel im Jahr",
};

export interface ReasonInfo {
  /** Bezeichnung in Liste und Auswahl. */
  label: string;
  /** Status, den der Grund vorschreibt; `null` = Anfangsstand/Bestand. */
  status: TaxStatus | null;
  /** Wirkung nur zum 1. Januar (§ 19 Abs. 1 und 3 UStG). */
  janFirst: boolean;
  /** Fragt nach dem Beleg, mit dem die Grenze überschritten wurde. */
  trigger: boolean;
  /** Kurze Erklärung je Fall (ADR 0042 Nr. 2). */
  explain: string;
}

export const REASONS: Record<TaxStatusReason, ReasonInfo> = {
  initial: {
    label: "Stand zu Beginn: Regelbesteuerung",
    status: null,
    janFirst: false,
    trigger: false,
    explain:
      "Du warst schon vor deinen ersten Belegen in Klein.Buch regelbesteuert. Der Eintrag gilt seit " +
      "Beginn der Aufzeichnungen, ohne eigenes Datum. Nur möglich, solange es keinen Eintrag gibt.",
  },
  verzicht: {
    label: "Verzicht auf §19 (§ 19 Abs. 3 UStG)",
    status: "regelbesteuerung",
    janFirst: true,
    trigger: false,
    explain:
      "Du verzichtest freiwillig. Der Verzicht wirkt ab dem 1. Januar des Jahres, für das er gilt, " +
      "und bindet dich fünf Kalenderjahre einschließlich dieses Jahres. Erklären kannst du ihn bis " +
      "Ende Februar des übernächsten Jahres.",
  },
  wegfall_vorjahr: {
    label: "Wegfall: Vorjahresumsatz über 25.000 €",
    status: "regelbesteuerung",
    janFirst: true,
    trigger: false,
    explain:
      "Dein Gesamtumsatz im Vorjahr lag über 25.000 €. Die Regelung gilt ab dem 1. Januar nicht " +
      "mehr, ohne Bindung (§ 19 Abs. 1 UStG).",
  },
  wegfall_laufend: {
    label: "Wegfall: Umsatz im laufenden Jahr über 100.000 €",
    status: "regelbesteuerung",
    janFirst: false,
    trigger: true,
    explain:
      "Dein Umsatz im laufenden Jahr überschreitet 100.000 €. Die Regelung endet mit dem Umsatz, " +
      "der die Grenze überschreitet: Er ist schon steuerpflichtig, die Umsätze davor bleiben " +
      "steuerfrei. Ohne Bindung.",
  },
  wegfall_gruendungsjahr: {
    label: "Wegfall: Umsatz im Gründungsjahr über 25.000 €",
    status: "regelbesteuerung",
    janFirst: false,
    trigger: true,
    explain:
      "Im Jahr der Gründung gilt schon im laufenden Jahr die Grenze von 25.000 €. Die Regelung " +
      "endet mit dem Umsatz, der sie überschreitet; die Umsätze davor bleiben steuerfrei. Ohne Bindung.",
  },
  rueckkehr: {
    label: "Rückkehr zu §19",
    status: "kleinunternehmer",
    janFirst: true,
    trigger: false,
    explain:
      "Du giltst wieder als Kleinunternehmer, zum 1. Januar eines Jahres. Voraussetzung: Gesamtumsatz " +
      "im Vorjahr höchstens 25.000 € und im laufenden Jahr voraussichtlich höchstens 100.000 €. Nach " +
      "einem Wegfall ohne Sperre, nach einem Verzicht erst nach Ablauf der fünf Jahre.",
  },
  legacy: {
    label: "Wechsel aus dem bisherigen Schalter übernommen",
    status: "regelbesteuerung",
    janFirst: true,
    trigger: false,
    explain: "",
  },
};

/** Gültige Einträge nach Wirkungsdatum (widerrufene zählen nicht). */
export function activeEntries(entries: TaxStatusChange[]): TaxStatusChange[] {
  return entries
    .filter((e) => !e.revokedAt)
    .sort((a, b) => (a.effectiveFrom < b.effectiveFrom ? -1 : a.effectiveFrom > b.effectiveFrom ? 1 : 0));
}

/** Status an einem Tag wie `TaxStatusHistory::status_at`: ohne Eintrag gilt §19. */
export function statusAt(entries: TaxStatusChange[], day: string): TaxStatus {
  let status: TaxStatus = "kleinunternehmer";
  for (const e of activeEntries(entries)) {
    if (e.effectiveFrom <= day) status = e.status;
  }
  return status;
}

/** Jüngster gültiger Eintrag oder `null`. */
export function lastEntry(entries: TaxStatusChange[]): TaxStatusChange | null {
  const a = activeEntries(entries);
  return a.length ? a[a.length - 1] : null;
}

/** Erster gültiger Eintrag, der nach `today` wirkt (ein schon eingetragener künftiger Wechsel). */
export function nextChangeAfter(entries: TaxStatusChange[], today: string): TaxStatusChange | null {
  return activeEntries(entries).find((e) => e.effectiveFrom > today) ?? null;
}

/**
 * Gründe, die als nächster Eintrag in Frage kommen. Ein neuer Eintrag wirkt nach
 * dem jüngsten (ADR 0042, keine Überschneidung) und muss den Status ändern —
 * maßgeblich ist darum der Status des jüngsten Eintrags, nicht der von heute.
 */
export function choosableReasons(entries: TaxStatusChange[]): TaxStatusReason[] {
  const last = lastEntry(entries);
  const wegfall: TaxStatusReason[] = ["verzicht", "wegfall_vorjahr", "wegfall_laufend", "wegfall_gruendungsjahr"];
  if (!last) return ["initial", ...wegfall];
  return last.status === "kleinunternehmer" ? wegfall : ["rueckkehr"];
}

/** `YYYY-01-01` */
function janFirst(year: number): string {
  return `${String(year).padStart(4, "0")}-01-01`;
}

function yearOf(iso: string): number {
  return Number(iso.slice(0, 4));
}

/** Nächster 1. Januar ab `day` (inklusive). */
export function nextJanFirst(day: string): string {
  return day.slice(5) === "01-01" ? day : janFirst(yearOf(day) + 1);
}

export interface SuggestContext {
  today: string;
  entries: TaxStatusChange[];
  earliestReturn: string | null;
  hints: TaxThresholdHint[];
}

/**
 * Vorschlag für Wirkungsdatum und Beleg je Grund (ADR 0042 Nr. 2 und 5):
 * - Verzicht: nächster 1. Januar (ohne Rückwirkung; ändern lässt er sich).
 * - Wegfall Vorjahr: 1. Januar des laufenden Jahres.
 * - Wegfall laufend/Gründungsjahr: aus dem Grenzhinweis (Leistungsdatum des
 *   auslösenden Belegs, frühestens 1. Januar), sonst leer.
 * - Rückkehr: frühestmögliche Rückkehr laut Server, sonst nächster 1. Januar.
 * Jeder Vorschlag liegt nach dem jüngsten Eintrag, soweit der Grund das zulässt.
 */
export function suggestChange(
  reason: TaxStatusReason,
  ctx: SuggestContext,
): { effectiveFrom: string; triggerDocument: string } {
  const last = lastEntry(ctx.entries);
  const after = (d: string) => {
    if (!last || last.effectiveFrom === ANFANG || d > last.effectiveFrom) return d;
    return REASONS[reason].janFirst ? janFirst(yearOf(last.effectiveFrom) + 1) : d;
  };
  const hint = ctx.hints.find((h) => h.suggestedReason === reason && h.exceeded);
  switch (reason) {
    case "initial":
    case "legacy":
      return { effectiveFrom: "", triggerDocument: "" };
    case "verzicht":
      return { effectiveFrom: after(nextJanFirst(ctx.today)), triggerDocument: "" };
    case "wegfall_vorjahr":
      return { effectiveFrom: after(hint?.suggestedEffectiveFrom ?? janFirst(yearOf(ctx.today))), triggerDocument: "" };
    case "wegfall_laufend":
    case "wegfall_gruendungsjahr":
      return {
        effectiveFrom: hint?.suggestedEffectiveFrom ?? "",
        triggerDocument: hint?.triggerDocument ?? "",
      };
    case "rueckkehr":
      return { effectiveFrom: after(ctx.earliestReturn ?? nextJanFirst(ctx.today)), triggerDocument: "" };
  }
}

/**
 * Feldprüfung des Wirkungsdatums wie `validate_change` (Pflicht, 1. Januar,
 * Wegfall nicht in der Zukunft). Überschneidung, Bindung und abgeschlossene Jahre
 * meldet die Server-Vorschau. Liefert die Meldung oder `""`.
 */
export function effectiveFromError(reason: TaxStatusReason, value: string, today: string): string {
  if (reason === "initial") return "";
  const v = (value ?? "").trim();
  if (!v) return "Bitte ein Wirkungsdatum angeben.";
  if (!isIsoDate(v) || yearOf(v) < 1900) return "Bitte ein gültiges Datum angeben (TT.MM.JJJJ).";
  const info = REASONS[reason];
  if (info.janFirst && v.slice(5) !== "01-01") {
    return `„${info.label}“ wirkt nur zum 1. Januar, z. B. ${fmtDate(janFirst(yearOf(v)))} oder ${fmtDate(janFirst(yearOf(v) + 1))}.`;
  }
  if ((reason === "wegfall_laufend" || reason === "wegfall_gruendungsjahr") && v > today) {
    return "Ein Wegfall lässt sich erst eintragen, wenn der Umsatz vorliegt: Das Datum darf nicht in der Zukunft liegen.";
  }
  if (reason === "wegfall_vorjahr" && yearOf(v) > yearOf(today)) {
    return "Den Wegfall wegen des Vorjahres trägst du im betroffenen Jahr ein, nicht im Voraus.";
  }
  return "";
}

/** Bindungsende eines Verzichts: erster Tag der möglichen Rückkehr (Wirkungsjahr + 5). */
export function bindingUntilFor(reason: TaxStatusReason, effectiveFrom: string): string | null {
  if (reason !== "verzicht" || !isIsoDate(effectiveFrom)) return null;
  return janFirst(yearOf(effectiveFrom) + 5);
}

/** Wirkungsdatum für Listen: „seit Beginn“ beim Anfangsstand, sonst TT.MM.JJJJ. */
export function effectiveFromLabel(effectiveFrom: string): string {
  return effectiveFrom === ANFANG ? "seit Beginn" : fmtDate(effectiveFrom);
}

/** Text eines Grenzhinweises (ADR 0042 Nr. 5). */
export function thresholdHintText(h: TaxThresholdHint): string {
  const what =
    h.kind === "vorjahr"
      ? `Dein Gesamtumsatz ${h.year} beträgt ${euro(h.turnoverCents)}`
      : `Dein Gesamtumsatz ${h.year} beträgt bisher ${euro(h.turnoverCents)}`;
  const limit = h.kind === "vorjahr" ? "die Vorjahresgrenze" : h.kind === "gruendungsjahr" ? "die Grenze im Gründungsjahr" : "die Grenze im laufenden Jahr";
  if (!h.exceeded) {
    return `${what} und erreicht damit genau ${limit} von ${euro(h.limitCents)}. Die Regelung gilt weiter; jeder weitere Umsatz überschreitet die Grenze.`;
  }
  let s = `${what} und liegt über ${limit} von ${euro(h.limitCents)}.`;
  if (h.triggerDocument) s += ` Die Grenze überschreitet der Umsatz aus ${h.triggerDocument}.`;
  if (h.suggestedEffectiveFrom) s += ` Vorschlag: Wegfall ab ${fmtDate(h.suggestedEffectiveFrom)}.`;
  if (h.presumed) {
    s += " Klein.Buch kennt keinen früheren Umsatz und nimmt darum an, dass dies dein Gründungsjahr ist; trifft das nicht zu, gilt die Grenze von 100.000 €.";
  }
  return s;
}

/** Folgen als kurze Zeilen für Vorschau und Ergebnis (ADR 0042 Nr. 4). */
export function consequenceLines(c: TaxStatusConsequences): string[] {
  const out: string[] = [];
  const n = c.affectedInvoices.length;
  if (n > 0) {
    out.push(
      `${n === 1 ? "1 festgeschriebene Rechnung passt" : `${n} festgeschriebene Rechnungen passen`} nicht mehr zum Steuerstatus: ` +
        `${c.affectedInvoices.map((a) => a.number).join(", ")}. Klein.Buch ändert sie nicht; kläre mit deiner Steuerberatung, ` +
        "ob eine Berichtigung (§ 31 Abs. 5 UStDV) oder Storno und Neuausstellung nötig ist.",
    );
  }
  if (c.affectedDrafts.length > 0) {
    out.push(`Entwürfe mit anderem Steuerstatus am Belegdatum: ${c.affectedDrafts.join(", ")}. Prüfe ihre Positionen vor dem Ausstellen.`);
  }
  if (c.recurringTemplates.length > 0) {
    out.push(
      `Abo-Vorlagen, deren Positionen nicht zum Status am nächsten Termin passen: ${c.recurringTemplates.join(", ")}. ` +
        "Bis du sie anpasst, erzeugen ihre Termine keine Rechnung.",
    );
  }
  if (c.capitalizedAssets.length > 0) {
    out.push(`Aus Kosten übernommene Anlagen mit abweichenden Anschaffungskosten: ${c.capitalizedAssets.join(", ")}.`);
  }
  if (c.reexportYears.length > 0) {
    const ys = c.reexportYears.join(", ");
    out.push(`EÜR, ELSTER-Ausfüllhilfe und DATEV-Stapel ${c.reexportYears.length === 1 ? "des Jahres" : "der Jahre"} ${ys} ändern sich: neu exportieren.`);
  }
  return out;
}

/**
 * Zwischenspeicher für den Status je Datum (`taxStatusAt`): eine Anfrage je Datum,
 * gleichzeitige Anfragen teilen sich ein Ergebnis, ein Fehler wird nicht gemerkt.
 */
export class TaxStatusCache {
  #values = new Map<string, boolean>();
  #pending = new Map<string, Promise<boolean>>();
  #fetch: (day: string) => Promise<boolean>;

  constructor(fetch: (day: string) => Promise<boolean>) {
    this.#fetch = fetch;
  }

  /** Bekannter Wert ohne Anfrage, sonst `undefined`. */
  peek(day: string): boolean | undefined {
    return this.#values.get(day);
  }

  /** §19 am `day`? */
  get(day: string): Promise<boolean> {
    const known = this.#values.get(day);
    if (known !== undefined) return Promise.resolve(known);
    let p = this.#pending.get(day);
    if (!p) {
      p = this.#fetch(day).then(
        (v) => {
          this.#values.set(day, v);
          this.#pending.delete(day);
          return v;
        },
        (e: unknown) => {
          this.#pending.delete(day);
          throw e;
        },
      );
      this.#pending.set(day, p);
    }
    return p;
  }
}

/**
 * Hinweis über den Editoren (ADR 0042 Nr. 3): Status am maßgeblichen Datum, mit
 * ausdrücklichem Hinweis, wenn er vom heutigen abweicht. Bei Regelbesteuerung wie
 * heute kein Hinweis. `day` ist schon formatiert (TT.MM.JJJJ).
 */
export function statusNoticeText(o: {
  isKlein: boolean;
  day: string;
  todayKlein: boolean | null;
  /** „Leistungsdatum“, „Angebotsdatum“, „ersten Termin“ (nach „am“). */
  dateLabel: string;
  /** „auf der Rechnung“, „auf dem Angebot“, „auf den erzeugten Rechnungen“. */
  noun: string;
}): { title?: string; body: string; differs: boolean } | null {
  const differs = o.todayKlein !== null && o.todayKlein !== o.isKlein;
  if (o.isKlein && !differs) {
    return {
      body:
        `Am ${o.dateLabel} (${o.day}) giltst du als Kleinunternehmer (§19) — ${o.noun} wird keine ` +
        "Umsatzsteuer ausgewiesen. Die USt-Felder sind deshalb ausgeblendet.",
      differs,
    };
  }
  if (o.isKlein) {
    return {
      title: `Am ${o.dateLabel} gilt die Kleinunternehmer-Regelung.`,
      body:
        `Heute giltst du als regelbesteuert, am ${o.dateLabel} (${o.day}) aber als Kleinunternehmer (§19). ` +
        `Maßgeblich ist das Datum des Belegs: ${o.noun} wird keine Umsatzsteuer ausgewiesen, die ` +
        "USt-Felder sind ausgeblendet.",
      differs,
    };
  }
  if (!differs) return null;
  return {
    title: `Am ${o.dateLabel} gilt die Regelbesteuerung.`,
    body:
      `Heute giltst du als Kleinunternehmer (§19), am ${o.dateLabel} (${o.day}) aber als regelbesteuert. ` +
      `Maßgeblich ist das Datum des Belegs: ${o.noun} wird Umsatzsteuer ausgewiesen. Prüfe die ` +
      "Steuersätze der Positionen (vorbelegt mit 19 %).",
    differs,
  };
}

/**
 * Umwandlung Angebot → Rechnung über einen Statuswechsel (ADR 0042 Nr. 3): Das
 * Angebot trägt den Status seines Datums, die Rechnung den ihres Leistungsdatums.
 * Liefert den Hinweis oder `null`, wenn beide gleich sind.
 */
export function quoteConversionHint(quoteKlein: boolean | null, invoiceKlein: boolean | null): string | null {
  if (quoteKlein === null || invoiceKlein === null || quoteKlein === invoiceKlein) return null;
  return quoteKlein
    ? "Das Angebot entstand als Kleinunternehmer (ohne Umsatzsteuer). Am Leistungsdatum der Rechnung " +
        "gilt die Regelbesteuerung: Die Positionen sind mit 19 % vorbelegt, die Nettopreise bleiben. Prüfe " +
        "die Steuersätze und sprich den Bruttobetrag mit dem Kunden ab."
    : "Das Angebot entstand mit Regelbesteuerung. Am Leistungsdatum der Rechnung giltst du als " +
        "Kleinunternehmer (§19): Die Positionen werden ohne Umsatzsteuer übernommen, die Nettopreise bleiben.";
}

/**
 * Letzter Tag der Bindung nach einem Verzicht. Der Server liefert den ersten Tag der
 * möglichen Rückkehr (01.01.2031); gebunden ist man bis zum Vortag (31.12.2030).
 */
export function bindingEnd(bindingUntil: string | null | undefined): string | null {
  if (!bindingUntil || !isIsoDate(bindingUntil)) return null;
  return addDaysIso(bindingUntil, -1);
}

/** Beschriftung des Knopfs im Dialog „Statuswechsel eintragen“ je Grund. */
export function changeConfirmLabel(reason: TaxStatusReason | null): string {
  switch (reason) {
    case "initial":
      return "Regelbesteuerung eintragen";
    case "verzicht":
      return "Verzicht eintragen";
    case "rueckkehr":
      return "Rückkehr eintragen";
    case null:
      return "Wechsel eintragen";
    default:
      return "Wegfall eintragen";
  }
}

/**
 * §19 heute oder ab einem eingetragenen künftigen Wechsel (wie
 * `TaxStatusHistory::klein_today_or_later`): Dann braucht die Standardvorlage die
 * §19-Klausel (ADR 0042 Nr. 3).
 */
export function kleinTodayOrLater(entries: TaxStatusChange[], today: string): boolean {
  return (
    statusAt(entries, today) === "kleinunternehmer" ||
    activeEntries(entries).some((e) => e.effectiveFrom > today && e.status === "kleinunternehmer")
  );
}
