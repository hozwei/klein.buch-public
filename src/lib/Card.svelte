<script lang="ts">
  // Flächen-Container (DS-1): weiße Karte mit Rahmen, Radius und weichem Schatten.
  import type { Snippet } from "svelte";

  let {
    padded = true,
    children,
  }: {
    padded?: boolean;
    children?: Snippet;
  } = $props();
</script>

<div class="card" class:padded>
  {@render children?.()}
</div>

<style>
  /* Basis-Look der Karte kommt aus tokens.css (.card, G2-UX.3.4).
     Hier nur die Komponenten-spezifische Variante: nicht-padded (raw)
     für Demo-Zwecke (siehe routes/design/+page.svelte). */
  .card:not(.padded) { padding: 0; }
</style>
