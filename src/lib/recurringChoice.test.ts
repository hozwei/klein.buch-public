// Lauf F1d: Laufzeit-Prüfung der Abo-Listen und Dialogtext nach einer
// Wiederherstellung.

import { describe, expect, it } from "vitest";
import { backlogChoiceOf, backlogDialogText, dueDatesText, withinTerm } from "./recurringChoice";
import type { BacklogPreview } from "./api";

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

// ADR 0042 Nr. 16 (V1-11): Anzahl, Daten und Grenze aus der Rückstand-Vorschau.

function preview(over: Partial<BacklogPreview> = {}): BacklogPreview {
  return {
    nextDueDate: "2026-07-15",
    backlog: true,
    due: 3,
    dates: ["2026-07-15", "2026-08-15", "2026-09-15"],
    cap: 24,
    exceedsCap: false,
    afterRestore: false,
    skipTo: "2026-11-15",
    skipped: 4,
    skipEndsSeries: false,
    ...over,
  };
}

describe("dueDatesText", () => {
  it("Daten nach DIN 5008, verbunden mit „und“", () => {
    expect(dueDatesText(["2026-07-15"], 1)).toBe("15.07.2026");
    expect(dueDatesText(["2026-07-15", "2026-08-15", "2026-09-15"], 3)).toBe(
      "15.07.2026, 15.08.2026 und 15.09.2026",
    );
  });

  it("gekürzt mit „und N weitere“ (auch über die gelieferte Liste hinaus)", () => {
    const dates = Array.from({ length: 24 }, (_, i) => `2024-${String((i % 12) + 1).padStart(2, "0")}-01`);
    expect(dueDatesText(dates, 30, 2)).toBe("01.01.2024, 01.02.2024 und 28 weitere");
    expect(dueDatesText([], 0)).toBe("");
  });
});

describe("backlogDialogText mit Vorschau", () => {
  it("nennt Anzahl, Daten und das Ziel von „ab heute fortsetzen“", () => {
    const t = backlogDialogText("invoice", "Wartung", "2026-07-15", false, preview());
    expect(t.body).toContain("3 Termine sind offen: 15.07.2026, 15.08.2026 und 15.09.2026.");
    expect(t.body).toContain("4 Termine werden ausgelassen");
    expect(t.body).toContain("Termin am 15.11.2026");
    expect(t.confirmLabel).toBe("Nachholen");
    expect(t.altLabel).toBe("Ab heute fortsetzen");
    expect(t.catchUp).toBe(true);
  });

  it("ein einzelner Termin in der Einzahl", () => {
    const t = backlogDialogText("expense", "Hosting", "2026-10-01", false, preview({ due: 1, dates: ["2026-10-01"], skipped: 1 }));
    expect(t.body).toContain("1 Termin ist offen: 01.10.2026.");
    expect(t.body).toContain("Ein Termin wird ausgelassen");
  });

  it("über der Grenze: kein „Nachholen“, Bestätigen heißt fortsetzen", () => {
    const dates = Array.from({ length: 24 }, (_, i) => `2024-01-${String(i + 1).padStart(2, "0")}`);
    const t = backlogDialogText("invoice", "Wartung", "2024-01-01", false, preview({ due: 30, dates, exceedsCap: true, skipped: 30 }));
    expect(t.body).toContain("30 Termine sind offen");
    expect(t.body).toContain("und 24 weitere");
    expect(t.body).toContain("mehr als 24 Termine");
    expect(t.confirmLabel).toBe("Ab heute fortsetzen");
    expect(t.altLabel).toBeUndefined();
    expect(t.catchUp).toBe(false);
    // Gefahr-Dialog: Startfokus auf „Abbrechen“, Enter lässt nichts aus (V4-R-03).
    expect(t.danger).toBe(true);
    expect(backlogChoiceOf(t, "confirm")).toBe("skip");
  });

  it("Laufzeit-Ende: „ab heute fortsetzen“ beendet die Vorlage", () => {
    const t = backlogDialogText("invoice", "Wartung", "2026-07-15", false, preview({ skipEndsSeries: true }));
    expect(t.body).toContain("Die Laufzeit der Vorlage endet davor");
  });

  it("Prüfung nach Wiederherstellung kommt aus der Vorschau", () => {
    const t = backlogDialogText("invoice", "Wartung", "2026-10-31", false, preview({ afterRestore: true, due: 1, dates: ["2026-10-31"], skipped: 1 }));
    expect(t.title).toContain("Nach der Wiederherstellung");
    expect(t.body).toContain("als Entwurf");
    expect(t.body).toContain("bis einschließlich heute");
  });
});

describe("backlogChoiceOf", () => {
  it("ordnet die Knöpfe der Wahl zu", () => {
    const t = backlogDialogText("invoice", "Wartung", "2026-07-15", false, preview());
    expect(backlogChoiceOf(t, "confirm")).toBe("catchUp");
    expect(t.danger).toBeUndefined();
    expect(backlogChoiceOf(t, "alt")).toBe("skip");
    expect(backlogChoiceOf(t, null)).toBeNull();
  });
});
