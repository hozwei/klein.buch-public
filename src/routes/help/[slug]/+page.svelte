<script lang="ts">
  // G2-DOC.3.2 + G2-DOC.3.4 — Render-Seite für eine Handbuch-Seite.
  //
  // Setzt das vom Loader (`+page.ts`) erzeugte HTML direkt via `{@html}`
  // ein. Das ist sicher, weil der Markdown-Inhalt aus dem Eigen-Bundle
  // kommt (read-only zur Laufzeit, kein User-Input, kein XSS-Vektor).
  // Externe Links bekommen vom Renderer `target="_blank"`/`rel="noopener
  // noreferrer"`, Bilder eine Build-Zeit-aufgelöste Asset-URL und Headings
  // stabile `id`-Anker (für die TOC rechts und für `<HelpAnchor>` aus
  // G2-DOC.4).
  //
  // 3.4 ergänzt:
  // - Disclaimer-Banner oben in allen „Recht und Steuern"-Kapiteln. Der
  //   Wortlaut ist G3-konform und nicht aus dem UI entfernbar.
  // - Hash-Deeplinks öffnen jetzt automatisch das passende `<details>`,
  //   sodass FAQ-Anker (`/help/faq#faq-storno`) den richtigen Eintrag
  //   aufklappen und in den Viewport scrollen.
  // - Beim Drucken (`beforeprint`) klappen alle `<details>` der Seite auf,
  //   damit Antworten nicht hinter dem Pfeil verschwinden. CSS würde das
  //   browserseitig je nach Engine verschieden behandeln; das JS-Hook ist
  //   robust.

  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import type { HandbookEntry } from "$lib/handbook";
  import type { TocEntry } from "$lib/handbook/render";

  let {
    data,
  }: { data: { entry: HandbookEntry; html: string; toc: TocEntry[] } } =
    $props();

  const LEGAL_DISCLAIMER = "Klein.Buch ist ein Werkzeug, kein Steuerberater.";

  // Container für `{@html}`. Per `bind:this` zugreifen, statt
  // `document.getElementById`, damit wir die Suche aufs gerenderte
  // Handbuch-Fragment beschränken und nicht versehentlich z. B. das
  // Sidebar-Suchfeld treffen.
  let proseEl: HTMLElement | undefined = $state();

  // Hash-Deeplink: nach jedem Render (Mount und Page-Change) prüfen, ob
  // der aktuelle URL-Hash auf ein `<details>` in dieser Seite zeigt, und
  // es ggf. aufklappen + in den Viewport scrollen.
  function applyHashFocus(): void {
    if (typeof window === "undefined") return;
    const hash = window.location.hash.replace(/^#/, "");
    if (!hash || !proseEl) return;
    // CSS.escape, falls IDs irgendwann Sonderzeichen enthalten.
    const sel = `#${CSS.escape(hash)}`;
    const target = proseEl.querySelector(sel) as HTMLElement | null;
    if (!target) return;
    if (target instanceof HTMLDetailsElement) {
      target.open = true;
    } else {
      // Heading innerhalb eines Details? Eltern-Details aufklappen.
      const owner = target.closest("details");
      if (owner instanceof HTMLDetailsElement) owner.open = true;
    }
    // `scrollIntoView` nach dem Aufklappen, sonst landet die Sicht oben.
    target.scrollIntoView({ block: "start" });
  }

  function openAllDetailsForPrint(): void {
    if (!proseEl) return;
    proseEl.querySelectorAll("details").forEach((d) => {
      (d as HTMLDetailsElement).open = true;
    });
  }

  onMount(() => {
    const onBeforePrint = () => openAllDetailsForPrint();
    window.addEventListener("beforeprint", onBeforePrint);
    return () => window.removeEventListener("beforeprint", onBeforePrint);
  });

  // Bei jedem Slug- oder Hash-Wechsel den Deeplink neu anwenden.
  $effect(() => {
    // Reactive Abhängigkeiten: Slug + Hash. `data.html` triggert die
    // erste Anwendung nach DOM-Mount.
    $page.url.pathname;
    $page.url.hash;
    data.html;
    queueMicrotask(applyHashFocus);
  });
</script>

<header class="entry-header">
  <h1>{data.entry.title}</h1>
  <p class="meta">
    Kategorie:&nbsp;{data.entry.category} · Reihenfolge:&nbsp;{data.entry.order}
  </p>
</header>

{#if data.entry.category === "recht-und-steuern"}
  <aside class="legal-banner" role="note" aria-label="Steuer-Disclaimer">
    {LEGAL_DISCLAIMER}
  </aside>
{/if}

<div class="prose" bind:this={proseEl}>
  {@html data.html}
</div>

<style>
  .entry-header {
    margin-bottom: 1.5rem;
    padding-bottom: 0.75rem;
    border-bottom: 1px solid var(--c-border);
  }
  .entry-header h1 {
    font-size: var(--fs-2xl);
    margin: 0 0 0.5rem;
    letter-spacing: -0.01em;
  }
  .meta {
    margin: 0;
    color: var(--c-text-subtle);
    font-size: var(--fs-xs);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    font-weight: 600;
  }

  /* Disclaimer-Banner für „Recht und Steuern"-Kapitel.
     Erscheint zwischen Header und Prose, damit niemand das Kapitel liest,
     ohne den Hinweis gesehen zu haben. */
  .legal-banner {
    background: var(--c-warning-50, #fdf6e3);
    border: 1px solid var(--c-warning-200, #f0d68c);
    color: var(--c-warning-800, #7a5a14);
    border-radius: var(--r-md);
    padding: 0.65rem 0.9rem;
    margin: 0 0 1.25rem;
    font-size: var(--fs-sm);
    font-weight: 600;
    line-height: 1.4;
  }

  /* Prose-Styles für gerenderten Markdown-Content. Bewusst :global, weil
     das HTML aus dem Renderer kommt und Svelte sonst die Klassen nicht
     scopen würde. */
  .prose :global(h1) {
    font-size: var(--fs-xl);
    margin: 1.5rem 0 0.75rem;
    letter-spacing: -0.01em;
  }
  .prose :global(h2) {
    font-size: var(--fs-lg);
    margin: 2rem 0 0.75rem;
    padding-top: 0.5rem;
    border-top: 1px solid var(--c-border-subtle, var(--c-border));
    scroll-margin-top: 1rem;
  }
  .prose :global(h3) {
    font-size: var(--fs-md);
    margin: 1.5rem 0 0.5rem;
    scroll-margin-top: 1rem;
  }
  .prose :global(p) {
    margin: 0 0 0.85rem;
    line-height: 1.6;
  }
  .prose :global(ul),
  .prose :global(ol) {
    margin: 0 0 0.85rem 1.5rem;
    line-height: 1.55;
  }
  .prose :global(li) {
    margin: 0.15rem 0;
  }
  .prose :global(a) {
    color: var(--c-primary-700);
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .prose :global(a:hover) {
    color: var(--c-primary-800);
  }
  .prose :global(strong) {
    font-weight: 700;
    color: var(--c-text);
  }
  .prose :global(em) {
    font-style: italic;
  }
  .prose :global(code) {
    font-family: var(--font-mono);
    font-size: 0.9em;
    background: var(--c-surface-2);
    padding: 0.1em 0.35em;
    border-radius: var(--r-sm);
  }
  .prose :global(pre) {
    background: var(--c-surface-2);
    border: 1px solid var(--c-border);
    border-radius: var(--r-md);
    padding: 0.85rem 1rem;
    overflow-x: auto;
    margin: 0 0 0.85rem;
    font-size: var(--fs-sm);
  }
  .prose :global(pre code) {
    background: transparent;
    padding: 0;
    font-size: inherit;
  }
  .prose :global(blockquote) {
    border-left: 4px solid var(--c-primary-200);
    margin: 0 0 0.85rem;
    padding: 0.25rem 0 0.25rem 0.9rem;
    color: var(--c-text-muted);
  }
  .prose :global(blockquote p) {
    margin: 0.15rem 0;
  }
  .prose :global(table) {
    border-collapse: collapse;
    margin: 0 0 1rem;
    width: 100%;
    font-size: var(--fs-sm);
  }
  .prose :global(th),
  .prose :global(td) {
    border: 1px solid var(--c-border);
    padding: 0.4rem 0.6rem;
    text-align: left;
    vertical-align: top;
  }
  .prose :global(th) {
    background: var(--c-surface-2);
    font-weight: 700;
  }
  .prose :global(hr) {
    border: 0;
    border-top: 1px solid var(--c-border);
    margin: 1.5rem 0;
  }
  .prose :global(img) {
    max-width: 100%;
    height: auto;
    border: 1px solid var(--c-border);
    border-radius: var(--r-md);
    display: block;
    margin: 0.5rem 0 1rem;
  }

  /* --- FAQ-Accordion (G2-DOC.3.4) ----------------------------------- */

  .prose :global(details.faq) {
    margin: 0.5rem 0 0.65rem;
    border: 1px solid var(--c-border);
    border-radius: var(--r-md);
    background: var(--c-surface);
    /* `scroll-margin-top` schiebt den Anker-Sprung unter den Klebe-Header
       (die Innenlayout-PageBar/Suche ist nicht klebend, aber der Wert
       sorgt für ein bisschen Atemluft). */
    scroll-margin-top: 1rem;
  }
  .prose :global(details.faq[open]) {
    background: var(--c-surface);
    border-color: var(--c-primary-200);
  }
  .prose :global(details.faq > summary) {
    cursor: pointer;
    padding: 0.6rem 0.9rem;
    font-weight: 600;
    color: var(--c-text);
    line-height: 1.4;
    list-style: none;
    display: flex;
    align-items: baseline;
    gap: 0.55rem;
  }
  /* Eigener Disclosure-Pfeil — der native ist je nach Browser hässlich.
     `::-webkit-details-marker` entfernt ihn in WebKit/Chromium. */
  .prose :global(details.faq > summary::-webkit-details-marker) {
    display: none;
  }
  .prose :global(details.faq > summary::before) {
    content: "▸";
    color: var(--c-text-subtle);
    font-size: 0.85em;
    flex: none;
    transform: translateY(-1px);
    transition: transform 0.12s ease;
  }
  .prose :global(details.faq[open] > summary::before) {
    transform: rotate(90deg);
  }
  .prose :global(details.faq > summary:hover) {
    background: var(--c-primary-50);
  }
  .prose :global(details.faq .faq-q) {
    flex: 1;
  }
  .prose :global(details.faq .faq-a) {
    padding: 0.1rem 0.95rem 0.4rem;
  }
  .prose :global(details.faq .faq-a > :first-child) {
    margin-top: 0.4rem;
  }
  .prose :global(details.faq .faq-a > :last-child) {
    margin-bottom: 0.3rem;
  }

  /* --- Druck (G2-DOC.3.4) ------------------------------------------- */
  @media print {
    .legal-banner {
      background: transparent;
      border: 1px solid #000;
      color: #000;
    }
    .prose :global(details.faq) {
      border: none;
      background: transparent;
      page-break-inside: avoid;
      margin: 0.4rem 0;
    }
    .prose :global(details.faq > summary) {
      padding: 0;
      font-size: var(--fs-md);
      cursor: auto;
    }
    .prose :global(details.faq > summary::before) {
      content: "";
    }
    .prose :global(details.faq .faq-a) {
      padding: 0.2rem 0 0;
    }
  }
</style>
