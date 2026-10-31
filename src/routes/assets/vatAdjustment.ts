// Übersicht der Vorsteuerberichtigung nach § 15a UStG (ADR 0042 Nr. 14):
// Texte und Auswahl für `/assets/vat-adjustment`. Gerechnet wird nur im Server
// (`domain::vat_adjustment`); hier nur die Darstellung.

import { date, euro } from "#lib/format.ts";
import type {
  VatAdjustmentAsset,
  VatAdjustmentDirection,
  VatAdjustmentExemption,
  VatAdjustmentFiling,
  VatAdjustmentPeriod,
  VatAdjustmentUnknown,
  VatAdjustmentYear,
} from "#lib/types.ts";

/** Ziel des Inbox-Hinweises (`commands::vat_adjustment::OVERVIEW_URL`). */
export const VAT_ADJUSTMENT_URL = "/assets/vat-adjustment";

/** Berichtigungszeitraum höchstens fünf Jahre (60 Monate), § 15a Abs. 1 UStG. */
const PERIOD_YEARS = 5;

/**
 * Wählbare Jahre, neueste zuerst: vom ältesten Anschaffungsjahr bis zum letzten
 * Jahr, das ein Zeitraum noch berühren kann (Anschaffungsjahr + 5), mindestens
 * das laufende Jahr.
 */
export function vatYearOptions(current: number, acquisitionYears: Iterable<number>): number[] {
  const ys = [...acquisitionYears].filter((y) => Number.isInteger(y));
  const min = Math.min(current, ...ys);
  const max = Math.max(current, ...ys.map((y) => y + PERIOD_YEARS));
  const out: number[] = [];
  for (let y = max; y >= min; y--) out.push(y);
  return out;
}

/** „1 Monat“ / „12 Monate“. */
export function monthsLabel(n: number): string {
  return `${n} ${n === 1 ? "Monat" : "Monate"}`;
}

/** „01.03.2026 bis 28.02.2031“ (DIN 5008, ohne Gedankenstrich als „bis“). */
export function periodText(p: VatAdjustmentPeriod): string {
  return `${date(p.from)} bis ${date(p.to)}`;
}

export function directionLabel(d: VatAdjustmentDirection | null): string {
  if (d === "zugunsten") return "zugunsten";
  if (d === "zulasten") return "zulasten";
  return "";
}

/** Grenze nach § 44 UStDV, mit Fundstelle. */
export function exemptionLabel(e: VatAdjustmentExemption | null): string {
  if (e === "input_tax_bagatelle") return "Entfällt: Vorsteuer bis 1.000 € (§ 44 Abs. 1 UStDV)";
  if (e === "below_ten_points") {
    return "Entfällt: Änderung unter 10 Prozentpunkten (§ 44 Abs. 2 UStDV)";
  }
  return "";
}

/** Wo die Berichtigung hingehört (§ 44 Abs. 3 UStDV). */
export function filingLabel(f: VatAdjustmentFiling | null): string {
  if (f === "annual_return") return "Umsatzsteuer-Jahreserklärung";
  if (f === "advance_return") return "Voranmeldung (über 6.000 €)";
  if (f === "advance_return_of_disposal") return "Voranmeldung des Verkaufs bzw. der Entnahme";
  return "";
}

/** Anteil des Jahres: „7 von 12 Monaten geändert“, ohne Änderung „keine Änderung“. */
export function shareText(y: VatAdjustmentYear): string {
  if (y.monthsChanged === 0) return `keine Änderung (${monthsLabel(y.months)} im Zeitraum)`;
  return `${y.monthsChanged} von ${y.months} ${y.months === 1 ? "Monat" : "Monaten"} geändert`;
}

/** Ergebnis eines Jahres in einem Satz: Betrag mit Richtung oder die Ausnahme. */
export function yearOutcome(y: VatAdjustmentYear): string {
  if (y.monthsChanged === 0) return "keine Berichtigung";
  const ex = exemptionLabel(y.exemption);
  if (ex) return ex;
  if (y.adjustmentCents === 0) return "keine Berichtigung";
  return `${euro(Math.abs(y.adjustmentCents))} ${directionLabel(y.direction)}`.trim();
}

/** Summe eines Jahres: Betrag mit Richtung (positiv zugunsten, negativ zulasten). */
export function totalText(cents: number): string {
  if (cents === 0) return euro(0);
  return `${euro(Math.abs(cents))} ${cents > 0 ? "zugunsten" : "zulasten"}`;
}

/** Ursprünglicher Status beim Kauf. */
export function originalStatusLabel(originalKlein: boolean): string {
  return originalKlein ? "Kleinunternehmer (§ 19)" : "Regelbesteuerung";
}

/** Kurzform des Grunds einer Anlage ohne bekannte Vorsteuer. */
export function unknownReasonLabel(r: VatAdjustmentUnknown["reason"]): string {
  if (r === "no_expense") return "Ohne gültigen Kostenbeleg";
  if (r === "reverse_charge") return "Reverse-Charge (§ 13b UStG)";
  return "Ohne Umsatzsteuer auf dem Beleg";
}

/** Ändern sich die Verhältnisse im gewählten Jahr? */
export function changedInYear(a: VatAdjustmentAsset): boolean {
  return (a.yearShare?.monthsChanged ?? 0) > 0;
}

/** Teilt die Anlagen in „mit Änderung im Jahr“ und „ohne“. */
export function splitByChange(assets: VatAdjustmentAsset[]): {
  changed: VatAdjustmentAsset[];
  unchanged: VatAdjustmentAsset[];
} {
  const changed: VatAdjustmentAsset[] = [];
  const unchanged: VatAdjustmentAsset[] = [];
  for (const a of assets) (changedInYear(a) ? changed : unchanged).push(a);
  return { changed, unchanged };
}
