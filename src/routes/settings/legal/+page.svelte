<script lang="ts">
  import { onMount } from "svelte";
  import PageBar from "$lib/PageBar.svelte";
  import {
    legalDocumentsList,
    legalDocumentsUpload,
    legalDocumentsActivate,
    legalDocumentsDeactivate,
    attachmentsOpen,
  } from "$lib/api";
  import type { LegalDocument } from "$lib/types";
  import { date } from "$lib/format";
  import Banner from "$lib/Banner.svelte";
  import { flash } from "$lib/toast.svelte";

  let docs = $state<LegalDocument[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let busy = $state(false);

  // Upload-Formular je Typ
  const DOC_TYPES = [
    { key: "agb", label: "AGB (Allgemeine Geschäftsbedingungen)" },
    { key: "privacy", label: "Datenschutzerklärung" },
  ] as const;

  let uploadFile = $state<Record<string, File | null>>({ agb: null, privacy: null });
  let uploadTitle = $state<Record<string, string>>({ agb: "", privacy: "" });

  async function load() {
    loading = true;
    error = null;
    try {
      docs = await legalDocumentsList();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  function versionsOf(docType: string): LegalDocument[] {
    return docs.filter((d) => d.docType === docType);
  }

  function activeOf(docType: string): LegalDocument | undefined {
    return docs.find((d) => d.docType === docType && d.isActive === 1);
  }

  async function upload(docType: string) {
    const file = uploadFile[docType];
    if (!file) {
      flash("Bitte eine PDF-Datei wählen.", "error");
      return;
    }
    busy = true;
    try {
      const buf = await file.arrayBuffer();
      const bytes = Array.from(new Uint8Array(buf));
      await legalDocumentsUpload(
        docType,
        uploadTitle[docType].trim() || null,
        bytes,
        file.name,
      );
      uploadFile[docType] = null;
      uploadTitle[docType] = "";
      flash("Version hochgeladen. Zum Verwenden noch aktivieren.");
      await load();
    } catch (e) {
      flash("Upload fehlgeschlagen: " + String(e), "error");
    } finally {
      busy = false;
    }
  }

  async function activate(id: string) {
    busy = true;
    try {
      await legalDocumentsActivate(id);
      flash("Version aktiviert.");
      await load();
    } catch (e) {
      flash("Aktivieren fehlgeschlagen: " + String(e), "error");
    } finally {
      busy = false;
    }
  }

  async function deactivate(id: string) {
    busy = true;
    try {
      await legalDocumentsDeactivate(id);
      flash("Version deaktiviert.");
      await load();
    } catch (e) {
      flash("Deaktivieren fehlgeschlagen: " + String(e), "error");
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

  function fmtBytes(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
    return `${(n / 1024 / 1024).toFixed(1)} MB`;
  }
</script>

<PageBar back="/settings" backLabel="Einstellungen" title="AGB & Datenschutz" />

<p class="muted">
  Hinterlege deine AGB und Datenschutz-Bedingungen als PDF. Beim Versand eines
  Angebots werden sie automatisch beigelegt und fest mit dem Angebot verknüpft —
  als Nachweis, welche Fassung galt. Frühere Fassungen bleiben erhalten und werden
  nie gelöscht. Zum Versenden brauchst du je eine <strong>aktive</strong> AGB- und
  Datenschutz-Fassung.
</p>
<p class="caveat">
  Tipp: Lass deine AGB- und Datenschutz-Texte vor dem echten Einsatz anwaltlich
  prüfen. Klein.Buch verwaltet die Dokumente, ersetzt aber keine Rechtsberatung.
</p>

{#if loading}
  <p class="muted">Lade …</p>
{:else if error}
  <Banner>{error}</Banner>
{:else}
  {#each DOC_TYPES as t (t.key)}
    {@const active = activeOf(t.key)}
    {@const versions = versionsOf(t.key)}
    <section class="card">
      <header class="sec-hdr">
        <h2>{t.label}</h2>
        {#if active}
          <span class="badge ok">Aktiv: v{active.version}</span>
        {:else}
          <span class="badge warn">Keine aktive Version</span>
        {/if}
      </header>

      <div class="upload">
        <label>
          Neue Version (PDF)
          <input
            type="file"
            accept="application/pdf"
            onchange={(e) => (uploadFile[t.key] = (e.currentTarget as HTMLInputElement).files?.[0] ?? null)}
          />
        </label>
        <label>
          Titel (optional)
          <input type="text" bind:value={uploadTitle[t.key]} placeholder={`z. B. ${t.label} Stand ${new Date().toLocaleDateString("de-DE")}`} />
        </label>
        <button class="btn-primary" onclick={() => upload(t.key)} disabled={busy || !uploadFile[t.key]}>
          {busy ? "…" : "Hochladen"}
        </button>
      </div>

      {#if versions.length === 0}
        <p class="muted">Noch keine Version hochgeladen.</p>
      {:else}
        <table>
          <thead>
            <tr>
              <th>Version</th>
              <th>Titel</th>
              <th>Datei</th>
              <th>Hochgeladen</th>
              <th>Status</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {#each versions as d (d.id)}
              <tr class={d.isActive === 1 ? "active-row" : ""}>
                <td>v{d.version}</td>
                <td>{d.title}</td>
                <td class="muted">{d.fileName} · {fmtBytes(d.fileSizeBytes)}</td>
                <td>{date(d.createdAt)}</td>
                <td>
                  {#if d.isActive === 1}<span class="badge ok">aktiv</span>{:else}<span class="badge muted-badge">inaktiv</span>{/if}
                </td>
                <td class="row-actions">
                  <button class="btn-secondary btn-sm" onclick={() => open(d.archiveEntryId)}>Öffnen</button>
                  {#if d.isActive === 1}
                    <button class="btn-secondary btn-sm" onclick={() => deactivate(d.id)} disabled={busy}>Deaktivieren</button>
                  {:else}
                    <button class="btn-primary btn-sm" onclick={() => activate(d.id)} disabled={busy}>Aktivieren</button>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </section>
  {/each}
{/if}

<style>
  /* .card entfernt — globale Definition aus tokens.css. */
  .sec-hdr { display: flex; justify-content: space-between; align-items: center; }
  .upload { display: grid; grid-template-columns: 1fr 1fr auto; gap: 0.75rem; align-items: end; margin: 0.75rem 0 1rem; }
  label { display: flex; flex-direction: column; font-size: 0.85rem; color: #4b5563; gap: 0.25rem; }
  input { padding: 0.4rem 0.5rem; border: 1px solid #d1d5db; border-radius: 4px; font-size: 0.95rem; font-family: inherit; }
  table { width: 100%; border-collapse: collapse; }
  th, td { padding: 0.4rem; text-align: left; border-bottom: 1px solid #e5e7eb; font-size: 0.9rem; }
  th { background: #f3f4f6; font-weight: 600; font-size: 0.8rem; }
  .active-row { background: #f0fdf4; }
  .row-actions { display: flex; gap: 0.4rem; justify-content: flex-end; }
  .badge { padding: 0.1rem 0.5rem; border-radius: 4px; font-size: 0.75rem; }
  .badge.ok { background: #d1fae5; color: #065f46; }
  .badge.warn { background: #fef3c7; color: #92400e; }
  .muted-badge { background: #e5e7eb; color: #374151; }
  /* Pre-DS-Style-Block entfernt (G2-UX.3.x Konsistenz-Fix): tokens.css greift. */
  button:disabled { opacity: 0.6; cursor: not-allowed; }
  .muted { color: #6b7280; }
  /* .caveat entfernt — globale Definition aus tokens.css greift. */
</style>
