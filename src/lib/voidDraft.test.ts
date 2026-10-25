// Lauf F3b: Begründung beim Verwerfen eines Rechnungs-Entwurfs (wie der Server).

import { describe, expect, it } from "vitest";
import { VOID_REASON_MIN_CHARS, voidReasonError } from "./voidDraft";

describe("voidReasonError", () => {
  it("verlangt mindestens zehn Zeichen nach dem Trimmen", () => {
    expect(VOID_REASON_MIN_CHARS).toBe(10);
    expect(voidReasonError("")).not.toBeNull();
    expect(voidReasonError("   kurz    ")).not.toBeNull();
    expect(voidReasonError("Doppelt angelegt")).toBeNull();
  });

  it("zählt Zeichen statt Bytes und ignoriert Steuerzeichen", () => {
    expect(voidReasonError("Äöüß-Grund")).toBeNull();
    expect(voidReasonError("Grund\u0000\u0001\u0002ok")).not.toBeNull();
  });
});
