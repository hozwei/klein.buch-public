<script lang="ts">
  // Block P3: wiederverwendbarer „Paket"-Block für die Positions-Editoren
  // (Rechnung, Angebot, Umwandlung). Gruppiertes Dropdown (Kategorie → Paket) →
  // materialisiert die aktive Revision als Beleg-Position.
  import { onMount } from "svelte";
  import { packagesList, packageCategoriesList, packageMaterializeItem } from "#lib/api.ts";
  import type { Package, PackageCategory, MaterializedPackageItem } from "#lib/types.ts";
  import { flash } from "#lib/toast.svelte.ts";
  import { randomId } from "#lib/browser.ts";

  const selId = `pkg-sel-${randomId()}`;
  const errId = `${selId}-err`;

  // ADR 0042 Nr. 3: `documentDate` = maßgebliches Datum des Zielbelegs; die
  // Position bekommt die Steuerart des Status an diesem Datum (ohne Angabe heute).
  let {
    onAdd,
    documentDate = null,
  }: { onAdd: (item: MaterializedPackageItem) => void; documentDate?: string | null } = $props();

  let packages = $state<Package[]>([]);
  let categories = $state<PackageCategory[]>([]);
  let selected = $state("");
  let loaded = $state(false);
  let adding = $state(false);
  // Feldfehler statt Toast (ADR 0041 Nr. 22), erst nach einem Versuch ohne Wahl.
  let pkgError = $state("");

  // Nur aktive (nicht archivierte) Pakete mit einer Version sind einfügbar.
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
    if (adding) return;
    if (!selected) {
      pkgError = "Bitte zuerst ein Paket wählen.";
      document.getElementById(selId)?.focus();
      return;
    }
    pkgError = "";
    adding = true;
    try {
      const item = await packageMaterializeItem(selected, documentDate);
      onAdd(item);
      flash(`Paket „${item.packageName}“ eingefügt.`);
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
    <label class="lbl" for={selId}>Paket</label>
    <select
      id={selId}
      bind:value={selected}
      class="sel"
      onchange={() => (pkgError = "")}
      aria-invalid={pkgError ? "true" : undefined}
      aria-describedby={pkgError ? errId : undefined}
    >
      <option value="">— Paket wählen —</option>
      {#each groups as g (g.label)}
        <optgroup label={g.label}>
          {#each g.items as p (p.id)}
            <option value={p.id}>{p.name}</option>
          {/each}
        </optgroup>
      {/each}
    </select>
    <!-- Nicht gesperrt: ohne Wahl erscheint der Grund am Feld (design.md §4.1). -->
    <button type="button" class="btn-secondary" onclick={add} aria-disabled={adding ? "true" : undefined}>
      + Paket
    </button>
    {#if pkgError}<span id={errId} class="err">{pkgError}</span>{/if}
  </div>
{:else if loaded}
  <div class="pkg">
    <span class="hint">
      Keine aktiven Pakete —
      <a class="kb-link" href="/packages">Paket-Katalog öffnen</a>, um eins anzulegen.
    </span>
  </div>
{/if}

<style>
  .pkg { display: flex; align-items: center; gap: 0.6rem; flex-wrap: wrap; padding: 0.6rem 0 0.2rem; }
  .lbl { font-weight: 600; font-size: var(--fs-sm); }
  /* Kante, Höhe und Pfeil kommen aus tokens.css (U4); hier nur die Breite. */
  .sel { min-width: min(16rem, 100%); max-width: 100%; }
  .hint { color: var(--text-muted); font-size: var(--fs-sm); }
  .err { flex-basis: 100%; font-size: var(--fs-xs); color: var(--status-danger-fg); }
  /* Link im Satz: `.kb-link`, 2.5.8-Ausnahme „inline“ ohne 44-px-Zeile (ADR 0041 Nr. 31,
     ADR 0042 Nr. 16). */
  /* Lokale btn-Defs entfernt — globale aus tokens.css greifen
     (Manuel-Hardline 2026-05-26: alle Buttons app-weit gleich groß). */
</style>
