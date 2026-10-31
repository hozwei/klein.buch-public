// Angebots-Status in der Oberfläche und Bearbeitbarkeit (ADR 0042 Nr. 13).

import { describe, expect, it } from "vitest";
import { canEditQuote, quoteStatusView } from "./quoteStatus";

describe("canEditQuote", () => {
  it("nur ein offener Entwurf ist bearbeitbar", () => {
    expect(canEditQuote({ status: "draft", lockedAt: null })).toBe(true);
    expect(canEditQuote({ status: "draft" })).toBe(true);
  });

  it("fertiggestellt, angenommen, abgelehnt, umgewandelt, verworfen: nein", () => {
    const at = "2026-10-01T10:00:00Z";
    expect(canEditQuote({ status: "sent", lockedAt: at })).toBe(false);
    expect(canEditQuote({ status: "accepted", lockedAt: at })).toBe(false);
    expect(canEditQuote({ status: "rejected", lockedAt: at })).toBe(false);
    expect(canEditQuote({ status: "converted", lockedAt: at })).toBe(false);
    expect(canEditQuote({ status: "canceled", lockedAt: null })).toBe(false);
    // Einen festgeschriebenen Entwurf gibt es nicht; trotzdem gesperrt.
    expect(canEditQuote({ status: "draft", lockedAt: at })).toBe(false);
  });
});

describe("quoteStatusView", () => {
  it("verworfen ohne lockedAt, storniert mit", () => {
    expect(quoteStatusView({ status: "canceled", lockedAt: null }).label).toBe("Verworfen");
    expect(quoteStatusView({ status: "canceled", lockedAt: "2026-10-01T10:00:00Z" }).label).toBe("Storniert");
  });
});
