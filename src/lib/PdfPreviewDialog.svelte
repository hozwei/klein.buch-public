<script lang="ts">
  // Plan W7: PDF-Vorschau im iframe (Server liefert inline mit
  // `frame-ancestors 'self'`). Außerhalb von `<main>` gemountet, siehe
  // pdfPreviewModal.svelte.ts.

  import { pdfPreviewStore, closePdfPreview } from "#lib/pdfPreviewModal.svelte.ts";
  import { pushModal, popModal } from "#lib/modalStack.svelte.ts";

  let closeBtn = $state<HTMLButtonElement | null>(null);
  let triggerEl: HTMLElement | null = null;

  $effect(() => {
    if (pdfPreviewStore.visible) {
      triggerEl =
        document.activeElement instanceof HTMLElement ? document.activeElement : null;
      pushModal();
      queueMicrotask(() => closeBtn?.focus());
      return () => {
        popModal();
        triggerEl?.focus();
        triggerEl = null;
      };
    }
  });

  function onKeydown(e: KeyboardEvent): void {
    if (!pdfPreviewStore.visible) return;
    if (e.key === "Escape") {
      e.preventDefault();
      closePdfPreview();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if pdfPreviewStore.visible}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div
    class="backdrop"
    role="presentation"
    onclick={(e) => {
      if (e.target === e.currentTarget) closePdfPreview();
    }}
  >
    <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="pdf-preview-title">
      <header>
        <h2 id="pdf-preview-title">{pdfPreviewStore.title}</h2>
        <button
          bind:this={closeBtn}
          type="button"
          class="close"
          aria-label="Vorschau schließen"
          onclick={closePdfPreview}>×</button
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
      <footer>
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
  /* Sheet-Look analog XmlViewerDialog. */
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(16, 40, 50, 0.32);
    -webkit-backdrop-filter: blur(6px) saturate(140%);
    backdrop-filter: blur(6px) saturate(140%);
    display: grid;
    place-items: center;
    z-index: 1000;
    padding: 2rem;
    box-sizing: border-box;
  }
  .dialog {
    background: var(--c-surface);
    border-radius: var(--r-xl);
    box-shadow: var(--sh-xl);
    width: min(1000px, 100%);
    height: calc(100vh - 4rem);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 1rem 1.25rem 0.85rem;
    border-bottom: 1px solid rgba(16, 36, 44, 0.08);
  }
  header h2 {
    margin: 0;
    font-size: var(--fs-lg);
    color: var(--c-text);
  }
  .close {
    background: transparent;
    border: 0;
    font-size: 1.5rem;
    line-height: 1;
    padding: 0.15rem 0.55rem;
    border-radius: var(--r-md);
    cursor: pointer;
    color: var(--c-text-muted);
  }
  .close:hover {
    background: var(--c-primary-50);
    color: var(--c-primary-700);
  }
  .body {
    flex: 1;
    width: 100%;
    border: 0;
    background: var(--c-surface-2);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    padding: 0.75rem 1.25rem;
    border-top: 1px solid var(--c-border);
    background: var(--c-surface-2);
  }
  footer a {
    text-decoration: none;
  }
  .mobile-open {
    display: none;
  }
  @media (max-width: 600px) {
    .backdrop {
      padding: 0;
    }
    .dialog {
      width: 100%;
      height: 100dvh;
      border-radius: 0;
    }
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
      color: var(--c-text-muted);
    }
    .close {
      min-width: 44px;
      min-height: 44px;
    }
    footer {
      flex-wrap: wrap;
    }
  }
</style>
