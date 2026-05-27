<script lang="ts">
  // Block 14a — EÜR-Export (Selbst-Abgabe): Anlage EÜR + Anlageverzeichnis
  // (AVEÜR) + Einzelaufstellung. Anzeige + CSV/ZIP. (Das druckbare Typst-PDF
  // folgt in Schritt 2.)
  import { onMount } from "svelte";
  import {
    euerAvailableYears,
    euerPackage,
    euerExportElster,
    euerExportDetailZip,
    euerExportPdf,
    euerExportDatev,
    euerExportStbZip,
    euerRevealPath,
    euerAfaPending,
    depreciationAccrueYear,
    backupGetSettings,
  } from "$lib/api";
  import type { EuerPackage } from "$lib/types";
  import { euro } from "$lib/format";
  import { expenseCategoryLabel, depreciationMethodShort } from "$lib/labels";
  import { flash } from "$lib/toast.svelte";
  import Banner from "$lib/Banner.svelte";
  import Button from "$lib/Button.svelte";
  import HelpAnchor from "$lib/HelpAnchor.svelte";
  import PageBar from "$lib/PageBar.svelte";

  let years = $state<number[]>([]);
  let selectedYear = $state<number | null>(null);
  let pkg = $state<EuerPackage | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);

  let defaultDir = $state("");
  let pdfPath = $state("");
  let csvPath = $state("");
  let zipPath = $state("");
  let datevPath = $state("");
  let stbPath = $state("");
  let busyPdf = $state(false);
  let busyCsv = $state(false);
  let busyZip = $state(false);
  let busyDatev = $state(false);
  let busyStb = $state(false);
  let lastPdfPath = $state<string | null>(null);
  let lastCsvPath = $state<string | null>(null);
  let lastZipPath = $state<string | null>(null);
  let lastDatevPath = $state<string | null>(null);
  let lastStbPath = $state<string | null>(null);
  let afaPending = $state(0);
  let bookingAfa = $state(false);

  // Kontenrahmen für den DATEV-Export (gemerkt; Default SKR03).
  let skr = $state(
    (typeof localStorage !== "undefined" && localStorage.getItem("datevSkr")) || "SKR03",
  );
  $effect(() => {
    if (typeof localStorage !== "undefined") localStorage.setItem("datevSkr", skr);
  });

  const currentYear = new Date().getFullYear();
  let canBookAfa = $derived(selectedYear != null && selectedYear <= currentYear);

  onMount(async () => {
    try {
      years = await euerAvailableYears();
      selectedYear = years[0] ?? new Date().getFullYear();
    } catch (e) {
      error = String(e);
      loading = false;
    }
    try {
      const s = await backupGetSettings();
      defaultDir = s.defaultSuggestion;
    } catch {
      defaultDir = "";
    }
  });

  $effect(() => {
    const y = selectedYear;
    if (y == null) return;
    loading = true;
    error = null;
    Promise.all([euerPackage(y), euerAfaPending(y)])
      .then(([p, ap]) => {
        pkg = p;
        afaPending = ap.pendingCount;
      })
      .catch((e) => {
        error = String(e);
        pkg = null;
      })
      .finally(() => {
        loading = false;
      });
  });

  $effect(() => {
    const y = selectedYear;
    if (y == null) return;
    const sep = defaultDir.includes("\\") ? "\\" : "/";
    const base = defaultDir ? `${defaultDir}${sep}` : "";
    pdfPath = `${base}klein-buch-EUER-${y}.pdf`;
    csvPath = `${base}klein-buch-EUER-${y}-ELSTER.csv`;
    zipPath = `${base}klein-buch-EUER-${y}-Einzelaufstellung.zip`;
    datevPath = `${base}klein-buch-EUER-${y}-DATEV.csv`;
    stbPath = `${base}klein-buch-EUER-${y}-Steuerberater.zip`;
  });

  let form = $derived(pkg?.form ?? null);
  let entryLines = $derived(form ? form.lines.filter((l) => l.isEntry) : []);
  let sumLines = $derived(form ? form.lines.filter((l) => !l.isEntry) : []);
  let incomeSum = $derived((pkg?.income ?? []).reduce((s, i) => s + i.amountCents, 0));
  let expenseSum = $derived((pkg?.expenses ?? []).reduce((s, e) => s + e.grossCents, 0));

  async function exportPdf() {
    if (selectedYear == null) return;
    busyPdf = true;
    try {
      const r = await euerExportPdf(selectedYear, pdfPath.trim());
      lastPdfPath = r.pdfPath;
      flash(`PDF gespeichert (${(r.sizeBytes / 1024).toFixed(0)} KB).`);
    } catch (e) {
      flash(String(e), "error");
    } finally {
      busyPdf = false;
    }
  }

  async function exportDatev() {
    if (selectedYear == null) return;
    busyDatev = true;
    try {
      const r = await euerExportDatev(selectedYear, skr, datevPath.trim());
      lastDatevPath = r.csvPath;
      flash(`DATEV-Buchungsstapel gespeichert (${r.bookingCount} Buchungen, ${skr}).`);
    } catch (e) {
      flash(String(e), "error");
    } finally {
      busyDatev = false;
    }
  }

  async function exportStb() {
    if (selectedYear == null) return;
    busyStb = true;
    try {
      const r = await euerExportStbZip(selectedYear, skr, stbPath.trim());
      lastStbPath = r.zipPath;
      flash(`Steuerberater-Paket gespeichert (${r.fileCount} Dateien).`);
    } catch (e) {
      flash(String(e), "error");
    } finally {
      busyStb = false;
    }
  }

  async function exportCsv() {
    if (selectedYear == null) return;
    busyCsv = true;
    try {
      const r = await euerExportElster(selectedYear, csvPath.trim());
      lastCsvPath = r.csvPath;
      flash(`ELSTER-CSV gespeichert (${r.entryCount} Positionen).`);
    } catch (e) {
      flash(String(e), "error");
    } finally {
      busyCsv = false;
    }
  }

  async function exportZip() {
    if (selectedYear == null) return;
    busyZip = true;
    try {
      const r = await euerExportDetailZip(selectedYear, zipPath.trim());
      lastZipPath = r.zipPath;
      flash(
        `Einzelaufstellung gespeichert (${r.incomeCount} Einnahmen, ${r.expenseCount} Ausgaben, ${r.assetCount} Anlagen).`,
      );
    } catch (e) {
      flash(String(e), "error");
    } finally {
      busyZip = false;
    }
  }

  async function revealPath(p: string) {
    try {
      await euerRevealPath(p);
    } catch (e) {
      flash(String(e), "error");
    }
  }

  async function copyPath(p: string) {
    try {
      await navigator.clipboard.writeText(p);
      flash("Pfad kopiert.");
    } catch {
      flash("Kopieren nicht möglich.", "error");
    }
  }

  async function bookAfa() {
    if (selectedYear == null) return;
    bookingAfa = true;
    try {
      const r = await depreciationAccrueYear(selectedYear);
      if (r.skippedLocked) {
        flash("Bitte zuerst die Sicherung (Backup) entsperren, dann AfA buchen.", "error");
      } else {
        flash(
          `AfA gebucht: ${r.bookedEntries} Buchung(en) für ${r.processedAssets} Anlage(n).`,
        );
        const [p, ap] = await Promise.all([
          euerPackage(selectedYear),
          euerAfaPending(selectedYear),
        ]);
        pkg = p;
        afaPending = ap.pendingCount;
      }
    } catch (e) {
      flash(String(e), "error");
    } finally {
      bookingAfa = false;
    }
  }
</script>

<PageBar back="/euer" backLabel="Steuer-Übersicht" title={`EÜR-Export ${selectedYear ?? ""}`}>
  {#snippet actions()}
    {#if years.length > 0 && selectedYear != null}
      <label class="year">
        Geschäftsjahr
        <select bind:value={selectedYear}>
          {#each years as y}<option value={y}>{y}</option>{/each}
        </select>
      </label>
    {/if}
    <HelpAnchor slug="euer-export" />
  {/snippet}
</PageBar>

<p class="lead">
  Vollständige EÜR für ein Geschäftsjahr: die <strong>Anlage EÜR</strong> (zum
  Übertragen in Mein&nbsp;ELSTER), das <strong>Anlageverzeichnis (AVEÜR)</strong>
  und die <strong>Einzelaufstellung</strong> aller Einnahmen und Ausgaben (für deine
  Unterlagen und eine etwaige Betriebsprüfung). ELSTER hat keinen CSV-Import — du
  überträgst die Anlage-EÜR-Summen ins Online-Formular; ELSTER rechnet die Summen
  selbst.
</p>

<Banner kind="warning">
  Die Zeilen-Zuordnung ist ein <strong>Vorschlag</strong> — bitte vor Abgabe mit
  deinem Steuerberater abgleichen (v.&nbsp;a. AfA, Kfz, Raumkosten). Die elektronische
  Übermittlung ans Finanzamt (ERiC) ist noch nicht enthalten.
</Banner>

{#if error}
  <Banner>{error}</Banner>
{:else if loading}
  <p class="muted">Lade …</p>
{:else if pkg && form}
  {#if afaPending > 0}
    <Banner kind="warning">
      Für {selectedYear} ist die Abschreibung (AfA) von {afaPending}
      {afaPending === 1 ? "Anlage" : "Anlagen"} noch nicht gebucht — sie würde im
      Export <strong>fehlen</strong>.
      {#if canBookAfa}
        <div class="afa-action">
          <Button variant="secondary" size="sm" onclick={bookAfa} disabled={bookingAfa}>
            {bookingAfa ? "Buche …" : `AfA für ${selectedYear} jetzt buchen`}
          </Button>
        </div>
      {/if}
    </Banner>
  {/if}

  <!-- Export-Aktionen -->
  <section class="card export">
    <h2>Exportieren</h2>
    <div class="exp-row">
      <label>Vollständige EÜR (PDF) <input type="text" bind:value={pdfPath} /></label>
      <Button variant="primary" onclick={exportPdf} disabled={busyPdf || !pdfPath.trim()}>
        {busyPdf ? "…" : "PDF speichern"}
      </Button>
    </div>
    {#if lastPdfPath}
      <p class="saved">
        <span class="ok">✓ Gespeichert:</span> <code>{lastPdfPath}</code>
        <Button variant="secondary" size="sm" onclick={() => revealPath(lastPdfPath!)}>Ordner öffnen</Button>
        <Button variant="secondary" size="sm" onclick={() => copyPath(lastPdfPath!)}>Pfad kopieren</Button>
      </p>
    {/if}
    <div class="exp-row">
      <label>ELSTER-Ausfüllhilfe (CSV) <input type="text" bind:value={csvPath} /></label>
      <Button variant="primary" onclick={exportCsv} disabled={busyCsv || !csvPath.trim()}>
        {busyCsv ? "…" : "CSV speichern"}
      </Button>
    </div>
    {#if lastCsvPath}
      <p class="saved">
        <span class="ok">✓ Gespeichert:</span> <code>{lastCsvPath}</code>
        <Button variant="secondary" size="sm" onclick={() => revealPath(lastCsvPath!)}>Ordner öffnen</Button>
        <Button variant="secondary" size="sm" onclick={() => copyPath(lastCsvPath!)}>Pfad kopieren</Button>
      </p>
    {/if}
    <div class="exp-row">
      <label>Einzelaufstellung (ZIP) <input type="text" bind:value={zipPath} /></label>
      <Button variant="primary" onclick={exportZip} disabled={busyZip || !zipPath.trim()}>
        {busyZip ? "…" : "ZIP speichern"}
      </Button>
    </div>
    {#if lastZipPath}
      <p class="saved">
        <span class="ok">✓ Gespeichert:</span> <code>{lastZipPath}</code>
        <Button variant="secondary" size="sm" onclick={() => revealPath(lastZipPath!)}>Ordner öffnen</Button>
        <Button variant="secondary" size="sm" onclick={() => copyPath(lastZipPath!)}>Pfad kopieren</Button>
      </p>
    {/if}

    <div class="exp-row">
      <label
        >DATEV-Buchungsstapel (Steuerberater) <input type="text" bind:value={datevPath} /></label
      >
      <label class="skr"
        >Kontenrahmen
        <select bind:value={skr}>
          <option value="SKR03">SKR03</option>
          <option value="SKR04">SKR04</option>
        </select>
      </label>
      <Button variant="primary" onclick={exportDatev} disabled={busyDatev || !datevPath.trim()}>
        {busyDatev ? "…" : "DATEV speichern"}
      </Button>
    </div>
    {#if lastDatevPath}
      <p class="saved">
        <span class="ok">✓ Gespeichert:</span> <code>{lastDatevPath}</code>
        <Button variant="secondary" size="sm" onclick={() => revealPath(lastDatevPath!)}>Ordner öffnen</Button>
        <Button variant="secondary" size="sm" onclick={() => copyPath(lastDatevPath!)}>Pfad kopieren</Button>
      </p>
    {/if}

    <div class="exp-row">
      <label
        >Steuerberater-Paket (ZIP, alles in einem) <input type="text" bind:value={stbPath} /></label
      >
      <Button variant="primary" onclick={exportStb} disabled={busyStb || !stbPath.trim()}>
        {busyStb ? "…" : "Paket speichern"}
      </Button>
    </div>
    {#if lastStbPath}
      <p class="saved">
        <span class="ok">✓ Gespeichert:</span> <code>{lastStbPath}</code>
        <Button variant="secondary" size="sm" onclick={() => revealPath(lastStbPath!)}>Ordner öffnen</Button>
        <Button variant="secondary" size="sm" onclick={() => copyPath(lastStbPath!)}>Pfad kopieren</Button>
      </p>
    {/if}

    <p class="muted hint">
      Das PDF „Anlage EÜR {selectedYear}" enthält Anlage EÜR, Anlageverzeichnis und
      die komplette Einzelaufstellung — als druckbares Dokument für deine Unterlagen.
      Der <strong>DATEV-Buchungsstapel</strong> ist für die Übergabe an den
      Steuerberater (Buchungen inkl. AfA; Konten als Vorschlag, vom Steuerberater zu
      prüfen). Das <strong>Steuerberater-Paket</strong> bündelt Deckblatt, EÜR-PDF,
      DATEV-Stapel ({skr}), Einzelaufstellung und Stammdaten in einem ZIP.
    </p>
  </section>

  <!-- 1. Anlage EÜR (Formularzeilen) -->
  <section class="card">
    <h2>Anlage EÜR — zum Übertragen ins ELSTER-Formular</h2>
    <table>
      <thead><tr><th class="z">Zeile</th><th>Position</th><th class="num">Betrag</th></tr></thead>
      <tbody>
        {#each entryLines as l (l.zeile + l.bezeichnung)}
          <tr>
            <td class="z">{l.zeile === 0 ? "—" : l.zeile}</td>
            <td>{l.bezeichnung}</td>
            <td class="num">{euro(l.amountCents)}</td>
          </tr>
        {/each}
        {#if entryLines.length === 0}
          <tr><td colspan="3" class="muted">Keine einzutragenden Positionen.</td></tr>
        {/if}
      </tbody>
      <tfoot>
        {#each sumLines as l (l.bezeichnung)}
          <tr class="sum">
            <td class="z">{l.zeile === 0 ? "—" : l.zeile}</td>
            <td>{l.bezeichnung}</td>
            <td class="num">{euro(l.amountCents)}</td>
          </tr>
        {/each}
      </tfoot>
    </table>
  </section>

  <!-- 2. Anlageverzeichnis (AVEÜR) -->
  <section class="card">
    <h2>Anlageverzeichnis (AVEÜR)</h2>
    {#if pkg.assets.length === 0}
      <p class="muted">Keine Anlagegüter im Geschäftsjahr.</p>
    {:else}
      <table>
        <thead>
          <tr>
            <th class="z">AV-Nr.</th><th>Bezeichnung</th><th>Anschaffung</th>
            <th class="num">AK/HK</th><th>Methode</th>
            <th class="num">AfA {selectedYear}</th><th class="num">Restwert Ende</th>
          </tr>
        </thead>
        <tbody>
          {#each pkg.assets as a (a.assetNumber)}
            <tr>
              <td class="z">{a.assetNumber}</td>
              <td>{a.label}{#if a.disposedInYear}<span class="tag">Abgang {a.disposalDate}</span>{/if}</td>
              <td>{a.acquisitionDate}</td>
              <td class="num">{euro(a.acquisitionCostCents)}</td>
              <td>{depreciationMethodShort(a.depreciationMethod)}{#if a.businessSharePercent < 100}<span class="muted"> · {a.businessSharePercent}% betr.</span>{/if}</td>
              <td class="num">{euro(a.afaYearCents)}</td>
              <td class="num">{euro(a.bookValueEndCents)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </section>

  <!-- 3. Einzelaufstellung Einnahmen -->
  <section class="card">
    <h2>Einzelaufstellung — Betriebseinnahmen</h2>
    {#if pkg.income.length === 0 && pkg.storno.length === 0}
      <p class="muted">Keine Zahlungseingänge im Geschäftsjahr.</p>
    {:else}
      <table>
        <thead><tr><th>Datum</th><th>Rechnungsnr.</th><th>Kunde</th><th>Beschreibung</th><th class="num">Betrag</th></tr></thead>
        <tbody>
          {#each pkg.income as i (i.invoiceNumber + i.paidDate + i.amountCents)}
            <tr>
              <td>{i.paidDate}</td><td>{i.invoiceNumber}</td><td>{i.customer}</td>
              <td>{i.description}</td>
              <td class="num">{euro(i.amountCents)}</td>
            </tr>
          {/each}
          {#each pkg.storno as s (s.stornoNumber)}
            <tr class="reduce">
              <td>{s.stornoDate}</td><td>{s.stornoNumber}</td>
              <td>Storno zu {s.originalNumber}</td>
              <td>—</td>
              <td class="num">−{euro(s.refundedCents)}</td>
            </tr>
          {/each}
        </tbody>
        <tfoot>
          <tr class="sum">
            <td colspan="4">Summe Zahlungseingänge</td>
            <td class="num">{euro(incomeSum)}</td>
          </tr>
        </tfoot>
      </table>
    {/if}
  </section>

  <!-- 4. Einzelaufstellung Ausgaben -->
  <section class="card">
    <h2>Einzelaufstellung — Betriebsausgaben</h2>
    {#if pkg.expenses.length === 0}
      <p class="muted">Keine bezahlten Kosten im Geschäftsjahr.</p>
    {:else}
      <table>
        <thead><tr><th>Datum</th><th>Beleg-Nr.</th><th>Lieferant</th><th>Kategorie</th><th>Beschreibung</th><th class="num">Betrag</th></tr></thead>
        <tbody>
          {#each pkg.expenses as e (e.expenseNumber)}
            <tr>
              <td>{e.paidDate}</td><td>{e.expenseNumber}</td><td>{e.vendor}</td>
              <td>{expenseCategoryLabel(e.category)}</td>
              <td>{e.description}</td>
              <td class="num">{euro(e.grossCents)}</td>
            </tr>
          {/each}
        </tbody>
        <tfoot>
          <tr class="sum">
            <td colspan="5">Summe Betriebsausgaben (Kosten)</td>
            <td class="num">{euro(expenseSum)}</td>
          </tr>
        </tfoot>
      </table>
    {/if}
  </section>

  <!-- 5. Veräußerungen -->
  {#if pkg.disposals.length > 0}
    <section class="card">
      <h2>Anlagen-Veräußerungen</h2>
      <table>
        <thead><tr><th>Datum</th><th>AV-Nr.</th><th>Bezeichnung</th><th class="num">Erlös</th><th class="num">Restwert</th><th class="num">Gewinn/Verlust</th></tr></thead>
        <tbody>
          {#each pkg.disposals as d (d.assetNumber)}
            <tr>
              <td>{d.disposalDate}</td><td class="z">{d.assetNumber}</td><td>{d.label}</td>
              <td class="num">{euro(d.proceedsCents)}</td>
              <td class="num">{euro(d.residualBookValueCents)}</td>
              <td class="num">{euro(d.gainLossCents)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </section>
  {/if}

  <p class="muted disclaimer">
    Klein.Buch ist ein Werkzeug, kein Steuerberater. Beträge nach bestem Wissen
    berechnet; für die Steuererklärung bitte mit deinem Steuerberater abgleichen.
    Privatentnahmen/-einlagen sind in der EÜR bewusst nicht enthalten.
  </p>
{/if}

<style>
  .year { display: inline-flex; align-items: center; gap: 0.4rem; font-size: 0.85rem; color: var(--c-text-muted); }
  .year select {
    padding: 0.4rem 0.5rem; border: 1px solid var(--c-border-strong);
    border-radius: var(--r-md); font-size: 0.95rem; font-family: inherit;
    background: var(--c-surface); color: var(--c-text);
  }
  /* .intro / .card / .card h2 entfernt — globale .lead, .card aus tokens.css. */
  table { width: 100%; border-collapse: collapse; }
  th, td { padding: 0.4rem 0.5rem; border-bottom: 1px solid var(--c-border); font-size: 0.88rem; text-align: left; vertical-align: top; }
  th { color: var(--c-text-subtle); font-weight: 600; font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.03em; }
  .z { width: 5.5rem; color: var(--c-text-muted); font-variant-numeric: tabular-nums; }
  .num { text-align: right; font-variant-numeric: tabular-nums; white-space: nowrap; }
  tr.reduce td { color: var(--c-danger-700); }
  tfoot tr.sum td { border-top: 2px solid var(--c-border-strong); border-bottom: none; padding-top: 0.55rem; font-weight: 600; color: var(--c-text); }
  .tag { display: inline-block; margin-left: 0.4rem; padding: 0.05rem 0.4rem; background: var(--c-warning-50); border: 1px solid #f3dcae; border-radius: var(--r-sm); font-size: 0.7rem; color: var(--c-warning-700); }
  .export .exp-row { display: flex; gap: 0.6rem; align-items: flex-end; flex-wrap: wrap; margin-bottom: 0.6rem; }
  .export label { flex: 1; min-width: 18rem; font-weight: 600; font-size: 0.8rem; display: block; }
  .export label.skr { flex: 0 0 auto; min-width: 0; }
  .export label.skr select {
    display: block;
    margin-top: 0.2rem;
    padding: 0.45rem 0.6rem;
    border: 1px solid var(--c-border-strong);
    border-radius: var(--r-md);
    font-size: 0.9rem;
    font-family: inherit;
    background: var(--c-surface);
    color: var(--c-text);
  }
  .export input {
    width: 100%; box-sizing: border-box; margin-top: 0.2rem;
    padding: 0.45rem 0.6rem; border: 1px solid var(--c-border-strong);
    border-radius: var(--r-md); font-size: 0.9rem; font-weight: 400;
    background: var(--c-surface); color: var(--c-text); font-family: inherit;
  }
  .afa-action { margin-top: 0.5rem; }
  .hint { font-size: 0.8rem; margin: 0.3rem 0 0; }
  .saved {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.4rem 0.6rem;
    margin: -0.2rem 0 0.7rem;
    font-size: 0.82rem;
    color: var(--c-text-muted);
  }
  .saved .ok { color: var(--c-success-700); font-weight: 600; }
  .saved code {
    background: var(--c-surface-2);
    border: 1px solid var(--c-border);
    border-radius: var(--r-sm);
    padding: 0.1rem 0.35rem;
    font-size: 0.8rem;
    word-break: break-all;
  }
  .disclaimer { max-width: 48rem; line-height: 1.5; font-size: 0.8rem; margin-top: 1.25rem; padding-top: 0.75rem; border-top: 1px solid var(--c-border); }
  .muted { color: var(--c-text-muted); }
</style>
