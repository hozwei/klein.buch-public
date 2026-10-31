// ADR 0042 Nr. 14: Darstellung der Übersicht § 15a UStG.
import { describe, expect, it } from "vitest";
import type { VatAdjustmentAsset, VatAdjustmentYear } from "#lib/types.ts";
import {
  exemptionLabel,
  filingLabel,
  monthsLabel,
  periodText,
  shareText,
  splitByChange,
  totalText,
  unknownReasonLabel,
  vatYearOptions,
  yearOutcome,
} from "./vatAdjustment.ts";

const NB = " ";

function year(over: Partial<VatAdjustmentYear> = {}): VatAdjustmentYear {
  return {
    year: 2026,
    months: 12,
    monthsRegular: 12,
    monthsChanged: 12,
    changePoints: 100,
    amountCents: 38_000,
    adjustmentCents: 38_000,
    direction: "zugunsten",
    exemption: null,
    filing: "annual_return",
    projected: false,
    ...over,
  };
}

function asset(no: string, y: VatAdjustmentYear | null): VatAdjustmentAsset {
  return {
    assetId: no,
    assetNumber: no,
    label: "Kamera",
    expenseNumber: "KO-2025-0001",
    firstUse: "2025-03-10",
    depreciationMethod: "linear",
    adjustment: {
      period: { from: "2025-03-01", to: "2030-02-28", months: 60 },
      inputTaxCents: 190_000,
      originalKlein: true,
      inputTaxBagatelle: false,
      years: y ? [y] : [],
      totalAdjustmentCents: y?.adjustmentCents ?? 0,
    },
    yearShare: y,
    hasChange: !!y && y.monthsChanged > 0,
    hints: [],
  };
}

describe("vatYearOptions", () => {
  it("reicht vom ältesten Kauf bis fünf Jahre nach dem jüngsten", () => {
    expect(vatYearOptions(2026, [2024, 2025])).toEqual([2030, 2029, 2028, 2027, 2026, 2025, 2024]);
  });
  it("ohne Anlagen nur das laufende Jahr", () => {
    expect(vatYearOptions(2026, [])).toEqual([2026]);
  });
  it("ein Kauf vor mehr als fünf Jahren reicht bis heute", () => {
    expect(vatYearOptions(2026, [2019])[0]).toBe(2026);
    expect(vatYearOptions(2026, [2019]).at(-1)).toBe(2019);
  });
});

describe("Texte", () => {
  it("Monate und Zeitraum nach DIN 5008", () => {
    expect(monthsLabel(1)).toBe("1 Monat");
    expect(monthsLabel(60)).toBe("60 Monate");
    expect(periodText({ from: "2025-03-01", to: "2030-02-28", months: 60 })).toBe("01.03.2025 bis 28.02.2030");
  });

  it("Anteil des Jahres", () => {
    expect(shareText(year({ monthsChanged: 7 }))).toBe("7 von 12 Monaten geändert");
    expect(shareText(year({ months: 1, monthsChanged: 1 }))).toBe("1 von 1 Monat geändert");
    expect(shareText(year({ monthsChanged: 0 }))).toBe("keine Änderung (12 Monate im Zeitraum)");
  });

  it("Ergebnis mit Richtung oder Ausnahme nach § 44 UStDV", () => {
    expect(yearOutcome(year())).toBe(`380,00${NB}€ zugunsten`);
    expect(yearOutcome(year({ adjustmentCents: -38_000, direction: "zulasten" }))).toBe(`380,00${NB}€ zulasten`);
    expect(yearOutcome(year({ adjustmentCents: 0, exemption: "input_tax_bagatelle" }))).toMatch(/§ 44 Abs. 1 UStDV/);
    expect(yearOutcome(year({ adjustmentCents: 0, exemption: "below_ten_points" }))).toMatch(/§ 44 Abs. 2 UStDV/);
    expect(yearOutcome(year({ monthsChanged: 0, adjustmentCents: 0, direction: null }))).toBe("keine Berichtigung");
  });

  it("Summe mit Vorzeichen als Richtung", () => {
    expect(totalText(0)).toBe(`0,00${NB}€`);
    expect(totalText(12_345)).toBe(`123,45${NB}€ zugunsten`);
    expect(totalText(-12_345)).toBe(`123,45${NB}€ zulasten`);
  });

  it("Erklärungsart, Ausnahmen und Gründe", () => {
    expect(filingLabel("annual_return")).toBe("Umsatzsteuer-Jahreserklärung");
    expect(filingLabel("advance_return")).toMatch(/Voranmeldung/);
    expect(filingLabel(null)).toBe("");
    expect(exemptionLabel(null)).toBe("");
    expect(unknownReasonLabel("reverse_charge")).toMatch(/§ 13b/);
    expect(unknownReasonLabel("no_expense")).toBe("Ohne gültigen Kostenbeleg");
  });
});

describe("splitByChange", () => {
  it("trennt Anlagen mit Änderung im Jahr von den übrigen", () => {
    const a = asset("AV-1", year());
    const b = asset("AV-2", year({ monthsChanged: 0, adjustmentCents: 0 }));
    const c = asset("AV-3", null);
    const r = splitByChange([a, b, c]);
    expect(r.changed.map((x) => x.assetNumber)).toEqual(["AV-1"]);
    expect(r.unchanged.map((x) => x.assetNumber)).toEqual(["AV-2", "AV-3"]);
  });
});
