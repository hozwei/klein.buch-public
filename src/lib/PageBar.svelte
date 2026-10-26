<script lang="ts">
  // Sticky-Aktionsleiste oben (DS-2 UX). Bleibt beim Scrollen am oberen Rand des
  // Hauptbereichs stehen: links Zurück + Titel, rechts die Hauptaktionen.
  // Spannt über das Padding des Hauptbereichs (--main-pad), damit es wie eine
  // durchgehende Leiste wirkt; Inhalt scrollt darunter durch.
  //
  // 2.4.11 (U1): Solange die Leiste klebt, meldet sie ihre Höhe als
  // `--pagebar-h` an <html>; `scroll-padding-top` (tokens.css, +layout.svelte)
  // hält fokussierte Elemente darunter sichtbar.
  import type { Snippet } from "svelte";

  let {
    back,
    backLabel = "Zurück",
    title,
    actions,
  }: {
    back?: string;
    backLabel?: string;
    title?: string;
    actions?: Snippet;
  } = $props();

  let bar = $state<HTMLDivElement | null>(null);

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
</script>

<div class="pagebar" bind:this={bar}>
  <div class="pb-left">
    {#if back}<a class="pb-back" href={back}><span aria-hidden="true">←</span> {backLabel}</a>{/if}
    {#if title}<h1 class="pb-title">{title}</h1>{/if}
  </div>
  {#if actions}<div class="pb-right">{@render actions()}</div>{/if}
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
    gap: 0.5rem 1rem;
    flex-wrap: wrap;
    /* U1: deckende Fläche statt Glas-Effekt (Designleitlinie §7). */
    background: var(--surface-page);
    /* bündig oben (main hat kein Top-Padding); volle Breite über das seitliche
       main-Padding hinweg, damit Inhalt sauber darunter durchscrollt. */
    margin: 0 calc(var(--main-pad) * -1) 1.25rem;
    /* U3: Fläche über die volle Breite, Aktionen aber bündig mit der Inhaltsbreite
       (--content-max aus tokens.css; 100 % = Inhaltsbreite von <main>). */
    padding: 0.7rem calc(var(--main-pad) + max(0px, 100% - var(--content-max, 100%))) 0.7rem
      var(--main-pad);
    border-bottom: 1px solid var(--line);
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
    .pb-left { flex-wrap: wrap; row-gap: 0.5rem; }
  }
  .pb-left { display: flex; align-items: center; gap: 0.85rem; min-width: 0; flex: 1 1 auto; }
  /* Zurück-Pille — identisch zu allen .pb-right-Buttons (Manuel-Hardline
     2026-05-26: oben links = oben rechts = pixelgenau gleich). */
  .pb-back {
    /* nie schmaler als die Beschriftung (U1: Text lief bei 320 px über den Rahmen) */
    flex: none;
    display: inline-flex !important;
    align-items: center !important;
    justify-content: center !important;
    gap: 8px !important;
    white-space: nowrap;
    padding: 9px 16px !important;
    min-height: var(--target);
    min-width: var(--target);
    font: inherit !important;
    font-weight: 600 !important;
    line-height: 1.2 !important;
    border-width: 1px !important;
    border-style: solid !important;
    border-color: var(--line-control);
    border-radius: var(--r-md) !important;
    box-sizing: border-box !important;
    background: var(--surface-card);
    color: var(--accent-on-soft);
    text-decoration: none;
  }
  @media (hover: hover) {
    .pb-back:hover { background: var(--surface-hover); border-color: var(--c-primary-300); }
  }
  /* Titel bricht um statt abgeschnitten zu werden (1.4.12 Textabstände). */
  .pb-title {
    font-size: var(--fs-xl); font-weight: 700; margin: 0;
    min-width: 0; overflow-wrap: anywhere; line-height: 1.25;
  }
  .pb-right { display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap; justify-content: flex-end; }
  /* PageBar-Forcing — ALLE Buttons in .pb-right werden auf identische
     Specs gezwungen, egal aus welchem Ökosystem (Button.svelte = .btn /
     tokens.css = .btn-primary/… /.link*). Manuel-Hardline 2026-05-26. */
  :global(.pb-right .btn),
  :global(.pb-right .btn-primary),
  :global(.pb-right .btn-secondary),
  :global(.pb-right .btn-ghost),
  :global(.pb-right .btn-danger),
  :global(.pb-right .btn-ghost-danger),
  :global(.pb-right .link),
  :global(.pb-right .link-a),
  :global(.pb-right .link-btn) {
    padding: 9px 16px !important;
    min-height: var(--target);
    font: inherit !important;
    font-weight: 600 !important;
    line-height: 1.2 !important;
    border-width: 1px !important;
    border-style: solid !important;
    border-radius: var(--r-md) !important;
    box-sizing: border-box !important;
    display: inline-flex !important;
    align-items: center !important;
    justify-content: center !important;
    gap: 8px !important;
  }
</style>
