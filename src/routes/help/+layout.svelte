<script module lang="ts">
  import type { HandbookCategory } from "$lib/handbook/categories";

  // G2-DOC.3.4 — Modul-weiter Open/Collapse-State des Sidebar-Kapitelbaums.
  //
  // Bewusst auf Modul-Ebene: bleibt erhalten, solange das JS-Modul lebt,
  // also über (Un-)Mounts des `/help`-Layouts hinweg — z. B. wenn der
  // Nutzer kurz nach /invoices wechselt und zurückkommt. Kein
  // `localStorage` (PRD §G2-DOC.3.4: „in-memory, kein Storage-Pflicht").
  //
  // Default-Politik (siehe `ensureDefaults`): die Kategorie der aktiven
  // Seite ist offen, alle anderen kollabiert. Sobald der Nutzer eine
  // Kategorie manuell aufklappt, bleibt das Modul auf seiner Wahl.
  const openByCategory = $state<Record<HandbookCategory, boolean>>({
    "erste-schritte": false,
    bedienen: false,
    "recht-und-steuern": false,
    faq: false,
    troubleshooting: false,
    glossar: false,
  });
  // `touchedByCategory` merkt, welche Sektionen der Nutzer schon mal
  // bewusst gesetzt hat. Nur unberührte Sektionen folgen dem Default
  // („aktive Kategorie offen"); berührte bleiben so, wie er sie zuletzt
  // gesehen hat.
  const touchedByCategory = $state<Record<HandbookCategory, boolean>>({
    "erste-schritte": false,
    bedienen: false,
    "recht-und-steuern": false,
    faq: false,
    troubleshooting: false,
    glossar: false,
  });

  export function isCategoryOpen(cat: HandbookCategory): boolean {
    return openByCategory[cat];
  }
  export function toggleCategory(cat: HandbookCategory): void {
    openByCategory[cat] = !openByCategory[cat];
    touchedByCategory[cat] = true;
  }
  /**
   * Default-Politik anwenden: alle Kategorien, die der Nutzer noch nicht
   * angefasst hat, werden synchron zur aktiven Kategorie gesetzt (aktive
   * = offen, Rest = zu). Wird vom Layout bei jedem Slug-Wechsel aufgerufen.
   */
  export function ensureDefaults(active: HandbookCategory | null): void {
    for (const cat of Object.keys(openByCategory) as HandbookCategory[]) {
      if (touchedByCategory[cat]) continue;
      openByCategory[cat] = active !== null && cat === active;
    }
  }
</script>

<script lang="ts">
  // G2-DOC.3.1 + 3.2 + 3.3 + 3.4 — Innenlayout des /help-Bereichs.
  //
  // Drei Spalten INNERHALB der App-`<main>`-Section: Sidebar-Kapitelbaum links,
  // Inhalt mittig, TOC-Anker rechts. Die App-Sidebar (Haupt-Navigation)
  // liegt außerhalb und bleibt sichtbar.
  //
  // TOC kommt von der jeweiligen Slug-Seite (`[slug]/+page.ts` legt
  // `toc` ins `data`). Die /help-Index-Route hat keinen TOC — dann fällt
  // die rechte Spalte auf einen schmalen Stub zurück.
  //
  // Sidebar-Kopf trägt seit 3.3 das Suchfeld: bei `query.length >= 2`
  // ersetzen die Treffer den Kapitelbaum, bei leerer Query erscheint
  // wieder der Baum. Suche ist 120 ms debounced; Esc leert das Feld;
  // ein Klick auf einen Treffer navigiert auf `/help/<slug>` und
  // resettet die Eingabe, damit man beim Zurücknavigieren wieder den
  // Kapitelbaum sieht.
  //
  // 3.4 ergänzt:
  // - Kollabierbare Kategorien (Default: nur die Kategorie der aktiven
  //   Seite ist offen). Der Open-State liegt modul-weit (siehe
  //   `<script module>` oben) und überlebt Layout-Remount.
  // - Druck-Stylesheet (`@media print`): nur der Inhalt wird gedruckt,
  //   Sidebar und TOC werden ausgeblendet.

  import { goto } from "$app/navigation";
  import { page } from "$app/stores";
  import { listByCategory, getEntry } from "$lib/handbook";
  // `HandbookCategory` ist über das `<script module>` oben bereits im Scope —
  // Svelte 5 teilt Identifier zwischen Modul- und Instanz-Skript. Erneuter
  // Import würde svelte-check mit „Duplicate identifier" abbrechen.
  import type { TocEntry } from "$lib/handbook/render";
  import {
    searchHandbook,
    SEARCH_MIN_QUERY_LENGTH,
    type HandbookHit,
  } from "$lib/handbook/search";

  const groups = listByCategory();
  let { children } = $props();

  // TOC ist Page-spezifisch (Slug-Loader). Defensive Lesart, damit das
  // Layout auch auf der Index-Route (`/help` → Redirect) nicht stolpert.
  let toc = $derived<TocEntry[]>(
    ($page.data as { toc?: TocEntry[] }).toc ?? [],
  );

  function isActive(pathname: string, slug: string): boolean {
    return pathname === `/help/${slug}`;
  }

  /** Aktive Kategorie aus dem aktuellen `/help/<slug>`-Pfad ableiten. */
  function activeCategory(pathname: string): HandbookCategory | null {
    const m = pathname.match(/^\/help\/([^/?#]+)/);
    if (!m) return null;
    const entry = getEntry(m[1]);
    return entry ? entry.category : null;
  }

  // Default-Politik bei jedem Pfadwechsel anwenden (unberührte Sektionen
  // werden zur aktiven Kategorie umgeschaltet).
  $effect(() => {
    ensureDefaults(activeCategory($page.url.pathname));
  });

  // --- Suche ---------------------------------------------------------------

  let query = $state("");
  let hits = $state<HandbookHit[]>([]);
  let debounceHandle: ReturnType<typeof setTimeout> | null = null;

  function runSearch(q: string): void {
    hits = searchHandbook(q);
  }

  function onQueryInput(): void {
    if (debounceHandle !== null) clearTimeout(debounceHandle);
    const snapshot = query;
    debounceHandle = setTimeout(() => {
      debounceHandle = null;
      runSearch(snapshot);
    }, 120);
  }

  function clearQuery(): void {
    if (debounceHandle !== null) {
      clearTimeout(debounceHandle);
      debounceHandle = null;
    }
    query = "";
    hits = [];
  }

  function onQueryKeydown(ev: KeyboardEvent): void {
    if (ev.key === "Escape") {
      ev.preventDefault();
      clearQuery();
    }
  }

  async function openHit(slug: string): Promise<void> {
    // Erst navigieren, dann Query leeren — sonst flackert die Sidebar
    // zwischen Treffern und Kapitelbaum, bevor die neue Seite gerendert
    // wird.
    await goto(`/help/${slug}`);
    clearQuery();
  }

  let showResults = $derived(query.trim().length >= SEARCH_MIN_QUERY_LENGTH);
  let noResults = $derived(showResults && hits.length === 0);
</script>

<div class="help-shell">
  <aside class="help-sidebar" aria-label="Handbuch-Kapitelbaum">
    <a class="back" href="/" aria-label="Zurück zur App">← Zur App</a>
    <h2 class="help-title">Handbuch</h2>

    <div class="search">
      <label class="visually-hidden" for="handbook-search">
        Im Handbuch suchen
      </label>
      <input
        id="handbook-search"
        type="search"
        class="search-input"
        placeholder="Suchen …"
        autocomplete="off"
        spellcheck="false"
        bind:value={query}
        oninput={onQueryInput}
        onkeydown={onQueryKeydown}
      />
      {#if query.length > 0}
        <button
          type="button"
          class="search-clear"
          aria-label="Suche leeren"
          onclick={clearQuery}>×</button
        >
      {/if}
    </div>

    {#if showResults}
      <div class="results" role="region" aria-label="Suchergebnisse">
        {#if noResults}
          <p class="results-empty">
            Keine Treffer für „{query.trim()}".
          </p>
        {:else}
          <p class="results-count">
            {hits.length} Treffer
          </p>
          <ul class="results-list">
            {#each hits as hit (hit.slug)}
              <li>
                <button
                  type="button"
                  class="result"
                  class:active={isActive($page.url.pathname, hit.slug)}
                  onclick={() => openHit(hit.slug)}
                >
                  <span class="result-title">{hit.title}</span>
                  <span class="result-cat">{hit.categoryLabel}</span>
                  <span class="result-snippet">{@html hit.snippet}</span>
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    {:else}
      <nav>
        {#each groups as group (group.category)}
          {#if group.entries.length > 0}
            <section class="cat" class:open={isCategoryOpen(group.category)}>
              <button
                type="button"
                class="cat-header"
                aria-expanded={isCategoryOpen(group.category)}
                aria-controls="cat-list-{group.category}"
                onclick={() => toggleCategory(group.category)}
              >
                <span class="cat-arrow" aria-hidden="true">▸</span>
                <span class="cat-label">{group.label}</span>
                <span class="cat-count" aria-label="Anzahl Kapitel"
                  >{group.entries.length}</span
                >
              </button>
              {#if isCategoryOpen(group.category)}
                <ul id="cat-list-{group.category}">
                  {#each group.entries as entry (entry.slug)}
                    <li
                      class:active={isActive($page.url.pathname, entry.slug)}
                    >
                      <a href="/help/{entry.slug}">{entry.title}</a>
                    </li>
                  {/each}
                </ul>
              {/if}
            </section>
          {/if}
        {/each}
      </nav>
    {/if}
  </aside>

  <article class="help-content">
    {@render children?.()}
  </article>

  <aside class="help-toc" aria-label="Auf dieser Seite">
    {#if toc.length > 0}
      <p class="toc-label">Auf dieser Seite</p>
      <ul class="toc-list">
        {#each toc as item (item.id)}
          <li class="toc-level-{item.level}">
            <a href="#{item.id}">{item.text}</a>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="toc-stub">Inhaltsverzeichnis</p>
    {/if}
  </aside>
</div>

<style>
  .help-shell {
    display: grid;
    grid-template-columns: 240px 1fr 220px;
    gap: 1.5rem;
    align-items: start;
    /* Wir scrollen die Spalten unabhängig; die äußere `main` scrollt nicht
       mit. Höhe = Viewport minus den Top-Pad, den die App-Shell setzt. */
    height: calc(100vh - var(--main-pad));
  }

  /* Schmaler werdende Fenster: TOC-Spalte fällt weg, Sidebar wird schmaler. */
  @media (max-width: 1100px) {
    .help-shell {
      grid-template-columns: 220px 1fr;
    }
    .help-toc {
      display: none;
    }
  }

  .help-sidebar {
    background: var(--c-surface);
    border: 1px solid var(--c-border);
    border-radius: var(--r-lg);
    box-shadow: var(--sh-sm);
    padding: 1rem 0.75rem 1.25rem;
    overflow-y: auto;
    height: 100%;
    box-sizing: border-box;
  }

  /* --- Suche ----------------------------------------------------------- */

  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }
  .search {
    position: relative;
    margin: 0 0.4rem 0.85rem;
  }
  .search-input {
    width: 100%;
    box-sizing: border-box;
    padding: 0.4rem 1.8rem 0.4rem 0.6rem;
    font-size: var(--fs-sm);
    border: 1px solid var(--c-border);
    border-radius: var(--r-md);
    background: var(--c-surface-2);
    color: var(--c-text);
  }
  .search-input:focus {
    outline: 2px solid var(--c-primary-300);
    outline-offset: 1px;
    border-color: var(--c-primary-400);
  }
  /* WebKit blendet das eingebaute „×" beim type=search ein — wir setzen
     unseren eigenen Clear-Button, also nehmen wir das native weg. */
  .search-input::-webkit-search-cancel-button {
    -webkit-appearance: none;
    appearance: none;
  }
  .search-clear {
    position: absolute;
    top: 50%;
    right: 0.35rem;
    transform: translateY(-50%);
    border: 0;
    background: transparent;
    cursor: pointer;
    color: var(--c-text-muted);
    font-size: 1.1rem;
    line-height: 1;
    padding: 0.1rem 0.35rem;
    border-radius: var(--r-sm);
  }
  .search-clear:hover {
    background: var(--c-primary-50);
    color: var(--c-primary-700);
  }

  /* --- Trefferliste ---------------------------------------------------- */

  .results {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    padding: 0 0.2rem;
  }
  .results-empty,
  .results-count {
    margin: 0 0.4rem 0.25rem;
    font-size: var(--fs-xs);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--c-text-subtle);
    font-weight: 700;
  }
  .results-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .results-list li {
    margin: 0;
  }
  .result {
    display: grid;
    grid-template-columns: 1fr auto;
    grid-template-rows: auto auto;
    column-gap: 0.5rem;
    width: 100%;
    text-align: left;
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--r-md);
    padding: 0.4rem 0.55rem;
    cursor: pointer;
    color: var(--c-text);
    font-family: inherit;
  }
  .result:hover {
    background: var(--c-primary-50);
    border-color: var(--c-primary-100);
  }
  .result.active {
    background: var(--c-primary-100);
    border-color: var(--c-primary-200);
  }
  .result-title {
    grid-column: 1;
    grid-row: 1;
    font-size: var(--fs-sm);
    font-weight: 600;
    line-height: 1.3;
    color: var(--c-text);
  }
  .result-cat {
    grid-column: 2;
    grid-row: 1;
    align-self: center;
    font-size: var(--fs-xs);
    color: var(--c-text-subtle);
    background: var(--c-surface-2);
    border-radius: var(--r-sm);
    padding: 0.05rem 0.4rem;
    white-space: nowrap;
  }
  .result-snippet {
    grid-column: 1 / -1;
    grid-row: 2;
    margin-top: 0.2rem;
    font-size: var(--fs-xs);
    line-height: 1.4;
    color: var(--c-text-muted);
    word-break: break-word;
  }
  /* Highlight aus `buildSnippet()` — Treffer-Terme stehen in `<mark>`. */
  .result-snippet :global(mark) {
    background: var(--c-primary-100);
    color: var(--c-primary-800);
    padding: 0 0.1em;
    border-radius: 2px;
  }
  .help-sidebar .back {
    /* Schrumpfen wir den globalen .back-Button minimal, damit er gut in die
       schmale Spalte passt. */
    margin: 0 0 0.75rem !important;
    font-size: var(--fs-xs) !important;
    padding: 5px 10px !important;
  }
  .help-title {
    font-size: var(--fs-md);
    margin: 0 0 0.75rem;
    color: var(--c-text);
    padding: 0 0.4rem;
  }
  .help-sidebar nav {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  /* --- Kollabierbare Kategorien (G2-DOC.3.4) ------------------------- */
  .cat-header {
    width: 100%;
    text-align: left;
    background: transparent;
    border: 0;
    padding: 0.35rem 0.55rem;
    border-radius: var(--r-md);
    display: flex;
    align-items: center;
    gap: 0.45rem;
    cursor: pointer;
    color: var(--c-text-subtle);
    font-family: inherit;
    font-size: var(--fs-xs);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    font-weight: 700;
  }
  .cat-header:hover {
    background: var(--c-primary-50);
    color: var(--c-primary-700);
  }
  .cat-arrow {
    display: inline-block;
    font-size: 0.7rem;
    color: var(--c-text-subtle);
    transition: transform 0.12s ease;
  }
  .cat.open .cat-arrow {
    transform: rotate(90deg);
  }
  .cat-label {
    flex: 1;
  }
  .cat-count {
    color: var(--c-text-subtle);
    background: var(--c-surface-2);
    padding: 0.05rem 0.4rem;
    border-radius: var(--r-pill);
    font-size: var(--fs-xs);
    letter-spacing: 0;
    text-transform: none;
    font-weight: 600;
  }
  .cat ul {
    list-style: none;
    padding: 0;
    margin: 0.15rem 0 0.4rem 0.25rem;
  }
  .cat li {
    margin: 0;
  }
  .cat a {
    display: block;
    padding: 0.4rem 0.55rem;
    border-radius: var(--r-md);
    color: var(--c-text);
    font-size: var(--fs-sm);
    text-decoration: none;
    line-height: 1.35;
  }
  .cat a:hover {
    background: var(--c-primary-50);
    color: var(--c-primary-700);
    text-decoration: none;
  }
  .cat li.active a {
    background: var(--c-primary-100);
    color: var(--c-primary-800);
    font-weight: 600;
  }

  .help-content {
    background: var(--c-surface);
    border: 1px solid var(--c-border);
    border-radius: var(--r-lg);
    box-shadow: var(--sh-sm);
    padding: 1.5rem 1.75rem;
    overflow-y: auto;
    height: 100%;
    box-sizing: border-box;
    min-width: 0; /* verhindert, dass <pre>-Inhalt das Grid sprengt */
  }

  .help-toc {
    background: var(--c-surface-2);
    border: 1px dashed var(--c-border);
    border-radius: var(--r-lg);
    padding: 1rem;
    color: var(--c-text-muted);
    font-size: var(--fs-sm);
    overflow-y: auto;
    height: 100%;
    box-sizing: border-box;
  }
  .toc-stub,
  .toc-label {
    margin: 0 0 0.6rem;
    font-size: var(--fs-xs);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--c-text-subtle);
    font-weight: 700;
  }
  .toc-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
  }
  .toc-list li {
    margin: 0;
  }
  .toc-list a {
    display: block;
    padding: 0.25rem 0.4rem;
    border-radius: var(--r-sm);
    color: var(--c-text-muted);
    font-size: var(--fs-sm);
    text-decoration: none;
    line-height: 1.35;
  }
  .toc-list a:hover {
    background: var(--c-primary-50);
    color: var(--c-primary-700);
    text-decoration: none;
  }
  .toc-list .toc-level-3 a {
    padding-left: 1.2rem;
    font-size: var(--fs-xs);
  }

  /* --- Druck (G2-DOC.3.4) ------------------------------------------- */
  @media print {
    .help-shell {
      display: block;
      height: auto;
      gap: 0;
    }
    .help-sidebar,
    .help-toc {
      display: none;
    }
    .help-content {
      border: none;
      box-shadow: none;
      padding: 0;
      overflow: visible;
      height: auto;
    }
  }
</style>
