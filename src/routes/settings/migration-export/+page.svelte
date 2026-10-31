<script lang="ts">
  // Block 4 — Migrations-Export (offenes ZIP aller Daten); seit Plan W7 als
  // Download (einmalig abrufbar, 1 h).
  import PageBar from "#lib/PageBar.svelte";
  import Banner from "#lib/Banner.svelte";
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
  <Banner kind="warning">
    Achtung: Diese Datei ist <strong>nicht verschlüsselt</strong> und enthält alle
    deine Daten lesbar. Bewahre sie sicher auf.
  </Banner>
  <p class="text hint">
    Der Download lässt sich nur einmal abrufen. Bricht er ab, erstelle den Export einfach neu.
  </p>

  <!-- U4 (V2-06): während des Exports fokussierbar (ariaDisabled, ADR 0041 Nr. 33). -->
  <Button onclick={runExport} busy={busy}>
    {busy ? "Exportiere …" : "Export erstellen und herunterladen"}
  </Button>

  {#if report}
    <Banner kind="success">
      <p class="report-line">Export heruntergeladen: <strong class="file">{report.download?.fileName ?? ""}</strong></p>
      <ul class="report-list">
        <li>{INT_DE.format(report.tableCount)} Datenbereiche, {INT_DE.format(report.totalRows)} Einträge gesamt</li>
        <li>{INT_DE.format(report.archiveFileCount)} Belege/Dateien</li>
        <li>{KB_DE.format(report.zipSizeBytes / 1024)}&nbsp;KB</li>
      </ul>
    </Banner>
  {/if}
</section>

<style>
  /* .card — globale Definition aus tokens.css; Hinweise über Banner (V3-18). */
  .card { max-width: 48rem; }
  .text { color: var(--text); line-height: 1.5; }
  .hint { color: var(--text-muted); font-size: var(--fs-sm); }
  /* Ergebnis des Exports als Banner „success“ (V3-18, ADR 0042 Nr. 16). */
  .report-line { margin: 0 0 0.4rem; }
  .report-list { margin: 0; padding-left: 1.2rem; }
  .file { overflow-wrap: anywhere; }
</style>
