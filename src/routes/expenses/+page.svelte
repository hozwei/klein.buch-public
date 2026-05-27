<script lang="ts">
  import { onMount } from "svelte";
  import { expensesList, recurringList } from "$lib/api";
  import type { ExpenseListItem } from "$lib/types";
  import { euro, date } from "$lib/format";
  import { expenseCategoryLabel, EXPENSE_CATEGORIES } from "$lib/labels";
  import Banner from "$lib/Banner.svelte";
  import Button from "$lib/Button.svelte";
  import Badge from "$lib/Badge.svelte";
  import PageBar from "$lib/PageBar.svelte";

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
      const today = new Date().toISOString().slice(0, 10);
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
    <span class="due-dot"></span>
    {dueAbos} {dueAbos === 1 ? "wiederkehrendes Abo ist" : "wiederkehrende Abos sind"} fällig
    <span class="due-cta">→ ansehen &amp; buchen</span>
  </a>
{/if}

<div class="filters">
  <input
    type="search"
    class="kb-input search"
    placeholder="Suche: Beleg-Nr., Lieferant, Beschreibung …"
    bind:value={search}
  />
  <select class="kb-input" bind:value={yearFilter}>
    <option value="">Alle Jahre</option>
    {#each years as y}<option value={y}>{y}</option>{/each}
  </select>
  <select class="kb-input" bind:value={categoryFilter}>
    <option value="">Alle Kategorien</option>
    {#each EXPENSE_CATEGORIES as c}<option value={c.value}>{c.label}</option>{/each}
  </select>
  <label class="chk">
    <input type="checkbox" bind:checked={includeCanceled} /> Stornierte
  </label>
  <Button variant="secondary" size="sm" onclick={resetFilters}>Zurücksetzen</Button>
</div>

{#if loading}
  <p class="muted">Lade …</p>
{:else if error}
  <Banner>{error}</Banner>
{:else if all.length === 0}
  <p class="muted">Noch keine Kosten erfasst.</p>
{:else}
  <p class="result-info">
    {items.length} {items.length === 1 ? "Eintrag" : "Einträge"} · Summe (ohne Storno): <strong>{euro(sum)}</strong>
  </p>
  {#if items.length === 0}
    <p class="muted">Keine Kosten passen zu den Filtern.</p>
  {:else}
    <table class="kb-table">
      <thead>
        <tr>
          <th>Beleg-Nr.</th>
          <th>Datum</th>
          <th>Lieferant</th>
          <th>Beschreibung</th>
          <th>Kategorie</th>
          <th class="num">Brutto</th>
          <th>Bezahlt</th>
          <th>Status</th>
          <th></th>
        </tr>
      </thead>
      <tbody>
        {#each items as e (e.id)}
          <tr class={e.status === "canceled" ? "canceled" : ""}>
            <td><a href={`/expenses/${e.id}`}>{e.expenseNumber}</a></td>
            <td>{date(e.expenseDate)}</td>
            <td>{e.vendorNameSnapshot}</td>
            <td class="desc">{e.description}</td>
            <td>
              <span class="cat">
                {expenseCategoryLabel(e.category)}
                {#if e.recurringSubscriptionId}<Badge tone="info">Abo</Badge>{/if}
                {#if e.reverseCharge13b === 1}<Badge tone="primary">§13b</Badge>{/if}
              </span>
            </td>
            <td class="num">{euro(e.grossAmountCents)}</td>
            <td>{e.paidDate ? date(e.paidDate) : "offen"}</td>
            <td>
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
            <td class="right"><Button variant="secondary" size="sm" href={`/expenses/${e.id}`}>Öffnen</Button></td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
{/if}

<style>
  .filters { display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; margin: 0.75rem 0; }
  .filters .kb-input { width: auto; font-size: 0.9rem; }
  .search { flex: 1 1 18rem; }
  .chk { display: inline-flex; align-items: center; gap: 0.35rem; font-size: 0.85rem; color: var(--c-text-muted); }
  .result-info { color: var(--c-text-muted); font-size: 0.85rem; margin: 0.25rem 0 0.5rem; }
  table { width: 100%; }
  .num { text-align: right; }
  td.num { text-align: right; font-variant-numeric: tabular-nums; }
  td.desc { max-width: 18rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  td.right { text-align: right; }
  .cat { display: inline-flex; align-items: center; gap: 0.35rem; flex-wrap: wrap; }
  .badges { display: inline-flex; align-items: center; gap: 0.35rem; flex-wrap: wrap; }
  tr.canceled td a { color: var(--c-text-subtle); }
  .due-banner { display: flex; align-items: center; gap: 0.5rem; background: var(--c-warning-50); border: 1px solid #f3dcae; color: var(--c-warning-700); padding: 0.6rem 0.85rem; border-radius: var(--r-md); text-decoration: none; font-size: 0.9rem; margin: 0.75rem 0; }
  .due-banner:hover { background: #fbe9c8; }
  .due-dot { width: 0.55rem; height: 0.55rem; border-radius: 50%; background: var(--c-warning-500); flex: none; }
  .due-cta { margin-left: auto; font-weight: 600; }
  .muted { color: var(--c-text-muted); }
</style>
