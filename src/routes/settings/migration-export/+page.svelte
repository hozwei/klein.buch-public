<script lang="ts">
  // Block 4 — Migrations-Export (offenes ZIP aller Daten).
  import { onMount } from "svelte";
  import PageBar from "$lib/PageBar.svelte";
  import { flash } from "$lib/toast.svelte";
  import {
    backupGetSettings,
    migrationExportRun,
    type ExportReport,
  } from "$lib/api";

  let targetPath = $state("");
  let busy = $state(false);
  let report = $state<ExportReport | null>(null);

  onMount(async () => {
    try {
      const s = await backupGetSettings();
      const stamp = new Date().toISOString().slice(0, 10);
      const sep = s.defaultSuggestion.includes("\\") ? "\\" : "/";
      // Standard-Vorschlag: neben dem Backup-Ordner ablegen.
      targetPath = `${s.defaultSuggestion}${sep}klein-buch-export-${stamp}.zip`;
    } catch {
      targetPath = "klein-buch-export.zip";
    }
  });

  async function runExport() {
    busy = true;
    report = null;
    try {
      report = await migrationExportRun(targetPath.trim());
      flash("Export erstellt.");
    } catch (e) {
      flash(String(e), "error");
    } finally {
      busy = false;
    }
  }
</script>

<PageBar back="/settings" backLabel="Einstellungen" title="Daten exportieren" />

<section class="card">
  <p class="muted">
    Speichert <strong>alle</strong> deine Daten in einer ZIP-Datei — Rechnungen,
    Kosten, Belege, Kontakte und Einstellungen. So kommst du jederzeit an deine
    Daten, auch unabhängig von Klein.Buch (z. B. für den Steuerberater oder einen
    Programm&shy;wechsel).
  </p>
  <p class="caveat">
    Achtung: Diese Datei ist <strong>nicht verschlüsselt</strong> und enthält alle
    deine Daten lesbar. Bewahre sie sicher auf.
  </p>

  <label>
    Speicherort (.zip-Datei)
    <input type="text" bind:value={targetPath} />
  </label>
  <button class="primary" onclick={runExport} disabled={busy || !targetPath.trim()}>
    {busy ? "Exportiere …" : "Export erstellen"}
  </button>

  {#if report}
    <div class="report">
      <p>Export erstellt: <strong>{report.zipPath}</strong></p>
      <ul>
        <li>{report.tableCount} Datenbereiche, {report.totalRows} Einträge gesamt</li>
        <li>{report.archiveFileCount} Belege/Dateien</li>
        <li>{(report.zipSizeBytes / 1024).toFixed(1)} KB</li>
      </ul>
    </div>
  {/if}
</section>

<style>
  /* .card / .warn entfernt — globale Definitionen aus tokens.css (.card, .caveat). */
  .card {
    max-width: 48rem;
  }
  .muted {
    color: var(--c-text);
    font-size: 0.92rem;
    line-height: 1.5;
  }
  label {
    display: block;
    margin: 0.8rem 0 0.2rem;
    font-weight: 600;
    font-size: 0.85rem;
  }
  input {
    width: 100%;
    box-sizing: border-box;
    padding: 0.5rem 0.65rem;
    border: 1px solid #d1d5db;
    border-radius: 6px;
    font-size: 0.95rem;
  }
  button.primary {
    margin-top: 0.9rem;
    padding: 0.55rem 1rem;
    background: var(--c-primary-600);
    color: #fff;
    border: none;
    border-radius: var(--r-md);
    cursor: pointer;
  }
  button.primary:disabled {
    opacity: 0.55;
    cursor: default;
  }
  .report {
    margin-top: 1rem;
    padding: 0.8rem;
    background: #d1fae5;
    border: 1px solid #6ee7b7;
    border-radius: 6px;
    color: #065f46;
    word-break: break-all;
  }
</style>
