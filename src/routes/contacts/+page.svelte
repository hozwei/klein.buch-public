<script lang="ts">
  import { onMount } from "svelte";
  import { contactsList, contactsSearch } from "#lib/api.ts";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import Badge from "#lib/Badge.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import Table from "#lib/Table.svelte";
  import type { Contact } from "#lib/types.ts";

  // U4 (V4-06/V2-02): eine dauerhafte Statuszeile (`role="status"`) für Laden und
  // Trefferzahl, damit Suchergebnisse angesagt werden (4.1.3); sie steht immer im DOM.
  // U4 (V2-07/V2-09/V2-10): Ladefehler als Banner mit „Erneut laden“; leerer Erststart mit
  // derselben Beschriftung wie die PageBar (secondary), leere Suche mit „Suche zurücksetzen“;
  // Checkbox „Archivierte anzeigen“ wie in den anderen Listen.

  let contacts: Contact[] = $state([]);
  let query: string = $state("");
  /** Suchbegriff des zuletzt geladenen Ergebnisses (Text der Statuszeile). */
  let shownQuery: string = $state("");
  let includeArchived: boolean = $state(false);
  let loading: boolean = $state(true);
  let error: string | null = $state(null);

  async function load() {
    loading = true;
    error = null;
    const q = query.trim();
    try {
      contacts = q
        ? await contactsSearch(q, includeArchived)
        : sortByName(await contactsList(includeArchived));
      shownQuery = q;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  // U3: Der Server sortiert mit NOCASE (nur ASCII) — „Ärztehaus“ und „Öko-…“ standen
  // hinter „Zahnrad“. Anzeige nach deutscher Sortierung (design.md §6, Intl.Collator).
  const byName = new Intl.Collator("de", { sensitivity: "base", numeric: true });
  function sortByName(list: Contact[]): Contact[] {
    return [...list].sort((a, b) => byName.compare(a.name, b.name));
  }

  onMount(load);

  function onSubmit(e: Event) {
    e.preventDefault();
    load();
  }

  /** Leere Suche zurücksetzen; der Knopf verschwindet danach, darum Fokus ins Suchfeld. */
  function resetSearch() {
    query = "";
    document.getElementById("contacts-search")?.focus();
    load();
  }

  const statusText = $derived.by(() => {
    if (loading) return "Wird geladen …";
    if (error) return "";
    const n = contacts.length;
    if (shownQuery) {
      return n === 0
        ? `Keine Treffer für „${shownQuery}“.`
        : `${n === 1 ? "1 Kontakt" : `${n} Kontakte`} zu „${shownQuery}“.`;
    }
    return n === 1 ? "1 Kontakt." : `${n} Kontakte.`;
  });

  function typeLabel(t: string): string {
    switch (t) {
      case "customer": return "Kunde";
      case "vendor":   return "Lieferant";
      case "both":     return "Kunde + Lieferant";
      case "partner":  return "Partner";
      default:         return t;
    }
  }
</script>

<PageBar title="Kontakte">
  {#snippet actions()}
    <Button variant="primary" href="/contacts/new">+ Neuer Kontakt</Button>
  {/snippet}
</PageBar>

<form class="toolbar" role="search" onsubmit={onSubmit} novalidate>
  <label class="kb-visually-hidden" for="contacts-search">Kontakte durchsuchen</label>
  <input
    id="contacts-search"
    class="kb-input search"
    type="search"
    enterkeyhint="search"
    autocomplete="off"
    placeholder="Name, E-Mail, USt-IdNr., Ort oder PLZ"
    bind:value={query}
  />
  <label class="chk">
    <input type="checkbox" bind:checked={includeArchived} onchange={load} />
    Archivierte anzeigen
  </label>
  <Button type="submit" variant="secondary" busy={loading}>Suchen</Button>
</form>

<p class="result-info" role="status">{statusText}</p>

{#if error}
  <Banner title="Die Kontakte konnten nicht geladen werden.">{error}
    {#snippet actions()}<Button variant="secondary" onclick={load}>Erneut laden</Button>{/snippet}
  </Banner>
{:else if !loading && contacts.length === 0}
  <div class="empty">
    {#if shownQuery}
      <p class="muted">Keine Treffer für diese Suche.</p>
      <Button variant="secondary" onclick={resetSearch}>Suche zurücksetzen</Button>
    {:else}
      <p class="muted">Noch keine Kontakte. Leg Kunden und Lieferanten hier an.</p>
      <Button variant="secondary" href="/contacts/new">+ Neuer Kontakt</Button>
    {/if}
  </div>
{:else if !loading}
  <!-- U4 (U3-030, design.md §4.5): Liste im Table-Baustein (weiße Karte, Lineatur). -->
  <Table cards cardGrid>
    <caption class="kb-visually-hidden">Kontakte</caption>
    <thead>
      <tr>
        <th>Name</th>
        <th>Typ</th>
        <th>Ort</th>
        <th class="kb-col-opt2">USt-IdNr.</th>
        <th>E-Mail</th>
        <th><span class="kb-visually-hidden">Aktion</span></th>
      </tr>
    </thead>
    <tbody>
      {#each contacts as c (c.id)}
        <tr class:archived={c.archived === 1}>
          <td data-label="Name">
            <a class="name" href={`/contacts/${c.id}`}>{c.name}</a>
            {#if c.archived === 1}<Badge tone="neutral">Archiviert</Badge>{/if}
          </td>
          <td data-label="Typ">{typeLabel(c.contactType)}</td>
          <td data-label="Ort">{[c.postalCode, c.city].filter(Boolean).join(" ") || "—"}</td>
          <td class="kb-col-opt2" data-label="USt-IdNr.">{c.vatId ?? "—"}</td>
          <td data-label="E-Mail">{c.email ?? "—"}</td>
          <td class="right"><Button variant="secondary" href={`/contacts/${c.id}`} ariaLabel={`${c.name} öffnen`}>Öffnen</Button></td>
        </tr>
      {/each}
    </tbody>
  </Table>
{/if}

<style>
  .toolbar {
    display: flex;
    gap: 0.75rem;
    align-items: center;
    margin-bottom: 0.5rem;
    flex-wrap: wrap;
  }
  .search { flex: 1; min-width: 14rem; max-width: 100%; }
  /* Ganze Beschriftung ist Trefferfläche (U1). */
  .chk { display: flex; align-items: center; gap: 0.5rem; min-height: var(--target); color: var(--c-text-muted); cursor: pointer; }
  .result-info { color: var(--text-muted); font-size: var(--fs-sm); margin: var(--sp-1) 0 var(--sp-2); min-height: 1.4em; }
  /* Name als Kartentitel: Trefferfläche ≥ --target (U1, 2.5.8). */
  a.name { display: inline-flex; align-items: center; min-height: var(--target); overflow-wrap: anywhere; }
  .empty { display: flex; flex-direction: column; align-items: flex-start; gap: 0.6rem; }
  .empty p { margin: 0; }
  td.right { text-align: right; }
  @media screen and (max-width: 820px) { td { overflow-wrap: anywhere; } }
  tr.archived td { color: var(--c-text-subtle); }
  .muted { color: var(--c-text-muted); }
</style>
