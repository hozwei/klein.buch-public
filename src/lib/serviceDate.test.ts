// ADR 0042 Nr. 10–12: Leistungszeitpunkt und Leitweg-ID in der Oberfläche
// (spiegeln domain::invoice und domain::recurring; der Server prüft erneut).

import { describe, expect, it } from "vitest";
import {
  BUYER_REFERENCE_MAX_CHARS,
  advanceInvoiceHint,
  billingPeriod,
  buyerReferenceError,
  buyerReferenceFormatHint,
  deliveryDateError,
  formatServicePeriod,
  normalizeBuyerReference,
  serviceDateErrors,
  serviceDateInvalid,
  serviceDateModeOf,
  serviceDatePayload,
  servicePeriodErrors,
  servicePeriodLabel,
  serviceSummary,
} from "./serviceDate";

describe("serviceDatePayload", () => {
  it("schickt nie Datum und Zeitraum zugleich (ServiceDateAndPeriod)", () => {
    expect(serviceDatePayload("date", "2026-05-04", "2026-05-01", "2026-05-31")).toEqual({
      deliveryDate: "2026-05-04",
      servicePeriodStart: null,
      servicePeriodEnd: null,
    });
    expect(serviceDatePayload("period", "2026-05-04", "2026-05-01", "2026-05-31")).toEqual({
      deliveryDate: null,
      servicePeriodStart: "2026-05-01",
      servicePeriodEnd: "2026-05-31",
    });
    expect(serviceDatePayload("period", "", "", "")).toEqual({
      deliveryDate: null,
      servicePeriodStart: null,
      servicePeriodEnd: null,
    });
  });

  it("Modus eines gespeicherten Belegs", () => {
    expect(serviceDateModeOf({ servicePeriodStart: null, servicePeriodEnd: null })).toBe("date");
    expect(serviceDateModeOf({ servicePeriodStart: "2026-05-01", servicePeriodEnd: "2026-05-31" })).toBe("period");
    expect(serviceDateModeOf({})).toBe("date");
  });
});

describe("Feldprüfungen", () => {
  it("Leistungsdatum ist im Modus Datum Pflicht", () => {
    expect(deliveryDateError("")).not.toBe("");
    expect(deliveryDateError("2026-02-30")).not.toBe("");
    expect(deliveryDateError("2026-05-04")).toBe("");
  });

  it("Zeitraum: beide Grenzen, Ende nicht vor Beginn (BR-29)", () => {
    expect(servicePeriodErrors("", "")).toEqual({
      start: "Bitte den Beginn des Leistungszeitraums angeben.",
      end: "Bitte das Ende des Leistungszeitraums angeben.",
    });
    expect(servicePeriodErrors("2026-05-10", "2026-05-09").end).toBe(
      "Das Ende des Leistungszeitraums liegt vor dessen Beginn.",
    );
    expect(servicePeriodErrors("2026-05-10", "2026-05-10")).toEqual({ start: "", end: "" });
    expect(servicePeriodErrors("2026-05-01", "2026-07-31")).toEqual({ start: "", end: "" });
  });

  it("prüft nur die Felder des gewählten Modus", () => {
    expect(serviceDateErrors("date", "2026-05-04", "", "")).toEqual({ delivery: "", start: "", end: "" });
    expect(serviceDateInvalid("date", "2026-05-04", "", "2026-01-01")).toBe(false);
    expect(serviceDateInvalid("period", "2026-05-04", "", "")).toBe(true);
    expect(serviceDateInvalid("period", "", "2026-05-01", "2026-05-31")).toBe(false);
  });
});

describe("advanceInvoiceHint (ADR 0042 Nr. 11)", () => {
  it("Hinweis, kein Fehler, wenn die Leistung nach dem Rechnungsdatum liegt", () => {
    expect(advanceInvoiceHint("date", "2026-06-01", "", "2026-05-20")).toMatch(/^Rechnung vor der Leistung/);
    expect(advanceInvoiceHint("period", "", "2026-06-30", "2026-06-01")).toMatch(/endet nach dem Rechnungsdatum/);
  });

  it("kein Hinweis am oder vor dem Rechnungsdatum oder bei unvollständigen Daten", () => {
    expect(advanceInvoiceHint("date", "2026-05-20", "", "2026-05-20")).toBe("");
    expect(advanceInvoiceHint("date", "2026-05-01", "", "2026-05-20")).toBe("");
    expect(advanceInvoiceHint("period", "2026-09-01", "2026-05-31", "2026-06-01")).toBe("");
    expect(advanceInvoiceHint("date", "2026-06-01", "", "")).toBe("");
  });
});

describe("Anzeige", () => {
  it("Zeitraum nach DIN 5008", () => {
    expect(formatServicePeriod("2026-04-01", "2026-06-30")).toBe("01.04.2026 – 30.06.2026");
  });

  it("Klartext wie Position 1 der Abo-Rechnung", () => {
    expect(servicePeriodLabel("2026-06-01", "2026-06-30")).toBe("Juni 2026");
    expect(servicePeriodLabel("2028-02-01", "2028-02-29")).toBe("Februar 2028");
    expect(servicePeriodLabel("2026-04-01", "2026-06-30")).toBe("01.04.2026 – 30.06.2026");
    expect(servicePeriodLabel("2026-06-15", "2026-07-14")).toBe("15.06.2026 – 14.07.2026");
    expect(servicePeriodLabel("", "2026-07-14")).toBe("");
  });

  it("Leistungsangabe einer gespeicherten Rechnung", () => {
    expect(serviceSummary({ deliveryDate: "2026-05-04" })).toEqual({ label: "Leistungsdatum", text: "04.05.2026" });
    expect(
      serviceSummary({ deliveryDate: null, servicePeriodStart: "2026-05-01", servicePeriodEnd: "2026-05-31" }),
    ).toEqual({ label: "Leistungszeitraum", text: "01.05.2026 – 31.05.2026" });
    expect(serviceSummary({ deliveryDate: null })).toEqual({
      label: "Leistungsdatum",
      text: "entspricht dem Rechnungsdatum",
    });
  });
});

describe("billingPeriod (domain::recurring::billing_period)", () => {
  it("Termin bis Tag vor dem folgenden Termin, je Frequenz", () => {
    expect(billingPeriod("monthly", 1, "2026-04-01")).toEqual({ start: "2026-04-01", end: "2026-04-30" });
    expect(billingPeriod("quarterly", 1, "2026-04-01")).toEqual({ start: "2026-04-01", end: "2026-06-30" });
    expect(billingPeriod("semiannually", 1, "2026-04-01")).toEqual({ start: "2026-04-01", end: "2026-09-30" });
    expect(billingPeriod("annually", 1, "2026-04-01")).toEqual({ start: "2026-04-01", end: "2027-03-31" });
  });

  it("lückenlos am Monatsende (Stichtag 31)", () => {
    expect(billingPeriod("monthly", 31, "2026-01-31")).toEqual({ start: "2026-01-31", end: "2026-02-27" });
    expect(billingPeriod("monthly", 31, "2026-02-28")).toEqual({ start: "2026-02-28", end: "2026-03-30" });
    expect(billingPeriod("monthly", 31, "2026-12-31")).toEqual({ start: "2026-12-31", end: "2027-01-30" });
  });

  it("ungültiges Datum ergibt keinen Zeitraum", () => {
    expect(billingPeriod("monthly", 1, "")).toBeNull();
    expect(billingPeriod("monthly", 1, "2026-02-30")).toBeNull();
  });
});

describe("Leitweg-ID (ADR 0042 Nr. 12)", () => {
  it("leer oder N/A bedeutet keine", () => {
    expect(normalizeBuyerReference(null)).toBeNull();
    expect(normalizeBuyerReference("   ")).toBeNull();
    expect(normalizeBuyerReference("N/A")).toBeNull();
    expect(normalizeBuyerReference(" n/a ")).toBeNull();
    expect(normalizeBuyerReference(" 04011000-12345-67 ")).toBe("04011000-12345-67");
  });

  it("Länge und Steuerzeichen wie buyer_reference_valid", () => {
    expect(buyerReferenceError("")).toBe("");
    expect(buyerReferenceError("04011000-12345-67")).toBe("");
    expect(buyerReferenceError("Abt.\t4")).toMatch(/Steuerzeichen/);
    expect(buyerReferenceError("a\u0085b")).toMatch(/Steuerzeichen/);
    expect(buyerReferenceError("x".repeat(BUYER_REFERENCE_MAX_CHARS))).toBe("");
    expect(buyerReferenceError("x".repeat(BUYER_REFERENCE_MAX_CHARS + 1))).toMatch(/höchstens 200 Zeichen/);
    // Zeichen, nicht UTF-16-Einheiten (wie Rusts chars().count()).
    expect(buyerReferenceError("€".repeat(BUYER_REFERENCE_MAX_CHARS))).toBe("");
    expect(buyerReferenceError("😀".repeat(BUYER_REFERENCE_MAX_CHARS))).toBe("");
  });

  it("Formathilfe nur als Hinweis bei abweichendem Aufbau", () => {
    expect(buyerReferenceFormatHint("04011000-12345-67")).toBe("");
    expect(buyerReferenceFormatHint("991-67")).toBe("");
    expect(buyerReferenceFormatHint("")).toBe("");
    expect(buyerReferenceFormatHint("Bestellung 4711")).toMatch(/keine Leitweg-ID im üblichen Aufbau/);
  });
});
