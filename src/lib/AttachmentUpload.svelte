<script lang="ts">
  import { attachmentsAdd, saveArchiveFile, uploadFile } from "#lib/api.ts";
  import type { AttachmentView } from "#lib/types.ts";
  import { flash } from "#lib/toast.svelte.ts";
  import Button from "#lib/Button.svelte";
  import { randomId } from "#lib/browser.ts";

  // Zulässige Typen wie im Server (uploads.rs, UploadKind::Attachment = DOCS).
  // Bewusst ohne `capture`: so bietet das Handy Kamera, Galerie und Dateien an.
  const ACCEPT =
    "application/pdf,.pdf,image/png,image/jpeg,image/webp,image/gif,image/tiff,.tif,.tiff,application/xml,text/xml,.xml";
  const uid = randomId();

  let {
    parentType,
    parentId,
    attachments = [],
    onchange = undefined,
  }: {
    parentType: string;
    parentId: string;
    attachments?: AttachmentView[];
    onchange?: (list: AttachmentView[]) => void;
  } = $props();

  let file = $state<File | null>(null);
  let label = $state("");
  let busy = $state(false);
  let fileInput = $state<HTMLInputElement | null>(null);
  /** U4 (ADR 0041 Nr. 135): „Anhängen“ bleibt bedienbar; ohne Datei steht der Grund am Feld. */
  let fileError = $state("");

  const SIZE_DE = new Intl.NumberFormat("de-DE", { minimumFractionDigits: 1, maximumFractionDigits: 1 });
  function fmtBytes(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${SIZE_DE.format(n / 1024)} KB`;
    return `${SIZE_DE.format(n / 1024 / 1024)} MB`;
  }

  async function upload() {
    if (busy) return;
    if (!file) {
      fileError = "Bitte wähl zuerst eine Datei aus.";
      fileInput?.focus();
      return;
    }
    fileError = "";
    busy = true;
    try {
      const { uploadId } = await uploadFile("anhang", file);
      const list = await attachmentsAdd({
        parentType,
        parentId,
        uploadId,
        label: label.trim() || null,
      });
      file = null;
      label = "";
      if (fileInput) fileInput.value = "";
      flash("Anhang hinzugefügt.");
      onchange?.(list);
    } catch (e) {
      flash(`Anhang hinzufügen fehlgeschlagen: ${String(e)}`, "error");
    } finally {
      busy = false;
    }
  }

  function open(archiveEntryId: string, fileName: string) {
    saveArchiveFile(archiveEntryId, fileName);
  }
</script>

<div class="att">
  {#if attachments.length === 0}
    <p class="kb-muted empty">Keine zusätzlichen Anhänge.</p>
  {:else}
    <ul class="list">
      {#each attachments as a (a.id)}
        <li>
          <span class="fname">{a.fileName}</span>
          <span class="kb-subtle meta">· {fmtBytes(a.fileSizeBytes)}{a.label ? ` · ${a.label}` : ""}</span>
          <!-- U4 (design.md §4.1): ghost nie allein; Name mit Dateiname (mehrere Anhänge). -->
          <Button variant="secondary" onclick={() => open(a.archiveEntryId, a.fileName)} ariaLabel={`Herunterladen: ${a.fileName}`}>Herunterladen</Button>
        </li>
      {/each}
    </ul>
  {/if}

  <div class="row">
    <div class="fld file">
      <label for="att-file-{uid}">Datei</label>
      <input
        id="att-file-{uid}"
        bind:this={fileInput}
        type="file"
        accept={ACCEPT}
        aria-invalid={fileError ? "true" : undefined}
        aria-describedby={fileError ? `att-err-${uid} att-hint-${uid}` : `att-hint-${uid}`}
        onchange={(e) => {
          file = (e.currentTarget as HTMLInputElement).files?.[0] ?? null;
          if (file) fileError = "";
        }}
      />
      {#if fileError}<span id="att-err-{uid}" class="err">{fileError}</span>{/if}
      <small id="att-hint-{uid}" class="kb-muted hint">PDF, Bild (PNG, JPEG, WebP, GIF, TIFF) oder XML, höchstens 20 MB.</small>
    </div>
    <div class="fld name">
      <label for="att-label-{uid}">Bezeichnung (optional)</label>
      <input
        id="att-label-{uid}"
        class="kb-input"
        type="text"
        bind:value={label}
        autocomplete="off"
        enterkeyhint="done"
      />
    </div>
    <Button variant="secondary" onclick={upload} busy={busy}>
      {busy ? "Hängt an …" : "Anhängen"}
    </Button>
  </div>
</div>

<style>
  .att { display: flex; flex-direction: column; gap: 0.5rem; }
  .empty { font-size: var(--type-body); margin: 0; }
  .list { list-style: none; padding: 0; margin: 0; }
  .list li {
    padding: 0.25rem 0; font-size: var(--type-body);
    display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap;
  }
  .fname { font-weight: 500; }
  .meta { font-size: var(--type-small); }
  .row { display: flex; gap: var(--sp-3); align-items: flex-end; flex-wrap: wrap; }
  .fld { display: flex; flex-direction: column; gap: var(--sp-1); min-width: 0; }
  .fld label { font-size: var(--fs-sm); font-weight: 600; }
  .fld.file { flex: 1 1 16rem; }
  .fld.name { flex: 1 1 12rem; }
  .hint { font-size: var(--fs-xs); }
  .err { font-size: var(--fs-sm); font-weight: 600; color: var(--status-danger-fg); }
</style>
