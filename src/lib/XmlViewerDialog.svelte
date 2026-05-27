<script lang="ts">
  // PV1-A5: Roh-XML-Viewer-Dialog. Genau einmal im Root-Layout gemountet (siehe
  // routes/+layout.svelte) — außerhalb von `<main>`, damit der Dialog nicht durch
  // `inert={modalStack.count>0}` mitgesperrt wird (AboutDialog-Pattern).

  import { xmlViewerStore, closeXmlViewer } from "$lib/xmlViewerModal.svelte";
  import { pushModal, popModal } from "$lib/modalStack.svelte";
  import { flash } from "$lib/toast.svelte";

  let closeBtn = $state<HTMLButtonElement | null>(null);
  let triggerEl: HTMLElement | null = null;

  $effect(() => {
    if (xmlViewerStore.visible) {
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
    if (!xmlViewerStore.visible) return;
    if (e.key === "Escape") {
      e.preventDefault();
      closeXmlViewer();
    }
  }

  async function copyXml(): Promise<void> {
    const payload = xmlViewerStore.payload;
    if (!payload) return;
    try {
      await navigator.clipboard.writeText(payload.xml);
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

<svelte:window onkeydown={onKeydown} />

{#if xmlViewerStore.visible && xmlViewerStore.payload}
  {@const payload = xmlViewerStore.payload}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div
    class="backdrop"
    role="presentation"
    onclick={(e) => {
      if (e.target === e.currentTarget) closeXmlViewer();
    }}
  >
    <div
      class="dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="rawxml-title"
    >
      <header>
        <h2 id="rawxml-title">Roh-XML der E-Rechnung</h2>
        <button
          bind:this={closeBtn}
          type="button"
          class="close"
          aria-label="Dialog schließen"
          onclick={closeXmlViewer}>×</button
        >
      </header>
      <div class="meta">
        <span><strong>Format:</strong> {sourceFormatLabel(payload.sourceFormat)}</span>
        <span><strong>SHA-256:</strong> <code>{payload.sha256Hex}</code></span>
        <span><strong>Größe:</strong> {formatBytes(payload.byteSize)}</span>
      </div>
      <pre class="body">{payload.xml}</pre>
      <footer>
        <button type="button" class="btn-secondary btn-sm" onclick={copyXml}>
          XML kopieren
        </button>
        <button type="button" class="btn-primary btn-sm" onclick={closeXmlViewer}>
          Schließen
        </button>
      </footer>
    </div>
  </div>
{/if}

<style>
  /* Sheet-Look analog AboutDialog. */
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
    width: min(900px, 100%);
    max-height: calc(100vh - 4rem);
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
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem 1.5rem;
    padding: 0.65rem 1.25rem;
    background: var(--c-surface-2);
    border-bottom: 1px solid var(--c-border);
    font-size: var(--fs-xs);
    color: var(--c-text-muted);
  }
  .meta code {
    background: var(--c-surface);
    padding: 0.05em 0.4em;
    border-radius: var(--r-sm);
    font-size: 0.9em;
    word-break: break-all;
  }
  .body {
    flex: 1;
    margin: 0;
    padding: 1rem 1.25rem;
    overflow: auto;
    white-space: pre;
    font-family: var(--font-mono, ui-monospace, "SF Mono", Menlo, Consolas, monospace);
    font-size: var(--fs-xs);
    line-height: 1.5;
    color: var(--c-text);
    background: var(--c-surface);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    padding: 0.75rem 1.25rem;
    border-top: 1px solid var(--c-border);
    background: var(--c-surface-2);
  }
</style>
