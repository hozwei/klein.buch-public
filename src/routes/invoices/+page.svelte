<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoicesList } from "#lib/api.ts";
  import type { InvoiceListItem, InvoiceStatus } from "#lib/types.ts";
  import { euro, date } from "#lib/format.ts";
  import { todayBerlin, yearChoices } from "#lib/date.ts";
  import { dayClock } from "#lib/dayClock.svelte.ts";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import Badge from "#lib/Badge.svelte";
  import DocNo from "#lib/DocNo.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import Table from "#lib/Table.svelte";
  import { invoiceStatusView, isOverdue } from "./invoiceStatus.ts";

  let rows: InvoiceListItem[] = $state([]);
  let loading = $state(false);
  let error: string | null = $state(null);
  let fiscalYear: number | null = $state(null);
  // U1-062: Jahr als Auswahl. Angeboten werden die Jahre aller bisher geladenen
  // Belege (der erste Aufruf ist ungefiltert) und das laufende Jahr; ein
  // gefilterter Aufruf lässt keine Jahre aus der Auswahl verschwinden.
  let seenYears: number[] = $state([]);
  let yearOptions = $derived(yearChoices([...seenYears, fiscalYear]));
  // „overdue“ filtert im Browser (der Server kennt „Überfällig“ nicht als Status).
  let statusFilter: InvoiceStatus | "overdue" | "" = $state("");
  let includeCanceled = $state(true);
  // Stand des letzten Abrufs: Ergebniszeile und leerer Zustand richten sich danach,
  // nicht nach den Feldern, die gerade bearbeitet werden (V2-09, V4-06).
  let appliedFiltered = $state(false);
  let appliedOverdue = $state(false);

  // Heute in Europe/Berlin, reaktiv über Mitternacht (V2-03, wie Start).
  const todayIso = $derived(todayBerlin(dayClock.now));
  let shown = $derived(appliedOverdue ? rows.filter((r) => isOverdue(r, todayIso)) : rows);

  async function load() {
    loading = true;
    error = null;
    try {
      const overdue = statusFilter === "overdue";
      const filtered = fiscalYear !== null || statusFilter !== "" || !includeCanceled;
      rows = await invoicesList({
        fiscalYear: fiscalYear ?? undefined,
        status: (overdue ? undefined : statusFilter || undefined) as InvoiceStatus | undefined,
        includeCanceled,
      });
      seenYears = yearChoices([...seenYears, ...rows.map((r) => r.fiscalYear)]);
      appliedOverdue = overdue;
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
    includeCanceled = true;
    await load();
    await tick();
    document.getElementById("inv-filter-year")?.focus();
  }

  // Ergebniszeile (4.1.3): steht dauerhaft im DOM, damit Screenreader die neue
  // Trefferzahl nach „Filtern“ ansagen; beim Laden „Wird geladen …“ (V2-02).
  let resultText = $derived(
    loading
      ? "Wird geladen …"
      : error
        ? ""
        : shown.length === 0
          ? appliedFiltered
            ? "Keine Treffer für diese Filter."
            : "Noch keine Rechnungen."
          : `${shown.length} ${shown.length === 1 ? "Rechnung" : "Rechnungen"}`,
  );
</script>

<!-- V2-10: Nebenziel als `secondary` (bis 600 px im Menü „Mehr“), Hauptaktion zuletzt. -->
<PageBar title="Rechnungen" secondary={[{ label: "Abo-Rechnungen", href: "/recurring-invoices" }]}>
  {#snippet actions()}
    <Button variant="primary" href="/invoices/new">+ Neue Rechnung</Button>
  {/snippet}
</PageBar>

<form class="toolbar" onsubmit={(e) => { e.preventDefault(); if (!loading) load(); }} novalidate>
  <label>
    Jahr
    <select id="inv-filter-year" class="kb-input" bind:value={fiscalYear}>
      <option value={null}>Alle</option>
      {#each yearOptions as y (y)}<option value={y}>{y}</option>{/each}
    </select>
  </label>
  <label>
    Status
    <select class="kb-input" bind:value={statusFilter}>
      <option value="">Alle</option>
      <option value="draft">Entwurf</option>
      <option value="issued">Ausgestellt</option>
      <option value="sent">Versendet</option>
      <option value="partially_paid">Teilbezahlt</option>
      <option value="overdue">Überfällig</option>
      <option value="paid">Bezahlt</option>
      <option value="canceled">Storniert</option>
    </select>
  </label>
  <label class="chk">
    <input type="checkbox" bind:checked={includeCanceled} />
    Stornos anzeigen
  </label>
  <Button type="submit" variant="secondary" ariaDisabled={loading}>Filtern</Button>
</form>

<p class="result-info" role="status">{resultText}</p>

{#if loading}
  <!-- Ladezustand: die Ergebniszeile darüber sagt „Wird geladen …“ an. -->
{:else if error}
  <Banner>Fehler: {error}</Banner>
{:else if shown.length === 0}
  <!-- V2-09: Erststart mit der Hauptaktion (secondary, die PageBar hat die primary),
       gefiltert mit dem Weg zurück aus dem Filter. -->
  <div class="empty">
    {#if appliedFiltered}
      <Button variant="secondary" onclick={resetFilters}>Filter zurücksetzen</Button>
    {:else}
      <Button variant="secondary" href="/invoices/new">+ Neue Rechnung</Button>
    {/if}
  </div>
{:else}
  <!-- U3-030: Liste im Table-Baustein (weiße Karte, Lineatur), Nummer als DocNo. -->
  <Table caption="Rechnungen" cards cardGrid>
    <thead>
      <tr>
        <th>Nummer</th>
        <th class="kb-col-opt2">Datum</th>
        <th>Empfänger</th>
        <th class="kb-num">Brutto</th>
        <th class="kb-num kb-col-opt1">Bezahlt</th>
        <th>Status</th>
        <th>Fällig</th>
        <th><span class="kb-visually-hidden">Aktionen</span></th>
      </tr>
    </thead>
    <tbody>
      {#each shown as r (r.id)}
        {@const gone = r.status === "canceled" || !!r.voidedAt}
        {@const st = invoiceStatusView(r, todayIso)}
        <tr class:canceled={gone}>
          <td class="kb-card-head" data-label="Nummer">
            <span class="nr">
              <DocNo value={r.invoiceNumber} href={`/invoices/${r.id}`} muted={gone} />
              <!-- V2-05/V2-14, V3-08: Belegart als Merkmal (kind="tag", ohne Punkt), keine Warnfläche. -->
              {#if r.isStornoFor}<Badge kind="tag">Storno-Beleg</Badge>{/if}
            </span>
          </td>
          <td class="kb-col-opt2" data-label="Datum">{date(r.invoiceDate)}</td>
          <td data-label="Empfänger">{r.contactName}</td>
          <td class="kb-num kb-card-figure" data-label="Brutto">{euro(r.grossAmountCents)}</td>
          <td class="kb-num kb-col-opt1" data-label="Bezahlt">{euro(r.paidAmountCents)}</td>
          <td class="kb-card-status" data-label="Status"><Badge tone={st.tone} strike={st.strike}>{st.label}</Badge></td>
          <td class:kb-card-omit={!r.dueDate} data-label="Fällig">{date(r.dueDate)}</td>
          <td class="actions"><Button variant="secondary" href={`/invoices/${r.id}`} ariaLabel={`${r.isStornoFor ? "Storno-Beleg" : "Rechnung"} ${r.invoiceNumber} öffnen`}>Öffnen</Button></td>
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
  .nr { display: inline-flex; align-items: center; gap: var(--sp-2); flex-wrap: wrap; }
  .result-info { color: var(--text-muted); font-size: var(--fs-sm); margin: var(--sp-1) 0 var(--sp-2); min-height: 1.4em; }
  .empty { display: flex; flex-wrap: wrap; gap: var(--sp-3); align-items: center; margin: 0 0 var(--sp-3); }
  td.actions { text-align: right; }
  /* Storniert/verworfen: gedämpft; der Status steht durchgestrichen im Badge (design.md §4.9). */
  tr.canceled td { color: var(--text-muted); }
</style>
