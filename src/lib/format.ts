// Formatierungs-Helper für deutsche Zahlen-, Datums- und Geld-Darstellung.
//
// Datum und Uhrzeit immer in Europe/Berlin (Lauf F, D-25, ADR 0040): Die
// Datenbank liefert Zeitpunkte in UTC; angezeigt wird der Berliner Tag bzw. die
// Berliner Uhrzeit, unabhängig von der Zeitzone des Geräts. Reine
// Kalenderdaten (`YYYY-MM-DD`) bleiben der Tag, der sie sind.

import { APP_TIME_ZONE, isIsoDate, parseTimestamp } from "./date";
import { formatEuro } from "./money";

const PERCENT_DE = new Intl.NumberFormat("de-DE", {
  minimumFractionDigits: 0,
  maximumFractionDigits: 2,
});

const DAY_PARTS = { day: "2-digit", month: "2-digit", year: "numeric" } as const;

/** Reines Kalenderdatum: in UTC formatieren, damit kein Tag verrutscht. */
const DATE_DE_CALENDAR = new Intl.DateTimeFormat("de-DE", { ...DAY_PARTS, timeZone: "UTC" });

/** Tag eines Zeitpunkts in Berliner Ortszeit. */
const DATE_DE_BERLIN = new Intl.DateTimeFormat("de-DE", { ...DAY_PARTS, timeZone: APP_TIME_ZONE });

/** Datum und Uhrzeit eines Zeitpunkts in Berliner Ortszeit. */
const DATETIME_DE_BERLIN = new Intl.DateTimeFormat("de-DE", {
  ...DAY_PARTS,
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
  timeZone: APP_TIME_ZONE,
});

/**
 * Cent → `1.234,56 €` (DIN 5008, geschütztes Leerzeichen U+00A0 vor `€`;
 * Ganzzahl-Rechnung über `money.ts`, Lauf 2026.11 U1).
 */
export function euro(cents: number): string {
  return formatEuro(cents);
}

/** Prozentsatz → `19 %`, `7,5 %` (geschütztes Leerzeichen vor `%`, höchstens 2 Nachkommastellen). */
export function percent(value: number): string {
  if (!Number.isFinite(value)) return "—";
  return `${PERCENT_DE.format(value === 0 ? 0 : value)} %`;
}

/**
 * IBAN in Vierergruppen (`DE89 3704 0044 0532 0130 00`), Großbuchstaben;
 * vorhandene Leerzeichen werden neu gesetzt. Leer: „—“. Keine Prüfung der
 * Prüfziffer (Anzeige, keine Validierung).
 */
export function iban(value: string | null | undefined): string {
  const s = (value ?? "").replace(/[\s ]+/g, "").toUpperCase();
  if (!s) return "—";
  return s.replace(/(.{4})(?=.)/g, "$1 ");
}

/**
 * Datum `TT.MM.JJJJ`. Ein reines Datum (`YYYY-MM-DD`) erscheint unverändert,
 * ein Zeitstempel (UTC aus der DB oder RFC 3339) mit seinem Berliner Tag.
 * Leer oder ungültig: „—“.
 */
export function date(value: string | null | undefined): string {
  const s = (value ?? "").trim();
  if (!s) return "—";
  const d = parseTimestamp(s);
  if (!d) return "—";
  return (isIsoDate(s) ? DATE_DE_CALENDAR : DATE_DE_BERLIN).format(d);
}

/**
 * Datum und Uhrzeit `TT.MM.JJJJ, HH:MM:SS` in Berliner Ortszeit (Protokolle,
 * Hinweise, Prüfläufe). Ein reines Datum erscheint ohne Uhrzeit. Leer: `empty`
 * (Standard „—“); ein unbekanntes Format erscheint als Rohwert statt eines
 * Fehlers.
 */
export function dateTime(value: string | null | undefined, empty = "—"): string {
  const s = (value ?? "").trim();
  if (!s) return empty;
  const d = parseTimestamp(s);
  if (!d) return s;
  return (isIsoDate(s) ? DATE_DE_CALENDAR : DATETIME_DE_BERLIN).format(d);
}

/** Datum und Uhrzeit ohne Sekunden in Berliner Ortszeit. */
const DATETIME_SHORT_DE_BERLIN = new Intl.DateTimeFormat("de-DE", {
  ...DAY_PARTS,
  hour: "2-digit",
  minute: "2-digit",
  timeZone: APP_TIME_ZONE,
});

/**
 * Datum und Uhrzeit `TT.MM.JJJJ, HH:MM` in Berliner Ortszeit — wie
 * `dateTime()`, aber ohne Sekunden (Tabellen und Listen, DIN 5008; Lauf
 * 2026.11 U1). Ein reines Datum erscheint ohne Uhrzeit, leer: `empty`,
 * unbekanntes Format als Rohwert.
 */
export function dateTimeShort(value: string | null | undefined, empty = "—"): string {
  const s = (value ?? "").trim();
  if (!s) return empty;
  const d = parseTimestamp(s);
  if (!d) return s;
  return (isIsoDate(s) ? DATE_DE_CALENDAR : DATETIME_SHORT_DE_BERLIN).format(d);
}
