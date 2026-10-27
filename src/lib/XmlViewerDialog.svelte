<script lang="ts">
  // PV1-A5: Roh-XML-Viewer-Dialog. Genau einmal im Root-Layout gemountet (siehe
  // routes/+layout.svelte) — außerhalb von `<main>`, damit der Dialog nicht durch
  // `inert={modalStack.count>0}` mitgesperrt wird (AboutDialog-Pattern).

  import { xmlViewerStore, closeXmlViewer } from "#lib/xmlViewerModal.svelte.ts";
  import { pushModal, popModal } from "#lib/modalStack.svelte.ts";
  import { focusTrap } from "#lib/focusTrap.ts";
  import { flash } from "#lib/toast.svelte.ts";
  import { copyText } from "#lib/browser.ts";

  // Modal-Stack (App-Shell `inert`); Fokus, Escape und Rücksprung über
  // `focusTrap` (U1).
  $effect(() => {
    if (xmlViewerStore.visible) {
      pushModal();
      return () => popModal();
    }
  });

  async function copyXml(): Promise<void> {
    const payload = xmlViewerStore.payload;
    if (!payload) return;
    try {
      await copyText(payload.xml);
      flash("Roh-XML in die Zwischenablage kopiert.");
    } catch (e) {
      flash(
        `Kopieren fehlgeschlagen: ${e instanceof Error ? e.message : String(e)}`,
        "error",
      );
    }
  }

  function sourceFormatLabel(fmt: string): string {
    switch (fmt) {
      case "zugferd":
        return "ZUGFeRD (PDF mit eingebettetem XML)";
      case "xrechnung-cii":
        return "XRechnung CII (UN/CEFACT)";
      case "xrechnung-ubl":
        return "XRechnung UBL (OASIS)";
      default:
        return fmt;
    }
  }

  function formatBytes(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} kB`;
    return `${(n / (1024 * 1024)).toFixed(2)} MB`;
  }
</script>

{#if xmlViewerStore.visible && xmlViewerStore.payload}
  {@const payload = xmlViewerStore.payload}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div
    class="kb-backdrop"
    role="presentation"
    onclick={(e) => {
      if (e.target === e.currentTarget) closeXmlViewer();
    }}
  >
    <div
      class="kb-dialog xml"
      role="dialog"
      aria-modal="true"
      aria-labelledby="rawxml-title"
      use:focusTrap={{ onEscape: closeXmlViewer, initialFocus: ".kb-dialog-close" }}
    >
      <header class="kb-dialog-head">
        <h2 id="rawxml-title">Roh-XML der E-Rechnung</h2>
        <button
          type="button"
          class="kb-dialog-close"
          aria-label="Dialog schließen"
          onclick={closeXmlViewer}><span aria-hidden="true">×</span></button
        >
      </header>
      <div class="meta">
        <span><strong>Format:</strong> {sourceFormatLabel(payload.sourceFormat)}</span>
        <span><strong>SHA-256:</strong> <code>{payload.sha256Hex}</code></span>
        <span><strong>Größe:</strong> {formatBytes(payload.byteSize)}</span>
      </div>
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <pre class="body" tabindex="0" aria-label="XML-Inhalt">{payload.xml}</pre>
      <!-- U4 (V3-15): „Schließen“ wie „Abbrechen“ zuerst und secondary; die einzige primary
           ist das Ergebnis („XML kopieren“). -->
      <footer class="kb-dialog-foot">
        <button type="button" class="btn-secondary" onclick={closeXmlViewer}>
          Schließen
        </button>
        <button type="button" class="btn-primary" onclick={copyXml}>
          XML kopieren
        </button>
      </footer>
    </div>
  </div>
{/if}

<style>
  /* Rahmen aus tokens.css (.kb-dialog). */
  .xml { --dialog-w: 900px; }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem 1.5rem;
    padding: 0.65rem 1.25rem;
    background: var(--surface-sunken);
    border-bottom: 1px solid var(--line);
    font-size: var(--fs-xs);
    color: var(--text-muted);
    flex: none;
  }
  .meta code {
    background: var(--surface-card);
    padding: 0.05em 0.4em;
    border-radius: var(--r-sm);
    font-size: 0.9em;
    word-break: break-all;
  }
  /* Der XML-Text scrollt in sich (auch waagerecht), nie die Seite. */
  .body {
    flex: 1;
    min-height: 0;
    margin: 0;
    padding: 1rem 1.25rem;
    overflow: auto;
    overscroll-behavior: contain;
    white-space: pre;
    overflow-wrap: normal;
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
    line-height: 1.5;
    color: var(--text);
    background: var(--surface-card);
  }
  .body:focus-visible { outline-offset: var(--focus-offset-inset); }
  /* Niedrige Fenster (U3, Zoom 400 %): Der ganze Dialog scrollt (tokens.css); der
     XML-Text behält eine Mindesthöhe und scrollt in sich weiter. */
  @media screen and (max-height: 420px) {
    .body { min-height: min(12rem, 60dvh); }
  }
</style>
