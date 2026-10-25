// Lauf F5-fix: Tageswechsel nach einem Ruhezustand mit sichtbarem Tab (D-27).
// Der Mitternachts-Timer läuft nach dem Aufwachen auf monotoner Zeit weiter;
// Fokus, `pageshow` und der Minutentakt holen den neuen Tag sofort nach.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { todayBerlin } from "./date";

async function load(start: string) {
  vi.setSystemTime(new Date(start));
  const win = new EventTarget();
  const doc = Object.assign(new EventTarget(), { visibilityState: "visible" });
  vi.stubGlobal("window", win);
  vi.stubGlobal("document", doc);
  const mod = await import("./dayClock.svelte");
  return { ...mod, win, doc };
}

describe("dayClock", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it("zeigt nach dem Aufwachen beim Fokus den neuen Tag (auch Jahreswechsel)", async () => {
    const { dayClock, win } = await load("2026-12-31T22:00:00Z"); // 23:00 Berlin
    expect(todayBerlin(dayClock.now)).toBe("2026-12-31");
    // Ruhezustand bis 08:00 Berlin: Wanduhr springt, kein Timer ist gelaufen.
    vi.setSystemTime(new Date("2027-01-01T07:00:00Z"));
    expect(todayBerlin(dayClock.now)).toBe("2026-12-31");
    win.dispatchEvent(new Event("focus"));
    expect(todayBerlin(dayClock.now)).toBe("2027-01-01");
  });

  it("pageshow holt den neuen Tag nach", async () => {
    const { dayClock, win } = await load("2026-10-25T21:00:00Z");
    expect(todayBerlin(dayClock.now)).toBe("2026-10-25");
    vi.setSystemTime(new Date("2026-10-26T06:00:00Z"));
    win.dispatchEvent(new Event("pageshow"));
    expect(todayBerlin(dayClock.now)).toBe("2026-10-26");
  });

  it("der Minutentakt erkennt den Tageswechsel ohne Ereignis", async () => {
    const { dayClock, DAY_CHECK_INTERVAL_MS } = await load("2026-10-25T21:00:00Z");
    const before = dayClock.now;
    vi.setSystemTime(new Date("2026-10-26T06:00:00Z"));
    vi.advanceTimersByTime(DAY_CHECK_INTERVAL_MS);
    expect(todayBerlin(dayClock.now)).toBe("2026-10-26");
    // Innerhalb desselben Tages bleibt der Zeitpunkt stabil (keine Neuberechnung).
    const same = dayClock.now;
    vi.advanceTimersByTime(DAY_CHECK_INTERVAL_MS * 5);
    expect(dayClock.now).toBe(same);
    expect(same).not.toBe(before);
  });

  it("der Mitternachts-Timer setzt den Tag ohne Ruhezustand", async () => {
    const { dayClock } = await load("2026-10-25T22:59:00Z"); // 23:59 Berlin (MEZ)
    expect(todayBerlin(dayClock.now)).toBe("2026-10-25");
    vi.advanceTimersByTime(61_000);
    expect(todayBerlin(dayClock.now)).toBe("2026-10-26");
  });
});
