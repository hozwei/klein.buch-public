<script lang="ts">
  // Block P2b — Paket-Katalog: Liste, nach Kategorie gruppiert.
  import { onMount } from "svelte";
  import PageBar from "#lib/PageBar.svelte";
  import Button from "#lib/Button.svelte";
  import Badge from "#lib/Badge.svelte";
  import { flash } from "#lib/toast.svelte.ts";
  import {
    packagesList,
    packageCategoriesList,
    packageCategoryCreate,
    packagesArchive,
    packagesReactivate,
  } from "#lib/api.ts";
  import type { Package, PackageCategory } from "#lib/types.ts";

  let packages = $state<Package[]>([]);
  let categories = $state<PackageCategory[]>([]);
  let loading = $state(true);
  let loadError = $state<string | null>(null);
  let newCategory = $state("");
  let busy = $state(false);
  // U1: Fehler am Feld statt Toast.
  let categoryError = $state("");
  let categoryEl: HTMLInputElement | undefined = $state();

  async function reloadPackages() {
    packages = await packagesList();
  }

  onMount(async () => {
    try {
      [packages, categories] = await Promise.all([packagesList(), packageCategoriesList()]);
    } catch (e) {
      loadError = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  });

  let groups = $derived.by(() => {
    const g: { name: string; items: Package[] }[] = [];
    for (const c of categories) {
      const items = packages.filter((p) => p.categoryId === c.id);
      if (items.length) g.push({ name: c.name, items });
    }
    const orphans = packages.filter(
      (p) => !p.categoryId || !categories.some((c) => c.id === p.categoryId),
    );
    if (orphans.length) g.push({ name: "Ohne Kategorie", items: orphans });
    return g;
  });
  // Kategorien ohne Pakete stehen am Ende (Anzeige, U1): Sie sind angelegt
  // und in „Neues Paket“ wählbar, auch wenn sie noch leer sind.
  let emptyCategories = $derived(
    categories.filter((c) => !packages.some((p) => p.categoryId === c.id)),
  );

  async function addCategory(e?: Event) {
    e?.preventDefault();
    if (!newCategory.trim()) {
      categoryError = "Bitte einen Kategorie-Namen eingeben.";
      categoryEl?.focus();
      return;
    }
    categoryError = "";
    busy = true;
    try {
      await packageCategoryCreate(newCategory.trim());
      newCategory = "";
      categories = await packageCategoriesList();
      flash("Kategorie angelegt.");
    } catch (err) {
      categoryError = err instanceof Error ? err.message : String(err);
      categoryEl?.focus();
    } finally {
      busy = false;
    }
  }

  async function toggleArchive(p: Package) {
    busy = true;
    try {
      if (p.status === "archived") {
        await packagesReactivate(p.id);
        flash("Paket reaktiviert.");
      } else {
        await packagesArchive(p.id);
        flash("Paket archiviert.");
      }
      await reloadPackages();
    } catch (e) {
      flash(e instanceof Error ? e.message : String(e), "error");
    } finally {
      busy = false;
    }
  }
</script>

<PageBar title="Pakete">
  {#snippet actions()}
    <Button variant="secondary" href="/packages/brochure">Broschüre</Button>
    <Button variant="primary" href="/packages/new">Neues Paket</Button>
  {/snippet}
</PageBar>

<p class="lead">
  Wiederverwendbare Dienstleistungs-Pakete für Angebote und Rechnungen. Jede
  Änderung an einem Paket erzeugt eine neue Version — alte Versionen bleiben
  unverändert nachvollziehbar.
</p>

{#if loading}
  <p class="muted" role="status" aria-busy="true">Lädt …</p>
{:else if loadError}
  <div class="error-card" role="alert">Konnte Pakete nicht laden: {loadError}</div>
{:else}
  <form class="cat-add" onsubmit={addCategory} novalidate>
    <label for="cat-new">Neue Kategorie</label>
    <div class="cat-row">
      <input
        id="cat-new"
        class="kb-input"
        type="text"
        autocomplete="off"
        enterkeyhint="done"
        placeholder="z. B. Hochzeit, Porträt"
        aria-invalid={categoryError ? "true" : undefined}
        aria-describedby={categoryError ? "cat-new-err" : undefined}
        bind:this={categoryEl}
        bind:value={newCategory}
        oninput={() => (categoryError = "")}
      />
      <Button type="submit" variant="secondary" disabled={busy}>Kategorie anlegen</Button>
    </div>
    {#if categoryError}<p id="cat-new-err" class="err">{categoryError}</p>{/if}
  </form>

  {#if packages.length === 0}
    <div class="empty">
      <p>Noch keine Pakete. Leg bei Bedarf oben eine Kategorie an und dann dein erstes Paket.</p>
      <Button variant="primary" href="/packages/new">Neues Paket</Button>
    </div>
  {/if}

  {#each groups as group (group.name)}
    <h2 class="cat-h">{group.name}</h2>
    <div class="grid">
      {#each group.items as p (p.id)}
        <div class="pkg" class:archived={p.status === "archived"}>
          <div class="pkg-head">
            <a class="pkg-name" href={`/packages/${p.id}`}>{p.name}</a>
            {#if p.status === "archived"}
              <Badge tone="neutral">archiviert</Badge>
            {:else}
              <Badge tone="success">aktiv</Badge>
            {/if}
          </div>
          <div class="pkg-meta">
            {p.currentRevision ? `Version ${p.currentRevision}` : "noch keine Version"}
          </div>
          <div class="pkg-foot">
            <Button variant="secondary" href={`/packages/${p.id}`} ariaLabel={`${p.name} bearbeiten`}>
              Bearbeiten
            </Button>
            <Button
              variant="secondary"
              disabled={busy}
              onclick={() => toggleArchive(p)}
              ariaLabel={`${p.name} ${p.status === "archived" ? "reaktivieren" : "archivieren"}`}
            >
              {p.status === "archived" ? "Reaktivieren" : "Archivieren"}
            </Button>
          </div>
        </div>
      {/each}
    </div>
  {/each}

  {#if emptyCategories.length}
    <h2 class="cat-h">Kategorien ohne Pakete</h2>
    <ul class="cat-empty">
      {#each emptyCategories as c (c.id)}<li>{c.name}</li>{/each}
    </ul>
  {/if}
{/if}

<style>
  /* .intro entfernt — globale .lead aus tokens.css. */
  .muted {
    color: var(--c-text-muted);
  }
  .error-card {
    color: var(--c-danger-700);
    background: var(--c-danger-50);
    border: 1px solid var(--c-danger-500);
    border-radius: var(--r-lg);
    padding: 1rem 1.2rem;
    max-width: 46rem;
  }
  .cat-add { margin-bottom: 1.5rem; max-width: 46rem; }
  .cat-add label { display: block; margin-bottom: 5px; font-size: var(--fs-sm); font-weight: 600; }
  .cat-row { display: flex; gap: 0.5rem; flex-wrap: wrap; }
  .cat-row input { flex: 1 1 14rem; min-width: 0; width: auto; scroll-margin-block: var(--sp-6); }
  .err { margin: 4px 0 0; font-size: var(--fs-sm); font-weight: 600; color: var(--status-danger-fg); }
  .cat-empty { display: flex; flex-wrap: wrap; gap: 0.4rem; list-style: none; padding: 0; margin: 0; }
  .cat-empty li {
    padding: 0.2rem 0.7rem;
    border: 1px solid var(--line-control);
    border-radius: var(--r-pill);
    font-size: var(--fs-sm);
    color: var(--text-muted);
    overflow-wrap: anywhere;
  }
  .empty {
    color: var(--c-text-muted);
    background: var(--c-surface);
    border: 1px dashed var(--c-border-strong);
    border-radius: var(--r-lg);
    padding: 1.5rem;
    max-width: 46rem;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.7rem;
  }
  .empty p { margin: 0; }
  .cat-h {
    font-size: var(--fs-lg);
    margin: 1.5rem 0 0.8rem;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(17rem, 1fr));
    gap: 1rem;
  }
  .pkg {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    background: var(--c-surface);
    border: 1px solid var(--c-border);
    border-radius: var(--r-lg);
    padding: 1rem 1.2rem;
    box-shadow: var(--sh-sm);
  }
  /* Archiviert: gestrichelter Rahmen und Badge statt Transparenz (Kontrast 1.4.3). */
  .pkg.archived {
    border-style: dashed;
    background: var(--surface-page);
  }
  .pkg-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }
  .pkg-name {
    display: inline-flex;
    align-items: center;
    min-height: var(--target);
    min-width: 0;
    overflow-wrap: anywhere;
    font-weight: 600;
    color: var(--c-primary-700);
    text-decoration: none;
  }
  @media (hover: hover) {
    .pkg-name:hover { text-decoration: underline; }
  }
  .pkg-meta {
    font-size: var(--fs-sm);
    color: var(--c-text-muted);
  }
  .pkg-foot {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    margin-top: 0.3rem;
  }
</style>
