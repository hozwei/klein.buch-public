// U4 (U1-061): Belegnummern in Titeln werden als eigenes Element markiert.
import { describe, expect, it } from "vitest";
import { splitDocNumbers } from "./DocNo.svelte";

describe("splitDocNumbers", () => {
  it("markiert Belegnummern aller Nummernkreise", () => {
    expect(splitDocNumbers("Entwurf RE-2026-0001 bearbeiten")).toEqual([
      { text: "Entwurf ", docNo: false },
      { text: "RE-2026-0001", docNo: true },
      { text: " bearbeiten", docNo: false },
    ]);
    for (const nr of ["AN-2026-0003", "ST-2026-0001", "KO-2025-0123", "PV-2026-0002", "AV-2026-0010", "RE-2026-12345"]) {
      expect(splitDocNumbers(nr)).toEqual([{ text: nr, docNo: true }]);
    }
  });
  it("lässt anderen Text unverändert", () => {
    expect(splitDocNumbers("Neue Rechnung")).toEqual([{ text: "Neue Rechnung", docNo: false }]);
    expect(splitDocNumbers("XRE-2026-0001")).toEqual([{ text: "XRE-2026-0001", docNo: false }]);
    expect(splitDocNumbers("")).toEqual([]);
  });
  it("findet mehrere Nummern und gibt den Text vollständig wieder", () => {
    const t = "Storno ST-2026-0001 zu RE-2026-0004";
    const parts = splitDocNumbers(t);
    expect(parts.filter((p) => p.docNo).map((p) => p.text)).toEqual(["ST-2026-0001", "RE-2026-0004"]);
    expect(parts.map((p) => p.text).join("")).toBe(t);
  });
});
