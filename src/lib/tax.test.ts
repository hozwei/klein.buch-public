// KB-0014 / R1-S2-TESTS: Steuer-Regeln und Summen der Formulare wie der Server.

import { describe, expect, it } from "vitest";
import { applyTaxRules, documentTotals, roundHalfAwayFromZero } from "./tax";

describe("applyTaxRules", () => {
  it("§19: Steuerart E und Satz 0", () => {
    const items = [{ taxCategoryCode: "S", taxRatePercent: 19 }];
    applyTaxRules(items, true);
    expect(items[0]).toEqual({ taxCategoryCode: "E", taxRatePercent: 0 });
  });

  it("Regelbesteuerung: E wird S mit 19 %, gesetzte Sätze bleiben", () => {
    const items = [
      { taxCategoryCode: "E", taxRatePercent: 0 },
      { taxCategoryCode: "E", taxRatePercent: 7 },
      { taxCategoryCode: "Z", taxRatePercent: 0 },
    ];
    applyTaxRules(items, false);
    expect(items).toEqual([
      { taxCategoryCode: "S", taxRatePercent: 19 },
      { taxCategoryCode: "S", taxRatePercent: 7 },
      { taxCategoryCode: "Z", taxRatePercent: 0 },
    ]);
  });

  it("ohne Stammdaten nichts ändern", () => {
    const items = [{ taxCategoryCode: "E", taxRatePercent: 0 }];
    applyTaxRules(items, null);
    expect(items[0].taxCategoryCode).toBe("E");
  });
});

describe("documentTotals", () => {
  it("USt je Steuergruppe aus der Nettosumme (100 Kleinpositionen)", () => {
    const items = Array.from({ length: 100 }, () => ({
      quantity: 1,
      unitPriceCents: 1,
      taxRatePercent: 19,
      taxCategoryCode: "S",
    }));
    expect(documentTotals(items)).toEqual({ net: 100, tax: 19, gross: 119 });
  });

  it("Bruchteil-Mengen und gemischte Sätze", () => {
    expect(
      documentTotals([
        { quantity: 0.333, unitPriceCents: 333, taxRatePercent: 19, taxCategoryCode: "S" },
        { quantity: 3, unitPriceCents: 333, taxRatePercent: 7, taxCategoryCode: "S" },
      ]),
    ).toEqual({ net: 1110, tax: 21 + 70, gross: 1201 });
  });

  it("rundet Halbe weg von 0 (Storno symmetrisch)", () => {
    expect(roundHalfAwayFromZero(0.5)).toBe(1);
    expect(roundHalfAwayFromZero(-0.5)).toBe(-1);
    const original = documentTotals([
      { quantity: 2.5, unitPriceCents: 1999, taxRatePercent: 19, taxCategoryCode: "S" },
    ]);
    const storno = documentTotals([
      { quantity: 2.5, unitPriceCents: -1999, taxRatePercent: 19, taxCategoryCode: "S" },
    ]);
    expect(storno).toEqual({ net: -original.net, tax: -original.tax, gross: -original.gross });
  });
});
