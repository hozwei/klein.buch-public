<script lang="ts">
  // Dünner Tabellen-Wrapper (DS-1): umschließt die Tabelle in einer Karte.
  // Inhalt (<thead>/<tbody>) kommt als Slot; nutzt die globale .kb-table aus tokens.css.
  //
  // U1: Breite Tabellen scrollen in sich (1.4.10), nie die Seite.
  // - `caption`: Überschrift für Screenreader (sichtbar nur, wenn `captionVisible`).
  // - `cards`: unter 820 px als Karten (tokens.css `table.cards`); dann braucht
  //   jede <td> ein `data-label` (Aufgabe der Seite).
  import type { Snippet } from "svelte";

  let {
    caption,
    captionVisible = false,
    cards = false,
    children,
  }: {
    caption?: string;
    captionVisible?: boolean;
    cards?: boolean;
    children?: Snippet;
  } = $props();
</script>

<!-- Der Scroll-Container ist per Tastatur erreichbar, wenn er benannt ist
     (axe scrollable-region-focusable). -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
  class="table-wrap"
  class:as-cards={cards}
  role={caption ? "region" : undefined}
  aria-label={caption}
  tabindex={caption ? 0 : undefined}
>
  <table class="kb-table" class:cards>
    {#if caption}
      <caption class:kb-visually-hidden={!captionVisible}>{caption}</caption>
    {/if}
    {@render children?.()}
  </table>
</div>

<style>
  .table-wrap {
    position: relative;
    background: var(--surface-card);
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    box-shadow: var(--sh-sm);
    overflow-x: auto;
    max-width: 100%;
  }
  caption {
    text-align: left;
    font-weight: 600;
    padding: var(--sp-3) var(--sp-3) 0;
  }
  @media (max-width: 820px) {
    /* Als Karten: kein umgebender Kasten, die Zeilen sind selbst Karten. */
    .table-wrap.as-cards {
      background: transparent;
      border: 0;
      box-shadow: none;
      overflow: visible;
    }
  }
</style>
