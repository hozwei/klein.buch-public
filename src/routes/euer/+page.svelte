<script lang="ts">
  import { yearBerlin } from "#lib/date.ts";
  import { onMount } from "svelte";
  import { euerAvailableYears, euerComputeReport } from "#lib/api.ts";
  import type { EuerReport } from "#lib/types.ts";
  import { euro } from "#lib/format.ts";
  import { expenseCategoryLabel } from "#lib/labels.ts";
  import Banner from "#lib/Banner.svelte";
  import HelpAnchor from "#lib/HelpAnchor.svelte";
  import PageBar from "#lib/PageBar.svelte";

  let years = $state<number[]>([]);
  let selectedYear = $state<number | null>(null);
  let report = $state<EuerReport | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);

  onMount(async () => {
    try {
      years = await euerAvailableYears();
      selectedYear = years[0] ?? yearBerlin();
    } catch (e) {
      error = String(e);
      loading = false;
    }
  });

  // Report neu laden, wenn sich das gewählte Jahr ändert.
  $effect(() => {
    const y = selectedYear;
    if (y == null) return;
    loading = true;
    error = null;
    euerComputeReport(y)
      .then((r) => {
        report = r;
      })
      .catch((e) => {
        error = String(e);
        report = null;
      })
      .finally(() => {
        loading = false;
      });
  });

  let isEmpty = $derived(
    !!report && report.totalIncomeCents === 0 && report.totalExpensesCents === 0,
  );
  let isProfit = $derived(!!report && report.surplusCents >= 0);
  let hasDisposal = $derived(
    !!report &&
      (report.disposalProceedsCents !== 0 || report.disposalBookValueCents !== 0),
  );
</script>

<PageBar title="Steuer-Übersicht (EÜR)">
  {#snippet actions()}
    {#if years.length > 0 && selectedYear != null}
      <label class="year" for="euer-year">Geschäftsjahr</label>
      <select id="euer-year" class="kb-input year-select" bind:value={selectedYear}>
        {#each years as y}<option value={y}>{y}</option>{/each}
      </select>
    {/if}
    <HelpAnchor slug="euer-cash-basis" />
  {/snippet}
</PageBar>

<p class="lead">
  Deine <strong>Einnahmen-Überschuss-Rechnung</strong> für ein Jahr: was reingekommen
  ist minus was du ausgegeben hast. Maßgeblich ist, <strong>wann das Geld geflossen
  ist</strong> (nicht das Rechnungsdatum) — so will es das Finanzamt
  (§&nbsp;4&nbsp;Abs.&nbsp;3, §&nbsp;11&nbsp;EStG).
</p>

<p class="actions">
  <a class="btn-secondary" href="/euer/export">Für die Steuererklärung exportieren <span aria-hidden="true">→</span></a>
</p>

{#if error}
  <Banner>{error}</Banner>
{:else if loading}
  <p class="muted" aria-busy="true">Lädt …</p>
{:else if report}
  {#if isEmpty}
    <Banner kind="info">
      Für {report.fiscalYear} sind noch keine bezahlten Rechnungen, Kosten oder
      Abschreibungen erfasst. Sobald Geld geflossen ist, erscheint es hier.
    </Banner>
  {/if}

  <div class="grid">
    <!-- Einnahmen: Schlüssel-Wert-Liste statt Tabelle (zwei Spalten, auch bei 320 px lesbar) -->
    <section class="card" aria-labelledby="euer-income-h">
      <h2 id="euer-income-h">Betriebseinnahmen</h2>
      <dl class="ledger">
        <div>
          <dt>Bezahlte Rechnungen</dt>
          <dd>{euro(report.invoiceIncomeCents)}</dd>
        </div>
        {#if report.stornoRefundsCents !== 0}
          <div class="reduce">
            <dt>abzgl. Erstattungen aus Stornos</dt>
            <dd>−{euro(report.stornoRefundsCents)}</dd>
          </div>
        {/if}
        {#if report.disposalProceedsCents !== 0}
          <div>
            <dt>Verkauf von Anschaffungen</dt>
            <dd>{euro(report.disposalProceedsCents)}</dd>
          </div>
        {/if}
        <div class="sum">
          <dt>Summe Einnahmen</dt>
          <dd>{euro(report.totalIncomeCents)}</dd>
        </div>
      </dl>
    </section>

    <!-- Ausgaben -->
    <section class="card" aria-labelledby="euer-exp-h">
      <h2 id="euer-exp-h">Betriebsausgaben</h2>
      <dl class="ledger">
        {#if report.expensesByCategory.length === 0 && report.depreciationTotalCents === 0 && report.disposalBookValueCents === 0}
          <div class="none">
            <dt class="muted">Keine Ausgaben erfasst.</dt>
            <dd><a class="inline-link" href="/expenses/new">Kosten erfassen</a></dd>
          </div>
        {/if}
        {#each report.expensesByCategory as c (c.category)}
          <div>
            <dt>{expenseCategoryLabel(c.category)}</dt>
            <dd>{euro(c.amountCents)}</dd>
          </div>
        {/each}
        {#if report.depreciationTotalCents !== 0}
          <div>
            <dt>Abschreibungen (AfA)</dt>
            <dd>{euro(report.depreciationTotalCents)}</dd>
          </div>
        {/if}
        {#if report.disposalBookValueCents !== 0}
          <div>
            <dt>Restwert verkaufter/entsorgter Anschaffungen</dt>
            <dd>{euro(report.disposalBookValueCents)}</dd>
          </div>
        {/if}
        <div class="sum">
          <dt>Summe Ausgaben</dt>
          <dd>{euro(report.totalExpensesCents)}</dd>
        </div>
      </dl>
    </section>
  </div>

  <!-- Ergebnis -->
  <section class="result" class:profit={isProfit} class:loss={!isProfit} aria-label="Ergebnis {report.fiscalYear}">
    <span class="result-label">
      {isProfit ? "Überschuss (Gewinn)" : "Verlust"} {report.fiscalYear}
    </span>
    <span class="result-value">{euro(report.surplusCents)}</span>
  </section>

  {#if hasDisposal}
    <p class="muted note">
      Darin enthalten — Anschaffungs-Verkauf:
      {report.disposalGainLossCents >= 0 ? "Gewinn" : "Verlust"} von
      <strong>{euro(report.disposalGainLossCents)}</strong>
      (Erlös {euro(report.disposalProceedsCents)} − Restwert
      {euro(report.disposalBookValueCents)}). Tipp: einen Anschaffungs-Verkauf
      hier nicht zusätzlich als Rechnung erfassen, sonst zählt der Erlös doppelt.
    </p>
  {/if}

  <p class="muted disclaimer">
    Klein.Buch ist ein Werkzeug, kein Steuerberater. Die Zahlen sind nach bestem
    Wissen berechnet — für die Steuererklärung bitte mit deinem Steuerberater
    abgleichen. Privatentnahmen und -einlagen sind in der EÜR bewusst nicht
    enthalten (steuerlich neutral).
  </p>
{/if}

<style>
  .year { font-size: var(--fs-sm); font-weight: 600; color: var(--text-muted); align-self: center; }
  .year-select { width: auto; }
  /* .card / .card h2 / .lead / .kb-input: globale Definitionen aus tokens.css. */
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(20rem, 100%), 1fr));
    gap: var(--sp-4);
    margin: var(--sp-4) 0;
  }
  /* Kontorbuch-Lineatur: feine Linie je Zeile, doppelte Linie über der Summe. */
  .ledger { margin: 0; }
  .ledger > div {
    display: flex; justify-content: space-between; align-items: baseline; gap: var(--sp-4);
    padding: var(--sp-2) 0; border-bottom: 1px solid var(--line);
  }
  .ledger dt { overflow-wrap: anywhere; min-width: 0; }
  .ledger dd { margin: 0; text-align: right; font-variant-numeric: tabular-nums; white-space: nowrap; }
  .ledger > div.reduce { color: var(--status-danger-fg); }
  .ledger > div.sum {
    border-top: 3px double var(--line-control); border-bottom: none;
    padding-top: var(--sp-3); font-weight: 600;
  }
  .ledger > div.none { flex-wrap: wrap; }
  .result {
    display: flex; flex-wrap: wrap; justify-content: space-between; align-items: baseline;
    gap: var(--sp-4); padding: var(--sp-4) var(--sp-5); border-radius: var(--r-md);
    margin-top: var(--sp-2); border: 1px solid;
  }
  .result.profit { background: var(--status-success-bg); border-color: var(--status-success-line); }
  .result.loss { background: var(--status-danger-bg); border-color: var(--status-danger-line); }
  .result-label { font-weight: 600; }
  .result-value { font-size: var(--fs-xl); font-weight: 700; font-variant-numeric: tabular-nums; }
  .result.profit .result-value { color: var(--status-success-fg); }
  .result.loss .result-value { color: var(--status-danger-fg); }
  .note { max-width: 44rem; line-height: 1.5; margin-top: var(--sp-3); }
  .disclaimer {
    max-width: 44rem; line-height: 1.5; font-size: var(--fs-sm);
    margin-top: var(--sp-5); padding-top: var(--sp-3); border-top: 1px solid var(--line);
  }
  .muted { color: var(--text-muted); }
  /* U1 (2.5.8): steht allein in der Zeile (kein Fließtext) → volle Trefferfläche. */
  .inline-link { text-decoration: underline; display: inline-flex; align-items: center; min-height: var(--target); }
  .actions { margin: 0 0 var(--sp-2); }
</style>
