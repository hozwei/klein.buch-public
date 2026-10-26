<script lang="ts">
  import { onMount } from "svelte";
  import { contactsList, contactsSearch } from "#lib/api.ts";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import Badge from "#lib/Badge.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import type { Contact } from "#lib/types.ts";

  let contacts: Contact[] = $state([]);
  let query: string = $state("");
  let includeArchived: boolean = $state(false);
  let loading: boolean = $state(false);
  let error: string | null = $state(null);

  async function load() {
    loading = true;
    error = null;
    try {
      contacts = query.trim()
        ? await contactsSearch(query, includeArchived)
        : await contactsList(includeArchived);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  function onSubmit(e: Event) {
    e.preventDefault();
    load();
  }

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
    Archivierte einschließen
  </label>
  <Button type="submit" variant="secondary">Suchen</Button>
</form>

{#if loading}
  <p class="muted" role="status" aria-busy="true">Lädt …</p>
{:else if error}
  <Banner>Fehler: {error}</Banner>
{:else if contacts.length === 0}
  <div class="empty">
    {#if query.trim()}
      <p class="muted">Keine Kontakte zu „{query.trim()}“ gefunden.</p>
    {:else}
      <p class="muted">Noch keine Kontakte. Leg Kunden und Lieferanten hier an.</p>
      <Button variant="primary" href="/contacts/new">Kontakt anlegen</Button>
    {/if}
  </div>
{:else}
  <table class="kb-table cards">
    <caption class="kb-visually-hidden">Kontakte</caption>
    <thead>
      <tr>
        <th>Name</th>
        <th>Typ</th>
        <th>Ort</th>
        <th>USt-IdNr.</th>
        <th>E-Mail</th>
        <th><span class="kb-visually-hidden">Aktion</span></th>
      </tr>
    </thead>
    <tbody>
      {#each contacts as c (c.id)}
        <tr class:archived={c.archived === 1}>
          <td data-label="Name">
            <a class="name" href={`/contacts/${c.id}`}>{c.name}</a>
            {#if c.archived === 1}<Badge tone="warning">archiviert</Badge>{/if}
          </td>
          <td data-label="Typ">{typeLabel(c.contactType)}</td>
          <td data-label="Ort">{[c.postalCode, c.city].filter(Boolean).join(" ") || "—"}</td>
          <td data-label="USt-IdNr.">{c.vatId ?? "—"}</td>
          <td data-label="E-Mail">{c.email ?? "—"}</td>
          <td class="right"><Button variant="secondary" href={`/contacts/${c.id}`} ariaLabel={`${c.name} öffnen`}>Öffnen</Button></td>
        </tr>
      {/each}
    </tbody>
  </table>
{/if}

<style>
  .toolbar {
    display: flex;
    gap: 0.75rem;
    align-items: center;
    margin-bottom: 1rem;
    flex-wrap: wrap;
  }
  .search { flex: 1; min-width: 14rem; max-width: 100%; }
  /* Ganze Beschriftung ist Trefferfläche (U1). */
  .chk { display: flex; align-items: center; gap: 0.5rem; min-height: var(--target); color: var(--c-text-muted); cursor: pointer; }
  /* Name als Kartentitel: Trefferfläche ≥ --target (U1, 2.5.8). */
  a.name { display: inline-flex; align-items: center; min-height: var(--target); overflow-wrap: anywhere; }
  .empty { display: flex; flex-direction: column; align-items: flex-start; gap: 0.6rem; }
  .empty p { margin: 0; }
  td.right { text-align: right; }
  @media (max-width: 820px) { td { overflow-wrap: anywhere; } }
  tr.archived td { color: var(--c-text-subtle); }
  .muted { color: var(--c-text-muted); }
</style>
