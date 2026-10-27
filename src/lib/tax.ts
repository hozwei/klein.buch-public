// Steuer-Regeln der Beleg-Formulare (Rechnung, Angebot, Umwandlung, Abo-Vorlage).
// Spiegeln den Server (domain::invoice / domain::kleinunternehmer); der Server
// prüft unabhängig davon.

/** Steuerarten bei Regelbesteuerung (EN 16931); E gibt es nur mit §19 (KB-0014). */
export const REGEL_TAX_CODES = ["S", "Z", "AE", "K", "G", "O", "L", "M"] as const;

/** Regelsteuersatz nach §12 Abs. 1 UStG — Vorgabe für neue Positionen bei Regelbesteuerung. */
export const STANDARD_VAT_RATE = 19;

interface TaxLine {
  taxCategoryCode: string;
  taxRatePercent: number;
}

/**
 * Setzt die Steuerfelder der Positionen in place (keine neue Array-Identität,
 * sonst läuft ein `$effect` endlos):
 * - §19 (`isKlein === true`): Steuerart E, Satz 0 (§14c-Schutz).
 * - Regelbesteuerung (`false`): E wird zu S mit 19 %, wenn kein Satz gesetzt ist.
 * - Stammdaten noch nicht geladen (`null`): nichts.
 */
export function applyTaxRules(items: TaxLine[], isKlein: boolean | null): void {
  if (isKlein === null) return;
  for (const it of items) {
    if (isKlein) {
      if (it.taxRatePercent !== 0) it.taxRatePercent = 0;
      if (it.taxCategoryCode !== "E") it.taxCategoryCode = "E";
    } else if (it.taxCategoryCode === "E") {
      it.taxCategoryCode = "S";
      if (it.taxRatePercent === 0) it.taxRatePercent = STANDARD_VAT_RATE;
    }
  }
}

interface AmountLine extends TaxLine {
  quantity: number;
  unitPriceCents: number;
}

/**
 * Summen wie der Server: Netto je Position kaufmännisch gerundet, USt je
 * (Steuerart, Satz) aus der Nettosumme (EN 16931 BR-CO-17).
 */
export function documentTotals(items: AmountLine[]): { net: number; tax: number; gross: number } {
  let net = 0;
  const groups = new Map<string, { net: number; rate: number }>();
  for (const it of items) {
    const itNet = roundHalfAwayFromZero(it.quantity * it.unitPriceCents);
    net += itNet;
    const key = `${it.taxCategoryCode}|${it.taxRatePercent}`;
    const g = groups.get(key) ?? { net: 0, rate: it.taxRatePercent };
    g.net += itNet;
    groups.set(key, g);
  }
  let tax = 0;
  for (const g of groups.values()) tax += roundHalfAwayFromZero((g.net * g.rate) / 100);
  return { net, tax, gross: net + tax };
}

/** Wie Rusts `f64::round` (Halbe weg von 0); `Math.round` rundet -0,5 auf 0. */
export function roundHalfAwayFromZero(x: number): number {
  return Math.sign(x) * Math.round(Math.abs(x));
}

/**
 * §13b UStG bei Kosten (ADR 0039 Nr. 143): Der Lieferant weist keine Umsatzsteuer aus,
 * die Steuer schuldet und meldet der Empfänger selbst (auch mit §19). Hinweis am
 * Schalter der Kosten-Formulare (Erfassen, E-Rechnung übernehmen).
 */
export const REVERSE_CHARGE_13B_HINT =
  "Bei §13b kommt die Rechnung ohne Umsatzsteuer: USt-Betrag 0, Netto = Brutto. " +
  "Die selbst geschuldete Steuer meldest du in der Umsatzsteuer-Voranmeldung " +
  "(Handbuch „Kosten erfassen“).";

/**
 * Wie `domain::expense::validate_expense` (`ReverseChargeWithTax`): mit §13b muss der
 * USt-Betrag 0 sein. Liefert die Meldung oder `null`; der Server prüft unabhängig davon.
 */
export function reverseChargeTaxError(
  reverseCharge13b: boolean,
  taxAmountCents: number,
): string | null {
  if (!reverseCharge13b || taxAmountCents === 0) return null;
  return (
    "§13b (Reverse-Charge): Der Lieferant weist keine Umsatzsteuer aus, der USt-Betrag " +
    "muss 0 sein. Die Steuer meldest du selbst in der Umsatzsteuer-Voranmeldung an."
  );
}
