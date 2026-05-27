<script lang="ts">
  import { onMount } from "svelte";
  import { euerAvailableYears, euerComputeReport } from "$lib/api";
  import type { EuerReport } from "$lib/types";
  import { euro } from "$lib/format";
  import { expenseCategoryLabel } from "$lib/labels";
  import Banner from "$lib/Banner.svelte";
  import HelpAnchor from "$lib/HelpAnchor.svelte";
  import PageBar from "$lib/PageBar.svelte";

  let years = $state<number[]>([]);
  let selectedYear = $state<number | null>(null);
  let report = $state<EuerReport | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);

  onMount(async () => {
    try {
      years = await euerAvailableYears();
      selectedYear = years[0] ?? new Date().getFullYear();
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
      <label class="year">
        Geschäftsjahr
        <select bind:value={selectedYear}>
          {#each years as y}<option value={y}>{y}</option>{/each}
        </select>
      </label>
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
  <a class="export-link" href="/euer/export">Für die Steuererklärung exportieren →</a>
</p>

{#if error}
  <Banner>{error}</Banner>
{:else if loading}
  <p class="muted">Lade …</p>
{:else if report}
  {#if isEmpty}
    <Banner kind="info">
      Für {report.fiscalYear} sind noch keine bezahlten Rechnungen, Kosten oder
      Abschreibungen erfasst. Sobald Geld geflossen ist, erscheint es hier.
    </Banner>
  {/if}

  <div class="grid">
    <!-- Einnahmen -->
    <section class="card">
      <h2>Betriebseinnahmen</h2>
      <table>
        <tbody>
          <tr>
            <td>Bezahlte Rechnungen</td>
            <td class="num">{euro(report.invoiceIncomeCents)}</td>
          </tr>
          {#if report.stornoRefundsCents !== 0}
            <tr class="reduce">
              <td>abzgl. Erstattungen aus Stornos</td>
              <td class="num">−{euro(report.stornoRefundsCents)}</td>
            </tr>
          {/if}
          {#if report.disposalProceedsCents !== 0}
            <tr>
              <td>Verkauf von Anschaffungen</td>
              <td class="num">{euro(report.disposalProceedsCents)}</td>
            </tr>
          {/if}
        </tbody>
        <tfoot>
          <tr class="sum">
            <td>Summe Einnahmen</td>
            <td class="num">{euro(report.totalIncomeCents)}</td>
          </tr>
        </tfoot>
      </table>
    </section>

    <!-- Ausgaben -->
    <section class="card">
      <h2>Betriebsausgaben</h2>
      <table>
        <tbody>
          {#if report.expensesByCategory.length === 0 && report.depreciationTotalCents === 0 && report.disposalBookValueCents === 0}
            <tr><td colspan="2" class="muted">Keine Ausgaben erfasst.</td></tr>
          {/if}
          {#each report.expensesByCategory as c (c.category)}
            <tr>
              <td>{expenseCategoryLabel(c.category)}</td>
              <td class="num">{euro(c.amountCents)}</td>
            </tr>
          {/each}
          {#if report.depreciationTotalCents !== 0}
            <tr>
              <td>Abschreibungen (AfA)</td>
              <td class="num">{euro(report.depreciationTotalCents)}</td>
            </tr>
          {/if}
          {#if report.disposalBookValueCents !== 0}
            <tr>
              <td>Restwert verkaufter/entsorgter Anschaffungen</td>
              <td class="num">{euro(report.disposalBookValueCents)}</td>
            </tr>
          {/if}
        </tbody>
        <tfoot>
          <tr class="sum">
            <td>Summe Ausgaben</td>
            <td class="num">{euro(report.totalExpensesCents)}</td>
          </tr>
        </tfoot>
      </table>
    </section>
  </div>

  <!-- Ergebnis -->
  <section class="result" class:profit={isProfit} class:loss={!isProfit}>
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
  .year {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.85rem;
    color: #4b5563;
  }
  .year select {
    padding: 0.4rem 0.5rem;
    border: 1px solid #d1d5db;
    border-radius: 4px;
    font-size: 0.95rem;
    font-family: inherit;
  }
  /* .intro / .card / .card h2 entfernt — globale .lead, .card aus tokens.css. */
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(20rem, 1fr));
    gap: 1rem;
    margin: 1rem 0;
  }
  table {
    width: 100%;
    border-collapse: collapse;
  }
  td {
    padding: 0.4rem 0;
    border-bottom: 1px solid #f3f4f6;
    font-size: 0.9rem;
  }
  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  tr.reduce td {
    color: #b91c1c;
  }
  tfoot tr.sum td {
    border-top: 2px solid #e5e7eb;
    border-bottom: none;
    padding-top: 0.6rem;
    font-weight: 600;
    font-size: 0.95rem;
  }
  .result {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 1rem;
    padding: 1rem 1.25rem;
    border-radius: 8px;
    margin-top: 0.5rem;
  }
  .result.profit {
    background: #ecfdf5;
    border: 1px solid #a7f3d0;
  }
  .result.loss {
    background: #fef2f2;
    border: 1px solid #fecaca;
  }
  .result-label {
    font-weight: 600;
  }
  .result-value {
    font-size: 1.4rem;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }
  .result.profit .result-value {
    color: #047857;
  }
  .result.loss .result-value {
    color: #b91c1c;
  }
  .note {
    max-width: 44rem;
    line-height: 1.5;
    margin-top: 0.75rem;
  }
  .disclaimer {
    max-width: 44rem;
    line-height: 1.5;
    font-size: 0.8rem;
    margin-top: 1.25rem;
    padding-top: 0.75rem;
    border-top: 1px solid #f3f4f6;
  }
  .muted {
    color: #6b7280;
  }
  .actions {
    margin: 0 0 0.5rem;
  }
  .export-link {
    display: inline-block;
    padding: 0.45rem 0.85rem;
    background: var(--c-surface);
    border: 1px solid var(--c-border-strong);
    border-radius: var(--r-md);
    color: var(--c-primary-700);
    text-decoration: none;
    font-size: 0.9rem;
    font-weight: 600;
  }
  .export-link:hover {
    background: var(--c-primary-50);
    border-color: var(--c-primary-300);
    text-decoration: none;
  }
</style>
