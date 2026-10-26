<script lang="ts">
  import { onMount } from "svelte";
  import { quotesList } from "#lib/api.ts";
  import type { QuoteListItem, QuoteStatus } from "#lib/types.ts";
  import { euro, date } from "#lib/format.ts";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import Badge from "#lib/Badge.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import QuantityInput from "#lib/QuantityInput.svelte";

  let rows: QuoteListItem[] = $state([]);
  let loading = $state(false);
  let error: string | null = $state(null);
  let fiscalYear: number | null = $state(null);
  let statusFilter: QuoteStatus | "" = $state("");
  let includeInactive = $state(true);

  async function load() {
    loading = true;
    error = null;
    try {
      rows = await quotesList({
        fiscalYear: fiscalYear ?? undefined,
        status: (statusFilter || undefined) as QuoteStatus | undefined,
        includeInactive,
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
      case "draft":      return "Entwurf";
      case "sent":       return "Versendet";
      case "accepted":   return "Angenommen";
      case "rejected":   return "Abgelehnt";
      case "canceled":   return "Storniert";
      case "converted":  return "Umgewandelt";
      default:           return s;
    }
  }

  type Tone = "neutral" | "primary" | "info" | "success" | "warning" | "danger";
  function statusTone(s: string): Tone {
    switch (s) {
      case "draft":      return "neutral";
      case "sent":       return "info";
      case "accepted":   return "success";
      case "rejected":   return "danger";
      case "canceled":   return "danger";
      case "converted":  return "primary";
      default:           return "neutral";
    }
  }
</script>

<PageBar title="Angebote">
  {#snippet actions()}
    <Button variant="primary" href="/quotes/new">+ Neues Angebot</Button>
  {/snippet}
</PageBar>

<form class="toolbar" onsubmit={(e) => { e.preventDefault(); load(); }} novalidate>
  <div class="tb-field">
    <label for="quote-filter-year">Jahr</label>
    <QuantityInput id="quote-filter-year" bind:value={fiscalYear} integer grouping={false} noun="Jahr" placeholder="alle" />
  </div>
  <label>
    Status
    <select class="kb-input" bind:value={statusFilter}>
      <option value="">Alle</option>
      <option value="draft">Entwurf</option>
      <option value="sent">Versendet</option>
      <option value="accepted">Angenommen</option>
      <option value="rejected">Abgelehnt</option>
      <option value="canceled">Storniert</option>
      <option value="converted">Umgewandelt</option>
    </select>
  </label>
  <label class="chk">
    <input type="checkbox" bind:checked={includeInactive} />
    Abgelehnte &amp; stornierte anzeigen
  </label>
  <Button type="submit" variant="secondary">Filtern</Button>
</form>

{#if loading}
  <p class="muted">Lade …</p>
{:else if error}
  <Banner>Fehler: {error}</Banner>
{:else if rows.length === 0}
  <div class="empty">
    <p class="muted">Keine Angebote für diese Auswahl.</p>
    <Button variant="primary" href="/quotes/new">Neues Angebot anlegen</Button>
  </div>
{:else}
  <table class="kb-table cards kb-card-grid">
    <caption class="kb-visually-hidden">Angebote</caption>
    <thead>
      <tr>
        <th>Nummer</th>
        <th class="kb-col-opt2">Datum</th>
        <th>Gültig bis</th>
        <th>Empfänger</th>
        <th class="right">Brutto</th>
        <th>Status</th>
        <th><span class="kb-visually-hidden">Aktionen</span></th>
      </tr>
    </thead>
    <tbody>
      {#each rows as r (r.id)}
        <tr class:canceled={r.status === "canceled" || r.status === "rejected"}>
          <td data-label="Nummer"><a class="doc-no" href={`/quotes/${r.id}`}>{r.quoteNumber}</a></td>
          <td class="kb-col-opt2" data-label="Datum">{date(r.quoteDate)}</td>
          <td data-label="Gültig bis">{date(r.validUntil)}</td>
          <td data-label="Empfänger">{r.contactName}</td>
          <td class="right" data-label="Brutto">{euro(r.grossAmountCents)}</td>
          <td data-label="Status"><Badge tone={statusTone(r.status)} strike={r.status === "canceled"}>{statusLabel(r.status)}</Badge></td>
          <td class="right"><Button variant="secondary" size="sm" href={`/quotes/${r.id}`} ariaLabel={`Angebot ${r.quoteNumber} öffnen`}>Öffnen</Button></td>
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
  tr.canceled td { color: var(--text-muted); }
  .muted { color: var(--text-muted); }
</style>
