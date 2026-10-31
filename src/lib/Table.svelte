<script lang="ts">
  // Dünner Tabellen-Wrapper (DS-1): umschließt die Tabelle in einer Karte.
  // Inhalt (<thead>/<tbody>) kommt als Slot; nutzt die globale .kb-table aus tokens.css.
  //
  // U1: Breite Tabellen scrollen in sich (1.4.10), nie die Seite.
  // - `caption`: Überschrift für Screenreader (sichtbar nur, wenn `captionVisible`).
  // - `cards`: unter 820 px als Karten (tokens.css `table.cards`); dann braucht
  //   jede <td> ein `data-label` (Aufgabe der Seite).
  // - `cardGrid` (U2): Listen mit Detailseite stellen die Karten auf dem Tablet hoch
  //   (601–820 px) zweispaltig (tokens.css `.kb-card-grid`).
  // U4 (design.md §4.5, U3-030): Listen stehen immer in diesem Baustein (weiße
  // Karte mit Lineatur), nie frei auf dem Seitenhintergrund und nicht zusätzlich in
  // einer Card. Belegnummern in Zellen über DocNo, Beträge mit `.kb-num`.
  // V3-12 (ADR 0042 Nr. 16): In der Kartenansicht tragen `td.kb-card-head` (Nummer bzw.
  // Name), `td.kb-card-status` (Badge neben dem Kopf) und `td.kb-card-figure`
  // (Hauptbetrag) die Karte; `td.kb-card-omit` blendet leere Nebenangaben („—“) aus.
  // Die Klassen setzt die Seite an die Zellen (tokens.css, design.md §4.5).
  // U4 (V2-02/V4-07): `busy` setzt `aria-busy` am Bereich, solange er nachlädt
  // (z. B. neuer Filter); die Statusmeldung steht daneben (LoadingState).
  import type { Snippet } from "svelte";

  let {
    caption,
    captionVisible = false,
    cards = false,
    cardGrid = false,
    busy = false,
    children,
  }: {
    caption?: string;
    captionVisible?: boolean;
    cards?: boolean;
    cardGrid?: boolean;
    /** Lädt gerade nach (aria-busy am Bereich). */
    busy?: boolean;
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
  aria-busy={busy || undefined}
>
  <table class="kb-table" class:cards class:kb-card-grid={cards && cardGrid}>
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
    border: var(--rule);
    border-radius: var(--radius-card);
    box-shadow: var(--shadow-card);
    overflow-x: auto;
    max-width: 100%;
  }
  caption {
    text-align: left;
    font-weight: 600;
    padding: var(--sp-3) var(--sp-3) 0;
  }
  @media screen and (max-width: 820px) {
    /* Als Karten: kein umgebender Kasten, die Zeilen sind selbst Karten. */
    .table-wrap.as-cards {
      background: transparent;
      border: 0;
      box-shadow: none;
      overflow: visible;
    }
  }
</style>
