<script lang="ts">
  import { todayBerlin } from "#lib/date.ts";
  import { onMount } from "svelte";
  import { expensesList, recurringList } from "#lib/api.ts";
  import type { ExpenseListItem } from "#lib/types.ts";
  import { euro, date } from "#lib/format.ts";
  import { expenseCategoryLabel, EXPENSE_CATEGORIES } from "#lib/labels.ts";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import Badge from "#lib/Badge.svelte";
  import Table from "#lib/Table.svelte";
  import PageBar from "#lib/PageBar.svelte";

  // Anzahl aktuell fälliger Abos — als Hinweis oben in der Kosten-Übersicht.
  let dueAbos = $state(0);

  // Alle Kosten (inkl. stornierte) einmal laden; gefiltert wird client-seitig
  // (lokale App, beschränkte Datenmenge → sofort, ohne Backend-Roundtrips).
  let all = $state<ExpenseListItem[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  // Filter
  let search = $state("");
  let yearFilter = $state<number | "">("");
  let categoryFilter = $state<string>("");
  let includeCanceled = $state(false);

  async function load() {
    loading = true;
    error = null;
    try {
      all = await expensesList({ includeCanceled: true });
      const subs = await recurringList(false);
      const today = todayBerlin();
      dueAbos = subs.filter((s) => s.active === 1 && s.nextDueDate <= today).length;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  let years = $derived(
    [...new Set(all.map((e) => e.fiscalYear))].sort((a, b) => b - a),
  );

  let items = $derived(
    all.filter((e) => {
      if (!includeCanceled && e.status === "canceled") return false;
      if (yearFilter !== "" && e.fiscalYear !== yearFilter) return false;
      if (categoryFilter && e.category !== categoryFilter) return false;
      const q = search.trim().toLowerCase();
      if (q) {
        const hay = [
          e.expenseNumber,
          e.vendorNameSnapshot,
          e.vendorInvoiceNumber ?? "",
          e.description,
          expenseCategoryLabel(e.category),
        ]
          .join(" ")
          .toLowerCase();
        if (!hay.includes(q)) return false;
      }
      return true;
    }),
  );

  let sum = $derived(
    items
      .filter((e) => e.status !== "canceled")
      .reduce((acc, e) => acc + e.grossAmountCents, 0),
  );

  function resetFilters() {
    search = "";
    yearFilter = "";
    categoryFilter = "";
    includeCanceled = false;
  }
</script>

<PageBar title="Kosten">
  {#snippet actions()}
    <Button variant="secondary" size="sm" href="/expenses/recurring">Wiederkehrende Abos</Button>
    <Button variant="secondary" size="sm" href="/expenses/import">E-Rechnung importieren</Button>
    <Button variant="primary" href="/expenses/new">+ Neue Kosten</Button>
  {/snippet}
</PageBar>

<p class="muted">
  Hier trägst du ein, was du fürs Geschäft ausgibst — Einkäufe, Software, Miete,
  Versicherungen. Fürs Finanzamt zählt, <strong>wann du bezahlt hast</strong>
  (nicht das Rechnungsdatum).
</p>

{#if dueAbos > 0}
  <a class="due-banner" href="/expenses/recurring">
    <span class="due-dot" aria-hidden="true"></span>
    {dueAbos} {dueAbos === 1 ? "wiederkehrendes Abo ist" : "wiederkehrende Abos sind"} fällig
    <span class="due-cta">ansehen und buchen <span aria-hidden="true">→</span></span>
  </a>
{/if}

<div class="filters" role="search" aria-label="Kosten filtern">
  <div class="f-search">
    <label class="kb-visually-hidden" for="exp-search">Kosten durchsuchen</label>
    <input
      id="exp-search"
      type="search"
      class="kb-input"
      placeholder="Beleg-Nr., Lieferant, Beschreibung …"
      enterkeyhint="search"
      autocomplete="off"
      bind:value={search}
    />
  </div>
  <label class="f-sel">
    <span class="kb-visually-hidden">Geschäftsjahr</span>
    <select class="kb-input" bind:value={yearFilter}>
      <option value="">Alle Jahre</option>
      {#each years as y}<option value={y}>{y}</option>{/each}
    </select>
  </label>
  <label class="f-sel">
    <span class="kb-visually-hidden">Kategorie</span>
    <select class="kb-input" bind:value={categoryFilter}>
      <option value="">Alle Kategorien</option>
      {#each EXPENSE_CATEGORIES as c}<option value={c.value}>{c.label}</option>{/each}
    </select>
  </label>
  <label class="chk">
    <input type="checkbox" bind:checked={includeCanceled} /> Stornierte zeigen
  </label>
  <Button variant="secondary" size="sm" onclick={resetFilters}>Filter zurücksetzen</Button>
</div>

{#if loading}
  <p class="muted" aria-busy="true">Lädt …</p>
{:else if error}
  <Banner>{error}</Banner>
{:else if all.length === 0}
  <div class="empty">
    <p class="muted">Noch keine Kosten erfasst.</p>
    <Button variant="primary" href="/expenses/new">Erste Kosten erfassen</Button>
  </div>
{:else}
  <p class="result-info" role="status">
    {items.length} {items.length === 1 ? "Eintrag" : "Einträge"} · Summe (ohne Storno): <strong class="kb-num">{euro(sum)}</strong>
  </p>
  {#if items.length === 0}
    <div class="empty">
      <p class="muted">Keine Kosten passen zu den Filtern.</p>
      <Button variant="secondary" onclick={resetFilters}>Filter zurücksetzen</Button>
    </div>
  {:else}
    <Table caption="Kosten" cards cardGrid>
      <thead>
        <tr>
          <th>Beleg-Nr.</th>
          <th class="kb-col-opt2">Datum</th>
          <th>Lieferant</th>
          <th>Beschreibung</th>
          <th class="kb-col-opt1">Kategorie</th>
          <th class="num">Brutto</th>
          <th>Bezahlt</th>
          <th>Status</th>
          <th><span class="kb-visually-hidden">Aktion</span></th>
        </tr>
      </thead>
      <tbody>
        {#each items as e (e.id)}
          <tr class={e.status === "canceled" ? "canceled" : ""}>
            <td data-label="Beleg-Nr."><a class="doc-no" href={`/expenses/${e.id}`}>{e.expenseNumber}</a></td>
            <td class="kb-col-opt2" data-label="Datum">{date(e.expenseDate)}</td>
            <td class="wrap" data-label="Lieferant">{e.vendorNameSnapshot}</td>
            <td class="desc wrap cards-full" data-label="Beschreibung">{e.description}</td>
            <td class="kb-col-opt1" data-label="Kategorie">
              <span class="cat">
                {expenseCategoryLabel(e.category)}
                {#if e.recurringSubscriptionId}<Badge tone="info">Abo</Badge>{/if}
                {#if e.reverseCharge13b === 1}<Badge tone="primary">§13b</Badge>{/if}
              </span>
            </td>
            <td class="num kb-num" data-label="Brutto">{euro(e.grossAmountCents)}</td>
            <td data-label="Bezahlt">{e.paidDate ? date(e.paidDate) : "offen"}</td>
            <td data-label="Status">
              <span class="badges">
                {#if e.status === "canceled"}
                  <Badge tone="neutral" strike>storniert</Badge>
                {:else}
                  <Badge tone="success">erfasst</Badge>
                {/if}
                {#if !e.receiptArchiveId && e.status !== "canceled"}
                  <Badge tone="warning">kein Beleg</Badge>
                {/if}
              </span>
            </td>
            <td class="right">
              <Button variant="secondary" size="sm" href={`/expenses/${e.id}`} ariaLabel={`Kosten ${e.expenseNumber} öffnen`}>Öffnen</Button>
            </td>
          </tr>
        {/each}
      </tbody>
    </Table>
  {/if}
{/if}

<style>
  .filters { display: flex; gap: var(--sp-2); align-items: center; flex-wrap: wrap; margin: var(--sp-3) 0; }
  .filters .kb-input { width: auto; max-width: 100%; }
  .f-search { flex: 1 1 18rem; min-width: 0; }
  .f-search .kb-input { width: 100%; }
  .f-sel { display: inline-flex; min-width: 0; max-width: 100%; }
  .chk { display: inline-flex; align-items: center; gap: var(--sp-2); min-height: var(--target); color: var(--text-muted); }
  .result-info { color: var(--text-muted); font-size: var(--fs-sm); margin: var(--sp-1) 0 var(--sp-2); }
  .empty { display: flex; flex-wrap: wrap; align-items: center; gap: var(--sp-3); margin: var(--sp-3) 0; }
  .empty p { margin: 0; }
  .num { text-align: right; }
  td.num { text-align: right; font-variant-numeric: tabular-nums; }
  td.desc { max-width: 18rem; }
  /* U2 (U1-063): nicht mitten im Wort trennen („Arbeitsplatzrec/hner“) — `anywhere` lässt
     die Spalte im Tabellenmodus unter die Wortbreite schrumpfen; Silbentrennung (lang="de"). */
  td.wrap { overflow-wrap: break-word; hyphens: auto; }
  td.right { text-align: right; }
  @media (max-width: 820px) { td.desc { max-width: none; } }
  /* Belegnummer als Link: Trefferfläche ≥ --target, monospace (design.md §7). */
  a.doc-no {
    display: inline-flex; align-items: center; min-height: var(--target);
    font-family: var(--font-mono); white-space: nowrap; text-decoration: underline;
    text-underline-offset: 2px;
  }
  .cat { display: inline-flex; align-items: center; gap: var(--sp-1); flex-wrap: wrap; }
  .badges { display: inline-flex; align-items: center; gap: var(--sp-1); flex-wrap: wrap; }
  tr.canceled td a.doc-no { color: var(--text-muted); }
  .due-banner {
    display: flex; align-items: center; gap: var(--sp-2); flex-wrap: wrap;
    min-height: var(--target);
    background: var(--status-warning-bg); border: 1px solid var(--status-warning-line);
    color: var(--status-warning-fg); padding: var(--sp-2) var(--sp-3); border-radius: var(--r-md);
    text-decoration: none; margin: var(--sp-3) 0;
  }
  @media (hover: hover) {
    .due-banner:hover { background: var(--surface-hover); text-decoration: none; }
  }
  .due-dot { width: 0.55rem; height: 0.55rem; border-radius: 50%; background: var(--status-warning-line); flex: none; }
  .due-cta { margin-left: auto; font-weight: 600; text-decoration: underline; }
  .muted { color: var(--text-muted); }
</style>
