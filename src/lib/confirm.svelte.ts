// Globaler Bestätigungs-Dialog (Svelte 5 Runes).
//
// Ersetzt das browser-eigene confirm() durch einen einheitlichen In-App-Dialog.
// Promise-basiert, damit Aufrufer wie früher schreiben können:
//
//   if (!(await confirmDialog({ title: "…", danger: true }))) return;
//
// Gerendert wird genau einmal über <ConfirmDialog/> im Root-Layout.
//
// Abgrenzung: Dies ist der generische Bestätigungs-Dialog für folgenreiche, aber
// gewöhnliche Aktionen (archivieren, stornieren, löschen, wiederherstellen,
// ausstellen). Der Dialog „Statuswechsel eintragen“ in settings/seller (Steuerstatus,
// ADR 0042) ist ein eigener Gefahr-Dialog mit Formular und nutzt dies NICHT.

export interface ConfirmOptions {
  title: string;
  /** Fließtext; \n wird als Zeilenumbruch dargestellt. */
  body?: string;
  /** Optionale Aufzählungspunkte (z. B. Konsequenzen beim Ausstellen). */
  bullets?: string[];
  confirmLabel?: string;
  cancelLabel?: string;
  /**
   * Optionaler zweiter Wahl-Knopf (z. B. „Ab heute fortsetzen“ neben
   * „Nachholen“). Nur mit {@link chooseDialog} sinnvoll.
   */
  altLabel?: string;
  /** Rot eingefärbter Bestätigen-Button für destruktive/einschneidende Aktionen. */
  danger?: boolean;
}

/** Ergebnis von {@link chooseDialog}: Bestätigen, zweite Wahl oder Abbrechen (null). */
export type ConfirmChoice = "confirm" | "alt" | null;

interface ActiveConfirm extends ConfirmOptions {
  resolve: (choice: ConfirmChoice) => void;
}

export const confirmStore = $state<{ current: ActiveConfirm | null }>({ current: null });

/** Öffnet den Dialog und löst mit true (Bestätigen) bzw. false (Abbrechen) auf. */
export function confirmDialog(opts: ConfirmOptions): Promise<boolean> {
  return chooseDialog({ ...opts, altLabel: undefined }).then((c) => c === "confirm");
}

/**
 * Dialog mit bis zu drei Knöpfen (Abbrechen, `altLabel`, `confirmLabel`).
 * Escape und (nur ohne `danger`) Klick auf den Hintergrund brechen ab (null) —
 * nie die zweite Wahl.
 */
export function chooseDialog(opts: ConfirmOptions): Promise<ConfirmChoice> {
  // Ein bereits offener Dialog wird abgebrochen, bevor ein neuer erscheint.
  if (confirmStore.current) confirmStore.current.resolve(null);
  return new Promise<ConfirmChoice>((resolve) => {
    confirmStore.current = { ...opts, resolve };
  });
}

/** Vom <ConfirmDialog/> aufgerufen, wenn der Nutzer entscheidet. */
export function settleConfirm(ok: boolean): void {
  settleChoice(ok ? "confirm" : null);
}

/** Vom <ConfirmDialog/> aufgerufen (auch für die zweite Wahl). */
export function settleChoice(choice: ConfirmChoice): void {
  const c = confirmStore.current;
  confirmStore.current = null;
  c?.resolve(choice);
}
