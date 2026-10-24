// Formatierungs-Helper für deutsche Zahlen-, Datums- und Geld-Darstellung.
//
// Datum und Uhrzeit immer in Europe/Berlin (Lauf F, D-25, ADR 0040): Die
// Datenbank liefert Zeitpunkte in UTC; angezeigt wird der Berliner Tag bzw. die
// Berliner Uhrzeit, unabhängig von der Zeitzone des Geräts. Reine
// Kalenderdaten (`YYYY-MM-DD`) bleiben der Tag, der sie sind.

import { APP_TIME_ZONE, isIsoDate, parseTimestamp } from "./date";

const NUMBER_DE = new Intl.NumberFormat("de-DE", {
  minimumFractionDigits: 2,
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

export function euro(cents: number): string {
  return `${NUMBER_DE.format(cents / 100)} €`;
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
