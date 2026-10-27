// U4 (Warteliste (7)): Konto je Zahlung in der Zahlungshistorie.

import { describe, expect, it } from "vitest";
import type { PaymentAccount } from "#lib/types.ts";
import { accountText, parsePaymentHistory, paymentAccountId, shortIban } from "./paymentHistory.ts";

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

describe("parsePaymentHistory", () => {
  it("leer und null ergeben eine leere Liste", () => {
    expect(parsePaymentHistory(null)).toEqual([]);
    expect(parsePaymentHistory("")).toEqual([]);
  });

  it("liest Einträge mit und ohne Konto", () => {
    const h = parsePaymentHistory(
      '[{"amountCents":5000,"paidDate":"2026-10-01","note":null,"recordedAt":"x","accountId":"acc-1"},' +
        '{"amountCents":2500,"paidDate":"2026-10-05","note":"Rest","recordedAt":"y"}]',
    );
    expect(h).toHaveLength(2);
    expect(h?.[0].accountId).toBe("acc-1");
    expect(h?.[1].accountId).toBeUndefined();
  });

  it("beschädigtes JSON oder kein Array ergibt null", () => {
    expect(parsePaymentHistory("{kaputt")).toBeNull();
    expect(parsePaymentHistory('{"amountCents":1}')).toBeNull();
  });
});

describe("paymentAccountId", () => {
  const entry = { amountCents: 1, paidDate: "2026-10-01", note: null, recordedAt: "x" };
  it("Konto des Eintrags vor dem Eingangskonto der Rechnung (wie die EÜR)", () => {
    expect(paymentAccountId({ ...entry, accountId: "a" }, { paidToAccountId: "b" })).toBe("a");
    expect(paymentAccountId(entry, { paidToAccountId: "b" })).toBe("b");
    expect(paymentAccountId(entry, { paidToAccountId: null })).toBeNull();
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
