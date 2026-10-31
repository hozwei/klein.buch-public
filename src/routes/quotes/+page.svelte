<script lang="ts">
  import { onMount, tick } from "svelte";
  import { quotesList } from "#lib/api.ts";
  import type { QuoteListItem, QuoteStatus } from "#lib/types.ts";
  import { euro, date } from "#lib/format.ts";
  import { yearChoices } from "#lib/date.ts";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import Badge from "#lib/Badge.svelte";
  import DocNo from "#lib/DocNo.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import Table from "#lib/Table.svelte";
  import { quoteStatusView } from "./quoteStatus.ts";

  let rows: QuoteListItem[] = $state([]);
  let loading = $state(false);
  let error: string | null = $state(null);
  let fiscalYear: number | null = $state(null);
  // U1-062: Jahr als Auswahl. Angeboten werden die Jahre aller bisher geladenen
  // Belege (der erste Aufruf ist ungefiltert) und das laufende Jahr; ein
  // gefilterter Aufruf lässt keine Jahre aus der Auswahl verschwinden.
  let seenYears: number[] = $state([]);
  let yearOptions = $derived(yearChoices([...seenYears, fiscalYear]));
  let statusFilter: QuoteStatus | "" = $state("");
  let includeInactive = $state(true);
  // Stand des letzten Abrufs für Ergebniszeile und leeren Zustand (V2-09, V4-06).
  let appliedFiltered = $state(false);

  async function load() {
    loading = true;
    error = null;
    try {
      const filtered = fiscalYear !== null || statusFilter !== "" || !includeInactive;
      rows = await quotesList({
        fiscalYear: fiscalYear ?? undefined,
        status: (statusFilter || undefined) as QuoteStatus | undefined,
        includeInactive,
      });
      seenYears = yearChoices([...seenYears, ...rows.map((r) => r.fiscalYear)]);
      appliedFiltered = filtered;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  async function resetFilters() {
    fiscalYear = null;
    statusFilter = "";
    includeInactive = true;
    await load();
    await tick();
    document.getElementById("quote-filter-year")?.focus();
  }

  // Ergebniszeile (4.1.3): dauerhaft im DOM, damit die Trefferzahl nach „Filtern“
  // angesagt wird; beim Laden „Wird geladen …“ (V2-02).
  let resultText = $derived(
    loading
      ? "Wird geladen …"
      : error
        ? ""
        : rows.length === 0
          ? appliedFiltered
            ? "Keine Treffer für diese Filter."
            : "Noch keine Angebote."
          : `${rows.length} ${rows.length === 1 ? "Angebot" : "Angebote"}`,
  );
</script>

<PageBar title="Angebote">
  {#snippet actions()}
    <Button variant="primary" href="/quotes/new">+ Neues Angebot</Button>
  {/snippet}
</PageBar>

<form class="toolbar" onsubmit={(e) => { e.preventDefault(); if (!loading) load(); }} novalidate>
  <label>
    Jahr
    <select id="quote-filter-year" class="kb-input" bind:value={fiscalYear}>
      <option value={null}>Alle</option>
      {#each yearOptions as y (y)}<option value={y}>{y}</option>{/each}
    </select>
  </label>
  <label>
    Status
    <select class="kb-input" bind:value={statusFilter}>
      <option value="">Alle</option>
      <option value="draft">Entwurf</option>
      <option value="sent">Fertiggestellt</option>
      <option value="accepted">Angenommen</option>
      <option value="rejected">Abgelehnt</option>
      <option value="canceled">Verworfen oder storniert</option>
      <option value="converted">Umgewandelt</option>
    </select>
  </label>
  <label class="chk">
    <input type="checkbox" bind:checked={includeInactive} />
    Abgelehnte, verworfene und stornierte anzeigen
  </label>
  <Button type="submit" variant="secondary" ariaDisabled={loading}>Filtern</Button>
</form>

<p class="result-info" role="status">{resultText}</p>

{#if loading}
  <!-- Ladezustand: die Ergebniszeile darüber sagt „Wird geladen …“ an. -->
{:else if error}
  <Banner>Fehler: {error}</Banner>
{:else if rows.length === 0}
  <!-- V2-09: Erststart mit der Hauptaktion (secondary), gefiltert mit dem Weg zurück. -->
  <div class="empty">
    {#if appliedFiltered}
      <Button variant="secondary" onclick={resetFilters}>Filter zurücksetzen</Button>
    {:else}
      <Button variant="secondary" href="/quotes/new">+ Neues Angebot</Button>
    {/if}
  </div>
{:else}
  <!-- U3-030: Liste im Table-Baustein (weiße Karte, Lineatur), Nummer als DocNo. -->
  <Table caption="Angebote" cards cardGrid>
    <thead>
      <tr>
        <th>Nummer</th>
        <th class="kb-col-opt2">Datum</th>
        <th>Gültig bis</th>
        <th>Empfänger</th>
        <th class="kb-num">Brutto</th>
        <th>Status</th>
        <th><span class="kb-visually-hidden">Aktionen</span></th>
      </tr>
    </thead>
    <tbody>
      {#each rows as r (r.id)}
        {@const inactive = r.status === "canceled" || r.status === "rejected"}
        {@const st = quoteStatusView(r)}
        <tr class:canceled={inactive}>
          <td class="kb-card-head" data-label="Nummer"><DocNo value={r.quoteNumber} href={`/quotes/${r.id}`} muted={inactive} /></td>
          <td class="kb-col-opt2" data-label="Datum">{date(r.quoteDate)}</td>
          <td class:kb-card-omit={!r.validUntil} data-label="Gültig bis">{date(r.validUntil)}</td>
          <td data-label="Empfänger">{r.contactName}</td>
          <td class="kb-num kb-card-figure" data-label="Brutto">{euro(r.grossAmountCents)}</td>
          <td class="kb-card-status" data-label="Status"><Badge tone={st.tone} strike={st.strike}>{st.label}</Badge></td>
          <td class="actions"><Button variant="secondary" href={`/quotes/${r.id}`} ariaLabel={`Angebot ${r.quoteNumber} öffnen`}>Öffnen</Button></td>
        </tr>
      {/each}
    </tbody>
  </Table>
{/if}

<style>
  .toolbar { display: flex; gap: 0.75rem; align-items: end; margin-bottom: var(--sp-2); flex-wrap: wrap; }
  .toolbar label { display: flex; flex-direction: column; font-size: var(--fs-sm); font-weight: 600; color: var(--text-muted); gap: 0.2rem; }
  .toolbar .kb-input { min-width: 9rem; }
  .chk { flex-direction: row !important; align-items: center; gap: 0.4rem; min-height: var(--target); font-weight: 400 !important; }
  .result-info { color: var(--text-muted); font-size: var(--fs-sm); margin: var(--sp-1) 0 var(--sp-2); min-height: 1.4em; }
  .empty { display: flex; flex-wrap: wrap; gap: var(--sp-3); align-items: center; margin: 0 0 var(--sp-3); }
  td.actions { text-align: right; }
  tr.canceled td { color: var(--text-muted); }
</style>
