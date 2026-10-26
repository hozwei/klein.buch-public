<script lang="ts">
  import { yearBerlin } from "#lib/date.ts";
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
    euerAfaPending,
    depreciationAccrueYear,
    saveDownload,
  } from "#lib/api.ts";
  import type { EuerPackage } from "#lib/types.ts";
  import { euro, date, percent } from "#lib/format.ts";
  import { formatQuantity } from "#lib/money.ts";
  import { expenseCategoryLabel, depreciationMethodShort } from "#lib/labels.ts";
  import { flash } from "#lib/toast.svelte.ts";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import HelpAnchor from "#lib/HelpAnchor.svelte";
  import PageBar from "#lib/PageBar.svelte";

  let years = $state<number[]>([]);
  let selectedYear = $state<number | null>(null);
  let pkg = $state<EuerPackage | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);

  let busyPdf = $state(false);
  let busyCsv = $state(false);
  let busyZip = $state(false);
  let busyDatev = $state(false);
  let busyStb = $state(false);
  // Plan W7: Exporte kommen als Download (kein Serverpfad); angezeigt wird der
  // Dateiname des letzten Downloads.
  let lastPdf = $state<string | null>(null);
  let lastCsv = $state<string | null>(null);
  let lastZip = $state<string | null>(null);
  let lastDatev = $state<string | null>(null);
  let lastStb = $state<string | null>(null);
  let afaPending = $state(0);
  let bookingAfa = $state(false);

  // Kontenrahmen für den DATEV-Export (gemerkt; Default SKR03).
  let skr = $state(
    (typeof localStorage !== "undefined" && localStorage.getItem("datevSkr")) || "SKR03",
  );
  $effect(() => {
    if (typeof localStorage !== "undefined") localStorage.setItem("datevSkr", skr);
  });

  const currentYear = yearBerlin();
  let canBookAfa = $derived(selectedYear != null && selectedYear <= currentYear);

  onMount(async () => {
    try {
      years = await euerAvailableYears();
      selectedYear = years[0] ?? yearBerlin();
    } catch (e) {
      error = String(e);
      loading = false;
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

  let form = $derived(pkg?.form ?? null);
  let entryLines = $derived(form ? form.lines.filter((l) => l.isEntry) : []);
  let sumLines = $derived(form ? form.lines.filter((l) => !l.isEntry) : []);
  let incomeSum = $derived((pkg?.income ?? []).reduce((s, i) => s + i.amountCents, 0));
  let expenseSum = $derived((pkg?.expenses ?? []).reduce((s, e) => s + e.grossCents, 0));

  // Übergabe F2: Abweichung eines abgeschlossenen Jahres vom Festschreibungs-Snapshot.
  const DIFF_LABEL: Record<string, string> = {
    income: "Einnahmen",
    expenses: "Ausgaben",
    afa: "Abschreibungen",
    surplus: "Ergebnis",
  };
  let snapshotMismatch = $derived(pkg?.snapshotCheck && !pkg.snapshotCheck.matches ? pkg.snapshotCheck : null);

  async function exportPdf() {
    if (selectedYear == null) return;
    busyPdf = true;
    try {
      const r = await euerExportPdf(selectedYear);
      saveDownload(r.download);
      lastPdf = r.download.fileName;
      flash(`PDF erstellt (${formatQuantity(Math.round(r.sizeBytes / 1024), 0)} KB).`);
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
      const r = await euerExportDatev(selectedYear, skr);
      saveDownload(r.download);
      lastDatev = r.download.fileName;
      flash(`DATEV-Buchungsstapel erstellt (${r.bookingCount} Buchungen, ${skr}).`);
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
      const r = await euerExportStbZip(selectedYear, skr);
      saveDownload(r.download);
      lastStb = r.download.fileName;
      flash(`Steuerberater-Paket erstellt (${r.fileCount} Dateien).`);
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
      const r = await euerExportElster(selectedYear);
      saveDownload(r.download);
      lastCsv = r.download.fileName;
      flash(`ELSTER-CSV erstellt (${r.entryCount} Positionen).`);
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
      const r = await euerExportDetailZip(selectedYear);
      saveDownload(r.download);
      lastZip = r.download.fileName;
      flash(
        `Einzelaufstellung erstellt (${r.incomeCount} Einnahmen, ${r.expenseCount} Ausgaben, ${r.assetCount} Anlagen).`,
      );
    } catch (e) {
      flash(String(e), "error");
    } finally {
      busyZip = false;
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
      <label class="year" for="exp-year">Geschäftsjahr</label>
      <select id="exp-year" class="kb-input year-select" bind:value={selectedYear}>
        {#each years as y}<option value={y}>{y}</option>{/each}
      </select>
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
  <p class="muted" aria-busy="true">Lädt …</p>
{:else if pkg && form}
  {#if snapshotMismatch}
    <Banner kind="warning">
      <strong>Abweichung vom Jahresabschluss:</strong> Das Geschäftsjahr {snapshotMismatch.fiscalYear}
      wurde am {date(snapshotMismatch.closedAt)} abgeschlossen; die heutigen Zahlen weichen
      davon ab bei: {snapshotMismatch.differences.map((d) => DIFF_LABEL[d] ?? d).join(", ")}.
      Ergebnis beim Abschluss {euro(snapshotMismatch.snapshot.surplusCents)}, heute
      {euro(snapshotMismatch.live.surplusCents)}. Bitte vor der Abgabe klären.
    </Banner>
  {/if}
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
  <section class="card export" aria-labelledby="exp-h">
    <h2 id="exp-h">Exportieren</h2>
    <p class="info-banner dl-note">
      Jede Datei wird beim Klick neu erzeugt und kann <strong>einmal</strong> abgerufen
      werden. Bricht der Download ab, klickst du einfach noch einmal.
    </p>
    <div class="exp-row">
      <span class="exp-label" id="exp-pdf-l">Vollständige EÜR (PDF)</span>
      <Button variant="secondary" onclick={exportPdf} disabled={busyPdf} ariaDescribedby="exp-pdf-l">
        {busyPdf ? "Erstelle …" : "PDF herunterladen"}
      </Button>
    </div>
    {#if lastPdf}
      <p class="saved" role="status"><span class="ok"><span aria-hidden="true">✓</span> Heruntergeladen:</span> <code>{lastPdf}</code></p>
    {/if}
    <div class="exp-row">
      <span class="exp-label" id="exp-csv-l">ELSTER-Ausfüllhilfe (CSV)</span>
      <Button variant="secondary" onclick={exportCsv} disabled={busyCsv} ariaDescribedby="exp-csv-l">
        {busyCsv ? "Erstelle …" : "CSV herunterladen"}
      </Button>
    </div>
    {#if lastCsv}
      <p class="saved" role="status"><span class="ok"><span aria-hidden="true">✓</span> Heruntergeladen:</span> <code>{lastCsv}</code></p>
    {/if}
    <div class="exp-row">
      <span class="exp-label" id="exp-zip-l">Einzelaufstellung (ZIP)</span>
      <Button variant="secondary" onclick={exportZip} disabled={busyZip} ariaDescribedby="exp-zip-l">
        {busyZip ? "Erstelle …" : "ZIP herunterladen"}
      </Button>
    </div>
    {#if lastZip}
      <p class="saved" role="status"><span class="ok"><span aria-hidden="true">✓</span> Heruntergeladen:</span> <code>{lastZip}</code></p>
    {/if}

    <div class="exp-row">
      <span class="exp-label" id="exp-datev-l">DATEV-Buchungsstapel (Steuerberater)</span>
      <div class="skr">
        <label for="exp-skr">Kontenrahmen</label>
        <select id="exp-skr" class="kb-input" bind:value={skr}>
          <option value="SKR03">SKR03</option>
          <option value="SKR04">SKR04</option>
        </select>
      </div>
      <Button variant="secondary" onclick={exportDatev} disabled={busyDatev} ariaDescribedby="exp-datev-l">
        {busyDatev ? "Erstelle …" : "DATEV herunterladen"}
      </Button>
    </div>
    {#if lastDatev}
      <p class="saved" role="status"><span class="ok"><span aria-hidden="true">✓</span> Heruntergeladen:</span> <code>{lastDatev}</code></p>
    {/if}

    <div class="exp-row">
      <span class="exp-label" id="exp-stb-l">Steuerberater-Paket (ZIP, alles in einem)</span>
      <Button variant="primary" onclick={exportStb} disabled={busyStb} ariaDescribedby="exp-stb-l">
        {busyStb ? "Erstelle …" : "Paket herunterladen"}
      </Button>
    </div>
    {#if lastStb}
      <p class="saved" role="status"><span class="ok"><span aria-hidden="true">✓</span> Heruntergeladen:</span> <code>{lastStb}</code></p>
    {/if}

    <p class="muted hint">
      Das PDF „Anlage EÜR {selectedYear}“ enthält Anlage EÜR, Anlageverzeichnis und
      die komplette Einzelaufstellung — als druckbares Dokument für deine Unterlagen.
      Der <strong>DATEV-Buchungsstapel</strong> ist für die Übergabe an den
      Steuerberater (Buchungen inkl. AfA; Konten als Vorschlag, vom Steuerberater zu
      prüfen). Das <strong>Steuerberater-Paket</strong> bündelt Deckblatt, EÜR-PDF,
      DATEV-Stapel ({skr}), Einzelaufstellung und Stammdaten in einem ZIP.
    </p>
  </section>

  <!-- 1. Anlage EÜR (Formularzeilen) -->
  <section class="card" aria-labelledby="form-h">
    <h2 id="form-h">Anlage EÜR — zum Übertragen ins ELSTER-Formular</h2>
    {#if form}
      <p class="muted">Zeilen nach dem Vordruck {form.formYear}; die Kennzahl (Kz) gilt in jedem Jahr.</p>
      {#if form.notes.length > 0}
        <Banner kind="warning">
          <ul>
            {#each form.notes as n (n)}<li>{n}</li>{/each}
          </ul>
        </Banner>
      {/if}
    {/if}
    {#if entryLines.length === 0}
      <p class="muted">Keine einzutragenden Positionen.</p>
    {/if}
    <table class="cards">
      <caption class="kb-visually-hidden">Anlage EÜR {selectedYear}: Zeilen und Beträge</caption>
      <thead><tr><th class="z">Zeile</th><th class="z">Kz</th><th>Position</th><th class="num">Betrag</th></tr></thead>
      <tbody>
        {#each entryLines as l (l.zeile + l.bezeichnung)}
          <tr>
            <td class="z" data-label="Zeile">{l.zeile}</td>
            <td class="z" data-label="Kz">{l.kennzahl ?? "—"}</td>
            <td class="cards-full" data-label="Position">{l.bezeichnung}</td>
            <td class="num" data-label="Betrag">{euro(l.amountCents)}</td>
          </tr>
        {/each}
      </tbody>
      <tfoot>
        {#each sumLines as l (l.bezeichnung)}
          <tr class="sum">
            <td class="z" data-label="Zeile">{l.zeile}</td>
            <td class="z" data-label="Kz">{l.kennzahl ?? "—"}</td>
            <td class="cards-full" data-label="Position">{l.bezeichnung}</td>
            <td class="num" data-label="Summe">{euro(l.amountCents)}</td>
          </tr>
        {/each}
      </tfoot>
    </table>
  </section>

  <!-- 2. Anlageverzeichnis (AVEÜR) -->
  <section class="card" aria-labelledby="av-h">
    <h2 id="av-h">Anlageverzeichnis (AVEÜR)</h2>
    {#if pkg.assets.length === 0}
      <p class="muted">Keine Anlagegüter im Geschäftsjahr.</p>
    {:else}
      <!-- U1: 7 Spalten passen bei 821–900 px (Tabellen-Band) nicht in die Karte;
           die Tabelle scrollt in sich, per Tastatur erreichbar und benannt. -->
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <div class="tbl-scroll" role="region" aria-label="Anlageverzeichnis {selectedYear}" tabindex="0">
      <table class="cards">
        <caption class="kb-visually-hidden">Anlageverzeichnis {selectedYear}</caption>
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
              <td class="z doc-no" data-label="AV-Nr.">{a.assetNumber}</td>
              <td class="wrap" data-label="Bezeichnung">{a.label}{#if a.disposedInYear}<span class="tag">Abgang {date(a.disposalDate)}</span>{/if}</td>
              <td data-label="Anschaffung">{date(a.acquisitionDate)}</td>
              <td class="num" data-label="AK/HK">{euro(a.acquisitionCostCents)}</td>
              <td data-label="Methode">{depreciationMethodShort(a.depreciationMethod)}{#if a.businessSharePercent < 100}<span class="muted"> · {percent(a.businessSharePercent)} betrieblich</span>{/if}</td>
              <td class="num" data-label="AfA {selectedYear}">{euro(a.afaYearCents)}</td>
              <td class="num" data-label="Restwert Ende">{euro(a.bookValueEndCents)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
      </div>
    {/if}
  </section>

  <!-- 3. Einzelaufstellung Einnahmen -->
  <section class="card" aria-labelledby="inc-h">
    <h2 id="inc-h">Einzelaufstellung — Betriebseinnahmen</h2>
    {#if pkg.income.length === 0 && pkg.storno.length === 0}
      <p class="muted">Keine Zahlungseingänge im Geschäftsjahr.</p>
    {:else}
      <table class="cards">
        <caption class="kb-visually-hidden">Zahlungseingänge {selectedYear}</caption>
        <thead><tr><th>Datum</th><th>Rechnungsnr.</th><th>Kunde</th><th>Beschreibung</th><th class="num">Betrag</th></tr></thead>
        <tbody>
          {#each pkg.income as i (i.invoiceNumber + i.paidDate + i.amountCents)}
            <tr>
              <td data-label="Datum">{date(i.paidDate)}</td>
              <td class="doc-no" data-label="Rechnungsnr.">{i.invoiceNumber}</td>
              <td class="wrap" data-label="Kunde">{i.customer}</td>
              <td class="cards-full wrap" data-label="Beschreibung">{i.description}</td>
              <td class="num" data-label="Betrag">{euro(i.amountCents)}</td>
            </tr>
          {/each}
          {#each pkg.storno as s (s.stornoNumber)}
            <tr class="reduce">
              <td data-label="Datum">{date(s.stornoDate)}</td>
              <td class="doc-no" data-label="Rechnungsnr.">{s.stornoNumber}</td>
              <td data-label="Kunde">Storno zu {s.originalNumber}</td>
              <td class="no-card" data-label="Beschreibung">—</td>
              <td class="num" data-label="Betrag">−{euro(s.refundedCents)}</td>
            </tr>
          {/each}
        </tbody>
        <tfoot>
          <tr class="sum">
            <td colspan="4" class="sum-label">Summe Zahlungseingänge</td>
            <td class="num" data-label="Summe Zahlungseingänge">{euro(incomeSum)}</td>
          </tr>
        </tfoot>
      </table>
    {/if}
  </section>

  <!-- 4. Einzelaufstellung Ausgaben -->
  <section class="card" aria-labelledby="out-h">
    <h2 id="out-h">Einzelaufstellung — Betriebsausgaben</h2>
    {#if pkg.expenses.length === 0}
      <p class="muted">Keine bezahlten Kosten im Geschäftsjahr.</p>
    {:else}
      <table class="cards">
        <caption class="kb-visually-hidden">Bezahlte Kosten {selectedYear}</caption>
        <thead><tr><th>Datum</th><th>Beleg-Nr.</th><th>Lieferant</th><th>Kategorie</th><th>Beschreibung</th><th class="num">Betrag</th></tr></thead>
        <tbody>
          {#each pkg.expenses as e (e.expenseNumber)}
            <tr>
              <td data-label="Datum">{date(e.paidDate)}</td>
              <td class="doc-no" data-label="Beleg-Nr.">{e.expenseNumber}</td>
              <td class="wrap" data-label="Lieferant">{e.vendor}</td>
              <td data-label="Kategorie">{expenseCategoryLabel(e.category)}</td>
              <td class="cards-full wrap" data-label="Beschreibung">{e.description}</td>
              <td class="num" data-label="Betrag">{euro(e.grossCents)}</td>
            </tr>
          {/each}
        </tbody>
        <tfoot>
          <tr class="sum">
            <td colspan="5" class="sum-label">Summe Betriebsausgaben (Kosten)</td>
            <td class="num" data-label="Summe Betriebsausgaben">{euro(expenseSum)}</td>
          </tr>
        </tfoot>
      </table>
    {/if}
  </section>

  <!-- 5. Veräußerungen -->
  {#if pkg.disposals.length > 0}
    <section class="card" aria-labelledby="disp-h">
      <h2 id="disp-h">Anlagen-Veräußerungen</h2>
      <table class="cards">
        <caption class="kb-visually-hidden">Anlagen-Veräußerungen {selectedYear}</caption>
        <thead><tr><th>Datum</th><th>AV-Nr.</th><th>Bezeichnung</th><th class="num">Erlös</th><th class="num">Restwert</th><th class="num">Gewinn/Verlust</th></tr></thead>
        <tbody>
          {#each pkg.disposals as d (d.assetNumber)}
            <tr>
              <td data-label="Datum">{date(d.disposalDate)}</td>
              <td class="z doc-no" data-label="AV-Nr.">{d.assetNumber}</td>
              <td class="wrap" data-label="Bezeichnung">{d.label}</td>
              <td class="num" data-label="Erlös">{euro(d.proceedsCents)}</td>
              <td class="num" data-label="Restwert">{euro(d.residualBookValueCents)}</td>
              <td class="num" data-label="Gewinn/Verlust">{euro(d.gainLossCents)}</td>
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
  .tbl-scroll { overflow-x: auto; max-width: 100%; }
  @media (max-width: 820px) { .tbl-scroll { overflow: visible; } }
  .year { font-size: var(--fs-sm); font-weight: 600; color: var(--text-muted); align-self: center; }
  .year-select { width: auto; }
  /* .card / .card h2 / .lead / .info-banner / .kb-input: globale Definitionen aus tokens.css. */
  table { width: 100%; border-collapse: collapse; }
  td { vertical-align: top; }
  .z { width: 5.5rem; color: var(--text-muted); font-variant-numeric: tabular-nums; }
  @media (max-width: 820px) {
    td.z { width: auto; }
    /* Karten: Summentext steht im data-label der Betragszelle; leere Platzhalter weg. */
    tr.sum td.sum-label, td.no-card { display: none; }
  }
  .num { text-align: right; font-variant-numeric: tabular-nums; white-space: nowrap; }
  td.wrap { overflow-wrap: anywhere; }
  .doc-no { font-family: var(--font-mono); white-space: nowrap; }
  tr.reduce td { color: var(--status-danger-fg); }
  tfoot tr.sum td { border-top: 3px double var(--line-control) !important; border-bottom: none !important; font-weight: 600; color: var(--text); }
  .tag { display: inline-block; margin-left: var(--sp-2); padding: 0.05rem 0.4rem; background: var(--status-warning-bg); border: 1px solid var(--status-warning-line); border-radius: var(--r-sm); font-size: var(--fs-xs); color: var(--status-warning-fg); }
  .dl-note { margin: 0 0 var(--sp-4); }
  .export .exp-row { display: flex; gap: var(--sp-3); align-items: flex-end; flex-wrap: wrap; margin-bottom: var(--sp-3); }
  .export .exp-label { flex: 1; min-width: min(18rem, 100%); font-weight: 600; align-self: center; }
  .skr { display: flex; flex-direction: column; gap: var(--sp-1); }
  .skr label { font-weight: 600; font-size: var(--fs-sm); }
  .skr .kb-input { width: auto; }
  .afa-action { margin-top: var(--sp-2); }
  .hint { font-size: var(--fs-sm); margin: var(--sp-1) 0 0; }
  .saved {
    display: flex; align-items: center; flex-wrap: wrap; gap: var(--sp-1) var(--sp-2);
    margin: calc(-1 * var(--sp-1)) 0 var(--sp-3); font-size: var(--fs-sm); color: var(--text-muted);
  }
  .saved .ok { color: var(--status-success-fg); font-weight: 600; }
  .saved code {
    background: var(--surface-sunken); border: 1px solid var(--line); border-radius: var(--r-sm);
    padding: 0.1rem 0.35rem; font-size: var(--fs-sm); overflow-wrap: anywhere;
  }
  .disclaimer { max-width: 48rem; line-height: 1.5; font-size: var(--fs-sm); margin-top: var(--sp-5); padding-top: var(--sp-3); border-top: 1px solid var(--line); }
  .muted { color: var(--text-muted); }
</style>
