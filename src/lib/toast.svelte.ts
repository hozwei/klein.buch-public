// Globaler Toast-Store (Svelte 5 Runes).
//
// Toast ist das app-weite Standard-Feedback für fast alles — Erfolg UND Fehler
// einer Aktion (gespeichert, gesendet, fehlgeschlagen …). Statt pro Seite eine
// eigene flash()-Funktion + eigenen <Toast/> zu mounten, rufen alle Seiten dieses
// zentrale flash() auf; gerendert wird genau einmal über <Toast/> im Root-Layout.
//
// Persistente Zustände ("nicht gefunden", "Firmendaten fehlen") gehören NICHT
// hierher — dafür gibt es das inline <Banner/>. Folgenreiche Bestätigungen laufen
// über confirmDialog() (siehe confirm.svelte.ts).

export type ToastKind = "ok" | "error";

export interface ToastItem {
  id: number;
  message: string;
  kind: ToastKind;
}

// Mutierbares State-Objekt (gleiches Muster wie stores.svelte.ts) — Reassignments
// von Top-Level-`let` werden über Modulgrenzen nicht reaktiv getrackt, Objekt-
// Mutationen schon.
export const toastStore = $state<{ items: ToastItem[] }>({ items: [] });

let nextId = 1;
const DEFAULT_TTL_MS = 4000;
// R5-010: Stack-Cap. Bei Loop-Fehlern (Scheduler-Tick, broken-Save mehrfach
// geklickt) sonst unbegrenztes Wachstum bis zur TTL-Räumung. Wenn der Stack
// voll ist, fliegt der älteste Toast (FIFO).
const MAX_STACK = 5;

/**
 * Zeigt einen Toast oben rechts. Standard-Anzeigedauer 4 s; Fehler bleiben länger,
 * damit man sie lesen kann.
 */
export function flash(message: string, kind: ToastKind = "ok", ttlMs?: number): number {
  const id = nextId++;
  toastStore.items.push({ id, message, kind });
  // R5-010: Cap durchsetzen — älteste verwerfen statt unbegrenzt stapeln.
  while (toastStore.items.length > MAX_STACK) {
    toastStore.items.shift();
  }
  const ttl = ttlMs ?? (kind === "error" ? 7000 : DEFAULT_TTL_MS);
  if (ttl > 0) {
    setTimeout(() => dismissToast(id), ttl);
  }
  return id;
}

export function dismissToast(id: number): void {
  const i = toastStore.items.findIndex((t) => t.id === id);
  if (i !== -1) toastStore.items.splice(i, 1);
}
