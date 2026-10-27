// U4 (Befund E): Einheiten außerhalb der UN/ECE-Liste werden am Feld gemeldet,
// mit einem Vorschlag aus der Liste, wo einer passt.
import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  isFormalUnitCode,
  isKnownUnit,
  REAUTH_ACTION_KEYS,
  reauthActionLabel,
  suggestUnit,
  unitCodeError,
  unitOptions,
} from "./labels.ts";

describe("Einheiten", () => {
  it("kennt die Codes der Liste, keinen Freitext", () => {
    expect(isKnownUnit("LS")).toBe(true);
    expect(isKnownUnit("HUR")).toBe(true);
    expect(isKnownUnit("Pauschal")).toBe(false);
    expect(isKnownUnit("hur")).toBe(false);
    expect(isKnownUnit("")).toBe(false);
    expect(isKnownUnit(null)).toBe(false);
  });

  it("schlägt zu Freitext eine Einheit vor", () => {
    expect(suggestUnit("Pauschal")?.label).toBe("Pauschale");
    expect(suggestUnit("Std.")?.label).toBe("Stunde");
    expect(suggestUnit("Stunden")?.label).toBe("Stunde");
    expect(suggestUnit("Stk.")?.label).toBe("Stück");
    expect(suggestUnit("Stueck")?.label).toBe("Stück");
    expect(suggestUnit("Monat")?.value).toBe("MON");
    expect(suggestUnit("hur")?.value).toBe("HUR");
    expect(suggestUnit("m")?.value).toBe("MTR");
    expect(suggestUnit("xyz")).toBeNull();
    expect(suggestUnit("h")).toBeNull();
    expect(suggestUnit("")).toBeNull();
  });

  it("meldet ungültige Einheiten mit Vorschlag", () => {
    expect(unitCodeError("C62")).toBe("");
    expect(unitCodeError("Pauschal")).toBe(
      "„Pauschal“ ist keine Einheit für die E-Rechnung — bitte aus der Liste wählen, z. B. Pauschale.",
    );
    expect(unitCodeError("xyz")).toBe("„xyz“ ist keine Einheit für die E-Rechnung — bitte aus der Liste wählen.");
    expect(unitCodeError("")).toBe("Bitte eine Einheit aus der Liste wählen.");
    expect(unitCodeError("Pauschal", true)).toMatch(/Pauschale\. Das löst die Position vom Paket\.$/);
  });

  // U7 (R1-03): formal gültige Rec-20/21-Codes außerhalb der Liste sperren nicht.
  it("lässt formal gültige Codes außerhalb der Liste zu", () => {
    for (const c of ["H87", "E48", "P1", "XPP", "KWH"]) {
      expect(isFormalUnitCode(c)).toBe(true);
      expect(unitCodeError(c)).toBe("");
      expect(unitCodeError(c, true)).toBe("");
    }
    for (const c of ["Pauschal", "h87", "Std", "H", "ABCD", "H-7", " H87"]) {
      expect(isFormalUnitCode(c)).toBe(false);
      expect(unitCodeError(c)).not.toBe("");
    }
    expect(unitOptions("H87").at(-1)).toEqual({ value: "H87", label: "H87 (Code, nicht in der Liste)" });
  });

  it("behält einen unbekannten Code als Option", () => {
    const opts = unitOptions("Pauschal");
    expect(opts.at(-1)).toEqual({ value: "Pauschal", label: "Pauschal" });
    expect(unitOptions("LS").filter((o) => o.value === "LS")).toHaveLength(1);
  });
});

// U4 (V1-09): Der Re-Auth-Dialog nennt die Aktion. Jeder Command, der laut
// Registry eine frische Bestätigung verlangen kann, hat einen Namen.
describe("Re-Auth: Name der Aktion", () => {
  it("deckt jede Re-Auth-Aktion der Registry ab", () => {
    const here = path.dirname(fileURLToPath(import.meta.url));
    const registry = readFileSync(
      path.join(here, "..", "..", "src-tauri", "src", "commands", "registry.rs"),
      "utf8",
    );
    const start = registry.indexOf("fn auth_of(name: &str) -> Auth {");
    const end = registry.indexOf("_ => Auth::Session", start);
    expect(start).toBeGreaterThan(0);
    expect(end).toBeGreaterThan(start);
    const names = [...registry.slice(start, end).matchAll(/"([a-z0-9_]+)"/g)].map((m) => m[1]);
    expect(names.length).toBeGreaterThan(20);
    for (const n of names) expect(REAUTH_ACTION_KEYS, n).toContain(n);
  });

  it("liest Command-Adressen und Auth-Pfade", () => {
    expect(reauthActionLabel("/api/cmd/euer_export_elster")).toBe(
      "ELSTER-Ausfüllhilfe (CSV) herunterladen",
    );
    expect(reauthActionLabel("/api/auth/passphrase")).toBe("Daten-Passwort ändern");
    expect(reauthActionLabel("/api/cmd/contacts_list")).toBeNull();
  });
});
