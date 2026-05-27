<script lang="ts">
  import { onMount } from "svelte";
  import { invoicesList } from "$lib/api";
  import type { InvoiceListItem, InvoiceStatus } from "$lib/types";
  import { euro, date } from "$lib/format";
  import Banner from "$lib/Banner.svelte";
  import Button from "$lib/Button.svelte";
  import Badge from "$lib/Badge.svelte";
  import PageBar from "$lib/PageBar.svelte";

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
  <label>
    Jahr
    <input class="kb-input" type="number" bind:value={fiscalYear} placeholder="z. B. 2026" min="2000" max="2100" />
  </label>
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
  <p class="muted">Keine Rechnungen.</p>
{:else}
  <table class="kb-table">
    <thead>
      <tr>
        <th>Nummer</th>
        <th>Datum</th>
        <th>Empfänger</th>
        <th class="right">Brutto</th>
        <th class="right">Bezahlt</th>
        <th>Status</th>
        <th>Fällig</th>
        <th></th>
      </tr>
    </thead>
    <tbody>
      {#each rows as r (r.id)}
        <tr class:storno={r.isStornoFor !== null} class:canceled={r.status === "canceled"}>
          <td>
            <a href={`/invoices/${r.id}`}>{r.invoiceNumber}</a>
            {#if r.isStornoFor}<Badge tone="warning">Storno</Badge>{/if}
          </td>
          <td>{date(r.invoiceDate)}</td>
          <td>{r.contactName}</td>
          <td class="right">{euro(r.grossAmountCents)}</td>
          <td class="right">{euro(r.paidAmountCents)}</td>
          <td><Badge tone={statusTone(r.status)} strike={r.status === "canceled"}>{statusLabel(r.status)}</Badge></td>
          <td>{date(r.dueDate)}</td>
          <td class="right"><Button variant="secondary" size="sm" href={`/invoices/${r.id}`}>Öffnen</Button></td>
        </tr>
      {/each}
    </tbody>
  </table>
{/if}

<style>
  .toolbar { display: flex; gap: 0.75rem; align-items: end; margin-bottom: 1rem; flex-wrap: wrap; }
  .toolbar label { display: flex; flex-direction: column; font-size: 0.85rem; color: var(--c-text-muted); gap: 0.2rem; }
  .toolbar .kb-input { min-width: 9rem; }
  .chk { flex-direction: row !important; align-items: center; gap: 0.4rem; }
  table { width: 100%; }
  .right { text-align: right; font-variant-numeric: tabular-nums; }
  tr.canceled td { color: var(--c-text-subtle); text-decoration: line-through; }
  tr.storno td { background: var(--c-warning-50); }
  .muted { color: var(--c-text-muted); }
</style>
