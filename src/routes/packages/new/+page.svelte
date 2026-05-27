<script lang="ts">
  // Block P2b — Neues Paket anlegen (= erste Revision).
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import PageBar from "$lib/PageBar.svelte";
  import Button from "$lib/Button.svelte";
  import { flash } from "$lib/toast.svelte";
  import { mdPreview } from "$lib/markdownPreview";
  import MarkdownEditor from "$lib/MarkdownEditor.svelte";
  import {
    packageCategoriesList,
    packagesCreate,
    packagePreview,
    sellerProfileGet,
  } from "$lib/api";
  import type { PackageCategory } from "$lib/types";

  let categories = $state<PackageCategory[]>([]);
  let isKlein = $state(true);
  let loading = $state(true);

  let name = $state("");
  let categoryId = $state<string>("");
  let title = $state("");
  let body = $state("");
  let priceEuro = $state<number | null>(null);
  let unit = $state("Pauschal");

  let saving = $state(false);
  let previewing = $state(false);

  onMount(async () => {
    try {
      const [cats, seller] = await Promise.all([packageCategoriesList(), sellerProfileGet()]);
      categories = cats;
      if (seller) isKlein = seller.isKleinunternehmer === 1;
    } catch (e) {
      flash(e instanceof Error ? e.message : String(e), "error");
    } finally {
      loading = false;
    }
  });

  const cents = () => Math.round((priceEuro ?? 0) * 100);

  function checkForm(): boolean {
    if (!name.trim()) {
      flash("Bitte einen Paket-Namen eingeben.", "error");
      return false;
    }
    if (!title.trim()) {
      flash("Bitte einen Positions-Titel eingeben.", "error");
      return false;
    }
    if (priceEuro === null || priceEuro < 0) {
      flash("Bitte einen gültigen Netto-Preis eingeben.", "error");
      return false;
    }
    return true;
  }

  async function save() {
    if (!checkForm()) return;
    saving = true;
    try {
      const pkg = await packagesCreate(categoryId || null, name.trim(), {
        title: title.trim(),
        bodyMarkup: body,
        defaultUnitPriceCents: cents(),
        unitCode: unit.trim() || "Pauschal",
        taxCategoryCode: "E",
        note: null,
      });
      flash("Paket angelegt.");
      goto(`/packages/${pkg.id}`);
    } catch (e) {
      flash(e instanceof Error ? e.message : String(e), "error");
    } finally {
      saving = false;
    }
  }

  async function preview() {
    if (!title.trim()) {
      flash("Bitte zuerst einen Titel eingeben.", "error");
      return;
    }
    previewing = true;
    try {
      await packagePreview(title.trim(), body, cents());
    } catch (e) {
      flash(e instanceof Error ? e.message : String(e), "error");
    } finally {
      previewing = false;
    }
  }
</script>

<PageBar back="/packages" backLabel="Pakete" title="Neues Paket" />

{#if loading}
  <p class="muted">Lädt …</p>
{:else}
  <div class="editor">
    <section class="card">
      <label>
        Paket-Name (intern) *
        <input type="text" bind:value={name} placeholder="z. B. Hochzeit klein" />
      </label>
      <label>
        Kategorie
        <select bind:value={categoryId}>
          <option value="">– ohne –</option>
          {#each categories as c (c.id)}
            <option value={c.id}>{c.name}</option>
          {/each}
        </select>
      </label>
      <label>
        Positions-Titel (auf dem Beleg) *
        <input type="text" bind:value={title} placeholder="z. B. Fotopaket „Hochzeit klein“" />
      </label>
      <div class="row">
        <label>
          Netto-Preis (€) *
          <input type="number" step="0.01" min="0" bind:value={priceEuro} />
        </label>
        <label>
          Einheit
          <input type="text" bind:value={unit} />
        </label>
      </div>
      {#if isKlein}
        <p class="hint">§19: Preis ist <strong>netto</strong>, es wird keine Umsatzsteuer ausgewiesen.</p>
      {/if}
    </section>

    <section class="card">
      <div class="md-head">
        <h2>Beschreibung</h2>
        <Button variant="ghost" disabled={previewing} onclick={preview}>
          {previewing ? "Öffnet …" : "PDF-Vorschau"}
        </Button>
      </div>
      <p class="md-help">
        Formatierung: <code># Überschrift</code>, <code>**fett**</code>, <code>*kursiv*</code>,
        Listen mit <code>-</code>, Tabellen (GFM). Die PDF-Vorschau zeigt das exakte Ergebnis.
      </p>
      <div class="md-grid">
        <MarkdownEditor bind:value={body} placeholder="# Leistungen" />
        <div class="md-preview">{@html mdPreview(body)}</div>
      </div>
    </section>

    <div class="actions">
      <Button variant="primary" disabled={saving} onclick={save}>
        {saving ? "Speichert …" : "Paket anlegen"}
      </Button>
      <Button variant="ghost" onclick={() => goto("/packages")}>Abbrechen</Button>
    </div>
  </div>
{/if}

<style>
  .muted { color: var(--c-text-muted); }
  .editor { max-width: 60rem; }
  /* .card entfernt — globale Definition aus tokens.css. */
  label { display: block; margin-bottom: 0.8rem; font-size: var(--fs-sm); font-weight: 600; }
  input, select {
    display: block;
    width: 100%;
    box-sizing: border-box;
    margin-top: 0.25rem;
    padding: 8px 12px;
    border: 1px solid var(--c-border-strong);
    border-radius: var(--r-md);
    font-weight: 400;
  }
  .row { display: flex; gap: 1rem; }
  .row > label { flex: 1; }
  .hint { font-size: var(--fs-sm); color: var(--c-text-muted); margin: 0; }
  .md-head { display: flex; align-items: center; justify-content: space-between; }
  .md-head h2 { font-size: var(--fs-lg); margin: 0; }
  .md-help { font-size: var(--fs-sm); color: var(--c-text-muted); line-height: 1.5; }
  .md-help code {
    font-family: var(--font-mono, monospace);
    font-size: 0.85em;
    background: var(--c-surface-2, #eef1f3);
    padding: 1px 5px;
    border-radius: var(--r-sm);
  }
  .md-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
  .md-preview {
    border: 1px solid var(--c-border);
    border-radius: var(--r-md);
    padding: 0.5rem 1rem;
    background: #fff;
    overflow: auto;
    line-height: 1.5;
  }
  .actions { display: flex; gap: 0.5rem; margin-top: 0.5rem; }
</style>
