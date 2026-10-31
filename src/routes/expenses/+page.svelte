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
  import DocNo from "#lib/DocNo.svelte";
  import Table from "#lib/Table.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import HelpAnchor from "#lib/HelpAnchor.svelte";
  import { expenseStatusView } from "./expenseStatus.ts";

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

<!-- U4 (design.md §4.2): Nebenziele als `secondary` — bis 600 px im Menü „Mehr“. -->
<PageBar
  title="Kosten"
  secondary={[
    { label: "Kosten-Abos", href: "/expenses/recurring" },
    { label: "E-Rechnung importieren", href: "/expenses/import" },
  ]}
>
  {#snippet actions()}
    <Button variant="primary" href="/expenses/new">+ Neue Kosten</Button>
    <HelpAnchor slug="kosten" />
  {/snippet}
</PageBar>

<p class="muted">
  Hier trägst du ein, was du fürs Geschäft ausgibst — Einkäufe, Software, Miete,
  Versicherungen. Fürs Finanzamt zählt, <strong>wann du bezahlt hast</strong>
  (nicht das Rechnungsdatum).
</p>

{#if dueAbos > 0}
  <!-- V3-18 (ADR 0042 Nr. 16): Link-Banner statt eigener Fläche `.due-banner`. -->
  <Banner kind="warning" href="/expenses/recurring" linkLabel="ansehen und buchen">
    {dueAbos} {dueAbos === 1 ? "Kosten-Abo ist" : "Kosten-Abos sind"} fällig –
  </Banner>
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
    <input type="checkbox" bind:checked={includeCanceled} /> Stornierte anzeigen
  </label>
  <Button variant="secondary" size="sm" onclick={resetFilters}>Filter zurücksetzen</Button>
</div>

{#if loading}
  <p class="muted" role="status">Wird geladen …</p>
{:else if error}
  <Banner>{error}</Banner>
{:else if all.length === 0}
  <!-- U4 (V2-09): Erststart mit derselben Beschriftung wie die PageBar, als secondary. -->
  <div class="empty">
    <p class="muted">Noch keine Kosten erfasst.</p>
    <Button variant="secondary" href="/expenses/new">+ Neue Kosten</Button>
  </div>
{:else}
  <p class="result-info" role="status">
    {items.length} {items.length === 1 ? "Eintrag" : "Einträge"} · Summe (ohne Storno): <strong class="kb-num">{euro(sum)}</strong>
  </p>
  {#if items.length === 0}
    <div class="empty">
      <p class="muted">Keine Treffer für diese Filter.</p>
      <Button variant="secondary" onclick={resetFilters}>Filter zurücksetzen</Button>
    </div>
  {:else}
    <Table caption="Kosten" cards cardGrid>
      <thead>
        <!-- U4 (U3-016): Kategorie steht als zweite Zeile unter der Beschreibung statt in
             einer eigenen Spalte; Nummer, Daten, Betrag und Aktion brechen nicht um. So
             bekommen Lieferant und Beschreibung die frei werdende Breite. -->
        <tr>
          <th class="nw">Nummer</th>
          <th class="nw kb-col-opt1">Datum</th>
          <th class="vendor">Lieferant</th>
          <th class="desc">Beschreibung</th>
          <th class="kb-num nw">Brutto</th>
          <th>Bezahlt am</th>
          <th>Status</th>
          <th><span class="kb-visually-hidden">Aktion</span></th>
        </tr>
      </thead>
      <tbody>
        {#each items as e (e.id)}
          <!-- U4 (V3-09): Status aus expenseStatus.ts (Bezahlt/Unbezahlt/Storniert). -->
          {@const st = expenseStatusView(e)}
          <tr class={e.status === "canceled" ? "canceled" : ""}>
            <td class="nw kb-card-head" data-label="Nummer"><DocNo value={e.expenseNumber} href={`/expenses/${e.id}`} muted={e.status === "canceled"} /></td>
            <td class="nw kb-col-opt1" data-label="Datum">{date(e.expenseDate)}</td>
            <td class="wrap vendor" data-label="Lieferant">{e.vendorNameSnapshot}</td>
            <td class="desc wrap cards-full" data-label="Beschreibung">
              <span class="desc-text">{e.description}</span>
              <span class="cat">
                {expenseCategoryLabel(e.category)}
                <!-- U4 (V3-08): Merkmale ohne Statusfarbe und ohne Punkt. -->
                {#if e.recurringSubscriptionId}<Badge kind="tag">Abo</Badge>{/if}
                {#if e.reverseCharge13b === 1}<Badge kind="tag">§13b</Badge>{/if}
              </span>
            </td>
            <td class="kb-num kb-card-figure nw" data-label="Brutto">{euro(e.grossAmountCents)}</td>
            <td class="nw" class:kb-card-omit={!e.paidDate} data-label="Bezahlt am">{e.paidDate ? date(e.paidDate) : "—"}</td>
            <td class="kb-card-status" data-label="Status">
              <span class="badges">
                <Badge tone={st.tone} strike={st.strike}>{st.label}</Badge>
                {#if !e.receiptArchiveId && e.status !== "canceled"}
                  <Badge tone="warning">Kein Beleg</Badge>
                {/if}
              </span>
            </td>
            <td class="actions">
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
  /* U2 (U1-063): nicht mitten im Wort trennen („Arbeitsplatzrec/hner“) — `anywhere` lässt
     die Spalte im Tabellenmodus unter die Wortbreite schrumpfen; Silbentrennung (lang="de"). */
  td.wrap { overflow-wrap: break-word; hyphens: auto; }
  /* U3: Firmennamen nur an Leerzeichen umbrechen („Sys-teme GmbH“ las sich wie ein Tippfehler);
     ein Wort, das breiter als die Spalte ist, bricht trotzdem (break-word). */
  td.vendor { hyphens: manual; }
  td.actions { text-align: right; }
  /* U4 (U3-016): Tabellenmodus — feste Spalten so schmal wie ihr Inhalt; die frei
     werdende Breite teilen sich Lieferant und Beschreibung im Verhältnis 28 : 40
     (Vorgabe für die automatische Tabellenbreite, keine Mindestbreite, damit die Tabelle
     auch bei 1024 px nicht seitlich scrollt). */
  @media screen and (min-width: 821px) {
    th.nw, td.nw { white-space: nowrap; }
    th.vendor { width: 28%; }
    th.desc { width: 40%; }
  }
  .desc-text { display: block; }
  .cat {
    display: flex; align-items: center; gap: var(--sp-1); flex-wrap: wrap;
    margin-top: 2px; font-size: var(--type-small); color: var(--text-muted);
  }
  .badges { display: inline-flex; align-items: center; gap: var(--sp-1); flex-wrap: wrap; }
  .muted { color: var(--text-muted); }
</style>
