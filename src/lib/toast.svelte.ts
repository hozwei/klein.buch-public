// Globaler Toast-Store (Svelte 5 Runes).
//
// Toast ist das app-weite Standard-Feedback für Aktionen (gespeichert,
// gesendet, fehlgeschlagen …). Alle Seiten rufen dieses zentrale flash() auf;
// gerendert wird genau einmal über <ToastHost/> im Root-Layout.
//
// Persistente Zustände ("nicht gefunden", "Firmendaten fehlen") gehören NICHT
// hierher — dafür gibt es das inline <Banner/>. Folgenreiche Bestätigungen laufen
// über confirmDialog() (siehe confirm.svelte.ts).
//
// Arten (U4, Warteliste (19)): "ok" (Erfolg), "info" (neutral, z. B. „Abgebrochen –
// nichts geändert.“), "warning" (Warnung: Aktion ist gelungen, aber etwas braucht
// Aufmerksamkeit, z. B. „E-Mail-Protokoll unvollständig“), "error" (Fehler).
// Warnungen stehen wie Fehler, werden aber höflich angesagt (`role="status"`),
// Fehler sofort (`role="alert"`). „info“ schließt wie Erfolg nach 5 s.
//
// Re-Auth abgebrochen (U4, V1-09): Der Transport wirft dann `REAUTH_CANCELED`.
// Seiten melden Fehler meist mit `flash(String(e), "error")`, oft mit Präfix
// („Speichern fehlgeschlagen: …“); ein Abbruch ist aber kein Fehler. `flash`
// macht daraus zentral einen neutralen Hinweis, statt jede Seite anzupassen.
//
// Zeitbegrenzung (WCAG 2.2.1, Lauf 2026.11/U1):
// - Fehler, Warnungen und Toasts mit Aktion schließen NICHT automatisch — nur über
//   „Schließen“ (oder die Aktion).
// - Erfolgsmeldungen verschwinden nach 5 s; solange Maus oder Fokus auf
//   dem Toast liegt, steht die Uhr (pauseToast/resumeToast aus ToastHost).
//
// Sperre/Sitzungsablauf (U7, R2-02): `clearToasts()` verwirft alle Toasts samt
// Aktionen, sobald die App den Zustand „bereit“ verlässt (Layout). Meldungen mit
// Inhalten (Abo-Namen, Belegnummern) stünden sonst unter der Anmelde-Maske, und ein
// „Rückgängig“ ließe sich nach dem erneuten Anmelden beliebig spät auslösen.
//
// Fokus (U7, R3-04): `flash()` merkt sich das Element, das beim Erscheinen den
// Fokus hatte (`toastOrigin`); ToastHost führt den Fokus nach Schließen oder
// Aktion dorthin zurück statt auf <body>.

import { REAUTH_CANCELED } from "#lib/transport.ts";

export type ToastKind = "ok" | "info" | "warning" | "error";

export interface FlashOptions {
  /** Knopf im Toast, z. B. „Rückgängig“ (der Toast bleibt dann stehen). */
  action?: ToastAction;
  /** Nur Erfolg/Info ohne Aktion: Anzeigedauer in ms (0 = bleibt stehen). */
  ttlMs?: number;
}

export interface ToastAction {
  label: string;
  run: () => void;
}

export interface ToastItem {
  id: number;
  message: string;
  kind: ToastKind;
  /** Optionaler Knopf im Toast (z. B. „Rückgängig“, „Öffnen“). */
  action?: ToastAction;
  /** Schließt nicht von selbst. */
  persistent: boolean;
}

// Mutierbares State-Objekt (gleiches Muster wie stores.svelte.ts) — Reassignments
// von Top-Level-`let` werden über Modulgrenzen nicht reaktiv getrackt, Objekt-
// Mutationen schon.
export const toastStore = $state<{ items: ToastItem[] }>({ items: [] });

let nextId = 1;
const DEFAULT_TTL_MS = 5000;
// R5-010: Stack-Cap. Wenn der Stack voll ist, fliegt der älteste Toast (FIFO).
const MAX_STACK = 5;

// Restlaufzeit je Toast (außerhalb des reaktiven Zustands).
const timers = new Map<number, { handle: ReturnType<typeof setTimeout> | null; remaining: number; startedAt: number }>();
// Fokus beim Erscheinen je Toast (außerhalb des reaktiven Zustands, U7 R3-04).
const origins = new Map<number, HTMLElement>();

function activeElement(): HTMLElement | null {
  if (typeof document === "undefined") return null;
  const el = document.activeElement;
  return el instanceof HTMLElement && el !== document.body ? el : null;
}

function startTimer(id: number, ms: number): void {
  const t = { handle: null as ReturnType<typeof setTimeout> | null, remaining: ms, startedAt: Date.now() };
  t.handle = setTimeout(() => dismissToast(id), ms);
  timers.set(id, t);
}

/**
 * Zeigt einen Toast. Erfolg und Info: 5 s (pausiert bei Hover/Fokus). Fehler,
 * Warnungen und Toasts mit Aktion bleiben, bis sie geschlossen werden. `ttlMs`
 * verkürzt/verlängert nur Erfolgs-/Info-Meldungen ohne Aktion (0 = bleibt stehen).
 *
 * Dritter Parameter: Dauer in ms oder Optionen (U4, für „Rückgängig“):
 * `flash("Kontakt archiviert.", "ok", { action: { label: "Rückgängig", run } })`.
 */
export function flash(
  message: string,
  kind: ToastKind = "ok",
  ttlOrOpts?: number | FlashOptions,
  opts?: FlashOptions,
): number {
  const o: FlashOptions = typeof ttlOrOpts === "object" ? ttlOrOpts : { ...opts, ttlMs: ttlOrOpts ?? opts?.ttlMs };
  const ttlMs = o.ttlMs;
  // Abgebrochene Re-Auth ist kein Fehler (V1-09): neutraler Hinweis ohne Präfix.
  if (kind === "error" && message.includes(REAUTH_CANCELED)) {
    message = REAUTH_CANCELED;
    kind = "info";
  }
  const id = nextId++;
  const persistent = kind === "warning" || kind === "error" || !!o.action || ttlMs === 0;
  toastStore.items.push({ id, message, kind, action: o.action, persistent });
  // Liegt der Fokus schon in einem Toast, gilt dessen Ursprung.
  const active = activeElement();
  const origin = active?.closest(".toast-stack") ? lastOrigin() : active;
  if (origin) origins.set(id, origin);
  while (toastStore.items.length > MAX_STACK) {
    const old = toastStore.items.shift();
    if (old) clearTimer(old.id);
  }
  if (!persistent) startTimer(id, ttlMs ?? DEFAULT_TTL_MS);
  return id;
}

function lastOrigin(): HTMLElement | undefined {
  const ids = [...origins.keys()];
  return ids.length ? origins.get(ids[ids.length - 1]) : undefined;
}

function clearTimer(id: number): void {
  const t = timers.get(id);
  if (t?.handle) clearTimeout(t.handle);
  timers.delete(id);
  origins.delete(id);
}

/** Element, das beim Erscheinen des Toasts den Fokus hatte (falls noch bekannt). */
export function toastOrigin(id: number): HTMLElement | null {
  return origins.get(id) ?? null;
}

/** Alle Toasts samt Aktionen und Uhren verwerfen (Sperre, Sitzungsablauf; R2-02). */
export function clearToasts(): void {
  for (const id of [...timers.keys()]) clearTimer(id);
  origins.clear();
  toastStore.items.splice(0);
}

export function dismissToast(id: number): void {
  clearTimer(id);
  const i = toastStore.items.findIndex((t) => t.id === id);
  if (i !== -1) toastStore.items.splice(i, 1);
}

/** Uhr anhalten (Maus/Fokus auf dem Toast). */
export function pauseToast(id: number): void {
  const t = timers.get(id);
  if (!t || t.handle === null) return;
  clearTimeout(t.handle);
  t.handle = null;
  t.remaining = Math.max(0, t.remaining - (Date.now() - t.startedAt));
}

/** Uhr weiterlaufen lassen; mindestens 2 s, damit man zu Ende lesen kann. */
export function resumeToast(id: number): void {
  const t = timers.get(id);
  if (!t || t.handle !== null) return;
  t.startedAt = Date.now();
  t.remaining = Math.max(t.remaining, 2000);
  t.handle = setTimeout(() => dismissToast(id), t.remaining);
}
