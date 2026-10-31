// Wahl bei verpassten Abo-Terminen (Lauf F, ADR 0040 / W14-026).
//
// Beim Fortsetzen einer pausierten Reihe mit verpassten Terminen und bei einem
// Rückstand über der Nachhol-Grenze entscheidet der Inhaber: nachholen oder ab
// heute fortsetzen.
//
// ADR 0042 Nr. 16 (V1-11): Der Dialog nennt Anzahl und Daten der offenen
// Termine (DIN 5008) aus der Rückstand-Vorschau des Servers
// (`recurring_backlog_preview`, gerechnet wie der Lauf). Sind es mehr als die
// Grenze, bietet er „Nachholen“ nicht an — der Server lehnte es ab. Die Vorschau
// zeigt den Stand beim Öffnen; der Server prüft bei der Wahl erneut. Ohne
// Vorschau (Fehler beim Laden) bleibt der Text allgemein.

import { recurringBacklogPreview, type BacklogPreview } from "./api";
import { chooseDialog, type ConfirmChoice } from "./confirm.svelte";
import { date } from "./format";
import type { ResumeMode } from "./types";

export type RecurringKind = "invoice" | "expense";

/** Höchstzahl nachzuholender Termine nach Bestätigung (wie `catch_up::CONFIRMED_CAP`). */
export const CONFIRMED_CATCH_UP_CAP = 24;

/** So viele Daten nennt der Dialog, der Rest als „und N weitere“. */
export const SHOWN_DATES = 6;

/**
 * Liegt der nächste Termin noch in der Laufzeit der Reihe? Ohne Enddatum immer;
 * sonst nur bis einschließlich Enddatum (wie `has_backlog` im Server, F1d). Eine
 * abgelaufene Vorlage ist weder fällig noch überfällig.
 */
export function withinTerm(s: { nextDueDate: string; endDate?: string | null }): boolean {
  return !s.endDate || s.nextDueDate.slice(0, 10) <= s.endDate.slice(0, 10);
}

/**
 * Daten der offenen Termine als Text (TT.MM.JJJJ, DIN 5008): die ersten
 * `shown`, danach „und N weitere“. `total` ist die Zahl aller offenen Termine
 * (die Liste des Servers ist gekürzt).
 */
export function dueDatesText(dates: string[], total: number, shown = SHOWN_DATES): string {
  const head = dates.slice(0, shown).map((d) => date(d));
  if (head.length === 0) return "";
  const rest = Math.max(0, total - head.length);
  if (rest > 0) return `${head.join(", ")} und ${rest} weitere`;
  if (head.length === 1) return head[0];
  return `${head.slice(0, -1).join(", ")} und ${head[head.length - 1]}`;
}

/** Dialogtexte samt Zuordnung der Knöpfe. */
export interface BacklogDialog {
  title: string;
  body: string;
  confirmLabel: string;
  altLabel?: string;
  cancelLabel: string;
  /** Bestätigen heißt „nachholen“; `false`: Bestätigen ist „ab heute fortsetzen“ (über der Grenze). */
  catchUp: boolean;
  /**
   * Gefahr-Dialog: Startfokus auf „Abbrechen“, kein Abbruch per Klick auf den
   * Hintergrund. Über der Grenze lässt Bestätigen alle offenen Termine aus, das
   * lässt sich nicht zurücknehmen (ADR 0042 Nr. 16; design.md: bei folgenreichen
   * Aktionen hält „Abbrechen“ den Fokus).
   */
  danger?: boolean;
}

function termine(n: number): string {
  return n === 1 ? "1 Termin" : `${n} Termine`;
}

/**
 * Dialogtexte (rein, testbar). `afterRestore`: Prüfung nach einer
 * Wiederherstellung offen (F1d) — der Stand vor dem Restore ist unbekannt,
 * nachgeholte Rechnungen entstehen nur als Entwurf, „ab heute fortsetzen“
 * lässt auch einen heute fälligen Termin aus. Mit `preview` (Server) stehen
 * Anzahl, Daten und das Ziel von „ab heute fortsetzen“ im Text, und
 * `preview.afterRestore` gilt; über der Grenze entfällt „Nachholen“.
 */
export function backlogDialogText(
  kind: RecurringKind,
  label: string,
  nextDueIso: string,
  afterRestore = false,
  preview: BacklogPreview | null = null,
): BacklogDialog {
  const restore = preview ? preview.afterRestore : afterRestore;
  const cap = preview?.cap ?? CONFIRMED_CATCH_UP_CAP;
  const since = date(preview?.nextDueDate ?? nextDueIso);
  const title = restore ? `Nach der Wiederherstellung – „${label}“` : `Verpasste Termine – „${label}“`;

  // Was offen ist: mit Vorschau Anzahl und Daten, sonst nur der erste Termin.
  let open: string;
  if (preview && preview.due > 0) {
    const verb = preview.due === 1 ? "ist" : "sind";
    open = `${termine(preview.due)} ${verb} offen: ${dueDatesText(preview.dates, preview.due)}.`;
  } else {
    open = restore ? `Seit ${since} ist mindestens ein Termin offen.` : `Seit ${since} sind Termine offen.`;
  }
  if (restore) {
    open +=
      " Ob schon vor der Wiederherstellung abgerechnet wurde, weiß Klein.Buch nicht — bitte im " +
      "Mail-Ausgang bzw. auf dem Kontoauszug prüfen.";
  }

  // „Ab heute fortsetzen“: mit Vorschau das konkrete Ziel (wie `skip_target`).
  let skip: string;
  if (preview) {
    const left =
      preview.skipped === 0
        ? "Es wird kein Termin ausgelassen"
        : preview.skipped === 1
          ? "Ein Termin wird ausgelassen"
          : `${preview.skipped} Termine werden ausgelassen`;
    // Nach einer Wiederherstellung lässt „ab heute fortsetzen“ auch den heutigen Termin aus (F1d).
    const today = restore && preview.skipped > 0 ? " (bis einschließlich heute)" : "";
    skip = preview.skipEndsSeries
      ? `Ab heute fortsetzen: ${left}${today}. Die Laufzeit der Vorlage endet davor; es entsteht kein weiterer Termin.`
      : `Ab heute fortsetzen: ${left}${today}; weiter geht es mit dem Termin am ${date(preview.skipTo)}.`;
  } else {
    skip = restore
      ? "Ab heute fortsetzen: Die offenen Termine werden ausgelassen, auch ein heute fälliger; weiter geht es mit dem nächsten Termin."
      : "Ab heute fortsetzen: Die verpassten Termine werden ausgelassen; weiter geht es mit dem ersten Termin ab heute.";
  }

  // Über der Grenze lehnt der Server „nachholen“ ab — dann nur fortsetzen.
  if (preview?.exceedsCap) {
    const instead =
      kind === "invoice"
        ? "Vergangene Zeiträume bei Bedarf einzeln abrechnen."
        : "Vergangene Kosten bei Bedarf einzeln erfassen oder den nächsten Termin im Abo anpassen.";
    return {
      title,
      body:
        `${open}\n\nNachholen geht nicht: Das sind mehr als ${cap} Termine, so viele holt Klein.Buch nicht ` +
        `auf einmal nach. ${instead}\n\n${skip}`,
      confirmLabel: "Ab heute fortsetzen",
      cancelLabel: "Abbrechen",
      catchUp: false,
      danger: true,
    };
  }

  let catchUp: string;
  if (restore) {
    catchUp =
      kind === "invoice"
        ? "Nachholen: Der nächste Lauf erstellt die offenen Termine als Entwurf — nichts wird automatisch festgeschrieben oder versendet."
        : "Nachholen: Der nächste Lauf bucht die offenen Termine, je Termin eine Kosten-Position mit dem Stichtag als Belegdatum.";
  } else {
    catchUp =
      kind === "invoice"
        ? `Nachholen: Der nächste Lauf erstellt die offenen Termine (höchstens ${cap}). ` +
          "Nur der jüngste folgt der Automatik der Vorlage; ältere entstehen als Entwurf und werden nicht automatisch versendet."
        : `Nachholen: Der nächste Lauf bucht die offenen Termine (höchstens ${cap}), je Termin eine ` +
          "Kosten-Position mit dem Stichtag als Belegdatum. Termine in abgeschlossenen Geschäftsjahren werden übersprungen.";
  }
  return {
    title,
    body: `${open}\n\n${catchUp}\n\n${skip}`,
    confirmLabel: "Nachholen",
    altLabel: "Ab heute fortsetzen",
    cancelLabel: "Abbrechen",
    catchUp: true,
  };
}

/** Ordnet den gewählten Knopf der Wahl zu; `null` = abgebrochen (rein, testbar). */
export function backlogChoiceOf(dialog: BacklogDialog, choice: ConfirmChoice): ResumeMode | null {
  if (choice === "confirm") return dialog.catchUp ? "catchUp" : "skip";
  if (choice === "alt") return "skip";
  return null;
}

/** Die Reihe, für die gefragt wird (Abo-Rechnungs-Vorlage bzw. Kosten-Abo). */
export interface BacklogSeriesRef {
  id: string;
  label: string;
  nextDueDate: string;
}

/**
 * Lädt die Rückstand-Vorschau und fragt nach; `null` = abgebrochen. Scheitert
 * das Laden, erscheint der allgemeine Text (der Server prüft bei der Wahl).
 */
export async function askBacklogChoice(
  kind: RecurringKind,
  series: BacklogSeriesRef,
  afterRestore = false,
): Promise<ResumeMode | null> {
  let preview: BacklogPreview | null = null;
  try {
    preview = await recurringBacklogPreview(kind === "invoice" ? "template" : "subscription", series.id);
  } catch {
    preview = null;
  }
  const dialog = backlogDialogText(kind, series.label, series.nextDueDate, afterRestore, preview);
  const choice = await chooseDialog({
    title: dialog.title,
    body: dialog.body,
    confirmLabel: dialog.confirmLabel,
    altLabel: dialog.altLabel,
    cancelLabel: dialog.cancelLabel,
    danger: dialog.danger,
  });
  return backlogChoiceOf(dialog, choice);
}
