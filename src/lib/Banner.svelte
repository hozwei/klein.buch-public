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
  .banner.error {
    background: var(--c-danger-50);
    color: var(--c-danger-700);
    border-color: #efc6c9;
  }
  .banner.warning {
    background: var(--c-warning-50);
    color: var(--c-warning-700);
    border-color: #f3dcae;
  }
  .banner.info {
    background: var(--c-info-50);
    color: var(--c-info-700);
    border-color: #cfe0e6;
  }
</style>
