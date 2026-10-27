<script lang="ts">
  // Flächen-Container (DS-1): weiße Karte mit Haarlinie, --radius-card (12 px) und
  // flachem Schatten (U1: kein weicher Halo mehr). Look aus tokens.css (.card).
  //
  // Regel (design.md §4.4): Karten nur, wo sie gruppieren — ein Abschnitt mit
  // Überschrift (`h2` als erstes Kind), ein Formularteil, eine Liste. Keine Karte
  // in der Karte; Listen stehen in `Table` (eigene Karte), nicht zusätzlich in Card.
  // `danger`: Abschnitt mit Gefahraktionen (Anonymisieren, Passphrase ändern).
  import type { Snippet } from "svelte";

  let {
    padded = true,
    danger = false,
    class: className = "",
    children,
  }: {
    padded?: boolean;
    danger?: boolean;
    /** Zusatzklassen, z. B. `kb-span-all` im Formular-Raster. */
    class?: string;
    children?: Snippet;
  } = $props();
</script>

<div class="card {className}" class:padded class:danger>
  {@render children?.()}
</div>

<style>
  /* Basis-Look der Karte kommt aus tokens.css (.card, .card.danger).
     Hier nur die Komponenten-spezifische Variante: nicht-padded (raw)
     für Demo-Zwecke (siehe routes/design/+page.svelte). */
  .card:not(.padded) { padding: 0; }
</style>
