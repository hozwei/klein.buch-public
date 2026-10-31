// Leistungszeitpunkt und Leitweg-ID in der Oberfläche (ADR 0042 Nr. 10–12).
//
// Spiegelt domain::invoice (Leistungsdatum ODER Leistungszeitraum, Beginn ≤
// Ende, Vorausrechnung erlaubt; Leitweg-ID höchstens 200 Zeichen ohne
// Steuerzeichen) und domain::recurring::billing_period. Der Server prüft
// unabhängig davon erneut.

import { isIsoDate } from "#lib/date.ts";
import { date } from "#lib/format.ts";
import type { Frequency } from "#lib/types.ts";

/** Art der Angabe am Beleg: ein Leistungsdatum oder ein Zeitraum von–bis. */
export type ServiceDateMode = "date" | "period";

/** Höchstlänge der Leitweg-ID bzw. Käuferreferenz (BT-10), wie Migration 0035. */
export const BUYER_REFERENCE_MAX_CHARS = 200;

const MONTHS_DE = [
  "Januar", "Februar", "März", "April", "Mai", "Juni",
  "Juli", "August", "September", "Oktober", "November", "Dezember",
];

function iso(value: string | null | undefined): string {
  const s = (value ?? "").trim().slice(0, 10);
  return isIsoDate(s) ? s : "";
}

/** Modus eines gespeicherten Belegs: Zeitraum, sobald eine Grenze gesetzt ist. */
export function serviceDateModeOf(inv: {
  servicePeriodStart?: string | null;
  servicePeriodEnd?: string | null;
}): ServiceDateMode {
  return iso(inv.servicePeriodStart) || iso(inv.servicePeriodEnd) ? "period" : "date";
}

/**
 * Felder für den Server je nach Modus: nie Datum und Zeitraum zugleich
 * (domain::invoice, ServiceDateAndPeriod).
 */
export function serviceDatePayload(
  mode: ServiceDateMode,
  deliveryDate: string,
  periodStart: string,
  periodEnd: string,
): { deliveryDate: string | null; servicePeriodStart: string | null; servicePeriodEnd: string | null } {
  return mode === "period"
    ? { deliveryDate: null, servicePeriodStart: periodStart || null, servicePeriodEnd: periodEnd || null }
    : { deliveryDate: deliveryDate || null, servicePeriodStart: null, servicePeriodEnd: null };
}

/** Meldung am Leistungsdatum (Pflicht im Modus „Leistungsdatum“, U1). */
export function deliveryDateError(deliveryDate: string): string {
  return iso(deliveryDate) ? "" : "Bitte das Leistungsdatum angeben.";
}

/** Meldungen an Beginn und Ende des Leistungszeitraums (BG-14; BR-29: Ende ≥ Beginn). */
export function servicePeriodErrors(start: string, end: string): { start: string; end: string } {
  const s = iso(start);
  const e = iso(end);
  return {
    start: s ? "" : "Bitte den Beginn des Leistungszeitraums angeben.",
    end: !e
      ? "Bitte das Ende des Leistungszeitraums angeben."
      : s && e < s
        ? "Das Ende des Leistungszeitraums liegt vor dessen Beginn."
        : "",
  };
}

/** Alle Meldungen des Leistungszeitpunkts im gewählten Modus; leer = gültig. */
export function serviceDateErrors(
  mode: ServiceDateMode,
  deliveryDate: string,
  periodStart: string,
  periodEnd: string,
): { delivery: string; start: string; end: string } {
  if (mode === "period") return { delivery: "", ...servicePeriodErrors(periodStart, periodEnd) };
  return { delivery: deliveryDateError(deliveryDate), start: "", end: "" };
}

/** Hat der Leistungszeitpunkt im gewählten Modus eine Meldung? */
export function serviceDateInvalid(
  mode: ServiceDateMode,
  deliveryDate: string,
  periodStart: string,
  periodEnd: string,
): boolean {
  const e = serviceDateErrors(mode, deliveryDate, periodStart, periodEnd);
  return !!(e.delivery || e.start || e.end);
}

/**
 * Hinweis „Rechnung vor der Leistung“ (Vorausrechnung, ADR 0042 Nr. 11): das
 * Leistungsdatum bzw. das Ende des Zeitraums liegt nach dem Rechnungsdatum.
 * Zulässig (§ 14 Abs. 4 Satz 1 Nr. 6 UStG) — nur ein Hinweis, kein Fehler.
 */
export function advanceInvoiceHint(
  mode: ServiceDateMode,
  deliveryDate: string,
  periodEnd: string,
  invoiceDate: string,
): string {
  const inv = iso(invoiceDate);
  const service = iso(mode === "period" ? periodEnd : deliveryDate);
  if (!inv || !service || service <= inv) return "";
  return mode === "period"
    ? "Rechnung vor der Leistung: Der Leistungszeitraum endet nach dem Rechnungsdatum. Das ist zulässig (Vorausrechnung)."
    : "Rechnung vor der Leistung: Das Leistungsdatum liegt nach dem Rechnungsdatum. Das ist zulässig (Vorausrechnung).";
}

/** Zeitraum als „TT.MM.JJJJ – TT.MM.JJJJ“ (DIN 5008, Halbgeviertstrich mit Leerzeichen). */
export function formatServicePeriod(start: string | null | undefined, end: string | null | undefined): string {
  return `${date(start)} – ${date(end)}`;
}

/**
 * Klartext des Zeitraums wie in Position 1 der Abo-Rechnungen
 * (scheduler::recurring_invoice::period_label): ein voller Kalendermonat als
 * „Juni 2026“, sonst „01.04.2026 – 30.06.2026“.
 */
export function servicePeriodLabel(start: string, end: string): string {
  const s = iso(start);
  const e = iso(end);
  if (!s || !e) return "";
  const [y, m, d] = s.split("-").map(Number);
  const [ey, em, ed] = e.split("-").map(Number);
  if (d === 1 && ey === y && em === m && ed === daysInMonth(y, m)) return `${MONTHS_DE[m - 1]} ${y}`;
  return formatServicePeriod(s, e);
}

/** Leistungsangabe eines gespeicherten Belegs für Detail- und Storno-Seite. */
export function serviceSummary(inv: {
  deliveryDate?: string | null;
  servicePeriodStart?: string | null;
  servicePeriodEnd?: string | null;
}): { label: string; text: string } {
  if (iso(inv.servicePeriodStart) && iso(inv.servicePeriodEnd)) {
    return { label: "Leistungszeitraum", text: formatServicePeriod(inv.servicePeriodStart, inv.servicePeriodEnd) };
  }
  // Ohne Datum und Zeitraum gilt das Rechnungsdatum (Rückfall wie auf der PDF).
  if (!iso(inv.deliveryDate)) return { label: "Leistungsdatum", text: "entspricht dem Rechnungsdatum" };
  return { label: "Leistungsdatum", text: date(inv.deliveryDate) };
}

function daysInMonth(year: number, month: number): number {
  return new Date(Date.UTC(year, month, 0)).getUTCDate();
}

const FREQUENCY_MONTHS: Record<Frequency, number> = {
  monthly: 1,
  quarterly: 3,
  semiannually: 6,
  annually: 12,
};

function addMonthsClamped(from: string, months: number, dayOfPeriod: number): string {
  const [y, m] = from.split("-").map(Number);
  const total = y * 12 + (m - 1) + months;
  const year = Math.floor(total / 12);
  const month = (total % 12) + 1;
  const day = Math.max(1, Math.min(dayOfPeriod, daysInMonth(year, month)));
  return `${String(year).padStart(4, "0")}-${String(month).padStart(2, "0")}-${String(day).padStart(2, "0")}`;
}

/**
 * Abrechnungszeitraum eines Abo-Termins (domain::recurring::billing_period,
 * ADR 0042 Nr. 10): vom Termin bis zum Tag vor dem folgenden Termin. Bei
 * ungültigem Datum `null`.
 */
export function billingPeriod(
  frequency: Frequency,
  dayOfPeriod: number | null | undefined,
  occurrence: string,
): { start: string; end: string } | null {
  const start = iso(occurrence);
  const months = FREQUENCY_MONTHS[frequency];
  if (!start || !months) return null;
  const day = Math.max(1, Math.min(31, Math.trunc(dayOfPeriod ?? 1) || 1));
  const next = addMonthsClamped(start, months, day);
  const [ny, nm, nd] = next.split("-").map(Number);
  const prev = new Date(Date.UTC(ny, nm - 1, nd - 1));
  const end = prev.toISOString().slice(0, 10);
  return { start, end: end < start ? start : end };
}

/** Leitweg-ID wie gespeichert: getrimmt; leer oder „N/A“ = keine (`null`). */
export function normalizeBuyerReference(raw: string | null | undefined): string | null {
  const s = (raw ?? "").trim();
  return !s || s.toUpperCase() === "N/A" ? null : s;
}

// Steuerzeichen wie Rusts `char::is_control` (Unicode-Kategorie Cc).
const CONTROL_CHAR = /[\u0000-\u001F\u007F-\u009F]/;

/** Meldung am Feld Leitweg-ID (domain::invoice, BuyerReferenceInvalid); leer = gültig. */
export function buyerReferenceError(raw: string | null | undefined): string {
  const s = normalizeBuyerReference(raw);
  if (s === null) return "";
  if ([...s].length > BUYER_REFERENCE_MAX_CHARS) {
    return `Die Leitweg-ID darf höchstens ${BUYER_REFERENCE_MAX_CHARS} Zeichen lang sein.`;
  }
  if (CONTROL_CHAR.test(s)) return "Die Leitweg-ID darf keine Steuerzeichen (z. B. Tabulator) enthalten.";
  return "";
}

// Aufbau der Leitweg-ID: Grobadressierung (2–12 Ziffern), optional
// Feinadressierung (bis 30 Buchstaben/Ziffern), Prüfziffer (2 Ziffern).
const LEITWEG_ID = /^\d{2,12}(-[A-Za-z0-9]{1,30})?-\d{2}$/;

/**
 * Formathilfe: Hinweis, wenn ein Wert nicht wie eine Leitweg-ID aufgebaut ist.
 * Kein Fehler — BT-10 darf auch eine andere Käuferreferenz sein; die Prüfziffer
 * prüft die Eingangsplattform des Auftraggebers.
 */
export function buyerReferenceFormatHint(raw: string | null | undefined): string {
  const s = normalizeBuyerReference(raw);
  if (s === null || buyerReferenceError(s) || LEITWEG_ID.test(s)) return "";
  return "Das ist keine Leitweg-ID im üblichen Aufbau. Als Käuferreferenz geht der Wert trotzdem so in die E-Rechnung.";
}
