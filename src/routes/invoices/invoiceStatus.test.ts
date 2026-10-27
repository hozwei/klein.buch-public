// U4 (V2-03, V1-03): Status einer Rechnung in Liste und Detail, Zerlegen von
// Ablehnungen beim Ausstellen.

import { describe, expect, it } from "vitest";
import {
  invoiceStatusView,
  isOverdue,
  issueReasons,
  lastSentAt,
  reasonTarget,
  reasonTargets,
  statusOf,
} from "./invoiceStatus.ts";

const inv = (over: Partial<Parameters<typeof invoiceStatusView>[0]> = {}) => ({
  status: "issued",
  voidedAt: null,
  isStornoFor: null,
  dueDate: "2026-10-20",
  grossAmountCents: 10_000,
  paidAmountCents: 0,
  ...over,
});

describe("isOverdue / invoiceStatusView", () => {
  it("offen und Fälligkeit vor heute: Überfällig (danger)", () => {
    expect(isOverdue(inv(), "2026-10-21")).toBe(true);
    expect(invoiceStatusView(inv({ status: "sent" }), "2026-10-21")).toEqual({
      label: "Überfällig",
      tone: "danger",
      strike: false,
    });
  });

  it("am Fälligkeitstag noch nicht überfällig (Berliner Tag)", () => {
    expect(isOverdue(inv(), "2026-10-20")).toBe(false);
    expect(invoiceStatusView(inv(), "2026-10-20").label).toBe("Ausgestellt");
  });

  it("Zeitstempel als Fälligkeit zählt mit dem Datumsteil", () => {
    expect(isOverdue(inv({ dueDate: "2026-10-20T00:00:00Z" }), "2026-10-21")).toBe(true);
  });

  it("bezahlt, storniert, Entwurf, Storno-Beleg und ohne Fälligkeit nie überfällig", () => {
    expect(isOverdue(inv({ status: "paid", paidAmountCents: 10_000 }), "2026-12-01")).toBe(false);
    expect(isOverdue(inv({ status: "canceled" }), "2026-12-01")).toBe(false);
    expect(isOverdue(inv({ status: "draft" }), "2026-12-01")).toBe(false);
    expect(isOverdue(inv({ isStornoFor: "x", grossAmountCents: -10_000 }), "2026-12-01")).toBe(false);
    expect(isOverdue(inv({ dueDate: null }), "2026-12-01")).toBe(false);
  });

  it("Teilbezahlt mit Restbetrag nach der Fälligkeit: Überfällig", () => {
    expect(
      invoiceStatusView(inv({ status: "partially_paid", paidAmountCents: 4_000 }), "2026-11-01").label,
    ).toBe("Überfällig");
    expect(
      invoiceStatusView(inv({ status: "partially_paid", paidAmountCents: 4_000 }), "2026-10-01"),
    ).toEqual({ label: "Teilbezahlt", tone: "warning", strike: false });
  });

  it("verworfen und storniert neutral und durchgestrichen", () => {
    expect(invoiceStatusView(inv({ status: "draft", voidedAt: "2026-10-01 10:00:00" }), "2026-12-01")).toEqual({
      label: "Verworfen",
      tone: "neutral",
      strike: true,
    });
    expect(statusOf("canceled")).toEqual({ label: "Storniert", tone: "neutral", strike: true });
  });
});

describe("issueReasons", () => {
  it("altes Präfix des Servers", () => {
    expect(issueReasons("Validation failed (2 Fehler): Telefonnummer fehlt.; Fälligkeitsdatum fehlt.")).toEqual([
      "Telefonnummer fehlt.",
      "Fälligkeitsdatum fehlt.",
    ]);
  });

  it("neues Präfix für Rechnung und Angebot", () => {
    expect(issueReasons("Die Rechnung kann nicht ausgestellt werden: A; B")).toEqual(["A", "B"]);
    expect(issueReasons("Das Angebot kann nicht fertiggestellt werden: Nur ein Grund.")).toEqual([
      "Nur ein Grund.",
    ]);
  });

  it("andere Meldungen sind keine Prüf-Ablehnung", () => {
    expect(issueReasons("Server nicht erreichbar")).toBeNull();
    expect(issueReasons("Das Geschäftsjahr 2025 ist abgeschlossen.")).toBeNull();
  });
});

describe("reasonTarget", () => {
  it("mit Regel-Code eindeutig", () => {
    expect(reasonTarget("…", "XRechnungSellerPhoneMissing")).toBe("seller");
    expect(reasonTarget("…", "BuyerAddressIncomplete")).toBe("buyer");
    expect(reasonTarget("…", "XRechnungDueDateMissing")).toBe("draft");
  });

  it("ohne Code am Text", () => {
    expect(reasonTarget("Telefonnummer fehlt in den Firmendaten.")).toBe("seller");
    expect(reasonTarget("Empfänger-Adresse ist unvollständig (Straße, PLZ, Ort).")).toBe("buyer");
    expect(reasonTarget("Position 1: Beschreibung fehlt.")).toBe("draft");
  });

  it("Ziele in fester Reihenfolge, ohne Doppelte", () => {
    expect(
      reasonTargets([{ text: "Position 1: Beschreibung fehlt." }, { text: "Verkäufer-Name fehlt." }, { text: "x" }]),
    ).toEqual(["seller", "draft"]);
  });
});

describe("lastSentAt", () => {
  it("jüngster erfolgreicher Versand", () => {
    expect(
      lastSentAt([
        { status: "success", createdAt: "2026-10-01 09:00:00" },
        { status: "failed", createdAt: "2026-10-03 09:00:00" },
        { status: "success", createdAt: "2026-10-02 09:00:00" },
      ]),
    ).toBe("2026-10-02 09:00:00");
    expect(lastSentAt([{ status: "failed", createdAt: "2026-10-03 09:00:00" }])).toBeNull();
  });
});
