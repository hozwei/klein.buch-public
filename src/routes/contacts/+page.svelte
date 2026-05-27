<script lang="ts">
  import { onMount } from "svelte";
  import { contactsList, contactsSearch } from "$lib/api";
  import Banner from "$lib/Banner.svelte";
  import Button from "$lib/Button.svelte";
  import Badge from "$lib/Badge.svelte";
  import PageBar from "$lib/PageBar.svelte";
  import type { Contact } from "$lib/types";

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

<form class="toolbar" onsubmit={onSubmit} novalidate>
  <input
    class="kb-input search"
    type="search"
    placeholder="Suche (Name, E-Mail, USt-IdNr., Stadt, PLZ)"
    bind:value={query}
  />
  <label class="chk">
    <input type="checkbox" bind:checked={includeArchived} onchange={load} />
    Archivierte einschließen
  </label>
  <Button type="submit" variant="secondary">Suchen</Button>
</form>

{#if loading}
  <p class="muted">Lade …</p>
{:else if error}
  <Banner>Fehler: {error}</Banner>
{:else if contacts.length === 0}
  <p class="muted">Keine Kontakte gefunden.</p>
{:else}
  <table class="kb-table">
    <thead>
      <tr>
        <th>Name</th>
        <th>Typ</th>
        <th>Ort</th>
        <th>USt-IdNr.</th>
        <th>E-Mail</th>
        <th></th>
      </tr>
    </thead>
    <tbody>
      {#each contacts as c (c.id)}
        <tr class:archived={c.archived === 1}>
          <td>
            <a href={`/contacts/${c.id}`}>{c.name}</a>
            {#if c.archived === 1}<Badge tone="warning">archiviert</Badge>{/if}
          </td>
          <td>{typeLabel(c.contactType)}</td>
          <td>{c.postalCode ?? ""} {c.city ?? ""}</td>
          <td>{c.vatId ?? "—"}</td>
          <td>{c.email ?? "—"}</td>
          <td class="right"><Button variant="secondary" size="sm" href={`/contacts/${c.id}`}>Öffnen</Button></td>
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
  .search { flex: 1; min-width: 14rem; }
  .chk { display: flex; align-items: center; gap: 0.35rem; font-size: 0.9rem; color: var(--c-text-muted); }
  td.right { text-align: right; }
  tr.archived td { color: var(--c-text-subtle); }
  .muted { color: var(--c-text-muted); }
</style>
