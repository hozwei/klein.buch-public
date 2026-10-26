<script lang="ts">
  import { onMount } from "svelte";
  import PageBar from "#lib/PageBar.svelte";
  import {
    legalDocumentsList,
    legalDocumentsUpload,
    legalDocumentsActivate,
    legalDocumentsDeactivate,
    saveArchiveFile,
    uploadFile as sendUpload,
  } from "#lib/api.ts";
  import type { LegalDocument } from "#lib/types.ts";
  import { date } from "#lib/format.ts";
  import { todayBerlin } from "#lib/date.ts";
  import Badge from "#lib/Badge.svelte";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import { flash } from "#lib/toast.svelte.ts";

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
      const { uploadId } = await sendUpload("rechtsdokument", file);
      await legalDocumentsUpload(docType, uploadTitle[docType].trim() || null, uploadId);
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

  function open(archiveEntryId: string) {
    saveArchiveFile(archiveEntryId);
  }

  const SIZE_DE = new Intl.NumberFormat("de-DE", { minimumFractionDigits: 1, maximumFractionDigits: 1 });
  function fmtBytes(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${SIZE_DE.format(n / 1024)} KB`;
    return `${SIZE_DE.format(n / 1024 / 1024)} MB`;
  }
</script>

<PageBar back="/settings" backLabel="Einstellungen" title="AGB & Datenschutz" />

<p class="lead">
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
  <p class="muted" aria-busy="true">Lädt …</p>
{:else if error}
  <Banner>{error}</Banner>
{:else}
  {#each DOC_TYPES as t (t.key)}
    {@const active = activeOf(t.key)}
    {@const versions = versionsOf(t.key)}
    <section class="card" aria-labelledby="legal-{t.key}">
      <header class="sec-hdr">
        <h2 id="legal-{t.key}">{t.label}</h2>
        {#if active}
          <Badge tone="success">Aktiv: v{active.version}</Badge>
        {:else}
          <Badge tone="warning">Keine aktive Version</Badge>
        {/if}
      </header>

      <div class="upload">
        <label>
          Neue Version (PDF)
          <input
            type="file"
            accept="application/pdf,.pdf"
            onchange={(e) => (uploadFile[t.key] = (e.currentTarget as HTMLInputElement).files?.[0] ?? null)}
          />
        </label>
        <label>
          Titel (optional)
          <input type="text" autocomplete="off" bind:value={uploadTitle[t.key]} placeholder={`z. B. ${t.label} Stand ${date(todayBerlin())}`} />
        </label>
        <Button onclick={() => upload(t.key)} disabled={busy || !uploadFile[t.key]}>
          {busy ? "Lädt hoch …" : "Hochladen"}
        </Button>
      </div>

      {#if versions.length === 0}
        <p class="muted">Noch keine Version hochgeladen. Wähle oben eine PDF-Datei und tippe auf „Hochladen“.</p>
      {:else}
        <table class="cards">
          <caption class="kb-visually-hidden">Versionen: {t.label}</caption>
          <thead>
            <tr>
              <th>Version</th>
              <th>Titel</th>
              <th>Datei</th>
              <th>Hochgeladen</th>
              <th>Status</th>
              <th><span class="kb-visually-hidden">Aktionen</span></th>
            </tr>
          </thead>
          <tbody>
            {#each versions as d (d.id)}
              <tr class:active-row={d.isActive === 1}>
                <td data-label="Version">v{d.version}</td>
                <td data-label="Titel">{d.title}</td>
                <td class="muted" data-label="Datei"><span class="fname">{d.fileName}</span> · {fmtBytes(d.fileSizeBytes)}</td>
                <td data-label="Hochgeladen">{date(d.createdAt)}</td>
                <td data-label="Status">
                  {#if d.isActive === 1}<Badge tone="success">aktiv</Badge>{:else}<Badge>inaktiv</Badge>{/if}
                </td>
                <td>
                  <div class="row-actions">
                    <Button variant="secondary" onclick={() => open(d.archiveEntryId)} ariaLabel={`Herunterladen, Version ${d.version}`}>Herunterladen</Button>
                    {#if d.isActive === 1}
                      <Button variant="secondary" onclick={() => deactivate(d.id)} disabled={busy} ariaLabel={`Deaktivieren, Version ${d.version}`}>Deaktivieren</Button>
                    {:else}
                      <Button variant="secondary" onclick={() => activate(d.id)} disabled={busy} ariaLabel={`Aktivieren, Version ${d.version}`}>Aktivieren</Button>
                    {/if}
                  </div>
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
  /* .card / .lead / .caveat — globale Definitionen aus tokens.css. */
  .sec-hdr { display: flex; flex-wrap: wrap; gap: 0.5rem; justify-content: space-between; align-items: center; }
  .sec-hdr h2 { margin: 0; }
  .upload { display: grid; grid-template-columns: 1fr 1fr auto; gap: 0.75rem; align-items: end; margin: 0.75rem 0 1rem; }
  label { display: flex; flex-direction: column; font-size: var(--fs-sm); font-weight: 600; color: var(--text); gap: 0.25rem; min-width: 0; }
  input[type="text"] { padding: 0.4rem 0.5rem; border: 1px solid var(--line-control); border-radius: var(--r-sm); font: inherit; font-weight: 400; min-height: var(--target); }
  table { width: 100%; border-collapse: collapse; }
  th, td { text-align: left; font-size: var(--fs-sm); }
  .active-row td { background: var(--status-success-bg); }
  .fname { overflow-wrap: anywhere; }
  .row-actions { display: flex; flex-wrap: wrap; gap: 0.4rem; justify-content: flex-end; }
  .muted { color: var(--text-muted); }
  @media (max-width: 600px) {
    .upload { grid-template-columns: 1fr; }
  }
  @media screen and (max-width: 820px) {
    td { overflow-wrap: anywhere; }
  }
</style>
