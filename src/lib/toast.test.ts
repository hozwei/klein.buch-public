// U4 (Warteliste (19)): Toast-Art „Warnung“ bleibt stehen wie ein Fehler (2.2.1);
// Erfolgsmeldungen verschwinden nach 5 s.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

async function load() {
  return await import("./toast.svelte");
}

describe("flash", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("Erfolg schließt nach 5 s", async () => {
    const { flash, toastStore } = await load();
    flash("Gespeichert.");
    expect(toastStore.items).toHaveLength(1);
    vi.advanceTimersByTime(5000);
    expect(toastStore.items).toHaveLength(0);
  });

  it("Warnung und Fehler bleiben stehen, bis sie geschlossen werden", async () => {
    const { flash, toastStore, dismissToast } = await load();
    const w = flash("E-Mail-Protokoll unvollständig.", "warning");
    flash("Fehlgeschlagen.", "error");
    vi.advanceTimersByTime(60_000);
    expect(toastStore.items.map((t) => [t.kind, t.persistent])).toEqual([
      ["warning", true],
      ["error", true],
    ]);
    dismissToast(w);
    expect(toastStore.items.map((t) => t.kind)).toEqual(["error"]);
  });

  // U4 (V2-12): „Rückgängig“ über Optionen als dritten Parameter; der Toast bleibt stehen.
  it("Aktion als dritter Parameter, Toast bleibt stehen", async () => {
    const { flash, toastStore } = await load();
    let undone = 0;
    flash("Kontakt archiviert.", "ok", { action: { label: "Rückgängig", run: () => undone++ } });
    vi.advanceTimersByTime(60_000);
    expect(toastStore.items).toHaveLength(1);
    expect(toastStore.items[0].action?.label).toBe("Rückgängig");
    toastStore.items[0].action?.run();
    expect(undone).toBe(1);
  });

  // U4 (V1-09): Abgebrochene Re-Auth ist kein Fehler — neutraler Hinweis, schließt selbst.
  it("abgebrochene Bestätigung wird zum neutralen Hinweis", async () => {
    const { flash, toastStore } = await load();
    const { REAUTH_CANCELED } = await import("./transport");
    flash("Speichern fehlgeschlagen: " + REAUTH_CANCELED, "error");
    expect(toastStore.items.map((t) => [t.kind, t.message, t.persistent])).toEqual([
      ["info", REAUTH_CANCELED, false],
    ]);
    vi.advanceTimersByTime(5000);
    expect(toastStore.items).toHaveLength(0);
  });
});
