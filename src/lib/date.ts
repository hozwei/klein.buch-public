// Datumshelfer in Europe/Berlin (Plan W1.1, E12/E16).
//
// Belegdaten und Geschäftsjahre gelten in deutscher Ortszeit — nicht in UTC
// (`toISOString` lieferte zwischen 0 und 1–2 Uhr den Vortag) und nicht in der
// Ortszeit des Geräts (mobil kann das Gerät in einer anderen Zone stehen).
// ISO-Daten (`YYYY-MM-DD`) werden als reine Kalenderdaten in UTC gerechnet.

export const APP_TIME_ZONE = "Europe/Berlin";

const ymdFormat = new Intl.DateTimeFormat("en-CA", {
  timeZone: APP_TIME_ZONE,
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
});

const ISO_DATE = /^(\d{4})-(\d{2})-(\d{2})$/;

function berlinParts(now: Date): { y: string; m: string; d: string } {
  const parts = ymdFormat.formatToParts(now);
  const get = (t: string) => parts.find((p) => p.type === t)?.value ?? "";
  return { y: get("year"), m: get("month"), d: get("day") };
}

/** Heutiges Datum in Europe/Berlin als `YYYY-MM-DD`. */
export function todayBerlin(now: Date = new Date()): string {
  const { y, m, d } = berlinParts(now);
  return `${y}-${m}-${d}`;
}

/** Laufendes Kalenderjahr in Europe/Berlin. */
export function yearBerlin(now: Date = new Date()): number {
  return Number(berlinParts(now).y);
}

/** Jahr eines ISO-Datums (`YYYY-MM-DD`); bei ungültiger Eingabe das laufende Berliner Jahr. */
export function yearOfIsoDate(iso: string, now: Date = new Date()): number {
  const m = ISO_DATE.exec(iso ?? "");
  return m ? Number(m[1]) : yearBerlin(now);
}

function isoFromUtc(d: Date): string {
  const y = String(d.getUTCFullYear()).padStart(4, "0");
  const m = String(d.getUTCMonth() + 1).padStart(2, "0");
  const day = String(d.getUTCDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

/** Addiert Kalendertage zu einem ISO-Datum; ungültige Eingaben kommen unverändert zurück. */
export function addDaysIso(iso: string, days: number): string {
  const m = ISO_DATE.exec(iso ?? "");
  if (!m) return iso;
  const d = new Date(Date.UTC(Number(m[1]), Number(m[2]) - 1, Number(m[3])));
  if (Number.isNaN(d.getTime())) return iso;
  d.setUTCDate(d.getUTCDate() + days);
  return isoFromUtc(d);
}

/** Datum in `days` Tagen ab heute (Europe/Berlin). */
export function daysFromTodayBerlin(days: number, now: Date = new Date()): string {
  return addDaysIso(todayBerlin(now), days);
}

/** Tag im Jahr (1-basiert) des heutigen Berliner Datums. */
export function dayOfYearBerlin(now: Date = new Date()): number {
  const m = ISO_DATE.exec(todayBerlin(now));
  if (!m) return 1;
  const y = Number(m[1]);
  const t = Date.UTC(y, Number(m[2]) - 1, Number(m[3]));
  return Math.floor((t - Date.UTC(y, 0, 1)) / 86_400_000) + 1;
}

/** Anzahl Tage eines Kalenderjahres. */
export function daysInYear(year: number): number {
  return (year % 4 === 0 && year % 100 !== 0) || year % 400 === 0 ? 366 : 365;
}

// ---- Zeitstempel und Tagesabstände (Lauf F, D-15/D-25, ADR 0040) ------------
//
// Die Datenbank speichert Zeitpunkte in UTC ohne Zonenangabe
// (`YYYY-MM-DD HH:MM:SS`, `datetime('now','utc')`); einige Felder kommen als
// RFC 3339 (`…Z` oder mit Versatz). `new Date("2026-10-24 01:30:00")` läse
// das in der Zeitzone des Geräts (Safari/WebKit: ungültig) und
// `new Date("2026-10-24")` als UTC-Mitternacht — beides ergibt je nach Gerät
// einen anderen Tag. Deshalb lesen alle Anzeigen Zeitstempel über
// [`parseTimestamp`] und rechnen Tage über reine Kalenderdaten.

const TIMESTAMP =
  /^(\d{4}-\d{2}-\d{2})[ T](\d{2}:\d{2}(?::\d{2})?)(\.\d+)?(Z|[+-]\d{2}:?\d{2})?$/i;

/** UTC-Millisekunden eines gültigen ISO-Datums (`YYYY-MM-DD`), sonst `null`. */
function isoDateToUtcMs(iso: string): number | null {
  const m = ISO_DATE.exec(iso ?? "");
  if (!m) return null;
  const t = Date.UTC(Number(m[1]), Number(m[2]) - 1, Number(m[3]));
  // Kein stilles Überrollen (31.02. → 03.03.): nur echte Kalenderdaten.
  return isoFromUtc(new Date(t)) === iso ? t : null;
}

/** Ist `value` ein gültiges reines Kalenderdatum `YYYY-MM-DD`? */
export function isIsoDate(value: string | null | undefined): boolean {
  return isoDateToUtcMs((value ?? "").trim()) !== null;
}

/**
 * Liest einen Zeitstempel der API als Zeitpunkt.
 *
 * - `YYYY-MM-DD HH:MM[:SS[.fff]]` ohne Zone gilt als **UTC** (Format der DB).
 * - Mit `Z` oder Versatz (`+02:00`, `+0200`) wie angegeben.
 * - Ein reines Datum `YYYY-MM-DD` ergibt UTC-Mitternacht dieses Tages.
 *
 * Ungültige Eingaben ergeben `null` — kein `RangeError`, kein Parsen in der
 * Zeitzone des Geräts.
 */
export function parseTimestamp(value: string | null | undefined): Date | null {
  const s = (value ?? "").trim();
  const dayOnly = isoDateToUtcMs(s);
  if (dayOnly !== null) return new Date(dayOnly);
  const m = TIMESTAMP.exec(s);
  if (!m || isoDateToUtcMs(m[1]) === null) return null;
  const time = m[2].length === 5 ? `${m[2]}:00` : m[2];
  const frac = m[3] ? m[3].slice(0, 4).padEnd(4, "0") : "";
  let zone = (m[4] ?? "Z").toUpperCase();
  if (zone !== "Z" && !zone.includes(":")) zone = `${zone.slice(0, 3)}:${zone.slice(3)}`;
  const d = new Date(`${m[1]}T${time}${frac}${zone}`);
  return Number.isNaN(d.getTime()) ? null : d;
}

/**
 * Berliner Kalenderdatum (`YYYY-MM-DD`) eines Zeitstempels; ein reines Datum
 * bleibt, wie es ist. Ungültig: `null`.
 */
export function berlinDateOf(value: string | null | undefined): string | null {
  const s = (value ?? "").trim();
  if (isoDateToUtcMs(s) !== null) return s;
  const d = parseTimestamp(s);
  return d ? todayBerlin(d) : null;
}

/**
 * Kalendertage von `fromIso` bis `toIso` (beide `YYYY-MM-DD`; positiv, wenn
 * `toIso` später liegt). Unabhängig von Uhrzeit, Zeitumstellung und Zeitzone
 * des Geräts. Ungültige Eingabe: `NaN` (Vergleiche damit sind immer falsch).
 */
export function daysBetweenIso(fromIso: string, toIso: string): number {
  const a = isoDateToUtcMs(fromIso);
  const b = isoDateToUtcMs(toIso);
  if (a === null || b === null) return Number.NaN;
  return Math.round((b - a) / 86_400_000);
}

/**
 * Tage bis zu einem Fälligkeitsdatum ab dem heutigen Berliner Datum
 * (0 = heute fällig, negativ = überfällig). Ein Zeitstempel zählt mit seinem
 * Berliner Tag.
 */
export function daysUntilBerlin(iso: string, now: Date = new Date()): number {
  return daysBetweenIso(todayBerlin(now), berlinDateOf(iso) ?? "");
}
