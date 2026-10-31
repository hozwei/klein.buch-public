// Restore-Vormerkung: Rückmeldung nach dem Speichern (ADR 0042 Nr. 16, Warteliste 8).

import { describe, expect, it } from "vitest";
import { finalizeVerb, restoreSaveFlash } from "./restoreConflict";

describe("restoreSaveFlash", () => {
  it("bestätigter Entwurf: nach dem Speichern wieder vorgemerkt (Warnung)", () => {
    const r = restoreSaveFlash("confirmed", "quote", true);
    expect(r.kind).toBe("warning");
    expect(r.msg).toContain("wieder vorgemerkt");
    expect(r.msg).toContain("vor dem Fertigstellen");
    expect(restoreSaveFlash("confirmed", "invoice", true).msg).toContain("vor dem Ausstellen");
  });

  it("sonst die normale Bestätigung", () => {
    expect(restoreSaveFlash("flagged", "invoice", true)).toEqual({ msg: "Entwurf geändert.", kind: "ok" });
    expect(restoreSaveFlash(null, "quote", true)).toEqual({ msg: "Entwurf geändert.", kind: "ok" });
    expect(restoreSaveFlash(null, "quote", false)).toEqual({ msg: "Entwurf gespeichert.", kind: "ok" });
  });
});

describe("finalizeVerb", () => {
  it("Rechnung ausstellen, Angebot fertigstellen", () => {
    expect(finalizeVerb("invoice").inf).toBe("ausstellen");
    expect(finalizeVerb("quote").you).toBe("fertigstellst");
  });
});
