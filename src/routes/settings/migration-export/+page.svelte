<script lang="ts">
  // Block 4 — Migrations-Export (offenes ZIP aller Daten); seit Plan W7 als
  // Download (einmalig abrufbar, 1 h).
  import PageBar from "#lib/PageBar.svelte";
  import Button from "#lib/Button.svelte";
  import { flash } from "#lib/toast.svelte.ts";
  import { migrationExportRun, saveDownload, type ExportReport } from "#lib/api.ts";

  let busy = $state(false);
  let report = $state<ExportReport | null>(null);

  const KB_DE = new Intl.NumberFormat("de-DE", { minimumFractionDigits: 1, maximumFractionDigits: 1 });
  const INT_DE = new Intl.NumberFormat("de-DE");

  async function runExport() {
    if (busy) return;
    busy = true;
    report = null;
    try {
      report = await migrationExportRun();
      if (report.download) saveDownload(report.download);
      flash("Export erstellt.");
    } catch (e) {
      flash(`Export fehlgeschlagen: ${String(e)}`, "error");
    } finally {
      busy = false;
    }
  }
</script>

<PageBar back="/settings" backLabel="Einstellungen" title="Daten exportieren" />

<section class="card">
  <p class="text">
    Speichert <strong>alle</strong> deine Daten in einer ZIP-Datei — Rechnungen,
    Kosten, Belege, Kontakte und Einstellungen. So kommst du jederzeit an deine
    Daten, auch unabhängig von Klein.Buch (z. B. für den Steuerberater oder einen
    Programm&shy;wechsel).
  </p>
  <p class="caveat">
    Achtung: Diese Datei ist <strong>nicht verschlüsselt</strong> und enthält alle
    deine Daten lesbar. Bewahre sie sicher auf.
  </p>
  <p class="text hint">
    Der Download lässt sich nur einmal abrufen. Bricht er ab, erstelle den Export einfach neu.
  </p>

  <!-- U4 (V2-06): während des Exports fokussierbar (ariaDisabled, ADR 0041 Nr. 33). -->
  <Button onclick={runExport} busy={busy}>
    {busy ? "Exportiere …" : "Export erstellen und herunterladen"}
  </Button>

  {#if report}
    <div class="report" role="status">
      <p>Export heruntergeladen: <strong class="file">{report.download?.fileName ?? ""}</strong></p>
      <ul>
        <li>{INT_DE.format(report.tableCount)} Datenbereiche, {INT_DE.format(report.totalRows)} Einträge gesamt</li>
        <li>{INT_DE.format(report.archiveFileCount)} Belege/Dateien</li>
        <li>{KB_DE.format(report.zipSizeBytes / 1024)}&nbsp;KB</li>
      </ul>
    </div>
  {/if}
</section>

<style>
  /* .card / .caveat — globale Definitionen aus tokens.css. */
  .card { max-width: 48rem; }
  .text { color: var(--text); line-height: 1.5; }
  .hint { color: var(--text-muted); font-size: var(--fs-sm); }
  .report {
    margin-top: 1rem;
    padding: 0.8rem;
    background: var(--status-success-bg);
    border: 1px solid var(--status-success-line);
    border-radius: var(--r-md);
    color: var(--status-success-fg);
  }
  .report p { margin: 0 0 0.4rem; }
  .report ul { margin: 0; padding-left: 1.2rem; }
  .file { overflow-wrap: anywhere; }
</style>
