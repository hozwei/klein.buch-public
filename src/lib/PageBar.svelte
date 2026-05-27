<script lang="ts">
  // Sticky-Aktionsleiste oben (DS-2 UX). Bleibt beim Scrollen am oberen Rand des
  // Hauptbereichs stehen: links Zurück + Titel, rechts die Hauptaktionen.
  // Spannt über das Padding des Hauptbereichs (--main-pad), damit es wie eine
  // durchgehende Leiste wirkt; Inhalt scrollt darunter durch.
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
</script>

<div class="pagebar">
  <div class="pb-left">
    {#if back}<a class="pb-back" href={back}>← {backLabel}</a>{/if}
    {#if title}<h1 class="pb-title">{title}</h1>{/if}
  </div>
  {#if actions}<div class="pb-right">{@render actions()}</div>{/if}
</div>

<style>
  .pagebar {
    position: sticky;
    top: 0;
    z-index: 30;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    flex-wrap: wrap;
    /* G2-UX.3.2 — Frosted-Glass-Vibrancy: halbtransparenter Hintergrund über dem
       darunter scrollenden Inhalt. Beim Scrollen entsteht der typische macOS-
       Toolbar-Effekt (Content schimmert weich durch). Fallback ohne
       backdrop-filter: blickdichtes --c-bg, gleiches Layout. */
    background: rgba(244, 247, 249, 0.78);
    -webkit-backdrop-filter: blur(14px) saturate(180%);
    backdrop-filter: blur(14px) saturate(180%);
    /* bündig oben (main hat kein Top-Padding); volle Breite über das seitliche
       main-Padding hinweg, damit Inhalt sauber darunter durchscrollt. */
    margin: 0 calc(var(--main-pad) * -1) 1.25rem;
    padding: 0.7rem var(--main-pad);
    /* Hairline statt 1 px solid — wirkt dezenter, betont das Glas-Gefühl. */
    border-bottom: 1px solid rgba(16, 36, 44, 0.08);
  }
  /* Fallback für (sehr alte) Engines ohne backdrop-filter — auf WebView2 (Tauri)
     ist die API seit Chromium 76 (2019) verfügbar, deshalb in der Praxis selten. */
  @supports not ((backdrop-filter: blur(1px)) or (-webkit-backdrop-filter: blur(1px))) {
    .pagebar { background: var(--c-bg); }
  }
  .pb-left { display: flex; align-items: center; gap: 0.85rem; min-width: 0; }
  /* Zurück-Pille — IDENTISCH zu allen .pb-right-Buttons (Manuel-Hardline
     2026-05-26: oben links = oben rechts = pixelgenau gleich). */
  .pb-back {
    display: inline-flex !important;
    align-items: center !important;
    justify-content: center !important;
    gap: 8px !important;
    white-space: nowrap;
    padding: 9px 16px !important;
    font: inherit !important;
    font-weight: 600 !important;
    line-height: 1.2 !important;
    border-width: 1px !important;
    border-style: solid !important;
    border-color: var(--c-border-strong);
    border-radius: var(--r-md) !important;
    box-sizing: border-box !important;
    background: var(--c-surface);
    color: var(--c-primary-700);
    text-decoration: none;
  }
  .pb-back:hover { background: var(--c-primary-50); border-color: var(--c-primary-300); }
  .pb-title {
    font-size: var(--fs-xl); font-weight: 700; margin: 0;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .pb-right { display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap; justify-content: flex-end; }
  /* PageBar-Forcing — ALLE Buttons in .pb-right werden hart auf identische
     Specs gezwungen, egal aus welchem Ökosystem (Button.svelte = .btn /
     tokens.css = .btn-primary/.btn-secondary/.btn-ghost/.btn-danger/.link*).
     Erzwingt alle layouted-Eigenschaften, die Höhe und Schrift bestimmen.
     Manuel-Hardline 2026-05-26: Liste-PageBar und Unterseite-PageBar
     pixelgenau identisch. */
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
