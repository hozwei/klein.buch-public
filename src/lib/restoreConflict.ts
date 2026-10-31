// Restore-Vormerkung eines Entwurfs in der Oberfläche (ADR 0042 Nr. 16,
// Warteliste 8). Reine Funktionen, eine Quelle für Editor und Detailseite von
// Rechnung und Angebot.
//
// Der Server liefert am offenen Entwurf `restoreConflict`:
// - `flagged`: Die Nummer war vor der Wiederherstellung schon festgeschrieben;
//   Ausstellen bzw. Fertigstellen erst nach dem Abgleich (Einstellungen → Backups).
// - `confirmed`: Abgleich bestätigt; das nächste Speichern merkt den Entwurf
//   wieder vor (Lauf F8). Darauf weist der Editor VOR dem Speichern hin.

import type { RestoreConflictState } from "./types";

export type RestoreDoc = "invoice" | "quote";

/** Verb des Festschreibens je Belegart: Rechnung „ausstellen“, Angebot „fertigstellen“. */
export function finalizeVerb(doc: RestoreDoc): { inf: string; noun: string; you: string } {
  return doc === "invoice"
    ? { inf: "ausstellen", noun: "Ausstellen", you: "ausstellst" }
    : { inf: "fertigstellen", noun: "Fertigstellen", you: "fertigstellst" };
}

/**
 * Rückmeldung nach dem Speichern eines Entwurfs. War der Abgleich bestätigt,
 * ist der Entwurf jetzt wieder vorgemerkt (Server, `reflag_confirmed_in`) —
 * das sagt der Toast; sonst die normale Bestätigung.
 */
export function restoreSaveFlash(
  before: RestoreConflictState | null | undefined,
  doc: RestoreDoc,
  editing: boolean,
): { msg: string; kind: "ok" | "warning" } {
  if (editing && before === "confirmed") {
    return {
      msg:
        "Entwurf geändert. Er ist nach der Wiederherstellung wieder vorgemerkt — bestätige den Abgleich " +
        `vor dem ${finalizeVerb(doc).noun} erneut (Einstellungen → Backups).`,
      kind: "warning",
    };
  }
  return { msg: editing ? "Entwurf geändert." : "Entwurf gespeichert.", kind: "ok" };
}
