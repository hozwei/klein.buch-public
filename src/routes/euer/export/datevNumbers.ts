// Berater- und Mandantennummer für den DATEV-Buchungsstapel (ADR 0042 Nr. 15).
//
// Die DATEV-Formatbeschreibung verlangt beide im Kopf (Feld 11 und 12, kein
// Leerfeld). Der Server prüft dasselbe (`datev_csv::require_header_numbers`);
// hier nur die Meldung am Feld, bevor ein Export scheitert (ADR 0041 Nr. 22).
// Einzel-Export: beide Pflicht. Steuerberater-Paket: beide oder keine — ohne
// Nummern entsteht es ohne DATEV-Stapel, mit nur einer lehnt der Server ab.

export const CONSULTANT_RANGE = { min: 1001, max: 9_999_999 } as const;
export const CLIENT_RANGE = { min: 1, max: 99_999 } as const;

export type DatevNumberField = "consultant" | "client";
/** `datev`: Einzel-Export des Stapels; `package`: Steuerberater-Paket. */
export type DatevTarget = "datev" | "package";

export interface DatevNumberErrors {
  consultant: string;
  client: string;
}

const NAME: Record<DatevNumberField, string> = {
  consultant: "Beraternummer",
  client: "Mandantennummer",
};
const RANGE: Record<DatevNumberField, { min: number; max: number }> = {
  consultant: CONSULTANT_RANGE,
  client: CLIENT_RANGE,
};

/** Formathilfe unter dem Feld (bleibt bei einem Fehler stehen, design.md §4.7). */
export function numberHint(field: DatevNumberField): string {
  const r = RANGE[field];
  return `Pflicht für den DATEV-Stapel; ${r.min} bis ${r.max}.`;
}

/** Meldung, wenn die Nummer außerhalb des Bereichs liegt; leer = in Ordnung oder leer. */
export function rangeError(field: DatevNumberField, v: number | null): string {
  const r = RANGE[field];
  if (v == null || (Number.isInteger(v) && v >= r.min && v <= r.max)) return "";
  return `${NAME[field]} prüfen: Sie liegt zwischen ${r.min} und ${r.max}.`;
}

function missing(field: DatevNumberField, target: DatevTarget): string {
  if (target === "datev") {
    return `${NAME[field]} fehlt: Der DATEV-Buchungsstapel braucht Berater- und Mandantennummer. Du bekommst sie von deiner Steuerberatung.`;
  }
  return `${NAME[field]} fehlt: Trag beide Nummern ein oder lass beide leer, dann entsteht das Paket ohne DATEV-Stapel.`;
}

/** Meldungen je Feld für den gewählten Export (leer = kein Fehler). */
export function datevNumberErrors(
  consultant: number | null,
  client: number | null,
  target: DatevTarget,
): DatevNumberErrors {
  const out: DatevNumberErrors = {
    consultant: rangeError("consultant", consultant),
    client: rangeError("client", client),
  };
  const bothEmpty = consultant == null && client == null;
  if (target === "package" && bothEmpty) return out;
  if (consultant == null) out.consultant = missing("consultant", target);
  if (client == null) out.client = missing("client", target);
  return out;
}

/** Paket ohne DATEV-Stapel: beide Nummern leer. */
export function packageWithoutDatev(consultant: number | null, client: number | null): boolean {
  return consultant == null && client == null;
}

/** Ordnet eine Server-Meldung dem Feld zu („Beraternummer …“/„Mandantennummer …“). */
export function serverErrorField(message: string): DatevNumberField | null {
  const m = message.trim();
  if (m.startsWith("Beraternummer")) return "consultant";
  if (m.startsWith("Mandantennummer")) return "client";
  return null;
}

/** Dateiname des Stapels wie `datev_csv::file_name` (Präfix `EXTF_` nach der
 *  Formatbeschreibung); unbekannter Kontenrahmen wie beim Server SKR03. */
export function datevFileName(year: number, skr: string): string {
  const code = skr.trim().toUpperCase() === "SKR04" ? "SKR04" : "SKR03";
  return `EXTF_Buchungsstapel_${year}_${code}.csv`;
}

/** Gemerkte Nummer aus dem Browser (ganze Zahl > 0, sonst leer). */
export function parseStoredNumber(raw: string | null): number | null {
  if (raw == null || raw.trim() === "") return null;
  const v = Number(raw);
  return Number.isInteger(v) && v > 0 ? v : null;
}
