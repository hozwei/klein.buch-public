<script lang="ts">
  // Inline-Banner für legitim PERSISTENTE Zustände, die kein flüchtiger Toast sein
  // dürfen: "… nicht gefunden", "Firmendaten fehlen", Versions-Inkompatibilität.
  // Bleibt im Seitenfluss stehen, bis der Zustand behoben ist.
  //
  // Für flüchtiges Aktions-Feedback (gespeichert, gesendet, fehlgeschlagen) gilt
  // weiterhin flash() / Toast — NICHT dieses Banner.
  import type { Snippet } from "svelte";

  let {
    kind = "error",
    children,
  }: {
    kind?: "error" | "warning" | "info";
    children?: Snippet;
  } = $props();
</script>

<div class="banner {kind}" role={kind === "error" ? "alert" : "status"}>
  {@render children?.()}
</div>

<style>
  .banner {
    padding: 0.7rem 1rem;
    border-radius: var(--r-md);
    border: 1px solid transparent;
    font-size: 0.92rem;
    margin: 0.5rem 0 1rem;
    line-height: 1.45;
  }
  /* Rollen-Tokens; Kante links betont, Status nie nur über Farbe (Text trägt ihn). */
  .banner { border-left-width: 4px; overflow-wrap: anywhere; }
  .banner.error {
    background: var(--status-danger-bg);
    color: var(--status-danger-fg);
    border-color: var(--status-danger-line);
  }
  .banner.warning {
    background: var(--status-warning-bg);
    color: var(--status-warning-fg);
    border-color: var(--status-warning-line);
  }
  .banner.info {
    background: var(--status-info-bg);
    color: var(--status-info-fg);
    border-color: var(--status-info-line);
  }
</style>
