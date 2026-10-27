// Lauf F1d: Laufzeit-Prüfung der Abo-Listen und Dialogtext nach einer
// Wiederherstellung.

import { describe, expect, it } from "vitest";
import { backlogDialogText, withinTerm } from "./recurringChoice";

describe("withinTerm", () => {
  it("ohne Enddatum immer in der Laufzeit", () => {
    expect(withinTerm({ nextDueDate: "2026-10-01", endDate: null })).toBe(true);
    expect(withinTerm({ nextDueDate: "2026-10-01" })).toBe(true);
  });

  it("bis einschließlich Enddatum, danach beendet (wie has_backlog im Server)", () => {
    expect(withinTerm({ nextDueDate: "2026-09-30", endDate: "2026-09-30" })).toBe(true);
    expect(withinTerm({ nextDueDate: "2026-10-01", endDate: "2026-09-30" })).toBe(false);
  });
});

describe("backlogDialogText", () => {
  it("nach einer Wiederherstellung: nur Entwürfe, heutiger Termin wird ausgelassen", () => {
    const t = backlogDialogText("invoice", "Wartung", "2026-10-24", true);
    expect(t.title).toContain("Nach der Wiederherstellung");
    expect(t.body).toContain("als Entwurf");
    expect(t.body).toContain("auch ein heute fälliger");
    expect(t.confirmLabel).toBe("Nachholen");
    expect(t.altLabel).toBe("Ab heute fortsetzen");
  });

  it("ohne Wiederherstellung unverändert", () => {
    const t = backlogDialogText("expense", "Hosting", "2026-10-01");
    expect(t.title).toContain("Verpasste Termine");
    expect(t.body).toContain("höchstens 24");
  });
});
