// Reaktives „heute“ (Lauf F5, D-27): Ein über Mitternacht offener Tab zeigte
// den Vortag, weil Seiten `new Date()` einmal beim Laden lasen. `dayClock.now`
// ist ein Svelte-State, der an der Berliner Mitternacht (plus einer Sekunde)
// neu gesetzt wird; Ableitungen wie `todayBerlin(dayClock.now)` in `$derived`
// folgen ihm.
//
// Timer laufen nach einem Ruhezustand des Geräts je nach Browser und
// Betriebssystem auf monotoner Zeit weiter (ohne die Schlafzeit) und werden in
// Hintergrund-Tabs gedrosselt. Deshalb prüft zusätzlich ein grober Takt (jede
// Minute) und jedes Zurückkehren in den Tab (`visibilitychange`, `focus`,
// `pageshow`) das Berliner Datum (F5-fix). Der Vergleich ist billig; der State
// ändert sich nur beim Tageswechsel.

import { msUntilNextBerlinMidnight, todayBerlin } from "./date";

/** Abstand des groben Takts, der den Tageswechsel nach einem Ruhezustand erkennt. */
export const DAY_CHECK_INTERVAL_MS = 60_000;

const state = $state({ now: new Date() });
let started = false;
let timer: ReturnType<typeof setTimeout> | undefined;

/** Setzt `now` neu, wenn der Berliner Kalendertag gewechselt hat. */
function check(): void {
  const now = new Date();
  if (todayBerlin(now) !== todayBerlin(state.now)) {
    state.now = now;
    schedule();
  }
}

function schedule(): void {
  if (timer !== undefined) clearTimeout(timer);
  timer = setTimeout(
    () => {
      timer = undefined;
      check();
      // Zu früh gefeuert (Uhr verstellt): neu planen.
      if (timer === undefined) schedule();
    },
    msUntilNextBerlinMidnight(new Date()) + 1_000,
  );
}

function start(): void {
  if (started || typeof window === "undefined") return;
  started = true;
  schedule();
  setInterval(check, DAY_CHECK_INTERVAL_MS);
  window.addEventListener("focus", check);
  window.addEventListener("pageshow", check);
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "visible") check();
  });
}

export const dayClock = {
  /** Zeitpunkt, der sich mit jedem neuen Berliner Kalendertag ändert. */
  get now(): Date {
    start();
    return state.now;
  },
};
