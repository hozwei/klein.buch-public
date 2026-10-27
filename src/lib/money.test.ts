// Lauf 2026.11 U1: Leseregel der Betrags- und Mengeneingabe (Plan §4.2, ADR 0041).

import { describe, expect, it } from "vitest";
import {
  MAX_AMOUNT_CENTS,
  formatCents,
  formatEuro,
  formatQuantity,
  parseDecimal,
  parseMoneyToCents,
  parseQuantity,
} from "./money";

function cents(text: string, allowNegative = false): number | null | string {
  const r = parseMoneyToCents(text, { allowNegative });
  return r.ok ? r.value : `ERR:${r.error}`;
}

function qty(text: string, maxFractionDigits = 4): number | null | string {
  const r = parseQuantity(text, { maxFractionDigits });
  return r.ok ? r.value : `ERR:${r.error}`;
}

describe("parseMoneyToCents: Leseregel", () => {
  it.each<[string, number]>([
    ["12", 1200],
    ["0", 0],
    ["0,00", 0],
    ["12,5", 1250],
    ["12,50", 1250],
    ["12.5", 1250],
    ["12.50", 1250],
    ["1.234", 123400],
    ["1.234.567", 123456700],
    ["1.234,56", 123456],
    ["1,234.56", 123456],
    ["1.234.567,89", 123456789],
    ["1,234,567.89", 123456789],
    [",5", 50],
    ["5,", 500],
    ["007", 700],
    ["0,05", 5],
    ["0.05", 5],
    ["  12,50  ", 1250],
    ["12,50 €", 1250],
    ["12,50€", 1250],
    ["€ 12,50", 1250],
    [" 12,50 €", 1250],
    ["+12,50", 1250],
    [" 1 234,56 €", 123456],
    ["1 234,56", 123456],
    ["1 234 567", 123456700],
    ["1 234.5", 123450],
  ])("%j → %d Cent", (text, expected) => {
    expect(cents(text)).toBe(expected);
  });

  it.each<[string, RegExp]>([
    ["1.234.56", /wie 1\.234,56/],
    ["12,345", /Höchstens 2 Nachkommastellen/],
    ["12.345,678", /Höchstens 2 Nachkommastellen/],
    ["12,500", /Höchstens 2 Nachkommastellen/],
    ["1,2,3", /wie 1\.234,56/],
    ["1.23.4", /wie 1\.234,56/],
    ["12.34.567", /wie 1\.234,56/],
    ["1.2345", /wie 1\.234,56/],
    ["1,234.567.89", /wie 1\.234,56/],
    ["0.123", /wie 1\.234,56/],
    ["12 5", /wie 1\.234,56/],
    ["1  234", /wie 1\.234,56/],
    ["1 234.567,00", /wie 1\.234,56/],
    ["abc", /wie 1\.234,56/],
    ["12a", /wie 1\.234,56/],
    ["1e3", /wie 1\.234,56/],
    [",", /wie 1\.234,56/],
    [".", /wie 1\.234,56/],
    ["5.", /wie 1\.234,56/],
    ["-", /wie 1\.234,56/],
    ["--5", /Nur Beträge ab 0/],
    ["5-", /wie 1\.234,56/],
    ["€", /^$/],
    ["12 € 50", /wie 1\.234,56/],
    ["-5", /Nur Beträge ab 0/],
    ["−5", /Nur Beträge ab 0/],
  ])("%j → Fehler", (text, msg) => {
    const r = parseMoneyToCents(text);
    if (text === "€") {
      // Nur das Währungszeichen: leer.
      expect(r).toEqual({ ok: true, value: null, normalized: "" });
      return;
    }
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.error).toMatch(msg);
  });

  it("leer und nur Leerzeichen → value null (kein Fehler)", () => {
    expect(parseMoneyToCents("")).toEqual({ ok: true, value: null, normalized: "" });
    expect(parseMoneyToCents("   ")).toEqual({ ok: true, value: null, normalized: "" });
    expect(parseMoneyToCents(" ")).toEqual({ ok: true, value: null, normalized: "" });
  });

  it("negativ nur mit allowNegative, auch mit U+2212 und €", () => {
    expect(cents("-5", true)).toBe(-500);
    expect(cents("−5", true)).toBe(-500);
    expect(cents("-1.234,56 €", true)).toBe(-123456);
    expect(cents("- 12,5", true)).toBe(-1250);
    expect(cents("€ -3", true)).toBe(-300);
    expect(cents("-0", true)).toBe(0);
    expect(Object.is(cents("-0,00", true), 0)).toBe(true);
    expect(cents("--5", true)).toMatch(/^ERR:/);
  });

  it("normalized zeigt die Auslegung im DIN-Format", () => {
    const r = parseMoneyToCents("1,234.5");
    expect(r).toEqual({ ok: true, value: 123450, normalized: "1.234,50" });
    expect(parseMoneyToCents("12.5")).toMatchObject({ normalized: "12,50" });
    expect(parseMoneyToCents("1.234")).toMatchObject({ normalized: "1.234,00" });
  });

  it("große Beträge: bis 1 Mrd. € (Server-Grenze), darüber Fehler", () => {
    expect(cents("1.000.000,00")).toBe(100_000_000);
    expect(cents("999.999.999,99")).toBe(99_999_999_999);
    expect(cents("1.000.000.000")).toBe(MAX_AMOUNT_CENTS);
    expect(cents("1.000.000.000,01")).toMatch(/^ERR:Betrag zu groß \(höchstens 1\.000\.000\.000,00 €\)/);
    expect(cents("-1.000.000.000,01", true)).toMatch(/^ERR:Betrag zu groß/);
  });

  it("Grenze safe integer bei aufgehobener Obergrenze", () => {
    const max = Number.MAX_SAFE_INTEGER; // 9.007.199.254.740.991 Cent
    const opts = { maxAbsCents: Number.POSITIVE_INFINITY };
    expect(parseMoneyToCents("90.071.992.547.409,91", opts)).toMatchObject({ ok: true, value: max });
    expect(parseMoneyToCents("90.071.992.547.409,92", opts)).toMatchObject({ ok: false });
    expect(parseMoneyToCents("99999999999999999999999999", opts)).toMatchObject({ ok: false });
    const r = parseMoneyToCents("90071992547409,91", opts);
    expect(r.ok && Number.isSafeInteger(r.value)).toBe(true);
  });

  it("keine Float-Fehler: 0,1 + 0,2 und 19,99 exakt", () => {
    expect(cents("0,1")).toBe(10);
    expect(cents("0,29")).toBe(29);
    expect(cents("19,99")).toBe(1999);
    expect(cents("1.234.567,89")).toBe(123456789);
    expect(cents("4,35")).toBe(435); // 4.35 * 100 = 434.99999999999994
  });
});

describe("formatCents / formatEuro", () => {
  it.each<[number, string]>([
    [0, "0,00"],
    [5, "0,05"],
    [99, "0,99"],
    [100, "1,00"],
    [123456, "1.234,56"],
    [100_000_000, "1.000.000,00"],
    [-123456, "\u22121.234,56"],
    [-0, "0,00"],
    [Number.MAX_SAFE_INTEGER, "90.071.992.547.409,91"],
    [-Number.MAX_SAFE_INTEGER, "\u221290.071.992.547.409,91"],
  ])("%d → %s", (c, expected) => {
    expect(formatCents(c)).toBe(expected);
  });

  it("Anzeige-Rundung für nicht ganze Cent (Menge × Preis), kaufmännisch", () => {
    expect(formatCents(12.5)).toBe("0,13");
    expect(formatCents(-12.5)).toBe("\u22120,13");
    expect(formatCents(12.4)).toBe("0,12");
    expect(formatCents(Number.NaN)).toBe("—");
  });

  it("formatEuro mit U+00A0 vor €", () => {
    expect(formatEuro(123456)).toBe("1.234,56 €");
  });

  it("negative Beträge mit echtem Minus (U+2212), der Parser liest es zurück (V3-22)", () => {
    expect(formatEuro(-45000)).toBe("\u2212450,00\u00a0€");
    expect(formatCents(-5)).not.toContain("-");
    expect(cents("\u2212450,00 €", true)).toBe(-45000);
    expect(cents(formatEuro(-123456), true)).toBe(-123456);
  });

  it("Rundtrip parse(format(x)) == x", () => {
    const values = [
      0, 1, 9, 10, 99, 100, 101, 999, 1000, 1001, 12345, 99999, 100000, 123456, 1000000, 123456789,
      99_999_999_999, MAX_AMOUNT_CENTS,
    ];
    for (const v of values) {
      expect(cents(formatCents(v))).toBe(v);
      expect(cents(formatEuro(v))).toBe(v);
      expect(cents(formatCents(-v), true)).toBe(v === 0 ? 0 : -v);
    }
  });
});

describe("parseQuantity (Mengen, 4 Nachkommastellen wie der Server)", () => {
  it.each<[string, number]>([
    ["1", 1],
    ["1,5", 1.5],
    ["0,125", 0.125],
    ["0,0001", 0.0001],
    ["2,5000", 2.5],
    ["1.5", 1.5],
    ["1.25", 1.25],
    ["1.000", 1000],
    ["1.234", 1234], // Plan-Regel gilt auch für Mengen: 3 Ziffern nach einzelnem Punkt = Tausender
    ["1.234,5", 1234.5],
    ["1,234.5", 1234.5],
    ["1.000.000", 1_000_000],
    [",75", 0.75],
  ])("%j → %d", (text, expected) => {
    expect(qty(text)).toBe(expected);
  });

  it("Fehler", () => {
    expect(qty("1,23456")).toMatch(/Höchstens 4 Nachkommastellen/);
    expect(qty("0.125")).toMatch(/^ERR:Bitte eine Menge wie/); // nicht still 125
    expect(qty("1.2345")).toMatch(/^ERR:/);
    expect(qty("-1")).toMatch(/Nur Werte ab 0/);
    expect(qty("x")).toMatch(/^ERR:/);
  });

  it("3 Nachkommastellen als Grenze", () => {
    expect(qty("0,125", 3)).toBe(0.125);
    expect(qty("0,1255", 3)).toMatch(/Höchstens 3 Nachkommastellen/);
  });

  it("ganze Zahlen (maxFractionDigits 0) und Obergrenze", () => {
    expect(qty("12", 0)).toBe(12);
    expect(qty("1.000", 0)).toBe(1000);
    expect(qty("12,5", 0)).toMatch(/ganze Zahl/);
    expect(qty("12.5", 0)).toMatch(/ganze Zahl/);
    expect(parseQuantity("65536", { maxFractionDigits: 0, max: 65535 })).toMatchObject({
      ok: false,
      error: "Höchstens 65.535 möglich.",
    });
    expect(parseQuantity("1.000.001", { max: 1_000_000 })).toMatchObject({ ok: false });
  });

  it("leer → null", () => {
    expect(qty("")).toBeNull();
  });

  it("formatQuantity ohne überflüssige Nullen und Rundtrip", () => {
    expect(formatQuantity(1.5)).toBe("1,5");
    expect(formatQuantity(1000)).toBe("1.000");
    expect(formatQuantity(0.125)).toBe("0,125");
    expect(formatQuantity(2)).toBe("2");
    expect(formatQuantity(-0)).toBe("0");
    expect(formatQuantity(1234.5678)).toBe("1.234,5678");
    expect(formatQuantity(8080, 0, false)).toBe("8080");
    expect(formatQuantity(2026, 0, false)).toBe("2026");
    expect(qty(formatQuantity(8080, 0, false), 0)).toBe(8080);
    for (const v of [0, 0.0001, 0.5, 1, 1.5, 12.25, 999, 1000, 1234.5, 99999.9999, 1_000_000]) {
      expect(qty(formatQuantity(v))).toBe(v);
    }
  });
});

describe("parseDecimal (Grundbaustein)", () => {
  it("liefert skaliert als bigint", () => {
    expect(parseDecimal("1.234,5", { maxFractionDigits: 3 })).toEqual({
      ok: true,
      value: 1234500n,
      normalized: "1.234,5",
    });
    expect(parseDecimal("-0,001", { maxFractionDigits: 3, allowNegative: true })).toMatchObject({
      value: -1n,
      normalized: "-0,001",
    });
  });
});
