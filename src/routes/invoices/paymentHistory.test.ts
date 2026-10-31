// U4 (Warteliste (7)): Konto je Zahlung; V2 (ADR 0042 Nr. 7–9): Zahlungsbuch mit
// Gegenbuchungen — Verweise, wirksame Zahlungen, frühestes Storno-Datum, Grund.

import { describe, expect, it } from "vitest";
import type { InvoicePayment, PaymentAccount } from "#lib/types.ts";
import {
  accountText,
  earliestStornoDate,
  effectivePayments,
  isReasonRejection,
  lastEffectivePaidDate,
  paymentAccountId,
  paymentLines,
  reverseReasonError,
  shortIban,
} from "./paymentHistory.ts";

const account = (over: Partial<PaymentAccount>): PaymentAccount => ({
  id: "acc-1",
  label: "Geschäftskonto",
  accountType: "bank",
  iban: "DE89370400440532013000",
  bic: null,
  isDefault: 1,
  active: 1,
  showOnInvoice: 1,
  details: null,
  createdAt: "2026-01-01 00:00:00",
  ...over,
});

const pay = (id: string, seq: number, amountCents: number, paidDate: string, over: Partial<InvoicePayment> = {}): InvoicePayment => ({
  id,
  invoiceId: "inv-1",
  seq,
  kind: "payment",
  amountCents,
  paidDate,
  accountId: "acc-1",
  note: null,
  reversesPaymentId: null,
  reason: null,
  createdAt: "2026-10-01 08:00:00",
  reversedById: null,
  reversible: true,
  notReversibleReason: null,
  ...over,
});

const rev = (id: string, seq: number, of: InvoicePayment, reason = "Betrag falsch erfasst"): InvoicePayment =>
  pay(id, seq, -of.amountCents, of.paidDate, {
    kind: "reversal",
    accountId: of.accountId,
    reversesPaymentId: of.id,
    reason,
    reversible: false,
  });

describe("paymentAccountId", () => {
  it("Konto der Buchung vor dem Eingangskonto der Rechnung (wie die EÜR)", () => {
    expect(paymentAccountId({ accountId: "a" }, { paidToAccountId: "b" })).toBe("a");
    expect(paymentAccountId({ accountId: null }, { paidToAccountId: "b" })).toBe("b");
    expect(paymentAccountId({ accountId: null }, { paidToAccountId: null })).toBeNull();
  });
});

describe("shortIban / accountText", () => {
  it("kürzt auf Ländercode, Prüfziffer und die letzten vier Stellen", () => {
    expect(shortIban("DE89 3704 0044 0532 0130 00")).toBe("DE89 … 3000");
    expect(shortIban(null)).toBe("");
  });

  it("nennt Bezeichnung, gekürzte IBAN und den Zustand", () => {
    const list = [account({}), account({ id: "acc-2", label: "Kasse", iban: null, active: 0 })];
    expect(accountText("acc-1", list)).toBe("Geschäftskonto (DE89 … 3000)");
    expect(accountText("acc-2", list)).toBe("Kasse, deaktiviert");
    expect(accountText(null, list)).toBe("nicht angegeben");
    expect(accountText("fehlt", list)).toBe("unbekanntes Konto");
  });
});

describe("paymentLines", () => {
  it("verbindet Zahlung und Gegenbuchung in beide Richtungen, Reihenfolge nach seq", () => {
    const a = pay("a", 1, 5000, "2026-10-01");
    const b = pay("b", 2, 2500, "2026-10-05");
    const c = rev("c", 3, b);
    const lines = paymentLines([c, a, b]);
    expect(lines.map((l) => l.entry.id)).toEqual(["a", "b", "c"]);
    expect(lines[0].reversedBy).toBeNull();
    expect(lines[1].reversedBy?.id).toBe("c");
    expect(lines[2].reverses?.id).toBe("b");
    expect(lines[2].reversedBy).toBeNull();
  });

  it("die stornierte Zahlung bleibt in der Historie stehen", () => {
    const a = pay("a", 1, 5000, "2026-10-01");
    const lines = paymentLines([a, rev("r", 2, a)]);
    expect(lines).toHaveLength(2);
    expect(lines[0].entry.amountCents).toBe(5000);
    expect(lines[1].entry.amountCents).toBe(-5000);
  });

  it("nutzt reversedById des Lesemodells, wenn der Verweis nur dort steht", () => {
    const a = pay("a", 1, 100, "2026-10-01", { reversedById: "r" });
    const r = pay("r", 2, -100, "2026-10-01", { kind: "reversal" });
    expect(paymentLines([a, r])[0].reversedBy?.id).toBe("r");
  });

  it("leer bleibt leer", () => {
    expect(paymentLines([])).toEqual([]);
  });
});

describe("effectivePayments / lastEffectivePaidDate", () => {
  it("lässt stornierte Paare weg (wie der Server)", () => {
    const a = pay("a", 1, 100, "2026-03-01");
    const b = pay("b", 2, 200, "2026-05-01");
    const list = [a, b, rev("c", 3, b)];
    expect(effectivePayments(list).map((p) => p.id)).toEqual(["a"]);
    expect(lastEffectivePaidDate(list)).toBe("2026-03-01");
  });

  it("ohne wirksame Zahlung null", () => {
    const a = pay("a", 1, 100, "2026-03-01");
    expect(lastEffectivePaidDate([a, rev("r", 2, a)])).toBeNull();
    expect(lastEffectivePaidDate([])).toBeNull();
  });

  it("späteste statt zuletzt erfasste Zahlung", () => {
    const list = [pay("a", 1, 100, "2026-06-01"), pay("b", 2, 100, "2026-04-01")];
    expect(lastEffectivePaidDate(list)).toBe("2026-06-01");
  });
});

describe("earliestStornoDate", () => {
  it("Rechnungsdatum ohne Zahlungen", () => {
    expect(earliestStornoDate("2026-02-10", [])).toBe("2026-02-10");
  });

  it("letzter Zahlungseingang, wenn er später liegt", () => {
    expect(earliestStornoDate("2026-02-10", [pay("a", 1, 100, "2026-03-15")])).toBe("2026-03-15");
  });

  it("ein storniertes spätes Zahldatum verschiebt das Storno-Datum nicht", () => {
    const a = pay("a", 1, 100, "2026-03-01");
    const wrong = pay("b", 2, 100, "2026-09-30");
    expect(earliestStornoDate("2026-02-10", [a, wrong, rev("c", 3, wrong)])).toBe("2026-03-01");
    expect(earliestStornoDate("2026-02-10", [wrong, rev("c", 2, wrong)])).toBe("2026-02-10");
  });

  it("Zeitanteil im Rechnungsdatum wird abgeschnitten", () => {
    expect(earliestStornoDate("2026-02-10 12:00:00", [])).toBe("2026-02-10");
  });
});

describe("reverseReasonError / isReasonRejection", () => {
  it("verlangt zehn Zeichen nach dem Bereinigen (wie der Server)", () => {
    expect(reverseReasonError("")).toMatch(/mindestens 10 Zeichen/);
    expect(reverseReasonError("   kurz   ")).toMatch(/mindestens 10 Zeichen/);
    expect(reverseReasonError("\u0007".repeat(10) + "abc")).toMatch(/mindestens 10 Zeichen/);
    expect(reverseReasonError("Betrag falsch erfasst")).toBeNull();
    expect(reverseReasonError("äöüßäöüßäö")).toBeNull();
  });

  it("ordnet die Mindestlängen-Ablehnung des Servers dem Feld zu", () => {
    expect(isReasonRejection("Bitte einen Grund angeben (mindestens 10 Zeichen) — er steht …")).toBe(true);
    expect(isReasonRejection("Das Geschäftsjahr 2025 ist abgeschlossen. … (§ 153 AO)")).toBe(false);
  });
});
