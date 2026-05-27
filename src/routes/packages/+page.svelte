<script lang="ts">
  // Block P2b — Paket-Katalog: Liste, nach Kategorie gruppiert.
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import PageBar from "$lib/PageBar.svelte";
  import Button from "$lib/Button.svelte";
  import Badge from "$lib/Badge.svelte";
  import { flash } from "$lib/toast.svelte";
  import {
    packagesList,
    packageCategoriesList,
    packageCategoryCreate,
    packagesArchive,
    packagesReactivate,
  } from "$lib/api";
  import type { Package, PackageCategory } from "$lib/types";

  let packages = $state<Package[]>([]);
  let categories = $state<PackageCategory[]>([]);
  let loading = $state(true);
  let loadError = $state<string | null>(null);
  let newCategory = $state("");
  let busy = $state(false);

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

  async function addCategory() {
    if (!newCategory.trim()) {
      flash("Bitte einen Kategorie-Namen eingeben.", "error");
      return;
    }
    busy = true;
    try {
      await packageCategoryCreate(newCategory.trim());
      newCategory = "";
      categories = await packageCategoriesList();
      flash("Kategorie angelegt.");
    } catch (e) {
      flash(e instanceof Error ? e.message : String(e), "error");
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
    <Button variant="secondary" onclick={() => goto("/packages/brochure")}>Broschüre</Button>
    <Button variant="primary" onclick={() => goto("/packages/new")}>Neues Paket</Button>
  {/snippet}
</PageBar>

<p class="lead">
  Wiederverwendbare Dienstleistungs-Pakete für Angebote und Rechnungen. Jede
  Änderung an einem Paket erzeugt eine neue Version — alte Versionen bleiben
  unverändert nachvollziehbar.
</p>

{#if loading}
  <p class="muted">Lädt …</p>
{:else if loadError}
  <div class="error-card">Konnte Pakete nicht laden: {loadError}</div>
{:else}
  <section class="cat-add">
    <input
      type="text"
      placeholder="Neue Kategorie (z. B. Hochzeit, Porträt)"
      bind:value={newCategory}
      onkeydown={(e) => e.key === "Enter" && addCategory()}
    />
    <Button variant="secondary" disabled={busy} onclick={addCategory}>
      Kategorie anlegen
    </Button>
  </section>

  {#if packages.length === 0}
    <div class="empty">
      Noch keine Pakete. Lege oben eine Kategorie an und dann dein erstes Paket über
      „Neues Paket".
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
            <Button variant="ghost" onclick={() => goto(`/packages/${p.id}`)}>
              Bearbeiten
            </Button>
            <Button variant="ghost" disabled={busy} onclick={() => toggleArchive(p)}>
              {p.status === "archived" ? "Reaktivieren" : "Archivieren"}
            </Button>
          </div>
        </div>
      {/each}
    </div>
  {/each}
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
  .cat-add {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 1.5rem;
    flex-wrap: wrap;
  }
  .cat-add input {
    flex: 1;
    min-width: 16rem;
    padding: 8px 12px;
    border: 1px solid var(--c-border-strong);
    border-radius: var(--r-md);
  }
  .empty {
    color: var(--c-text-muted);
    background: var(--c-surface);
    border: 1px dashed var(--c-border-strong);
    border-radius: var(--r-lg);
    padding: 1.5rem;
    max-width: 46rem;
  }
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
  .pkg.archived {
    opacity: 0.65;
  }
  .pkg-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }
  .pkg-name {
    font-weight: 600;
    color: var(--c-primary-700);
    text-decoration: none;
  }
  .pkg-name:hover {
    text-decoration: underline;
  }
  .pkg-meta {
    font-size: var(--fs-sm);
    color: var(--c-text-muted);
  }
  .pkg-foot {
    display: flex;
    gap: 0.4rem;
    margin-top: 0.3rem;
  }
</style>
