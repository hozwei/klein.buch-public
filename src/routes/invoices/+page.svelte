<script lang="ts">
  import { onMount } from "svelte";
  import { invoicesList } from "#lib/api.ts";
  import type { InvoiceListItem, InvoiceStatus } from "#lib/types.ts";
  import { euro, date } from "#lib/format.ts";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import Badge from "#lib/Badge.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import QuantityInput from "#lib/QuantityInput.svelte";

  let rows: InvoiceListItem[] = $state([]);
  let loading = $state(false);
  let error: string | null = $state(null);
  let fiscalYear: number | null = $state(null);
  let statusFilter: InvoiceStatus | "" = $state("");
  let includeCanceled = $state(true);

  async function load() {
    loading = true;
    error = null;
    try {
      rows = await invoicesList({
        fiscalYear: fiscalYear ?? undefined,
        status: (statusFilter || undefined) as InvoiceStatus | undefined,
        includeCanceled,
      });
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  function statusLabel(s: string): string {
    switch (s) {
      case "draft":           return "Entwurf";
      case "issued":          return "Ausgestellt";
      case "sent":            return "Versendet";
      case "partially_paid":  return "Teilzahlung";
      case "paid":            return "Bezahlt";
      case "canceled":        return "Storniert";
      default:                return s;
    }
  }

  type Tone = "neutral" | "primary" | "info" | "success" | "warning" | "danger";
  function statusTone(s: string): Tone {
    switch (s) {
      case "draft":           return "neutral";
      case "issued":          return "primary";
      case "sent":            return "info";
      case "partially_paid":  return "warning";
      case "paid":            return "success";
      case "canceled":        return "danger";
      default:                return "neutral";
    }
  }
</script>

<PageBar title="Rechnungen">
  {#snippet actions()}
    <Button variant="secondary" size="sm" href="/recurring-invoices">Wiederkehrende Rechnungen</Button>
    <Button variant="primary" href="/invoices/new">+ Neue Rechnung</Button>
  {/snippet}
</PageBar>

<form class="toolbar" onsubmit={(e) => { e.preventDefault(); load(); }} novalidate>
  <div class="tb-field">
    <label for="inv-filter-year">Jahr</label>
    <QuantityInput id="inv-filter-year" bind:value={fiscalYear} integer grouping={false} noun="Jahr" placeholder="alle" />
  </div>
  <label>
    Status
    <select class="kb-input" bind:value={statusFilter}>
      <option value="">Alle</option>
      <option value="draft">Entwurf</option>
      <option value="issued">Ausgestellt</option>
      <option value="sent">Versendet</option>
      <option value="partially_paid">Teilzahlung</option>
      <option value="paid">Bezahlt</option>
      <option value="canceled">Storniert</option>
    </select>
  </label>
  <label class="chk">
    <input type="checkbox" bind:checked={includeCanceled} />
    Stornos anzeigen
  </label>
  <Button type="submit" variant="secondary">Filtern</Button>
</form>

{#if loading}
  <p class="muted">Lade …</p>
{:else if error}
  <Banner>Fehler: {error}</Banner>
{:else if rows.length === 0}
  <div class="empty">
    <p class="muted">Keine Rechnungen für diese Auswahl.</p>
    <Button variant="primary" href="/invoices/new">Neue Rechnung anlegen</Button>
  </div>
{:else}
  <table class="kb-table cards">
    <caption class="kb-visually-hidden">Rechnungen</caption>
    <thead>
      <tr>
        <th>Nummer</th>
        <th>Datum</th>
        <th>Empfänger</th>
        <th class="right">Brutto</th>
        <th class="right">Bezahlt</th>
        <th>Status</th>
        <th>Fällig</th>
        <th><span class="kb-visually-hidden">Aktionen</span></th>
      </tr>
    </thead>
    <tbody>
      {#each rows as r (r.id)}
        <tr class:storno={r.isStornoFor !== null} class:canceled={r.status === "canceled"}>
          <td data-label="Nummer">
            <a class="doc-no" href={`/invoices/${r.id}`}>{r.invoiceNumber}</a>
            {#if r.isStornoFor}<Badge tone="warning">Storno</Badge>{/if}
          </td>
          <td data-label="Datum">{date(r.invoiceDate)}</td>
          <td data-label="Empfänger">{r.contactName}</td>
          <td class="right" data-label="Brutto">{euro(r.grossAmountCents)}</td>
          <td class="right" data-label="Bezahlt">{euro(r.paidAmountCents)}</td>
          <td data-label="Status">
            {#if r.voidedAt}
              <Badge tone="neutral" strike>Verworfen</Badge>
            {:else}
              <Badge tone={statusTone(r.status)} strike={r.status === "canceled"}>{statusLabel(r.status)}</Badge>
            {/if}
          </td>
          <td data-label="Fällig">{date(r.dueDate)}</td>
          <td class="right"><Button variant="secondary" size="sm" href={`/invoices/${r.id}`} ariaLabel={`Rechnung ${r.invoiceNumber} öffnen`}>Öffnen</Button></td>
        </tr>
      {/each}
    </tbody>
  </table>
{/if}

<style>
  .toolbar { display: flex; gap: 0.75rem; align-items: end; margin-bottom: 1rem; flex-wrap: wrap; }
  .toolbar label, .tb-field { display: flex; flex-direction: column; font-size: var(--fs-sm); font-weight: 600; color: var(--text-muted); gap: 0.2rem; }
  .toolbar .kb-input, .tb-field { min-width: 9rem; }
  .chk { flex-direction: row !important; align-items: center; gap: 0.4rem; min-height: var(--target); font-weight: 400 !important; }
  /* Belegnummer als markantes Monospace-Element; Trefferfläche ≥ --target (2.5.8). */
  .doc-no { display: inline-flex; align-items: center; min-height: var(--target); font-family: var(--font-mono); white-space: nowrap; }
  .empty { display: flex; flex-wrap: wrap; gap: var(--sp-3); align-items: center; }
  table { width: 100%; }
  .right { text-align: right; font-variant-numeric: tabular-nums; }
  tr.canceled td { color: var(--text-muted); text-decoration: line-through; }
  tr.storno td { background: var(--status-warning-bg); }
  .muted { color: var(--text-muted); }
</style>
