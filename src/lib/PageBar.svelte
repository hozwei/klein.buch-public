<script lang="ts">
  // Sticky-Aktionsleiste oben (DS-2 UX). Bleibt beim Scrollen am oberen Rand des
  // Hauptbereichs stehen: links Zurück + Titel, rechts die Aktionen.
  // Spannt über das Padding des Hauptbereichs (--main-pad), damit es wie eine
  // durchgehende Leiste wirkt; Inhalt scrollt darunter durch.
  //
  // 2.4.11 (U1): Solange die Leiste klebt, meldet sie ihre Höhe als
  // `--pagebar-h` an <html>; `scroll-padding-top` (tokens.css, +layout.svelte)
  // hält fokussierte Elemente darunter sichtbar.
  //
  // Aktionen (U4, U1-059; design.md §4.2), Reihenfolge rechts:
  //   [secondary …] [Mehr ▾] [actions]
  // Belegnummern im Titel (`RE-2026-0042`) erscheinen als `.kb-docno` (U1-061).
  // Seitentitel (U4, 2.4.2; ADR 0041 Nr. 160): Der Titel geht zusätzlich als
  // „<Titel> – Klein.Buch“ in `document.title` (Root-Layout, pageTitle.svelte.ts).
  // Ohne `title` (Detailseite lädt) gilt der Ersatztitel der Route — auch als
  // sichtbares `h1` (V4-08: Seitenstruktur nie ohne Überschrift).
  // - `actions` (Snippet): die Hauptaktion, höchstens eine `primary`, zuletzt
  //   (rechts, mit dem Daumen erreichbar). Bisherige Seiten geben hier alle
  //   Knöpfe an — das funktioniert unverändert.
  // - `secondary` (Daten): Nebenaktionen; ab 601 px als `secondary`-Knöpfe, bis
  //   600 px Breite im Menü „Mehr“ (dort höchstens Hauptaktion + „Mehr“).
  // - `more` (Daten): seltene Aktionen und Gefahraktionen, immer im Menü „Mehr“;
  //   Gefahraktionen (`danger`) stehen dort unten, abgesetzt.
  import { onDestroy, type Snippet } from "svelte";
  import ActionMenu, { type ActionMenuItem } from "#lib/ActionMenu.svelte";
  import { splitDocNumbers } from "#lib/DocNo.svelte";
  import { page } from "$app/state";
  import { claimPageTitle, releasePageTitle, routeTitle, setPageTitle } from "#lib/pageTitle.svelte.ts";

  let {
    back,
    backLabel = "Zurück",
    title,
    actions,
    secondary = [],
    more = [],
    moreLabel = "Mehr",
  }: {
    back?: string;
    backLabel?: string;
    title?: string;
    /** Hauptaktion(en), immer sichtbar. */
    actions?: Snippet;
    /** Nebenaktionen: breit als Knöpfe, bis 600 px im Menü „Mehr“. */
    secondary?: ActionMenuItem[];
    /** Immer im Menü „Mehr“ (selten Gebrauchtes, Gefahraktionen). */
    more?: ActionMenuItem[];
    /** Beschriftung des Menü-Knopfs. */
    moreLabel?: string;
  } = $props();

  let bar = $state<HTMLDivElement | null>(null);

  // Seitentitel: vor dem Zeichnen setzen, damit die Routen-Ansage von SvelteKit
  // (liest `document.title` nach der Navigation) schon den neuen Titel hat.
  const titleOwner = claimPageTitle();
  $effect.pre(() => {
    setPageTitle(titleOwner, title);
  });
  onDestroy(() => releasePageTitle(titleOwner));
  const shownTitle = $derived(title || routeTitle(page.route.id) || undefined);

  // Bis 600 px: Nebenaktionen ins Menü (gleiche Grenze wie „klebt nicht“ unten).
  const NARROW_MQ = "(max-width: 600px)";
  let narrow = $state(typeof window !== "undefined" && window.matchMedia(NARROW_MQ).matches);
  $effect(() => {
    const mq = window.matchMedia(NARROW_MQ);
    const update = () => (narrow = mq.matches);
    update();
    mq.addEventListener("change", update);
    return () => mq.removeEventListener("change", update);
  });

  const inlineSecondary = $derived(narrow ? [] : secondary);
  const menuItems = $derived(narrow ? [...secondary, ...more] : more);

  $effect(() => {
    const el = bar;
    if (!el || typeof ResizeObserver === "undefined") return;
    const root = document.documentElement;
    const update = () => {
      const sticky = getComputedStyle(el).position === "sticky";
      root.style.setProperty(
        "--pagebar-h",
        sticky ? `${Math.ceil(el.getBoundingClientRect().height)}px` : "0px",
      );
    };
    update();
    const ro = new ResizeObserver(update);
    ro.observe(el);
    // Breite und Höhe: unter 600 px Breite oder 500 px Höhe klebt die Leiste nicht.
    const mq = matchMedia("(max-width: 600px), (max-height: 500px)");
    mq.addEventListener("change", update);
    return () => {
      ro.disconnect();
      mq.removeEventListener("change", update);
      root.style.removeProperty("--pagebar-h");
    };
  });

  function run(item: ActionMenuItem, e: MouseEvent) {
    if (item.disabled) {
      e.preventDefault();
      return;
    }
    item.onSelect?.();
  }
</script>

<div class="pagebar" bind:this={bar}>
  <div class="pb-left">
    {#if back}<a class="pb-back" href={back}><span aria-hidden="true">←</span> {backLabel}</a>{/if}
    <!-- Belegnummern im Titel als Monospace, nie getrennt (U1-061); Text unverändert. -->
    {#if shownTitle}<h1 class="pb-title">{#each splitDocNumbers(shownTitle) as part}{#if part.docNo}<span class="kb-docno">{part.text}</span>{:else}{part.text}{/if}{/each}</h1>{/if}
  </div>
  {#if actions || inlineSecondary.length > 0 || menuItems.length > 0}
    <div class="pb-right">
      {#each inlineSecondary as item}
        {#if item.href && !item.disabled}
          <a
            class={item.danger ? "btn-danger" : "btn-secondary"}
            href={item.href}
            download={item.download === true ? "" : item.download || undefined}
            onclick={(e) => run(item, e)}>{item.label}</a
          >
        {:else}
          <!-- U4 (V2-06, ADR 0041 Nr. 33): gesperrt bleibt fokussierbar (aria-disabled),
               der Klick ist wirkungslos (run); Seiten sperren hier meist während einer Aktion. -->
          <button
            type="button"
            class={item.danger ? "btn-danger" : "btn-secondary"}
            aria-disabled={item.disabled || undefined}
            onclick={(e) => run(item, e)}>{item.label}</button
          >
        {/if}
      {/each}
      {#if menuItems.length > 0}<ActionMenu items={menuItems} label={moreLabel} />{/if}
      {@render actions?.()}
    </div>
  {/if}
</div>

<style>
  .pagebar {
    position: sticky;
    /* unter der mobilen Kopfleiste (Drawer-Band), sonst bündig oben */
    top: var(--sticky-top, 0);
    z-index: 30;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2) var(--sp-4);
    flex-wrap: wrap;
    /* U1: deckende Fläche statt Glas-Effekt (Designleitlinie §7). */
    background: var(--surface-page);
    /* bündig oben (main hat kein Top-Padding); volle Breite über das seitliche
       main-Padding hinweg, damit Inhalt sauber darunter durchscrollt. */
    margin: 0 calc(var(--main-pad) * -1) 1.25rem;
    /* U3: Fläche über die volle Breite, Aktionen aber bündig mit der Inhaltsbreite
       (--content-max aus tokens.css; 100 % = Inhaltsbreite von <main>). */
    padding: var(--sp-3) calc(var(--main-pad) + max(0px, 100% - var(--content-max, 100%))) var(--sp-3)
      var(--main-pad);
    border-bottom: var(--rule);
  }
  /* Plan W10: Auf dem Handy klebt nur die Kopfleiste; eine klebende PageBar
     mit umbrechenden Aktionen würde ein Viertel des Bildschirms belegen. */
  /* U3: Niedrige Fenster (Zoom 200 %, Handy quer) ebenso — Kopfleiste und eine
     mehrzeilige PageBar belegten bei 400 px Höhe bis zu 80 % des Bildschirms und
     verdeckten fokussierte Felder (2.4.11). Die Hauptaktion wiederholt dann das
     Formularende (.kb-form-end, tokens.css). */
  @media (max-height: 500px) {
    .pagebar { position: static; }
  }
  @media (max-width: 600px) {
    .pagebar { position: static; }
    /* Langer Titel (z. B. „Entwurf RE-2026-0001 bearbeiten“) bekommt eine eigene
       Zeile statt drei schmaler Zeilen neben dem Zurück-Knopf (U1-Sichtprüfung). */
    .pb-left { flex-wrap: wrap; row-gap: var(--sp-2); }
  }
  .pb-left { display: flex; align-items: center; gap: var(--sp-3); min-width: 0; flex: 1 1 auto; }
  /* Zurück-Pille — identisch zu allen .pb-right-Knöpfen (Manuel-Hardline
     2026-05-26: oben links = oben rechts = pixelgenau gleich). U4: ohne !important,
     Höhe --control-h wie alle Knöpfe und Felder. */
  .pb-back {
    /* nie schmaler als die Beschriftung (U1: Text lief bei 320 px über den Rahmen) */
    flex: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--sp-2);
    white-space: nowrap;
    padding: var(--sp-2) var(--sp-4);
    min-height: var(--control-h);
    min-width: var(--target);
    font: inherit;
    font-weight: 600;
    line-height: var(--lh-tight);
    border: var(--rule-control);
    border-radius: var(--radius-control);
    box-sizing: border-box;
    background: var(--surface-card);
    color: var(--accent-on-soft);
    text-decoration: none;
  }
  @media (hover: hover) {
    .pb-back:hover { background: var(--surface-hover); border-color: var(--line-hover); text-decoration: none; }
  }
  /* Titel bricht um statt abgeschnitten zu werden (1.4.12 Textabstände). */
  .pb-title {
    font-size: var(--type-title); font-weight: 700; margin: 0;
    min-width: 0; overflow-wrap: anywhere; line-height: 1.25;
  }
  .pb-right { display: flex; align-items: center; gap: var(--sp-2); flex-wrap: wrap; justify-content: flex-end; }
  /* Alle Knöpfe in .pb-right gleich, egal aus welchem Ökosystem (Button.svelte = .btn,
     tokens.css = .btn-primary/…/.link, ActionMenu). Manuel-Hardline 2026-05-26.
     U4: ohne !important — Spezifität (0,2,0) schlägt die Klassen (0,1,0). */
  :global(.pb-right :is(.btn, .btn-primary, .btn-secondary, .btn-ghost, .btn-danger, .btn-ghost-danger, .link)) {
    padding: var(--sp-2) var(--sp-4);
    min-height: var(--control-h);
    font: inherit;
    font-weight: 600;
    line-height: var(--lh-tight);
    border-width: 1px;
    border-style: solid;
    border-radius: var(--radius-control);
    box-sizing: border-box;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--sp-2);
  }
</style>
