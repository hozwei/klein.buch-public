// Wahl bei verpassten Abo-Terminen (Lauf F, ADR 0040 / W14-026).
//
// Beim Fortsetzen einer pausierten Reihe mit verpassten Terminen und bei einem
// Rückstand über der Nachhol-Grenze entscheidet der Inhaber: nachholen oder ab
// heute fortsetzen. Die Gestaltung des Dialogs übernimmt der UI/UX-Lauf; hier
// steht nur die fachlich nötige Wahl.

import { chooseDialog } from "./confirm.svelte";
import { date } from "./format";
import type { ResumeMode } from "./types";

export type RecurringKind = "invoice" | "expense";

/** Höchstzahl nachzuholender Termine nach Bestätigung (wie `catch_up::CONFIRMED_CAP`). */
export const CONFIRMED_CATCH_UP_CAP = 24;

/**
 * Liegt der nächste Termin noch in der Laufzeit der Reihe? Ohne Enddatum immer;
 * sonst nur bis einschließlich Enddatum (wie `has_backlog` im Server, F1d). Eine
 * abgelaufene Vorlage ist weder fällig noch überfällig.
 */
export function withinTerm(s: { nextDueDate: string; endDate?: string | null }): boolean {
  return !s.endDate || s.nextDueDate.slice(0, 10) <= s.endDate.slice(0, 10);
}

/**
 * Dialogtexte (rein, testbar). `afterRestore`: Prüfung nach einer
 * Wiederherstellung offen (F1d) — der Stand vor dem Restore ist unbekannt,
 * nachgeholte Rechnungen entstehen nur als Entwurf, „ab heute fortsetzen“
 * lässt auch einen heute fälligen Termin aus.
 */
export function backlogDialogText(
  kind: RecurringKind,
  label: string,
  nextDueIso: string,
  afterRestore = false,
) {
  const since = date(nextDueIso);
  if (afterRestore) {
    const catchUp =
      kind === "invoice"
        ? "Nachholen: Der nächste Lauf erstellt die offenen Termine als Entwurf — nichts wird automatisch festgeschrieben oder versendet."
        : "Nachholen: Der nächste Lauf bucht die offenen Termine, je Termin eine Kosten-Position mit dem Stichtag als Belegdatum.";
    return {
      title: `Nach der Wiederherstellung – „${label}“`,
      body:
        `Seit ${since} ist mindestens ein Termin offen. Ob er vor der Wiederherstellung schon ` +
        "abgerechnet wurde, weiß Klein.Buch nicht — bitte im Mail-Ausgang bzw. auf dem Kontoauszug prüfen.\n\n" +
        `${catchUp}\n\n` +
        "Ab heute fortsetzen: Die offenen Termine werden ausgelassen, auch ein heute fälliger; weiter geht es mit dem nächsten Termin.",
      confirmLabel: "Nachholen",
      altLabel: "Ab heute fortsetzen",
      cancelLabel: "Abbrechen",
    };
  }
  const catchUp =
    kind === "invoice"
      ? `Nachholen: Der nächste Lauf erstellt die offenen Termine (höchstens ${CONFIRMED_CATCH_UP_CAP}). ` +
        "Nur der jüngste folgt der Automatik der Vorlage; ältere entstehen als Entwurf und werden nicht automatisch versendet."
      : `Nachholen: Der nächste Lauf bucht die offenen Termine (höchstens ${CONFIRMED_CATCH_UP_CAP}), je Termin eine ` +
        "Kosten-Position mit dem Stichtag als Belegdatum. Termine in abgeschlossenen Geschäftsjahren werden übersprungen.";
  return {
    title: `Verpasste Termine – „${label}“`,
    body:
      `Seit ${since} sind Termine offen.\n\n${catchUp}\n\n` +
      "Ab heute fortsetzen: Die verpassten Termine werden ausgelassen; weiter geht es mit dem ersten Termin ab heute.",
    confirmLabel: "Nachholen",
    altLabel: "Ab heute fortsetzen",
    cancelLabel: "Abbrechen",
  };
}

/** Fragt nach; `null` = abgebrochen. */
export async function askBacklogChoice(
  kind: RecurringKind,
  label: string,
  nextDueIso: string,
  afterRestore = false,
): Promise<ResumeMode | null> {
  const choice = await chooseDialog(backlogDialogText(kind, label, nextDueIso, afterRestore));
  if (choice === "confirm") return "catchUp";
  if (choice === "alt") return "skip";
  return null;
}
