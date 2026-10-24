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
