// V8a-R-01: Die Einleitung der Dialoge im Abschnitt „Steuerstatus“ ist zugleich
// `aria-describedby` des alertdialog (Screenreader lesen sie beim Öffnen vor). Ein
// Ausdruck ohne geschweifte Klammern stünde dort als Klartext; weder `pnpm check`
// noch die übrigen Tests fänden das. Statische Prüfung des Quelltextes.

import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const DIR = path.dirname(fileURLToPath(import.meta.url));
const SRC = readFileSync(path.join(DIR, "TaxStatusSection.svelte"), "utf8");

/** Klartext eines Absatzes `<p id="…">`: ohne Kommentare, Svelte-Blöcke und Ausdrücke. */
function leadText(id: string): string {
  const m = SRC.match(new RegExp(`<p id="${id}">([\\s\\S]*?)</p>`));
  expect(m, `Absatz #${id}`).toBeTruthy();
  let s = m![1].replace(/<!--[\s\S]*?-->/g, " ");
  // Verschachtelte {…} von innen nach außen entfernen.
  for (let prev = ""; prev !== s; ) {
    prev = s;
    s = s.replace(/\{[^{}]*\}/g, " ");
  }
  return s.replace(/<[^>]+>/g, " ").replace(/\s+/g, " ").trim();
}

describe("Einleitung der Steuerstatus-Dialoge", () => {
  for (const id of ["ts-lead", "ts-revoke-lead"]) {
    it(`#${id} enthält keinen Code als Klartext`, () => {
      const t = leadText(id);
      expect(t).not.toMatch(/REASONS|revokeEntry|\?\?|\?\.|\[|\]|[{}]/);
      expect(t).toMatch(/^[A-ZÄÖÜ„]/);
    });
  }

  it("#ts-lead nennt die Wirkung des Eintrags", () => {
    expect(leadText("ts-lead")).toMatch(
      /^Der Eintrag ändert, ob deine Belege ab dem Wirkungsdatum Umsatzsteuer ausweisen\. Er bleibt dauerhaft im Verlauf/,
    );
  });
});
