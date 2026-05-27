<script lang="ts">
  // Block P3: wiederverwendbarer „Paket"-Block für die Positions-Editoren
  // (Rechnung, Angebot, Umwandlung). Gruppiertes Dropdown (Kategorie → Paket) →
  // materialisiert die aktive Revision als Beleg-Position.
  import { onMount } from "svelte";
  import { packagesList, packageCategoriesList, packageMaterializeItem } from "$lib/api";
  import type { Package, PackageCategory, MaterializedPackageItem } from "$lib/types";
  import { flash } from "$lib/toast.svelte";

  let { onAdd }: { onAdd: (item: MaterializedPackageItem) => void } = $props();

  let packages = $state<Package[]>([]);
  let categories = $state<PackageCategory[]>([]);
  let selected = $state("");
  let loaded = $state(false);
  let adding = $state(false);

  // Nur aktive Pakete mit veröffentlichter Revision sind einfügbar.
  let active = $derived(
    packages.filter((p) => p.status === "active" && p.currentRevision != null),
  );

  // Gruppiert nach Kategorie (+ „Ohne Kategorie" am Ende).
  let groups = $derived.by(() => {
    const byCat = new Map<string | null, Package[]>();
    for (const p of active) {
      const key = p.categoryId ?? null;
      const arr = byCat.get(key) ?? [];
      arr.push(p);
      byCat.set(key, arr);
    }
    const out: { label: string; items: Package[] }[] = [];
    for (const c of categories) {
      const items = byCat.get(c.id);
      if (items && items.length) out.push({ label: c.name, items });
    }
    const uncategorized = byCat.get(null);
    if (uncategorized && uncategorized.length)
      out.push({ label: "Ohne Kategorie", items: uncategorized });
    return out;
  });

  onMount(async () => {
    try {
      [packages, categories] = await Promise.all([packagesList(), packageCategoriesList()]);
    } catch (e) {
      flash(String(e), "error");
    } finally {
      loaded = true;
    }
  });

  async function add() {
    if (!selected) {
      flash("Bitte ein Paket wählen.", "error");
      return;
    }
    adding = true;
    try {
      const item = await packageMaterializeItem(selected);
      onAdd(item);
      flash(`Paket „${item.packageName}" eingefügt.`);
      selected = "";
    } catch (e) {
      flash(String(e), "error");
    } finally {
      adding = false;
    }
  }
</script>

{#if loaded && active.length > 0}
  <div class="pkg">
    <span class="lbl">Paket</span>
    <select bind:value={selected} class="sel">
      <option value="">— Paket wählen —</option>
      {#each groups as g (g.label)}
        <optgroup label={g.label}>
          {#each g.items as p (p.id)}
            <option value={p.id}>{p.name}</option>
          {/each}
        </optgroup>
      {/each}
    </select>
    <button type="button" class="btn-secondary" onclick={add} disabled={adding || !selected}>
      + Paket
    </button>
  </div>
{:else if loaded}
  <div class="pkg">
    <span class="hint">
      Keine veröffentlichten Pakete —
      <a href="/packages">Paket-Katalog öffnen</a>, um eins anzulegen.
    </span>
  </div>
{/if}

<style>
  .pkg { display: flex; align-items: center; gap: 0.6rem; flex-wrap: wrap; padding: 0.6rem 0 0.2rem; }
  .lbl { font-weight: 600; font-size: 0.9rem; }
  .sel { padding: 0.4rem 0.5rem; border: 1px solid #d1d5db; border-radius: 4px; font: inherit; min-width: 16rem; }
  .hint { color: #6b7280; font-size: 0.9rem; }
  /* Lokale btn-Defs entfernt — globale aus tokens.css greifen
     (Manuel-Hardline 2026-05-26: alle Buttons app-weit gleich groß). */
</style>
