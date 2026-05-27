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
// ausstellen). Der §19→Regelbesteuerung-Warn-Dialog in settings/seller bleibt ein
// eigener, bewusst gestalteter Modal (5-Jahres-Bindung) und nutzt dies NICHT.

export interface ConfirmOptions {
  title: string;
  /** Fließtext; \n wird als Zeilenumbruch dargestellt. */
  body?: string;
  /** Optionale Aufzählungspunkte (z. B. Konsequenzen beim Ausstellen). */
  bullets?: string[];
  confirmLabel?: string;
  cancelLabel?: string;
  /** Rot eingefärbter Bestätigen-Button für destruktive/einschneidende Aktionen. */
  danger?: boolean;
}

interface ActiveConfirm extends ConfirmOptions {
  resolve: (ok: boolean) => void;
}

export const confirmStore = $state<{ current: ActiveConfirm | null }>({ current: null });

/** Öffnet den Dialog und löst mit true (Bestätigen) bzw. false (Abbrechen) auf. */
export function confirmDialog(opts: ConfirmOptions): Promise<boolean> {
  // Ein bereits offener Dialog wird abgebrochen, bevor ein neuer erscheint.
  if (confirmStore.current) confirmStore.current.resolve(false);
  return new Promise<boolean>((resolve) => {
    confirmStore.current = { ...opts, resolve };
  });
}

/** Vom <ConfirmDialog/> aufgerufen, wenn der Nutzer entscheidet. */
export function settleConfirm(ok: boolean): void {
  const c = confirmStore.current;
  confirmStore.current = null;
  c?.resolve(ok);
}
