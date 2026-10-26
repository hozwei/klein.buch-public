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

  function fmtBytes(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
    return `${(n / 1024 / 1024).toFixed(1)} MB`;
  }

  async function upload() {
    if (!file) {
      flash("Bitte eine Datei wählen.", "error");
      return;
    }
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
      flash(String(e), "error");
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
          <Button variant="ghost" size="sm" onclick={() => open(a.archiveEntryId, a.fileName)}>Herunterladen</Button>
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
        aria-describedby="att-hint-{uid}"
        onchange={(e) => (file = (e.currentTarget as HTMLInputElement).files?.[0] ?? null)}
      />
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
    <Button variant="secondary" size="sm" onclick={upload} disabled={busy || !file} ariaLabel={busy ? "Wird angehängt" : undefined}>
      {busy ? "…" : "Anhängen"}
    </Button>
  </div>
</div>

<style>
  .att { display: flex; flex-direction: column; gap: 0.5rem; }
  .empty { font-size: 0.9rem; margin: 0; }
  .list { list-style: none; padding: 0; margin: 0; }
  .list li {
    padding: 0.25rem 0; font-size: 0.9rem;
    display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap;
  }
  .fname { font-weight: 500; }
  .meta { font-size: 0.85rem; }
  .row { display: flex; gap: var(--sp-3); align-items: flex-end; flex-wrap: wrap; }
  .fld { display: flex; flex-direction: column; gap: var(--sp-1); min-width: 0; }
  .fld label { font-size: var(--fs-sm); font-weight: 600; }
  .fld.file { flex: 1 1 16rem; }
  .fld.name { flex: 1 1 12rem; }
  .hint { font-size: var(--fs-xs); }
</style>
