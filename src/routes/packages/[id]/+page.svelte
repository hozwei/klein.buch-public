<script lang="ts">
  // Block P2b — Paket bearbeiten (= neue Revision) + Revisions-Panel
  // (Vergleich zweier Versionen + Rollback). Append-only: nichts wird überschrieben.
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import PageBar from "$lib/PageBar.svelte";
  import Button from "$lib/Button.svelte";
  import Badge from "$lib/Badge.svelte";
  import { flash } from "$lib/toast.svelte";
  import { euro } from "$lib/format";
  import { mdPreview } from "$lib/markdownPreview";
  import MarkdownEditor from "$lib/MarkdownEditor.svelte";
  import {
    packagesGet,
    packageRevisionsList,
    packagesUpdateAsNewRevision,
    packageRevisionsRollback,
    packageCategoriesList,
    packagePreview,
    sellerProfileGet,
  } from "$lib/api";
  import type { Package, PackageRevision, PackageCategory } from "$lib/types";

  let id = $derived($page.params.id ?? "");

  let pkg = $state<Package | null>(null);
  let revisions = $state<PackageRevision[]>([]);
  let categories = $state<PackageCategory[]>([]);
  let isKlein = $state(true);
  let loading = $state(true);
  let loadError = $state<string | null>(null);

  let name = $state("");
  let categoryId = $state<string>("");
  let title = $state("");
  let body = $state("");
  let priceEuro = $state<number | null>(null);
  let unit = $state("Pauschal");

  let saving = $state(false);
  let previewing = $state(false);
  let busy = $state(false);

  // Vergleich
  let compareA = $state<number | null>(null);
  let compareB = $state<number | null>(null);

  function prefillFrom(rev: PackageRevision | undefined) {
    if (!rev) return;
    title = rev.title;
    body = rev.bodyMarkup;
    priceEuro = rev.defaultUnitPriceCents / 100;
    unit = rev.unitCode;
  }

  async function load() {
    loading = true;
    try {
      const [p, revs, cats, seller] = await Promise.all([
        packagesGet(id),
        packageRevisionsList(id),
        packageCategoriesList(),
        sellerProfileGet(),
      ]);
      if (!p) {
        loadError = "Paket nicht gefunden.";
        return;
      }
      pkg = p;
      revisions = revs;
      categories = cats;
      if (seller) isKlein = seller.isKleinunternehmer === 1;
      name = p.name;
      categoryId = p.categoryId ?? "";
      const current = revs.find((r) => r.revision === p.currentRevision) ?? revs[0];
      prefillFrom(current);
      compareA = revs.length > 1 ? revs[1].revision : (revs[0]?.revision ?? null);
      compareB = revs[0]?.revision ?? null;
    } catch (e) {
      loadError = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  const cents = () => Math.round((priceEuro ?? 0) * 100);
  const fmtDate = (s: string) => (s ? s.slice(0, 10).split("-").reverse().join(".") : "");
  const revByNum = (n: number | null) => revisions.find((r) => r.revision === n);

  async function save() {
    if (!name.trim()) return flashErr("Bitte einen Paket-Namen eingeben.");
    if (!title.trim()) return flashErr("Bitte einen Positions-Titel eingeben.");
    if (priceEuro === null || priceEuro < 0) return flashErr("Bitte einen gültigen Netto-Preis eingeben.");
    saving = true;
    try {
      await packagesUpdateAsNewRevision(id, categoryId || null, name.trim(), {
        title: title.trim(),
        bodyMarkup: body,
        defaultUnitPriceCents: cents(),
        unitCode: unit.trim() || "Pauschal",
        taxCategoryCode: "E",
        note: null,
      });
      flash("Neue Version gespeichert.");
      await load();
    } catch (e) {
      flash(e instanceof Error ? e.message : String(e), "error");
    } finally {
      saving = false;
    }
  }

  function flashErr(m: string) {
    flash(m, "error");
  }

  async function rollback(toRevision: number) {
    busy = true;
    try {
      await packageRevisionsRollback(id, toRevision);
      flash(`Auf Version ${toRevision} zurückgesetzt (als neue Version).`);
      await load();
    } catch (e) {
      flash(e instanceof Error ? e.message : String(e), "error");
    } finally {
      busy = false;
    }
  }

  async function preview() {
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

<PageBar back="/packages" backLabel="Pakete" title="Paket bearbeiten" />

{#if loading}
  <p class="muted">Lädt …</p>
{:else if loadError}
  <div class="error-card">{loadError}</div>
{:else if pkg}
  <div class="editor">
    <section class="card">
      <div class="head-row">
        <h2>Aktuelle Version bearbeiten</h2>
        <span class="ver">Aktiv: Version {pkg.currentRevision ?? "–"}</span>
      </div>
      <p class="muted small">
        Speichern erzeugt eine <strong>neue Version</strong>. Bestehende Versionen
        bleiben unverändert erhalten.
      </p>
      <label>
        Paket-Name (intern) *
        <input type="text" bind:value={name} />
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
        <input type="text" bind:value={title} />
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
        <p class="hint">§19: Preis ist <strong>netto</strong>, keine Umsatzsteuer ausgewiesen.</p>
      {/if}
    </section>

    <section class="card">
      <div class="md-head">
        <h2>Beschreibung</h2>
        <Button variant="ghost" disabled={previewing} onclick={preview}>
          {previewing ? "Öffnet …" : "PDF-Vorschau"}
        </Button>
      </div>
      <div class="md-grid">
        <MarkdownEditor bind:value={body} />
        <div class="md-preview">{@html mdPreview(body)}</div>
      </div>
    </section>

    <div class="actions">
      <Button variant="primary" disabled={saving} onclick={save}>
        {saving ? "Speichert …" : "Als neue Version speichern"}
      </Button>
      <Button variant="ghost" onclick={() => goto("/packages")}>Zurück</Button>
    </div>

    <h2 class="section-h">Versionen</h2>
    <div class="revs">
      {#each revisions as r (r.id)}
        <div class="rev">
          <div class="rev-main">
            <strong>Version {r.revision}</strong>
            {#if r.revision === pkg.currentRevision}<Badge tone="primary">aktiv</Badge>{/if}
            <span class="muted small">· {fmtDate(r.createdAt)} · {r.title} · {euro(r.defaultUnitPriceCents)}</span>
            {#if r.note}<span class="muted small">· {r.note}</span>{/if}
          </div>
          {#if r.revision !== pkg.currentRevision}
            <Button variant="ghost" disabled={busy} onclick={() => rollback(r.revision)}>
              Auf diese Version zurücksetzen
            </Button>
          {/if}
        </div>
      {/each}
    </div>

    {#if revisions.length > 1}
      <h2 class="section-h">Versionen vergleichen</h2>
      <div class="cmp-pick">
        <label>
          Links
          <select bind:value={compareA}>
            {#each revisions as r (r.id)}<option value={r.revision}>Version {r.revision}</option>{/each}
          </select>
        </label>
        <label>
          Rechts
          <select bind:value={compareB}>
            {#each revisions as r (r.id)}<option value={r.revision}>Version {r.revision}</option>{/each}
          </select>
        </label>
      </div>
      <div class="cmp-grid">
        {#each [revByNum(compareA), revByNum(compareB)] as rev}
          <div class="cmp-col">
            {#if rev}
              <div class="cmp-h">Version {rev.revision} · {fmtDate(rev.createdAt)}</div>
              <div class="cmp-meta">{rev.title} · {euro(rev.defaultUnitPriceCents)}</div>
              <div class="md-preview">{@html mdPreview(rev.bodyMarkup)}</div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </div>
{/if}

<style>
  .muted { color: var(--c-text-muted); }
  .small { font-size: var(--fs-sm); }
  .editor { max-width: 64rem; }
  .error-card {
    color: var(--c-danger-700);
    background: var(--c-danger-50);
    border: 1px solid var(--c-danger-500);
    border-radius: var(--r-lg);
    padding: 1rem 1.2rem;
    max-width: 46rem;
  }
  /* .card entfernt — globale Definition aus tokens.css. */
  .head-row { display: flex; align-items: baseline; justify-content: space-between; gap: 0.5rem; }
  .head-row h2 { font-size: var(--fs-lg); margin: 0; }
  .ver { font-size: var(--fs-sm); color: var(--c-text-muted); }
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
  .md-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
  .md-preview {
    border: 1px solid var(--c-border);
    border-radius: var(--r-md);
    padding: 0.5rem 1rem;
    background: #fff;
    overflow: auto;
    line-height: 1.5;
  }
  .actions { display: flex; gap: 0.5rem; margin: 0.5rem 0 1.5rem; }
  .section-h { font-size: var(--fs-lg); margin: 1.5rem 0 0.8rem; }
  .revs { display: flex; flex-direction: column; gap: 0.4rem; }
  .rev {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    background: var(--c-surface);
    border: 1px solid var(--c-border);
    border-radius: var(--r-md);
    padding: 0.6rem 1rem;
  }
  .rev-main { display: flex; align-items: center; gap: 0.4rem; flex-wrap: wrap; }
  .cmp-pick { display: flex; gap: 1rem; margin-bottom: 0.8rem; }
  .cmp-pick label { flex: 0 0 12rem; }
  .cmp-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
  .cmp-col {
    border: 1px solid var(--c-border);
    border-radius: var(--r-lg);
    padding: 0.8rem 1rem;
    background: var(--c-surface);
  }
  .cmp-h { font-weight: 600; }
  .cmp-meta { font-size: var(--fs-sm); color: var(--c-text-muted); margin-bottom: 0.5rem; }
</style>
