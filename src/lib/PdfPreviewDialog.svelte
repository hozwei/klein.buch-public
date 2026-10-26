<script lang="ts">
  // Plan W7: PDF-Vorschau im iframe (Server liefert inline mit
  // `frame-ancestors 'self'`). Außerhalb von `<main>` gemountet, siehe
  // pdfPreviewModal.svelte.ts.

  import { pdfPreviewStore, closePdfPreview } from "#lib/pdfPreviewModal.svelte.ts";
  import { pushModal, popModal } from "#lib/modalStack.svelte.ts";
  import { focusTrap } from "#lib/focusTrap.ts";

  // Modal-Stack (App-Shell `inert`); Fokus, Escape und Rücksprung über
  // `focusTrap` (U1). ≤ 600 px Vollbild-Blatt mit „PDF öffnen“.
  $effect(() => {
    if (pdfPreviewStore.visible) {
      pushModal();
      return () => popModal();
    }
  });
</script>

{#if pdfPreviewStore.visible}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div
    class="kb-backdrop"
    role="presentation"
    onclick={(e) => {
      if (e.target === e.currentTarget) closePdfPreview();
    }}
  >
    <div
      class="kb-dialog pdf"
      role="dialog"
      aria-modal="true"
      aria-labelledby="pdf-preview-title"
      use:focusTrap={{ onEscape: closePdfPreview, initialFocus: ".kb-dialog-close" }}
    >
      <header class="kb-dialog-head">
        <h2 id="pdf-preview-title">{pdfPreviewStore.title}</h2>
        <button
          type="button"
          class="kb-dialog-close"
          aria-label="Vorschau schließen"
          onclick={closePdfPreview}><span aria-hidden="true">×</span></button
        >
      </header>
      <iframe class="body" src={pdfPreviewStore.url} title={pdfPreviewStore.title}></iframe>
      <!-- Plan W10: Mobile Browser zeigen PDFs im iframe nicht (Android) oder
           nur die erste Seite (iOS) — dort öffnet die PDF-Ansicht des Browsers. -->
      <div class="mobile-open">
        <p>Die PDF-Datei öffnet sich in der PDF-Ansicht deines Browsers.</p>
        <a
          class="btn-primary"
          href={pdfPreviewStore.url}
          target="_blank"
          rel="noopener noreferrer"
          data-sveltekit-reload>PDF öffnen</a
        >
      </div>
      <footer class="kb-dialog-foot">
        <a
          class="btn-secondary btn-sm"
          href={pdfPreviewStore.url}
          target="_blank"
          rel="noopener noreferrer"
          data-sveltekit-reload>In neuem Tab öffnen</a
        >
        <a
          class="btn-secondary btn-sm"
          href={pdfPreviewStore.url}
          download={pdfPreviewStore.fileName}
          data-sveltekit-reload>Herunterladen</a
        >
        <button type="button" class="btn-primary btn-sm" onclick={closePdfPreview}>
          Schließen
        </button>
      </footer>
    </div>
  </div>
{/if}

<style>
  /* Rahmen aus tokens.css (.kb-dialog); hier nur die feste Höhe für das PDF. */
  .pdf {
    --dialog-w: 1000px;
    height: calc(100vh - 2 * var(--sp-6));
    height: calc(100dvh - 2 * var(--sp-6));
  }
  .body {
    flex: 1;
    width: 100%;
    min-height: 0;
    border: 0;
    background: var(--surface-sunken);
  }
  footer a {
    text-decoration: none;
  }
  .mobile-open {
    display: none;
  }
  /* Mobile Browser zeigen PDFs im iframe nicht (Android) oder nur die erste
     Seite (iOS) — dort öffnet die PDF-Ansicht des Browsers. */
  @media (max-width: 600px) {
    .pdf { height: 100vh; height: 100dvh; }
    .body {
      display: none;
    }
    .mobile-open {
      flex: 1;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      gap: 1rem;
      padding: 1.5rem;
      text-align: center;
      color: var(--text-muted);
    }
  }
</style>
