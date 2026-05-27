<script lang="ts">
  // Globaler Toast-Host. Genau einmal im Root-Layout gemountet; rendert den Stack
  // oben rechts aus dem zentralen toastStore. Aufgerufen wird ausschließlich über
  // flash() aus $lib/toast.svelte.ts — Seiten mounten kein eigenes <Toast/> mehr.
  // Toasts verschwinden automatisch (siehe TTL in flash()).
  //
  // Dateiname bewusst ToastHost (nicht Toast) — sonst Casing-Kollision mit dem
  // Store-Modul toast.svelte.ts auf case-insensitiven Dateisystemen (Windows).
  import { toastStore, dismissToast } from "$lib/toast.svelte";
</script>

<!-- R5-009: pointer-events:auto auf jedem Toast (Stack-Wrapper bleibt
     none — durchklickbar zwischen den Toasts) + Close-Button. Bisher waren
     Toasts undismissbar.
     R5-A11Y-005: aria-live an role anpassen — `role="alert"` impliziert
     `assertive` (für Fehler), `role="status"` impliziert `polite` (für ok).
     Doppel-Deklaration mit explizitem `aria-live` widerspricht sich. -->
<div class="toast-stack">
  {#each toastStore.items as t (t.id)}
    <div class="toast {t.kind}" role={t.kind === "error" ? "alert" : "status"}>
      <span class="msg">{t.message}</span>
      <button
        type="button"
        class="close"
        aria-label="Hinweis schließen"
        onclick={() => dismissToast(t.id)}
      >×</button>
    </div>
  {/each}
</div>

<style>
  .toast-stack {
    position: fixed;
    /* Unterhalb der Sticky-PageBar (oben rechts), damit Toasts nie die
       Aktions-Buttons in der Leiste überdecken. */
    top: 4.25rem;
    right: var(--main-pad, 2rem);
    z-index: 1500;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    max-width: 28rem;
    /* Stack-Wrapper bleibt durchklickbar — sonst würden Klicks auf die
       darunterliegenden Aktions-Buttons der Seite zwischen Toast-Zeilen
       hängen bleiben. Einzelne Toasts haben unten pointer-events:auto. */
    pointer-events: none;
  }
  .toast {
    /* R5-009: einzelne Toasts klickbar (Close-Button braucht das). */
    pointer-events: auto;
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
    padding: 0.7rem 1.1rem;
    border-radius: 6px;
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.18);
    font-size: 0.95rem;
    animation: slide-in 0.18s ease-out;
  }
  .toast .msg {
    flex: 1;
  }
  .toast .close {
    background: transparent;
    border: 0;
    font-size: 1.25rem;
    line-height: 1;
    padding: 0 0.25rem;
    margin: -0.15rem -0.4rem -0.15rem 0;
    color: inherit;
    opacity: 0.65;
    cursor: pointer;
  }
  .toast .close:hover {
    opacity: 1;
  }
  .toast.ok {
    background: #d1fae5;
    color: #065f46;
    border: 1px solid #6ee7b7;
  }
  .toast.error {
    background: #fee2e2;
    color: #991b1b;
    border: 1px solid #fca5a5;
  }
  @keyframes slide-in {
    from { transform: translateY(-8px); opacity: 0; }
    to { transform: translateY(0); opacity: 1; }
  }
</style>
