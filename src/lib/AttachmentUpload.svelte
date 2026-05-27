<script lang="ts">
  import { attachmentsAdd, attachmentsOpen } from "$lib/api";
  import type { AttachmentView } from "$lib/types";
  import { flash } from "$lib/toast.svelte";
  import Button from "$lib/Button.svelte";

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
      const buf = await file.arrayBuffer();
      const bytes = Array.from(new Uint8Array(buf));
      const list = await attachmentsAdd({
        parentType,
        parentId,
        fileBytes: bytes,
        fileName: file.name,
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

  async function open(archiveEntryId: string) {
    try {
      await attachmentsOpen(archiveEntryId);
    } catch (e) {
      flash(String(e), "error");
    }
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
          <Button variant="ghost" size="sm" onclick={() => open(a.archiveEntryId)}>Öffnen</Button>
        </li>
      {/each}
    </ul>
  {/if}

  <div class="row">
    <input
      bind:this={fileInput}
      type="file"
      onchange={(e) => (file = (e.currentTarget as HTMLInputElement).files?.[0] ?? null)}
    />
    <input class="kb-input label-in" type="text" bind:value={label} placeholder="Bezeichnung (optional)" />
    <Button variant="secondary" size="sm" onclick={upload} disabled={busy || !file}>
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
  .row { display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; }
  .label-in { width: auto; flex: 1; min-width: 12rem; }
</style>
